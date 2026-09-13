//! Zapping a wand, and everything a bolt or a blast does once it lands.
//!
//! The catalog ([`crate::catalog::WANDS`]) says what each wand is called, how it
//! draws and how far it reaches; this file resolves the zap. A thrown wand — the
//! whole battery let go at once — is [`super::throwing`]'s job, but it borrows
//! most of its machinery ([`elemental_blast`], [`blast_palette`], the
//! per-creature effects) from here.

use bevy_ecs::{
    entity::Entity,
    prelude::{Component, With},
    world::World,
};
use crossterm::style::Color;
use rand::Rng;
use std::collections::{HashSet, VecDeque};

use crate::components::*;
use crate::conditions::{clear_player_conditions, confuse, shift_entity_speed};
use crate::effects::*;
use crate::equipment::{equipped_items, sync_equipment_effects};
use crate::helpers::{
    apply_damage, get_entities_at_position, get_line, item_label, leave_smoke, monster_at,
    player_sees, roll_dice,
};
use crate::identify::article_for;
use crate::map::{GameRng, MAP_HEIGHT, MAP_WIDTH, Map, Smoke, TileType, tile_index};
use crate::monsters::{BESTIARY, spawn_monster};
use crate::particles::{BlastPalette, Particles};
use crate::shake::{ShakeKind, kick_shake};
use crate::traps::random_open_tile;

use super::scrolls::teleport_reader;
use crate::constants::wands::{BLAST_RADIUS, DAMAGE_DICE, DAMAGE_SIDES, SMOKE_LINGER_TURNS};

/// Whether `entity` shrugs off `element`, from any source.
fn is_immune(world: &World, entity: Entity, element: Element) -> bool {
    element.immunity().probe(world, entity)
}

/// A wand's damage: `[DAMAGE_DICE]d[DAMAGE_SIDES]`, rolled once per zap and
/// applied whole to every creature it touches (armour is never subtracted — see
/// [`apply_damage`]).
fn roll_wand_damage(world: &mut World) -> i32 {
    roll_dice(world, DAMAGE_DICE, DAMAGE_SIDES)
}

/// Applies `damage` of `element` (or non-elemental if `None`) to `entity`,
/// respecting immunity. Returns how much HP was actually taken off — 0 if the
/// creature resisted or had no [`Fighter`]. Immunity is logged for named
/// creatures.
fn damage_with_element(
    world: &mut World,
    entity: Entity,
    damage: i32,
    element: Option<Element>,
) -> i32 {
    if let Some(el) = element.filter(|&el| is_immune(world, entity, el)) {
        if let Some(name) = world.get::<Name>(entity).map(|n| n.what.clone()) {
            world
                .resource_mut::<GameLog>()
                .add(format!("The {name} is unharmed by the {}.", el.noun()));
        }
        return 0;
    }
    let Some(hp_before) = world.get::<Fighter>(entity).map(|f| f.hp) else {
        return 0;
    };
    apply_damage(world, entity, damage);
    damage.min(hp_before.max(0))
}

