//! Zapping a wand, and everything a bolt or a blast does once it lands.
//!
//! The catalog ([`crate::catalog::WANDS`]) says what each wand is called, how it
//! draws and how far it reaches; this file resolves the zap. A thrown wand — the
//! whole battery let go at once — is [`super::throwing`]'s job, but it borrows
//! most of its machinery ([`elemental_blast`], [`blast_palette`], the
//! per-creature effects) from here.

use bevy_ecs::{entity::Entity, prelude::With, world::World};
use crossterm::style::Color;
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::{HashSet, VecDeque};

use crate::components::*;
use crate::conditions::{clear_player_conditions, confuse, shift_entity_speed};
use crate::effects::*;
use crate::equipment::{equipped_items, sync_equipment_effects};
use crate::helpers::{
    Hit, apply_hit, get_entities_at_position, get_line, item_label, leave_smoke, monster_at,
    monster_or_ally_at, player_sees, roll_dice, spill_blood,
};
use crate::identify::article_for;
use crate::map::{GameRng, MAP_HEIGHT, MAP_WIDTH, Map, Smoke, TileType, tile_index};
use crate::monsters::{BESTIARY, MonsterDef, reshape};
use crate::particles::{BlastPalette, Particles};
use crate::shake::{ShakeKind, kick_shake};
use crate::traps::{random_open_tile, things_in};

use super::scrolls::teleport_reader;
use crate::constants::spirits::HELPER_BLOWN_UP_ALIGNMENT;
use crate::constants::wands::{
    BLAST_RADIUS, DAMAGE_DICE, DAMAGE_SIDES, DIG_RANGE, SHOCK_GORE_DAMAGE, SHOCK_SPLASHES,
    SMOKE_LINGER_TURNS, SYSTEM_SHOCK_CHANCE,
};

/// A wand's damage: `[DAMAGE_DICE]d[DAMAGE_SIDES]`, rolled once per zap and
/// applied whole to every creature it touches (armour is never subtracted).
fn roll_wand_damage(world: &mut World) -> i32 {
    roll_dice(world, DAMAGE_DICE, DAMAGE_SIDES)
}

/// Applies `damage` of `element` (or non-elemental if `None`) to `entity`.
/// Returns how much HP was actually taken off.
///
/// This used to be where the ward and the immunity were decided, and it was
/// the only damage path in the game that decided both. It is a one-line
/// wrapper over [`apply_hit`] now, which is where every path decides them.
fn damage_with_element(
    world: &mut World,
    entity: Entity,
    damage: i32,
    element: Option<Element>,
) -> i32 {
    let hit = Hit {
        amount: damage,
        element,
        magical: true,
    };
    apply_hit(world, entity, hit, None)
}

/// The spell Magic Ward turning away a hit: a flash off the chest and
/// whatever was coming for it — a bolt, a breath, a fistful of fire —
/// ricochets off at a random angle in a random bright colour and is gone.
/// Purely cosmetic; the damage above is already zeroed by the time this
/// plays, so a headless world (no [`Particles`] resource) just skips it.
pub(crate) fn ward_ricochet(world: &mut World, victim: Entity) {
    let Some(pos) = world.get::<Position>(victim).copied() else {
        return;
    };
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    const COLORS: [Color; 6] = [
        Color::Yellow,
        Color::Cyan,
        Color::Magenta,
        Color::White,
        Color::Green,
        Color::Red,
    ];
    let (dx, dy, color) = {
        let mut rng = world.resource_mut::<GameRng>();
        let (dx, dy) = DIRS[rng.0.gen_range(0..DIRS.len())];
        let color = COLORS[rng.0.gen_range(0..COLORS.len())];
        (dx, dy, color)
    };
    let mut cells = Vec::new();
    for step in 1..=3 {
        let Some(cell) =
            crate::particles::on_map(pos.x as i32 + dx * step, pos.y as i32 + dy * step)
        else {
            break;
        };
        cells.push(cell);
    }
    let Some(mut fx) = world.get_resource_mut::<Particles>() else {
        return;
    };
    fx.hit_spark(pos.x, pos.y);
    fx.hurl(&cells, '*', color);
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
            .add(strings::drain_life_gained(taken));
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

/// Every tile a blast of `radius` around `center` reaches: within the disc and
/// in the centre's line of sight (walls stop the flames), each tagged with its
/// distance from the centre so the animation can ripple outward. What a blast
/// covers, asked by [`elemental_blast`] before it burns and by an agent before
/// it breathes (see `crate::agents`).
pub(crate) fn blast_cells(map: &Map, center: Position, radius: f32) -> Vec<(u16, u16, f32)> {
    let cx = center.x as i32;
    let cy = center.y as i32;
    let r = radius.ceil() as i32;
    let mut cells = Vec::new();
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
                cells.push((tx, ty, dist));
            }
        }
    }
    cells
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
pub(crate) fn elemental_blast(
    world: &mut World,
    shooter: Option<Entity>,
    center: Position,
    radius: f32,
    damage: i32,
    element: Option<Element>,
    palette: BlastPalette,
) -> Vec<Entity> {
    let blast_cells = blast_cells(world.resource::<Map>(), center, radius);

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

    for &entity in &affected_entities {
        if world.get::<Fighter>(entity).is_some_and(|f| f.hp <= 0) {
            crate::combat::finish_indirect_kill(world, entity, Some(center));
        }
    }

    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&blast_cells, palette);
        if matches!(element, Some(Element::Fire) | Some(Element::Cold)) {
            fx.smoke_burst(&blast_cells);
        }
    }

    if player_sees(world, center.x, center.y) {
        kick_shake(world, ShakeKind::Heavy);
    }
    if element == Some(Element::Fire) {
        let mut smoke = world.resource_mut::<Smoke>();
        for &(x, y, _) in &blast_cells {
            smoke.puff(x, y, SMOKE_LINGER_TURNS);
        }
    }

    for trap in things_in::<Trap>(world, &cell_set) {
        crate::traps::detonate_trap(world, trap, shooter);
    }
    for coin in things_in::<Pickup>(world, &cell_set) {
        crate::traps::detonate_pickup(world, coin, shooter);
    }
    for potion in things_in::<Potion>(world, &cell_set) {
        super::potions::detonate_potion(world, potion, shooter);
    }

    affected_entities
}

/// Puffs a small ring of smoke around `center` — the polymorph flourish.
/// Every open neighbouring tile (never through a wall) gets its own puff,
/// staggered a beat apart so it reads as smoke rolling outward from the
/// transformed creature rather than every tile igniting at once.
pub(crate) fn leave_smoke_ring(world: &mut World, center: Position) {
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
        WandEffect::DrainLife => BlastPalette::Death,
        WandEffect::Light => BlastPalette::Glam,
        WandEffect::Cancellation => BlastPalette::Void,
        WandEffect::Polymorph
        | WandEffect::HasteMonster
        | WandEffect::SlowMonster
        | WandEffect::TeleportAway
        | WandEffect::TeleportTo
        | WandEffect::Charming
        | WandEffect::Digging
        | WandEffect::Swapping
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
        strings::dazzle_player_line(),
        LogCategory::Dazzle,
        strings::dazzle_mob_verb(),
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

    if effect == WandEffect::Light {
        light_area(world, user, user_pos);
        return;
    }

    let target_pos = match target {
        Some(pos) => pos,
        None => return,
    };

    if let Some(seen) = monster_at(world, target_pos) {
        crate::abilities::fire_on_targeted(world, user, seen);
    }

    match effect {
        WandEffect::MagicMissile => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            strings::bolt_magic_missile(),
            Color::Cyan,
        ),
        WandEffect::Lightning => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            strings::bolt_lightning(),
            Color::Yellow,
        ),
        WandEffect::Striking => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            strings::bolt_striking(),
            Color::White,
        ),
        WandEffect::DrainLife => fire_bolt(
            world,
            user,
            user_pos,
            target_pos,
            effect,
            strings::bolt_drain_life(),
            Color::DarkMagenta,
        ),
        WandEffect::Fire | WandEffect::Cold => {
            let is_fire = effect == WandEffect::Fire;
            let msg = if is_fire {
                strings::blast_fire()
            } else {
                strings::blast_cold()
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
        WandEffect::Charming => charm_target(world, target_pos),
        WandEffect::Digging => dig_tunnel(world, user_pos, target_pos),
        WandEffect::Swapping => swap_with_target(world, user, user_pos, target_pos),
        WandEffect::Nothing => {
            world
                .resource_mut::<GameLog>()
                .add(strings::wand_does_nothing());
        }
        WandEffect::Light => {}
    }
}