/// Fires one of the straight-line "bolt" wands: a beam from the zapper to the
/// aimed tile that hurts everything caught along the way. Drain-life feeds the
/// HP it takes straight back to the zapper, never past their maximum.
fn fire_bolt(
    world: &mut World,
    user: Entity,
    user_pos: Position,
    target_pos: Position,
    effect: WandEffect,
    msg: &str,
    color: Color,
) {
    let element = Element::of(effect);
    let damage = roll_wand_damage(world);
    world.resource_mut::<GameLog>().add(msg.to_string());

    let bolt = trace_bolt(world, user, user_pos, target_pos, damage, element);

    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        let flight_ms = fx.beam(&bolt.cells, color);
        if let Some(&(lx, ly)) = bolt.cells.last() {
            fx.impact_sparks(lx, ly, color, flight_ms);
        }
    }

    // A bolt of the player's that actually bit something in sight gets the
    // same light thump a sword landing does: a wand is their weapon at range
    // and should land like one. Two gates, and each is one this file already
    // keeps elsewhere. Only *their* zap counts — a monster's reaches the map
    // the way its claws do, through the low-HP crossing or not at all. And it
    // has to have been seen, because unlike a melee blow or a thrown missile
    // nothing logs a bolt's damage: a shake for a bolt landing on something
    // down an unlit corridor would say there is a creature there, which is
    // the same leak the blast's own sight gate exists to close.
    if world.get::<Player>(user).is_some() && bolt.bit_something_seen {
        kick_shake(world, ShakeKind::Hit);
    }

    if effect == WandEffect::DrainLife && bolt.hp_taken > 0 {
        let taken = bolt.hp_taken;
        if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
            fighter.hp = (fighter.hp + taken).min(fighter.max_hp);
        }
        world
            .resource_mut::<GameLog>()
            .add(format!("You drain {taken} life."));
    }
}

/// What one traced bolt did, on its way to wherever it stopped.
struct Bolt {
    /// The tiles the beam animation streaks through, the zapper's own excluded.
    cells: Vec<(u16, u16)>,
    /// HP the bolt actually took off, summed over everything it touched — what
    /// drain-life feeds back to the zapper.
    hp_taken: i32,
    /// Whether any of that came off a creature standing where the player could
    /// see it. The sight gate on the bolt's screen shake, kept here because
    /// this is the only place that still knows *which tile* each victim was
    /// standing on.
    bit_something_seen: bool,
}

/// Walks the line from `user_pos` to `target_pos`, stopping at the first wall,
/// and damages every entity but the zapper along it.
fn trace_bolt(
    world: &mut World,
    user: Entity,
    user_pos: Position,
    target_pos: Position,
    damage: i32,
    element: Option<Element>,
) -> Bolt {
    let map = world.resource::<Map>().clone();
    let mut bolt = Bolt {
        cells: Vec::new(),
        hp_taken: 0,
        bit_something_seen: false,
    };
    for pos in get_line(user_pos, target_pos) {
        if map.blocks(pos.x, pos.y) {
            break;
        }
        if pos.x != user_pos.x || pos.y != user_pos.y {
            bolt.cells.push((pos.x, pos.y));
        }
        let victims = get_entities_at_position(world, pos)
            .into_iter()
            .filter(|&e| e != user);
        for entity in victims {
            let taken = damage_with_element(world, entity, damage, element);
            bolt.hp_taken += taken;
            bolt.bit_something_seen |= taken > 0 && player_sees(world, pos.x, pos.y);
        }
    }
    bolt
}