/// Wand of digging: every wall on the line from the zapper through the aimed
/// tile, out to [`DIG_RANGE`], becomes passage. The line runs on past the tile
/// the reticle is on, so aiming at the rock in front of you is enough. The
/// map's outer wall is not rock: it stays, or a tunnel would walk the player
/// off the grid.
fn dig_tunnel(world: &mut World, from: Position, aim: Position) {
    let (dx, dy) = (aim.x as i32 - from.x as i32, aim.y as i32 - from.y as i32);
    let reach = dx.abs().max(dy.abs());
    if reach == 0 {
        world
            .resource_mut::<GameLog>()
            .add(strings::wand_does_nothing().to_string());
        return;
    }
    let end = Position {
        x: (from.x as i32 + dx * DIG_RANGE / reach).clamp(0, MAP_WIDTH as i32 - 1) as u16,
        y: (from.y as i32 + dy * DIG_RANGE / reach).clamp(0, MAP_HEIGHT as i32 - 1) as u16,
    };
    let dug = break_rock(
        world,
        get_line(from, end).into_iter().skip(1).map(|c| (c.x, c.y)),
    );
    if dug.is_empty() {
        world
            .resource_mut::<GameLog>()
            .add(strings::wand_does_nothing().to_string());
        return;
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::dig_crumbles().to_string());
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.beam(&dug, Color::DarkYellow);
    }
}

/// Turns every wall among `cells` into passage and returns the ones it did.
/// What [`Map::diggable`] refuses stays: the outer wall, or a tunnel would
/// walk the player off the grid, and the walls of an undiggable room. What
/// the player can see changes shape, so every viewshed is redone.
fn break_rock(world: &mut World, cells: impl Iterator<Item = (u16, u16)>) -> Vec<(u16, u16)> {
    let mut dug = Vec::new();
    {
        let mut map = world.resource_mut::<Map>();
        for (x, y) in cells {
            if map.tile(x, y) == TileType::Wall && map.diggable(x, y) {
                map.tiles[tile_index(x, y)] = TileType::Passage;
                dug.push((x, y));
            }
        }
    }
    if !dug.is_empty() {
        let mut views = world.query::<&mut Viewshed>();
        for mut vs in views.iter_mut(world) {
            vs.dirty = true;
        }
    }
    dug
}

/// A thrown wand of digging bursting on `center`: every wall in the disc of
/// `radius` goes, in sight of the centre or not.
pub(super) fn crater(world: &mut World, center: Position, radius: f32) {
    let r = radius.ceil() as i32;
    let disc = (-r..=r)
        .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| ((dx * dx + dy * dy) as f32).sqrt() <= radius)
        .filter_map(|(dx, dy)| {
            crate::particles::on_map(center.x as i32 + dx, center.y as i32 + dy)
        });
    let dug = break_rock(world, disc);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        for &(x, y) in &dug {
            let dist = ((x as f32 - center.x as f32).powi(2)
                + (y as f32 - center.y as f32).powi(2))
            .sqrt();
            fx.poof(x, y, dist * 40.0);
        }
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
            .add(strings::light_reveals(&label));
    }

    let msg = if in_room {
        strings::light_floods_room()
    } else {
        strings::light_races_passage()
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Wand of polymorph: replace the monster on `pos` with a different species,
/// fresh, on the same tile. Aimed at the player's own tile, it polymorphs the
/// player ([`polymorph_player_into`]).
pub(super) fn polymorph_target(world: &mut World, pos: Position) {
    let player = world
        .query_filtered::<(Entity, &Position), With<Player>>()
        .iter(world)
        .find(|(_, p)| **p == pos)
        .map(|(e, _)| e);
    let Some(victim) = player.or_else(|| monster_or_ally_at(world, pos)) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::polymorph_fizzles());
        return;
    };
    polymorph_entity(world, victim);
}

/// Wand of charming: the monster on `pos`, tamed on the spot — a plain
/// [`Faction::Ally`], not the Helper (see [`crate::companion::charm`]). Aimed
/// at the player's own tile, it has nothing to take hold of and just tickles.
fn charm_target(world: &mut World, pos: Position) {
    if world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .any(|p| *p == pos)
    {
        world.resource_mut::<GameLog>().add(strings::charm_self());
        return;
    }
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::charm_fizzles());
        return;
    };
    let name = item_label(world, victim);
    crate::companion::charm(world, victim);
    world
        .resource_mut::<GameLog>()
        .add(strings::charm_target_line(&name));
}

/// Polymorph applied to one creature — a monster becomes a fresh random species
/// on its tile; the player, who cannot be swapped out from under themselves,
/// borrows a random species' powers for the floor ([`polymorph_player_into`]).
///
/// Either way the creature is left [`Polymorphed`], and polymorphing one that
/// already is has two outcomes at even odds: system shock ([`system_shock`]),
/// or the creature settles into a chimeric form ([`assume_form`]).
pub(super) fn polymorph_entity(world: &mut World, victim: Entity) {
    polymorph_entity_with(world, victim, true);
}

/// [`polymorph_entity`], with the system shock switched off for a polymorph
/// that must never kill: the ring's, which comes due on its own. Without the
/// shock, polymorphing the [`Polymorphed`] is simply a chimeric form.
pub(super) fn polymorph_entity_with(world: &mut World, victim: Entity, can_shock: bool) {
    let is_player = world.get::<Player>(victim).is_some();
    if !is_player && world.get::<Mob>(victim).is_none() {
        return;
    }
    if world.get::<SustainsForm>(victim).is_some() {
        let line = match is_player {
            true => strings::form_holds_player().to_string(),
            false => strings::form_holds_mob(&item_label(world, victim)),
        };
        world.resource_mut::<GameLog>().add(line);
        return;
    }
    let again = world.get::<Polymorphed>(victim).is_some();
    if again
        && can_shock
        && world
            .resource_mut::<GameRng>()
            .0
            .gen_bool(SYSTEM_SHOCK_CHANCE)
    {
        system_shock(world, victim);
        return;
    }
    if is_player {
        let pool: Vec<&'static MonsterDef> = BESTIARY
            .iter()
            .filter(|m| m.spirit_kind.is_none())
            .collect();
        let idx = world.resource_mut::<GameRng>().0.gen_range(0..pool.len());
        polymorph_player_into(world, victim, pool[idx]);
        return;
    }
    let pos = match world.get::<Position>(victim) {
        Some(p) => *p,
        None => return,
    };
    let old_name = item_label(world, victim);

    let idx = {
        let mut rng = world.resource_mut::<GameRng>();
        rng.0.gen_range(0..BESTIARY.len())
    };
    let def = &BESTIARY[idx];
    let mut new_name = def.display_name().to_string();
    let new = reshape(world, victim, def);
    lend(world, new, Grant::of::<Polymorphed>(), Lifetime::Permanent);
    if again {
        new_name = assume_form(world, new).to_string();
    }
    let line = match new_name == old_name {
        true => strings::polymorph_same_looking(&old_name, &new_name),
        false => strings::polymorph_different(&old_name, article_for(&new_name), &new_name),
    };
    world.resource_mut::<GameLog>().add(line);
    leave_smoke_ring(world, pos);
}