/// Blows a disc of `radius` tiles open around `center`: every creature standing
/// on a tile the centre can see (walls stop the flames) takes `damage` of
/// `element`, and the animation ripples outward from the core.
///
/// The one place an area blast is resolved — a zapped wand of fire and a thrown
/// one differ by the number passed in, and by nothing else. Nobody is exempt,
/// the thrower included.
///
/// `shooter` is whoever let it off. The blast itself does not care; the chain
/// reaction at the end of it does — a coin caught in a blast pays its effect to
/// whoever caused the blast, exactly as if they had shot the coin.
#[allow(clippy::too_many_arguments)] // one blast, and everything one is made of
pub(super) fn elemental_blast(
    world: &mut World,
    shooter: Option<Entity>,
    center: Position,
    radius: f32,
    damage: i32,
    element: Option<Element>,
    palette: BlastPalette,
) -> Vec<Entity> {
    let map = world.resource::<Map>().clone();
    let cx = center.x as i32;
    let cy = center.y as i32;
    let r = radius.ceil() as i32;

    // Every tile within the disc that the blast centre has line of sight to,
    // tagged with its distance from centre so the animation can ripple outward.
    let mut blast_cells: Vec<(u16, u16, f32)> = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist > radius {
                continue;
            }
            let Some((tx, ty)) = crate::particles::on_map(cx + dx, cy + dy) else {
                continue;
            };
            let ray = get_line(center, Position { x: tx, y: ty });
            let blocked = ray
                .iter()
                .any(|p| map.blocks(p.x, p.y) && !(p.x == tx && p.y == ty));
            if !blocked {
                blast_cells.push((tx, ty, dist));
            }
        }
    }

    // Damage every fighter standing in a blast cell.
    let cell_set: HashSet<(u16, u16)> = blast_cells.iter().map(|&(x, y, _)| (x, y)).collect();
    let mut affected_entities = Vec::new();
    let mut query = world.query::<(Entity, &Position)>();
    for (entity, pos) in query.iter(world) {
        if cell_set.contains(&(pos.x, pos.y)) {
            affected_entities.push(entity);
        }
    }
    for &entity in &affected_entities {
        damage_with_element(world, entity, damage, element);
    }

    // Anyone the blast just killed is finished off right here, rather than
    // waiting for the reaper's next sweep — so their death burst knows the
    // blast's own centre and flings the corpse radially outward, Mortal-
    // Kombat-style, instead of picking a random direction.
    for &entity in &affected_entities {
        if world.get::<Fighter>(entity).is_some_and(|f| f.hp <= 0) {
            crate::combat::finish_indirect_kill(world, entity, Some(center));
        }
    }

    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&blast_cells, palette);
        // Fire and cold both billow smoke a beat after the flames — purely
        // cosmetic. Only fire's actually lingers on the tiles afterward,
        // DCSS-style; cold's puff is just the one animation.
        if matches!(element, Some(Element::Fire) | Some(Element::Cold)) {
            fx.smoke_burst(&blast_cells);
        }
    }

    // Every blast in the game comes through here — a zapped wand, a thrown one
    // bursting on impact — so this is the one place the thump has to be armed.
    // Gated on actually seeing it: a shake for a blast in a room you have never
    // been in would hand you information the renderer is careful not to draw.
    if player_sees(world, center.x, center.y) {
        kick_shake(world, ShakeKind::Heavy);
    }
    if element == Some(Element::Fire) {
        let mut smoke = world.resource_mut::<Smoke>();
        for &(x, y, _) in &blast_cells {
            smoke.puff(x, y, SMOKE_LINGER_TURNS);
        }
    }

    // Anything in the blast that a shot could have set off goes off with it —
    // the trick shot, worked by a wand instead of a bowstring. Traps first,
    // then coins, and a coin hands its effect to whoever let the blast off.
    // None of these bursts is itself a blast, so nothing comes back through
    // here and a row of them cannot chain forever.
    for trap in things_in::<Trap>(world, &cell_set) {
        crate::traps::detonate_trap(world, trap);
    }
    for coin in things_in::<Pickup>(world, &cell_set) {
        crate::traps::detonate_pickup(world, coin, shooter);
    }

    affected_entities
}

/// Everything carrying `C` standing on one of `cells`. Collected up front
/// because setting one off mutates the world out from under the query.
fn things_in<C: Component>(world: &mut World, cells: &HashSet<(u16, u16)>) -> Vec<Entity> {
    world
        .query_filtered::<(Entity, &Position), With<C>>()
        .iter(world)
        .filter(|(_, p)| cells.contains(&(p.x, p.y)))
        .map(|(e, _)| e)
        .collect()
}

/// Puffs a small ring of smoke around `center` — the polymorph flourish.
/// Every open neighbouring tile (never through a wall) gets its own puff,
/// staggered a beat apart so it reads as smoke rolling outward from the
/// transformed creature rather than every tile igniting at once.
fn leave_smoke_ring(world: &mut World, center: Position) {
    const RING: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let map = world.resource::<Map>().clone();
    let mut cells: Vec<(u16, u16)> = vec![(center.x, center.y)];
    for &(dx, dy) in RING.iter() {
        let Some((x, y)) = crate::particles::on_map(center.x as i32 + dx, center.y as i32 + dy)
        else {
            continue;
        };
        if !map.blocks(x, y) {
            cells.push((x, y));
        }
    }
    {
        let mut smoke = world.resource_mut::<Smoke>();
        for &(x, y) in &cells {
            smoke.puff(x, y, crate::helpers::VANISHING_SMOKE_TURNS);
        }
    }
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        for (i, &(x, y)) in cells.iter().enumerate() {
            fx.poof(x, y, i as f32 * 40.0);
        }
    }
}

/// The blast-animation palette a wand's explosion burns in.
pub(super) fn blast_palette(effect: WandEffect) -> BlastPalette {
    match effect {
        WandEffect::Fire => BlastPalette::Fire,
        WandEffect::Cold => BlastPalette::Frost,
        WandEffect::Lightning => BlastPalette::Spark,
        WandEffect::MagicMissile => BlastPalette::Arcane,
        WandEffect::Striking => BlastPalette::Force,
        WandEffect::DrainLife => BlastPalette::Drain,
        WandEffect::Light => BlastPalette::Dazzle,
        WandEffect::Cancellation => BlastPalette::Void,
        WandEffect::Polymorph
        | WandEffect::HasteMonster
        | WandEffect::SlowMonster
        | WandEffect::TeleportAway
        | WandEffect::TeleportTo
        | WandEffect::Nothing => BlastPalette::Warp,
    }
}

/// Dazzle — the wand of light's confusion: the flash in the eyes, and then
/// [`crate::conditions::confuse`] does the rest. The wand owns the *flavour*
/// (a flash) and nothing else; what confusion means to a player as against a
/// monster is not this file's business.
pub(super) fn dazzle(world: &mut World, entity: Entity) {
    confuse(
        world,
        entity,
        "The flash leaves you reeling — you are dazzled!",
        "is dazzled",
    );
}

/// The wands that deal damage when zapped. Thrown, these go off wider and hotter
/// than the utility ("effect") wands.
pub(super) fn is_attack_wand(effect: WandEffect) -> bool {
    matches!(
        effect,
        WandEffect::Fire
            | WandEffect::Cold
            | WandEffect::Lightning
            | WandEffect::MagicMissile
            | WandEffect::Striking
            | WandEffect::DrainLife
    )
}

pub(super) fn apply_wand_effect(
    world: &mut World,
    user: Entity,
    target: Option<Position>,
    effect: WandEffect,
) {
    let user_pos = match world.get::<Position>(user) {
        Some(pos) => *pos,
        None => return,
    };

    // The wand of light takes no target: it floods the room (or passage) the
    // zapper is standing in.
    if effect == WandEffect::Light {
        light_area(world, user, user_pos);
        return;
    }

    let target_pos = match target {
        Some(pos) => pos,
        None => return, // Safety catch: every other wand requires a target.
    };

    // Exhaustive over `WandEffect`, deliberately with no catch-all: a wand
    // effect added to the enum and not given an arm here fails the build
    // instead of discharging with a generic "nothing happens" — the same
    // guarantee `crate::traps::apply_trap_effect` gives a new `TrapEffect`. See
    // `docs/explanation/data-driven-content.md`.
    match effect {
        WandEffect::MagicMissile => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            "A brilliant cyan bolt leaps from the wand!",
            Color::Cyan,
        ),
        WandEffect::Lightning => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            "A forking bolt of lightning cracks out!",
            Color::Yellow,
        ),
        WandEffect::Striking => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            "An invisible fist hammers down the line!",
            Color::White,
        ),
        WandEffect::DrainLife => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            "A tendril of black light drinks the life from its path.",
            Color::DarkMagenta,
        ),
        WandEffect::Fire | WandEffect::Cold => {
            let is_fire = effect == WandEffect::Fire;
            let msg = if is_fire {
                "A roaring sphere of fire erupts!"
            } else {
                "A blast of freezing air detonates!"
            };
            let damage = roll_wand_damage(world);
            world.resource_mut::<GameLog>().add(msg.to_string());
            elemental_blast(
                world,
                Some(user),
                target_pos,
                BLAST_RADIUS,
                damage,
                Element::of(effect),
                blast_palette(effect),
            );
        }
        WandEffect::Polymorph => polymorph_target(world, target_pos),
        WandEffect::HasteMonster => shift_target_speed(world, target_pos, true),
        WandEffect::SlowMonster => shift_target_speed(world, target_pos, false),
        WandEffect::TeleportAway => teleport_target_away(world, target_pos),
        WandEffect::TeleportTo => teleport_target_here(world, user, user_pos, target_pos),
        WandEffect::Cancellation => cancel_target(world, target_pos),
        WandEffect::Nothing => {
            world
                .resource_mut::<GameLog>()
                .add("The wand does nothing. It was well named.".to_string());
        }
        // Light has no target and returns above, before this match — it can
        // never actually reach here, but the arm still has to exist for the
        // match to stay exhaustive over the whole enum.
        WandEffect::Light => {}
    }
}