/// The player takes on `def`'s powers until they leave the floor: its innate
/// grants, each a [`Lifetime::Floor`] ledger entry, beside a [`Polymorphed`]
/// badge. Name, glyph, numbers and spells stay the player's own, so nothing is
/// swapped out and nothing new is saved — the ledger already is. What a dog
/// *is* is not a power and is not lent ([`Grant::is_identity`]).
///
/// A shape with no [`ItemUser`] has no hands: everything worn comes off into
/// the pack, and [`crate::body::equip_refusal`] keeps it there.
pub fn polymorph_player_into(world: &mut World, player: Entity, def: &'static MonsterDef) {
    let again = world.get::<Polymorphed>(player).is_some();
    lend(world, player, Grant::of::<Polymorphed>(), Lifetime::Floor);
    for grant in def.grants.iter().filter(|g| !g.is_identity()) {
        lend(world, player, *grant, Lifetime::Floor);
    }
    let name = match again {
        true => assume_form(world, player),
        false => def.display_name(),
    };
    world
        .resource_mut::<GameLog>()
        .add(strings::polymorph_self(article_for(name), name));

    let worn = equipped_items(world, player);
    if world.get::<ItemUser>(player).is_none() && !worn.is_empty() {
        for item in worn {
            crate::equipment::force_unequip(world, item);
        }
        sync_equipment_effects(world, player);
        world
            .resource_mut::<GameLog>()
            .add(strings::polymorph_drops_gear());
    }
    if let Some(pos) = world.get::<Position>(player).copied() {
        leave_smoke_ring(world, pos);
    }
}

/// Settles `who` into one of the chimeric [`FORMS`] for the floor, in place of
/// any form it held, and returns the form's name.
fn assume_form(world: &mut World, who: Entity) -> &'static str {
    let held: Vec<Grant> = FORMS.iter().map(|f| f.grant).collect();
    revoke_any(world, who, &held);
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..FORMS.len());
    lend(world, who, FORMS[idx].grant, Lifetime::Floor);
    FORMS[idx].name
}

/// What polymorphing the [`Polymorphed`] can do instead of a form: the creature
/// comes apart. A monster bursts, dead, in a great deal of gore. The player is
/// left on 1 HP, with the same gore: it is a near thing, not an ending.
fn system_shock(world: &mut World, victim: Entity) {
    for _ in 0..SHOCK_SPLASHES {
        spill_blood(world, victim, SHOCK_GORE_DAMAGE, false);
    }
    if world.get::<Player>(victim).is_some() {
        if let Some(mut f) = world.get_mut::<Fighter>(victim) {
            f.hp = 1;
        }
        world
            .resource_mut::<GameLog>()
            .add(strings::system_shock_player());
        return;
    }
    if world.get::<Helper>(victim).is_some() {
        crate::spirits::shift_player_alignment(world, HELPER_BLOWN_UP_ALIGNMENT);
    }
    let name = item_label(world, victim);
    world
        .resource_mut::<GameLog>()
        .add(strings::system_shock_mob(&name));
    crate::combat::finish_indirect_kill(world, victim, None);
}

/// Wand of haste / slow monster: step the target one notch along the speed scale
/// (permanently — but a hasted or slowed *player* loses the change on the next
/// floor, see [`crate::map::transition_level`]).
fn shift_target_speed(world: &mut World, pos: Position, faster: bool) {
    let Some(victim) = monster_or_ally_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::nothing_to_enchant());
        return;
    };
    shift_entity_speed(world, victim, faster);
}

/// Wand of teleport away: fling the target monster to a random open tile.
fn teleport_target_away(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::teleport_pull_finds_nothing());
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
    let Some((x, y)) = random_open_tile(world) else {
        burst_in_transit(world, victim, old_pos);
        return;
    };
    if let Some(mut p) = world.get_mut::<Position>(victim) {
        p.x = x;
        p.y = y;
    }
    crate::effects::revoke_any(world, victim, &crate::effects::HOLDS);
    if let Some(old_pos) = old_pos {
        leave_smoke(world, old_pos);
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::yanked_into_dark(&name));
}