/// Wand of light: reveal — instantly — the whole room the zapper stands in (a
/// dark room is lit for good), or the whole passage if they are in a corridor.
/// Any hidden trap in the lit area comes to light too.
fn light_area(world: &mut World, user: Entity, from: Position) {
    let map = world.resource::<Map>().clone();
    let here = map.tile(from.x, from.y);
    let in_room = matches!(
        here,
        TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
    );

    // Flood-fill from the zapper's tile through tiles of the same "space": room
    // floor + doorways + stairs for a room, passage tiles for a corridor.
    let connects = |t: TileType| {
        if !in_room {
            return t == TileType::Passage;
        }
        matches!(
            t,
            TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
        )
    };

    let mut area: Vec<(u16, u16)> = Vec::new();
    let mut seen: HashSet<(u16, u16)> = HashSet::new();
    let mut queue: VecDeque<(u16, u16)> = VecDeque::new();
    queue.push_back((from.x, from.y));
    seen.insert((from.x, from.y));
    while let Some((cx, cy)) = queue.pop_front() {
        area.push((cx, cy));
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if seen.contains(&(nx, ny)) || !connects(map.tile(nx, ny)) {
                    continue;
                }
                seen.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }

    // Clear the dark flag and fold every lit tile into the player's memory.
    {
        let mut map_mut = world.resource_mut::<Map>();
        for &(x, y) in &area {
            map_mut.light_tile(x, y);
        }
    }
    if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
        if vs.revealed_tiles.len() < MAP_WIDTH as usize * MAP_HEIGHT as usize {
            vs.revealed_tiles
                .grow(MAP_WIDTH as usize * MAP_HEIGHT as usize);
        }
        for &(x, y) in &area {
            vs.revealed_tiles.insert(tile_index(x, y));
        }
        vs.dirty = true;
    }

    // Bring any hidden trap in the lit area to light.
    let lit: HashSet<(u16, u16)> = area.iter().copied().collect();
    let sprung: Vec<(Entity, String)> = world
        .query_filtered::<(Entity, &Position, &Trap), With<Hidden>>()
        .iter(world)
        .filter(|(_, p, t)| !t.revealed && lit.contains(&(p.x, p.y)))
        .map(|(e, _, t)| {
            (
                e,
                format!("{} {}", t.effect.label_article(), t.effect.label()),
            )
        })
        .collect();
    for (trap, label) in sprung {
        world.entity_mut(trap).remove::<Hidden>();
        if let Some(mut t) = world.get_mut::<Trap>(trap) {
            t.revealed = true;
        }
        world
            .resource_mut::<GameLog>()
            .add(format!("The light reveals {label}!"));
    }

    let msg = if in_room {
        "Warm light floods the room."
    } else {
        "Light races the length of the passage."
    };
    world.resource_mut::<GameLog>().add(msg.to_string());
}