/// Wand of teleport to: drag the target monster to a tile next to the zapper.
/// With nothing on the aimed tile the magic still has to land on *someone* — and
/// the only body in range is the wielder's own.
///
/// Boxed in, the drag simply fails. It used to fall back to
/// [`random_open_tile`] — which is a teleport *away* wearing teleport-to's
/// "dragged to your side!" message, the one outcome this wand exists not to
/// produce.
fn teleport_target_here(world: &mut World, user: Entity, user_pos: Position, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        teleport_entity_to_self(world, user);
        return;
    };
    let name = item_label(world, victim);
    let Some((x, y)) = crate::helpers::free_adjacent_tile(world, user_pos) else {
        burst_in_transit(world, victim, Some(user_pos));
        return;
    };
    let old_pos = world.get::<Position>(victim).copied();
    if let Some(mut p) = world.get_mut::<Position>(victim) {
        p.x = x;
        p.y = y;
    }
    if let Some(old_pos) = old_pos {
        leave_smoke(world, old_pos);
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::dragged_to_your_side(&name));
}

/// A teleport with nowhere to put the passenger. The magic does not politely
/// fizzle: it pulls anyway, and what comes out the far end is not in one piece.
///
/// Both wands end here — teleport away with the floor full, teleport to with
/// no room at the zapper's side — because both are the same failure, and both
/// used to resolve it by quietly doing nothing (or, worse, by flinging the
/// target somewhere the message didn't claim). `source` is what the gore flies
/// away from. The kill goes through [`crate::combat::finish_indirect_kill`]
/// like every other death with no swinger behind it, so it pays score, drops
/// gear and bursts exactly as a bolt's kill does.
fn burst_in_transit(world: &mut World, victim: Entity, source: Option<Position>) {
    let name = item_label(world, victim);
    world
        .resource_mut::<GameLog>()
        .add(strings::bursts_in_transit(&name));
    crate::combat::finish_indirect_kill(world, victim, source);
}