/// Wand of polymorph: replace the monster on `pos` with a different species,
/// fresh, on the same tile.
fn polymorph_target(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add("The bolt of change fizzles against nothing.".to_string());
        return;
    };
    polymorph_entity(world, victim);
}

/// Polymorph applied to one creature — a monster becomes a fresh random species
/// on its tile; the player, who cannot be swapped out from under themselves,
/// just feels briefly rearranged.
pub(super) fn polymorph_entity(world: &mut World, victim: Entity) {
    if world.get::<Player>(victim).is_some() {
        world
            .resource_mut::<GameLog>()
            .add("You feel like a new person.".to_string());
        return;
    }
    if world.get::<Mob>(victim).is_none() {
        return;
    }
    let pos = match world.get::<Position>(victim) {
        Some(p) => *p,
        None => return,
    };
    let old_name = item_label(world, victim);
    world.entity_mut(victim).despawn();

    // One roll, not a re-roll until it differs. A loop that spins until the
    // bestiary hands back something else is a hang waiting for the day the
    // table is short enough — and the dungeon has a better answer anyway:
    // the magic worked, the creature changed, it simply changed into another
    // one of itself. A rat that is visibly not the rat you were fighting is
    // funnier than a guarantee, and costs one branch instead of a loop.
    let idx = {
        let mut rng = world.resource_mut::<GameRng>();
        rng.0.gen_range(0..BESTIARY.len())
    };
    let def = &BESTIARY[idx];
    let new_name = def.name.to_string();
    spawn_monster(world, def, pos);
    let line = match new_name == old_name {
        true => format!("The {old_name} twists and warps into a different-looking {new_name}!"),
        false => format!(
            "The {old_name} twists and warps into {} {new_name}!",
            article_for(&new_name)
        ),
    };
    world.resource_mut::<GameLog>().add(line);
    leave_smoke_ring(world, pos);
}

/// Wand of haste / slow monster: step the target one notch along the speed scale
/// (permanently — but a hasted or slowed *player* loses the change on the next
/// floor, see [`crate::map::transition_level`]).
fn shift_target_speed(world: &mut World, pos: Position, faster: bool) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add("Nothing there to enchant.".to_string());
        return;
    };
    shift_entity_speed(world, victim, faster);
}

/// Wand of teleport away: fling the target monster to a random open tile.
fn teleport_target_away(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add("The wand's pull finds nothing.".to_string());
        return;
    };
    teleport_entity_away(world, victim);
}

/// Fling one creature to a random open tile. For the player this is exactly the
/// scroll of teleportation (see [`teleport_reader`]); for a monster it is a
/// yank into the dark. Either way, the wand's own signature — smoke left where
/// they stood — marks the departure; the scroll gets no such flourish.
pub(super) fn teleport_entity_away(world: &mut World, victim: Entity) {
    if world.get::<Player>(victim).is_some() {
        let old_pos = world.get::<Position>(victim).copied();
        teleport_reader(world, victim);
        if let Some(old_pos) = old_pos {
            leave_smoke(world, old_pos);
        }
        return;
    }
    let name = item_label(world, victim);
    let old_pos = world.get::<Position>(victim).copied();
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
    }
    if let Some(old_pos) = old_pos {
        leave_smoke(world, old_pos);
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} is yanked away into the dark."));
}