/// Teleport-to with no other target: the creature arrives exactly where it
/// started, to no effect but the indignity.
pub(super) fn teleport_entity_to_self(world: &mut World, who: Entity) {
    let msg = if world.get::<Player>(who).is_some() {
        strings::teleport_self_player().to_string()
    } else {
        strings::teleport_self_mob(&item_label(world, who))
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Wand of swapping: the zapper and whatever stands on the aimed tile trade
/// places. A creature comes first, then a trap somebody has found (a hidden one
/// is passed over, the same rule as an aimed shot in
/// [`crate::traps::detonate_at`]), then an item. Nobody is set down where they
/// could not stand: aimed at a ghost in the rock, or by a ghost in the rock, the
/// swap fails.
///
/// A teleport by another name, so it keeps the teleport's rules: no trap
/// springs under either side, and whatever held them lets go.
fn swap_with_target(world: &mut World, user: Entity, user_pos: Position, pos: Position) {
    let partner = swap_partner(world, user, pos)
        .filter(|&other| can_stand(world, user, pos) && can_stand(world, other, user_pos));
    let Some(other) = partner else {
        world
            .resource_mut::<GameLog>()
            .add(strings::swap_finds_nothing());
        return;
    };
    let msg = if world.get::<Player>(user).is_some() {
        strings::you_swap_places(&item_label(world, other))
    } else {
        let with = world
            .get::<Player>(other)
            .is_none()
            .then(|| item_label(world, other));
        strings::mob_swaps_places(&item_label(world, user), with.as_deref())
    };
    relocate(world, user, pos);
    relocate(world, other, user_pos);
    world.resource_mut::<GameLog>().add(msg);
}

/// What on `pos` a swap trades with, in [`swap_with_target`]'s order.
fn swap_partner(world: &mut World, user: Entity, pos: Position) -> Option<Entity> {
    let rank = |world: &World, e: Entity| {
        if world.get::<Mob>(e).is_some() || world.get::<Player>(e).is_some() {
            Some(0)
        } else if world.get::<Trap>(e).is_some() {
            world.get::<Hidden>(e).is_none().then_some(1)
        } else {
            world.get::<Item>(e).map(|_| 2)
        }
    };
    get_entities_at_position(world, pos)
        .into_iter()
        .filter(|&e| e != user)
        .filter_map(|e| Some((rank(world, e)?, e)))
        .min_by_key(|&(r, _)| r)
        .map(|(_, e)| e)
}

/// Whether `who` can be set down on `pos`: anywhere for a phasing creature,
/// deep water too for a swimmer, dry floor for everyone and everything else.
fn can_stand(world: &World, who: Entity, pos: Position) -> bool {
    world.get::<Phasing>(who).is_some()
        || world
            .resource::<Map>()
            .walkable(pos.x, pos.y, world.get::<Swims>(who).is_some())
}

/// Sets `who` down on `to` the way a teleport does: a magenta puff where they
/// land, a fresh view, and out of whatever held them.
fn relocate(world: &mut World, who: Entity, to: Position) {
    if let Some(mut pos) = world.get_mut::<Position>(who) {
        *pos = to;
    }
    if let Some(mut vs) = world.get_mut::<Viewshed>(who) {
        vs.dirty = true;
    }
    revoke_any(world, who, &HOLDS);
    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.tinted_poof(to.x, to.y, 0.0, Color::Magenta);
    }
}

/// A thrown wand of swapping: the creatures the blast caught trade places, each
/// onto the tile of the next in a random ring, so nobody keeps their own. Only
/// creatures on dry floor join, since anyone can stand there; a ghost in the
/// rock or a swimmer in deep water sits it out.
pub(super) fn shuffle_places(world: &mut World, caught: &[Entity]) {
    let mut ring: Vec<(Entity, Position)> = caught
        .iter()
        .filter(|&&e| world.get::<Mob>(e).is_some() || world.get::<Player>(e).is_some())
        .filter_map(|&e| Some((e, *world.get::<Position>(e)?)))
        .filter(|(_, p)| world.resource::<Map>().walkable(p.x, p.y, false))
        .collect();
    if ring.len() < 2 {
        return;
    }
    ring.shuffle(&mut world.resource_mut::<GameRng>().0);
    for (i, &(who, _)) in ring.iter().enumerate() {
        relocate(world, who, ring[(i + 1) % ring.len()].1);
    }
    world
        .resource_mut::<GameLog>()
        .add(strings::blast_shuffles_places());
}

/// Wand of cancellation: strip every marker effect the target has and reset its
/// tempo to normal. It keeps its name, fighting stats, movement and everything
/// else that makes it a creature. Because it walks [`crate::effects::EFFECTS`],
/// a newly added effect is cancellable the moment it joins the registry.
fn cancel_target(world: &mut World, pos: Position) {
    let Some(victim) = monster_at(world, pos) else {
        world
            .resource_mut::<GameLog>()
            .add(strings::cancellation_strikes_stone());
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
        .add(strings::cancellation_sputters(&name));
}

/// Cancellation caught the player in its blast. This is a catastrophe: every
/// enchantment on their weapons and armour — good or ill — is wiped to zero,
/// every unread scroll goes blank, every potion turns to plain water. The one
/// mercy is that a curse counts as magic too, so it lifts without taking the
/// item with it.
fn cancel_player(world: &mut World, player: Entity) {
    clear_player_conditions(world, player);
    revoke_all(world, player);

    let mut carried: Vec<Entity> = world
        .get::<Backpack>(player)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();
    carried.extend(equipped_items(world, player));

    for item in carried {
        let mut em = world.entity_mut(item);
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
        if let Some(mut scroll) = em.get_mut::<Scroll>() {
            scroll.effect = ScrollEffect::BlankPaper;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = strings::content_name("scroll of blank paper").to_string();
            }
        }
        if let Some(mut rune) = em.get_mut::<Rune>() {
            rune.effect = RuneEffect::Blank;
            rune.charged = false;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = strings::content_name("blank rune").to_string();
            }
        }
        if let Some(mut potion) = em.get_mut::<Potion>() {
            potion.effect = PotionEffect::Water;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = strings::content_name("potion of thirst quenching").to_string();
            }
        }
    }

    sync_equipment_effects(world, player);
    world
        .resource_mut::<GameLog>()
        .add_colored(strings::cancellation_player_wave(), LogCategory::Curse);
}