/// Wand of teleport to: drag the target monster to a tile next to the zapper.
/// With nothing on the aimed tile the magic still has to land on *someone* — and
/// the only body in range is the wielder's own.
fn teleport_target_here(world: &mut World, user: Entity, user_pos: Position, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        teleport_entity_to_self(world, user);
        return;
    };
    let name = item_label(world, victim);
    let old_pos = world.get::<Position>(victim).copied();
    let spot =
        crate::helpers::free_adjacent_tile(world, user_pos).or_else(|| random_open_tile(world));
    if let Some((x, y)) = spot {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
    }
    if let Some(old_pos) = old_pos {
        leave_smoke(world, old_pos);
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} is dragged to your side!"));
}

/// Teleport-to with no other target: the creature arrives exactly where it
/// started, to no effect but the indignity.
pub(super) fn teleport_entity_to_self(world: &mut World, who: Entity) {
    let msg = if world.get::<Player>(who).is_some() {
        "You teleport straight to yourself. What a trip.".to_string()
    } else {
        format!(
            "The {} teleports directly to themselves.",
            item_label(world, who)
        )
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Wand of cancellation: strip every marker effect the target has and reset its
/// tempo to normal. It keeps its name, fighting stats, movement and everything
/// else that makes it a creature. Because it walks [`crate::effects::EFFECTS`],
/// a newly added effect is cancellable the moment it joins the registry.
fn cancel_target(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add("The grey ray strikes only stone.".to_string());
        return;
    };
    cancel_entity(world, victim);
}

/// Cancellation applied to one creature. A monster loses its magic and its
/// tempo. The player loses far more (see [`cancel_player`]).
pub(super) fn cancel_entity(world: &mut World, victim: Entity) {
    if world.get::<Player>(victim).is_some() {
        cancel_player(world, victim);
        return;
    }
    let name = item_label(world, victim);
    revoke_all(world, victim);
    if let Some(mut speed) = world.get_mut::<Speed>(victim) {
        speed.kind = SpeedKind::Normal;
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name}'s magic sputters and dies."));
}

/// Cancellation caught the player in its blast. This is a catastrophe: every
/// enchantment on their weapons and armour — good or ill — is wiped to zero,
/// every unread scroll goes blank, every potion turns to plain water. The one
/// mercy is that a curse counts as magic too, so it lifts without taking the
/// item with it.
fn cancel_player(world: &mut World, player: Entity) {
    revoke_all(world, player);
    clear_player_conditions(world, player);

    let mut carried: Vec<Entity> = world
        .get::<Backpack>(player)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();
    carried.extend(equipped_items(world, player));

    for item in carried {
        let mut em = world.entity_mut(item);
        // Which plus a piece of gear wears is read off the item, exactly as
        // `catalog::enchant_equipment` reads it when the dungeon rolls one on:
        // a `PowerDie` is a weapon, an `ArmorDie` is armour, a `Launcher` is a
        // bow whose enchantment had no melee roll to land on. Miss the third
        // and a +3 bow walks out of a grey wave still +3.
        if em.contains::<PowerDie>() && em.contains::<PowerBonus>() {
            em.insert(PowerBonus(0));
        }
        if em.contains::<ArmorDie>() && em.contains::<ArmorBonus>() {
            em.insert(ArmorBonus(0));
        }
        if em.contains::<Launcher>() && em.contains::<ThrowBonus>() {
            em.insert(ThrowBonus(0));
        }
        em.remove::<Curse>();
        // An item is only ever one of these, so the two blocks never both fire.
        if let Some(mut scroll) = em.get_mut::<Scroll>() {
            scroll.effect = ScrollEffect::BlankPaper;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = "scroll of blank paper".to_string();
            }
        }
        if let Some(mut potion) = em.get_mut::<Potion>() {
            potion.effect = PotionEffect::Water;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = "potion of thirst quenching".to_string();
            }
        }
    }

    // The gear's numbers just changed, so re-fold what it lends its wearer.
    sync_equipment_effects(world, player);
    world
        .resource_mut::<GameLog>()
        .add("A grey wave washes over you. Your pack goes quiet, your gear goes plain, and every curse on you simply lets go.".to_string());
}
