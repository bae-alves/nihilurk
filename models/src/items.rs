use crate::effects::{
    ArmorBonus, ArmorDie, ColdImmune, FireImmune, Grant, ItemUser, PowerBonus, PowerDie,
    ThrowBonus, Undead, equipped_total, revoke_all,
};
use crate::equipment::{
    Equipped, Slot, equip_silently, equipped_in, equipped_items, force_unequip,
    sync_equipment_effects, toggle_equipped,
};
use crate::identify::Identified;
use crate::magicmap::{MagicMapReveal, MagicMapStyle};
use crate::monsters::{BESTIARY, spawn_monster};
use crate::traps::{Trap, random_open_tile};
use crate::{
    components::*,
    helpers::{apply_damage, get_entities_at_position, get_line, total_armor_plus},
    map::{GameRng, MAP_HEIGHT, MAP_WIDTH, Map, TileType, tile_index},
    particles::{BlastPalette, Particles},
};
use bevy_ecs::{
    entity::Entity,
    prelude::{Or, With},
    world::World,
};
use crossterm::style::Color;
use rand::Rng;
use std::collections::HashSet;
use std::collections::VecDeque;

// --- Tuning constants ------------------------------------------------------
// Defined and documented in `constants.rs`. Re-exported under their historical
// names so `items::THROW_RANGE` / `items::BLAST_RADIUS` keep resolving.
//
//   THROW_RANGE                  how far any item can be hurled (tiles)
//   BLAST_RADIUS / GRENADE_RADIUS zapped disc vs. thrown-attack-wand grenade
//   wands::DAMAGE_DICE/SIDES      a zapped wand's damage roll (3d3)
//   wands::GRENADE_/EFFECT_DIE_PER_CHARGE   per-charge dice a thrown wand spends
pub use crate::constants::items::THROW_RANGE;
use crate::constants::loot::LAUNCHER_DIE_MULTIPLIER;
pub use crate::constants::wands::{BLAST_RADIUS, GRENADE_RADIUS};
use crate::constants::wands::{
    DAMAGE_DICE, DAMAGE_SIDES, EFFECT_DIE_PER_CHARGE, GRENADE_DIE_PER_CHARGE,
};

/// Works a potion on `user`. Returns whether the dose visibly took hold — the
/// player learns a potion by drinking it either way, but a potion *thrown* at a
/// monster only gives itself away when something plainly happens (see
/// [`resolve_throw`]).
fn apply_potion_effect(world: &mut World, user: Entity, effect: PotionEffect) -> bool {
    match effect {
        PotionEffect::Healing => {
            let Some(mut fighter) = world.get_mut::<Fighter>(user) else {
                return false;
            };
            let before = fighter.hp;
            fighter.hp = fighter.max_hp;
            let healed = fighter.hp > before;
            let msg = if world.get::<Player>(user).is_some() {
                "You feel refreshed as your wounds mend!".to_string()
            } else {
                format!(
                    "The {} glows eerily, wounds closing.",
                    item_label(world, user)
                )
            };
            world.resource_mut::<GameLog>().add(msg);
            healed
        }
        _ => false, /* handle other potion effects */
    }
}

/// The three flavours of elemental wand. A creature can be immune to one — and
/// the immunity is a plain component, so a dragon's innate `FireImmune` and a
/// future ring of fire resistance's are the same thing to this code.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Element {
    Fire,
    Cold,
    Drain,
}

impl Element {
    /// Which element a wand's damage carries, if any. Non-elemental damage
    /// (magic missile, lightning, striking) returns `None` and is never resisted.
    fn of(effect: WandEffect) -> Option<Element> {
        match effect {
            WandEffect::Fire => Some(Element::Fire),
            WandEffect::Cold => Some(Element::Cold),
            WandEffect::DrainLife => Some(Element::Drain),
            _ => None,
        }
    }

    /// The effect that shrugs this element off.
    fn immunity(self) -> Grant {
        match self {
            Element::Fire => Grant::of::<FireImmune>(),
            Element::Cold => Grant::of::<ColdImmune>(),
            Element::Drain => Grant::of::<Undead>(),
        }
    }

    /// The word for this element in an "unharmed by the ___" log line.
    fn noun(self) -> &'static str {
        match self {
            Element::Fire => "flames",
            Element::Cold => "cold",
            Element::Drain => "evil magic",
        }
    }
}

/// Whether `entity` shrugs off `element`, from any source.
fn is_immune(world: &World, entity: Entity, element: Element) -> bool {
    element.immunity().probe(world, entity)
}

/// Rolls `Nd[DAMAGE_SIDES]` — the die every wand and every blast is measured in.
fn roll_d3s(world: &mut World, dice: i32) -> i32 {
    let mut rng = world.resource_mut::<GameRng>();
    (0..dice).map(|_| rng.0.gen_range(1..=DAMAGE_SIDES)).sum()
}

/// A wand's damage: `[DAMAGE_DICE]d[DAMAGE_SIDES]`, rolled once per zap and
/// applied whole to every creature it touches (armour is never subtracted — see
/// [`apply_damage`]).
fn roll_wand_damage(world: &mut World) -> i32 {
    roll_d3s(world, DAMAGE_DICE)
}

/// The hostile monster standing on `pos`, if any.
fn monster_at(world: &mut World, pos: Position) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .find(|(_, p, f)| p.x == pos.x && p.y == pos.y && **f == Faction::Monster)
        .map(|(e, _, _)| e)
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
    if let Some(el) = element {
        if is_immune(world, entity, el) {
            if let Some(name) = world.get::<Name>(entity).map(|n| n.what.clone()) {
                world
                    .resource_mut::<GameLog>()
                    .add(format!("The {name} is unharmed by the {}.", el.noun()));
            }
            return 0;
        }
    }
    let Some(hp_before) = world.get::<Fighter>(entity).map(|f| f.hp) else {
        return 0;
    };
    apply_damage(world, entity, damage);
    damage.min(hp_before.max(0))
}

/// Blows a disc of `radius` tiles open around `center`: every creature standing
/// on a tile the centre can see (walls stop the flames) takes `damage` of
/// `element`, and the animation ripples outward from the core.
///
/// The one place an area blast is resolved — a zapped wand of fire and a thrown
/// one differ by the number passed in, and by nothing else. Nobody is exempt,
/// the thrower included.
fn elemental_blast(
    world: &mut World,
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

    if let Some(mut fx) = world.get_resource_mut::<Particles>() {
        fx.explosion(&blast_cells, palette);
    }

    affected_entities
}

/// The blast-animation palette a wand's explosion burns in.
fn blast_palette(effect: WandEffect) -> BlastPalette {
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

/// Dazzle — the wand of light's confusion. A monster is switched to a
/// random-walk ([`MovementType::Confused`]); the player picks up the [`Confused`]
/// condition, which rides along until they take a staircase or are cancelled.
fn dazzle(world: &mut World, entity: Entity) {
    if world.get::<Player>(entity).is_some() {
        if world.get::<Confused>(entity).is_none() {
            world.entity_mut(entity).insert(Confused);
            world
                .resource_mut::<GameLog>()
                .add("The flash leaves you reeling — you are dazzled!".to_string());
        }
        return;
    }
    if world.get::<Mob>(entity).is_some() {
        let name = item_label(world, entity);
        if let Some(mut mob) = world.get_mut::<Mob>(entity) {
            mob.movement_type = MovementType::Confused;
        }
        world
            .resource_mut::<GameLog>()
            .add(format!("The {name} is dazzled!"));
    }
}

/// Clears every transient condition the player is carrying — [`SpeedKind`]
/// haste/slow and [`Confused`] — logging each one it lifts. The only two things
/// that trigger it: using a staircase ([`crate::map::transition_level`]) and a
/// wand of cancellation ([`cancel_player`]).
pub(crate) fn clear_player_conditions(world: &mut World, player: Entity) {
    let mut lifted: Vec<&str> = Vec::new();
    if let Some(mut speed) = world.get_mut::<Speed>(player) {
        match speed.kind {
            SpeedKind::Fast => {
                speed.kind = SpeedKind::Normal;
                lifted.push("hasted");
            }
            SpeedKind::Slow => {
                speed.kind = SpeedKind::Normal;
                lifted.push("slowed");
            }
            SpeedKind::Normal => {}
        }
    }
    if world.get::<Confused>(player).is_some() {
        world.entity_mut(player).remove::<Confused>();
        lifted.push("confused");
    }
    for cond in lifted {
        world
            .resource_mut::<GameLog>()
            .add(format!("You are no longer {cond}."));
    }
}

/// Rolls `count` dice of `sides` and sums them — the measure a thrown wand's
/// blast is counted in (`d3` per remaining charge for an effect wand, `d4` for
/// an attack wand).
fn roll_dice(world: &mut World, count: i32, sides: i32) -> i32 {
    let mut rng = world.resource_mut::<GameRng>();
    (0..count.max(0)).map(|_| rng.0.gen_range(1..=sides)).sum()
}

/// The wands that deal damage when zapped. Thrown, these go off wider and hotter
/// than the utility ("effect") wands.
fn is_attack_wand(effect: WandEffect) -> bool {
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

fn apply_wand_effect(
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

    // Bolt wands: travel a straight line to the target, damaging everything on
    // the way. `None` means this effect isn't a damaging bolt. The colour is the
    // one the animated beam streaks in.
    let bolt: Option<(&str, Color)> = match effect {
        WandEffect::MagicMissile => {
            Some(("A brilliant cyan bolt leaps from the wand!", Color::Cyan))
        }
        WandEffect::Lightning => Some(("A forking bolt of lightning cracks out!", Color::Yellow)),
        WandEffect::Striking => Some(("An invisible fist hammers down the line!", Color::White)),
        WandEffect::DrainLife => Some((
            "A tendril of black light drinks the life from its path.",
            Color::DarkMagenta,
        )),
        _ => None,
    };

    match effect {
        _ if bolt.is_some() => {
            let (msg, color) = bolt.unwrap();
            let element = Element::of(effect);
            let damage = roll_wand_damage(world);
            world.resource_mut::<GameLog>().add(msg.to_string());
            let map = world.resource::<Map>().clone();
            let line_points = get_line(user_pos, target_pos);
            let mut beam_cells: Vec<(u16, u16)> = Vec::new();
            let mut drained = 0;
            for pos in line_points {
                if map.blocks(pos.x, pos.y) {
                    break;
                }
                if !(pos.x == user_pos.x && pos.y == user_pos.y) {
                    beam_cells.push((pos.x, pos.y));
                }
                let entities_at_pos = get_entities_at_position(world, pos);
                for entity in entities_at_pos {
                    if entity != user {
                        drained += damage_with_element(world, entity, damage, element);
                    }
                }
            }
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                fx.beam(&beam_cells, color);
            }
            // The wand of drain life feeds the life it takes straight back to the
            // zapper (never past their maximum).
            if effect == WandEffect::DrainLife && drained > 0 {
                if let Some(mut fighter) = world.get_mut::<Fighter>(user) {
                    fighter.hp = (fighter.hp + drained).min(fighter.max_hp);
                }
                world
                    .resource_mut::<GameLog>()
                    .add(format!("You drain {drained} life."));
            }
        }
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
        // Light is handled above; the remaining arms are the damaging wands.
        WandEffect::Light => {}
        _ => {
            world
                .resource_mut::<GameLog>()
                .add("The wand discharges with a faint hiss.".to_string());
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
        if in_room {
            matches!(
                t,
                TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
            )
        } else {
            t == TileType::Passage
        }
    };

    let mut area: Vec<(u16, u16)> = Vec::new();
    let mut seen: std::collections::HashSet<(u16, u16)> = std::collections::HashSet::new();
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
    let lit: std::collections::HashSet<(u16, u16)> = area.iter().copied().collect();
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
fn polymorph_entity(world: &mut World, victim: Entity) {
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

    // Roll a bestiary entry that isn't what we started with.
    let def = loop {
        let idx = {
            let mut rng = world.resource_mut::<GameRng>();
            rng.0.gen_range(0..BESTIARY.len())
        };
        let candidate = &BESTIARY[idx];
        if candidate.name != old_name {
            break candidate;
        }
    };
    let new_name = def.name.to_string();
    spawn_monster(world, def, pos);
    world.resource_mut::<GameLog>().add(format!(
        "The {old_name} twists and warps into {} {new_name}!",
        crate::identify::article_for(&new_name)
    ));
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

/// Step one creature — monster or player — along the speed scale.
fn shift_entity_speed(world: &mut World, victim: Entity, faster: bool) {
    let is_player = world.get::<Player>(victim).is_some();
    let name = item_label(world, victim);
    let Some(mut speed) = world.get_mut::<Speed>(victim) else {
        return;
    };
    let before = speed.kind;
    speed.kind = if faster {
        before.faster()
    } else {
        before.slower()
    };
    let after = speed.kind;
    let msg = if after == before && is_player {
        format!(
            "You are already as {} as you can be.",
            if faster { "quick" } else { "sluggish" }
        )
    } else if after == before {
        format!(
            "The {name} is already as {} as it can be.",
            if faster { "quick" } else { "sluggish" }
        )
    } else if is_player && faster {
        "The world lurches into slow motion around you.".to_string()
    } else if is_player {
        "Your limbs turn to lead.".to_string()
    } else if faster {
        format!("The {name} blurs into sudden speed.")
    } else {
        format!("The {name} lurches into slow motion.")
    };
    world.resource_mut::<GameLog>().add(msg);
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
/// yank into the dark.
fn teleport_entity_away(world: &mut World, victim: Entity) {
    if world.get::<Player>(victim).is_some() {
        teleport_reader(world, victim);
        return;
    }
    let name = item_label(world, victim);
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
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
    let spot = free_adjacent_tile(world, user_pos).or_else(|| random_open_tile(world));
    if let Some((x, y)) = spot {
        if let Some(mut p) = world.get_mut::<Position>(victim) {
            p.x = x;
            p.y = y;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add(format!("The {name} is dragged to your side!"));
}

/// Teleport-to with no other target: the creature arrives exactly where it
/// started, to no effect but the indignity.
fn teleport_entity_to_self(world: &mut World, who: Entity) {
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
fn cancel_entity(world: &mut World, victim: Entity) {
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
        let is_gear = em.contains::<PowerDie>() || em.contains::<ArmorDie>();
        if is_gear {
            if em.contains::<PowerBonus>() {
                em.insert(PowerBonus(0));
            }
            if em.contains::<ArmorBonus>() {
                em.insert(ArmorBonus(0));
            }
        }
        em.remove::<Curse>();
        if let Some(mut scroll) = em.get_mut::<Scroll>() {
            scroll.effect = ScrollEffect::BlankPaper;
            if let Some(mut name) = em.get_mut::<Name>() {
                name.what = "scroll of blank paper".to_string();
            }
        } else if let Some(mut potion) = em.get_mut::<Potion>() {
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

/// Destroys every cursed item `user` currently has equipped (a scroll of remove
/// curse): each one is unequipped, pulled out of the pack and despawned. Cursed
/// items sitting unequipped in the pack are left untouched. Returns how many
/// items were destroyed.
pub(crate) fn lift_curses(world: &mut World, user: Entity) -> usize {
    let doomed: Vec<Entity> = equipped_items(world, user)
        .into_iter()
        .filter(|&e| world.get::<Curse>(e).is_some())
        .collect();

    for &e in &doomed {
        force_unequip(world, e);
        if let Some(mut bp) = world.get_mut::<Backpack>(user) {
            bp.items.retain(|&i| i != e);
        }
        world.entity_mut(e).despawn();
    }
    // The gear is gone, so whatever it was lending its wearer goes with it.
    crate::equipment::sync_equipment_effects(world, user);
    doomed.len()
}

/// An item's display name, or a vague fallback.
pub(crate) fn item_label(world: &World, item: Entity) -> String {
    world
        .get::<Name>(item)
        .map(|n| n.what.clone())
        .unwrap_or_else(|| "item".to_string())
}

/// Picks a uniformly random item in `user`'s backpack whose true type isnwhich has no interactive item picker (yet).'t
/// identified yet and identifies it directly. Used by
/// [`ScrollEffect::Identify`],
fn identify_random_unknown_item(world: &mut World, user: Entity) {
    let candidates: Vec<Entity> = world
        .get::<Backpack>(user)
        .map(|bp| bp.items.clone())
        .unwrap_or_default();

    let is_unidentified = |world: &World, e: Entity| -> bool {
        let identified = world.resource::<Identified>();
        world
            .get::<Potion>(e)
            .is_some_and(|p| !identified.potions.contains(&p.effect))
            || world
                .get::<Scroll>(e)
                .is_some_and(|s| !identified.scrolls.contains(&s.effect))
            || world
                .get::<Wand>(e)
                .is_some_and(|w| !identified.wands.contains(&w.effect))
            || world
                .get::<Ring>(e)
                .is_some_and(|r| !identified.rings.contains(&r.effect))
    };

    let unknown: Vec<Entity> = candidates
        .into_iter()
        .filter(|&e| is_unidentified(world, e))
        .collect();
    if unknown.is_empty() {
        world
            .resource_mut::<GameLog>()
            .add("You already recognise everything in your pack.".to_string());
        return;
    }
    let target = {
        let mut rng = world.resource_mut::<GameRng>();
        unknown[rng.0.gen_range(0..unknown.len())]
    };

    let name = item_label(world, target);
    let potion_effect = world.get::<Potion>(target).map(|p| p.effect);
    let scroll_effect = world.get::<Scroll>(target).map(|s| s.effect);
    let wand_effect = world.get::<Wand>(target).map(|w| w.effect);
    let ring_effect = world.get::<Ring>(target).map(|r| r.effect);

    let mut identified = world.resource_mut::<Identified>();
    if let Some(effect) = potion_effect {
        identified.potions.insert(effect);
    }
    if let Some(effect) = scroll_effect {
        identified.scrolls.insert(effect);
    }
    if let Some(effect) = wand_effect {
        identified.wands.insert(effect);
    }
    if let Some(effect) = ring_effect {
        identified.rings.insert(effect);
    }
    drop(identified);

    world
        .resource_mut::<GameLog>()
        .add(format!("The scroll identifies your {name}!"));
}

fn apply_scroll_effect(world: &mut World, user: Entity, effect: ScrollEffect) {
    if effect == ScrollEffect::Identify {
        identify_random_unknown_item(world, user);
        return;
    }
    if effect == ScrollEffect::RemoveCurse {
        let freed = lift_curses(world, user);
        let msg = if freed > 0 {
            "You feel as though somebody is watching over you. Your cursed gear crumbles away."
        } else {
            "You feel as though somebody is watching over you."
        };
        world.resource_mut::<GameLog>().add(msg.to_string());
        return;
    }
    if effect == ScrollEffect::MagicMapping {
        // Roll the wipe's shape (or take the `ROOG_MAGICMAP` dev override), then
        // arm it centred on the reader. The engine plays it out frame by frame
        // after the turn (see [`crate::magicmap`]); headless callers with no
        // reveal resource just skip the animation.
        let hero = world
            .get::<Position>(user)
            .map(|p| (p.x, p.y))
            .unwrap_or((MAP_WIDTH / 2, MAP_HEIGHT / 2));
        let style = std::env::var("ROOG_MAGICMAP")
            .ok()
            .and_then(|v| MagicMapStyle::from_name(&v))
            .unwrap_or_else(|| MagicMapStyle::roll(&mut world.resource_mut::<GameRng>().0));
        if let Some(mut reveal) = world.get_resource_mut::<MagicMapReveal>() {
            reveal.start(hero, style);
        }
        world
            .resource_mut::<GameLog>()
            .add(style.flavour().to_string());
        return;
    }
    match effect {
        ScrollEffect::Teleportation => teleport_reader(world, user),
        ScrollEffect::AggravateMonsters => aggravate_floor(world, user),
        ScrollEffect::CreateMonster => create_monster(world, user),
        ScrollEffect::ScareMonster => {
            let scared = scare_in_view(world, user);
            let msg = if scared > 0 {
                "The parchment flares with the pathos of fear!"
            } else {
                "The parchment radiates a menacing aura, but nothing is here to feel it."
            };
            world.resource_mut::<GameLog>().add(msg.to_string());
        }
        ScrollEffect::VorpalizeWeapon => vorpalize_wielded_weapon(world, user),
        ScrollEffect::BlankPaper => {
            world
                .resource_mut::<GameLog>()
                .add("The scroll is blank. Someone got the last laugh.".to_string());
        }
        _ => {
            world
                .resource_mut::<GameLog>()
                .add("You read the scroll, but nothing obvious happens.".to_string());
        }
    }
}

/// Scroll of teleportation: whisk the reader to a random open tile somewhere on
/// the current floor.
fn teleport_reader(world: &mut World, user: Entity) {
    if let Some((x, y)) = random_open_tile(world) {
        if let Some(mut pos) = world.get_mut::<Position>(user) {
            pos.x = x;
            pos.y = y;
        }
        if let Some(mut vs) = world.get_mut::<Viewshed>(user) {
            vs.dirty = true;
        }
    }
    world
        .resource_mut::<GameLog>()
        .add("BLONK! You are whisked away!".to_string());
}

/// Scroll of aggravate monsters: every creature on the floor drops what it was
/// doing and homes in on the reader's tile — in or out of sight. See
/// [`MovementType::Aggravated`].
fn aggravate_floor(world: &mut World, user: Entity) {
    aggravate_all_monsters(world, user);
    world
        .resource_mut::<GameLog>()
        .add("A shrill shriek rips through the dungeon. Everything on this floor heard it — and it knows where you are.".to_string());
}

/// The bare mechanic: point every hostile on the floor at `origin`'s tile. The
/// scroll of aggravate monsters wraps this in its own flavour; so does the
/// [`crate::effects::AggravatesMonsters`] passive (see [`crate::abilities`]).
pub(crate) fn aggravate_all_monsters(world: &mut World, origin: Entity) {
    let Some(&hero) = world.get::<Position>(origin) else {
        return;
    };
    let mobs: Vec<Entity> = world
        .query_filtered::<Entity, With<Mob>>()
        .iter(world)
        .collect();
    for m in mobs {
        if world.get::<Faction>(m) != Some(&Faction::Monster) {
            continue;
        }
        if let Some(mut mob) = world.get_mut::<Mob>(m) {
            mob.movement_type = MovementType::Aggravated {
                tx: hero.x,
                ty: hero.y,
            };
        }
    }
}

/// Scroll of scare monster: every monster currently in the reader's view turns
/// tail for good. Returns how many were scared.
fn scare_in_view(world: &mut World, user: Entity) -> usize {
    let seen: HashSet<(u16, u16)> = world
        .get::<Viewshed>(user)
        .map(|v| v.visible_tiles.iter().copied().collect())
        .unwrap_or_default();
    let targets: Vec<Entity> = world
        .query_filtered::<(Entity, &Position, &Faction), With<Mob>>()
        .iter(world)
        .filter(|(_, p, f)| **f == Faction::Monster && seen.contains(&(p.x, p.y)))
        .map(|(e, _, _)| e)
        .collect();
    for t in &targets {
        if let Some(mut mob) = world.get_mut::<Mob>(*t) {
            mob.movement_type = MovementType::Flee;
        }
    }
    targets.len()
}

/// A free walkable tile next to `origin` that no entity is standing on, chosen
/// at random. `None` if the reader is boxed in.
fn free_adjacent_tile(world: &mut World, origin: Position) -> Option<(u16, u16)> {
    let occupied: HashSet<(u16, u16)> = world
        .query::<&Position>()
        .iter(world)
        .map(|p| (p.x, p.y))
        .collect();
    let opts: Vec<(u16, u16)> = {
        let map = world.resource::<Map>();
        let mut v = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (origin.x as i32 + dx, origin.y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if !map.blocks(nx, ny) && !occupied.contains(&(nx, ny)) {
                    v.push((nx, ny));
                }
            }
        }
        v
    };
    if opts.is_empty() {
        return None;
    }
    let idx = world.resource_mut::<GameRng>().0.gen_range(0..opts.len());
    Some(opts[idx])
}

/// Scroll of create monster: conjure any creature from the bestiary next to the
/// reader (or, failing an open adjacent tile, anywhere on the floor).
fn create_monster(world: &mut World, user: Entity) {
    let origin = world.get::<Position>(user).copied();
    let spot = origin
        .and_then(|o| free_adjacent_tile(world, o))
        .or_else(|| random_open_tile(world));
    let Some((x, y)) = spot else {
        world.resource_mut::<GameLog>().add(
            "The air curdles — then settles. Whatever was coming thought better of it.".to_string(),
        );
        return;
    };
    let idx = {
        let mut rng = world.resource_mut::<GameRng>();
        rng.0.gen_range(0..BESTIARY.len())
    };
    let e = spawn_monster(world, &BESTIARY[idx], Position { x, y });
    let name = item_label(world, e);
    world.resource_mut::<GameLog>().add(format!(
        "The air curdles into {} {name}, teeth and all!",
        crate::identify::article_for(&name)
    ));
}

/// Scroll of vorpalize weapon: brand the reader's wielded weapon [`Vorpal`]
/// against one random species (it already bites clean through any Jabberwock).
/// A weapon can only take the edge once — read it over an already-vorpal weapon
/// and the blade can't hold the second enchantment: it crumbles to nothing.
fn vorpalize_wielded_weapon(world: &mut World, user: Entity) {
    let weapon = equipped_in(world, user, Slot::Hand);
    let Some(weapon) = weapon else {
        world
            .resource_mut::<GameLog>()
            .add("The scroll gutters out, failing to brand a weapon.".to_string());
        return;
    };
    if world.get::<Vorpal>(weapon).is_some() {
        let wname = item_label(world, weapon);
        force_unequip(world, weapon);
        if let Some(mut bp) = world.get_mut::<Backpack>(user) {
            bp.items.retain(|&i| i != weapon);
        }
        world.entity_mut(weapon).despawn();
        world
            .resource_mut::<GameLog>()
            .add(format!("The {wname} screams in pain and crumbles to dust."));
        return;
    }
    let bane = {
        let mut rng = world.resource_mut::<GameRng>();
        BESTIARY[rng.0.gen_range(0..BESTIARY.len())]
            .name
            .to_string()
    };
    world
        .entity_mut(weapon)
        .insert(Vorpal { bane: bane.clone() });
    let wname = item_label(world, weapon);
    world.resource_mut::<GameLog>().add(format!(
        "The {wname} sings with a razor light, an omen of death to any {bane}."
    ));
}

// ---------------------------------------------------------------------------
// Throwing
// ---------------------------------------------------------------------------

/// Why `user` can't throw `item`, if they can't. Two things stay in the pack:
/// the Element of Yoord, which is the whole point of the run and is not to be
/// flung down a corridor, and cursed gear, which is welded on — it won't come
/// off, so it can be neither dropped nor hurled. Anything else is fair game.
pub fn throw_refusal(world: &World, user: Entity, item: Entity) -> Option<String> {
    if world.get::<Amulet>(item).is_some() {
        return Some("The Element of Yoord will not leave your hand.".to_string());
    }
    drop_refusal(world, user, item)
}

/// Why `user` can't put `item` down, if they can't. Cursed gear is welded on;
/// the Element of Yoord, unlike a thrown one, *can* be set down — abandoning the
/// run's prize on the floor is the player's business.
pub fn drop_refusal(world: &World, user: Entity, item: Entity) -> Option<String> {
    let equipped = world.get::<Equipped>(item)?;
    if equipped.by != Some(user) || world.get::<Curse>(item).is_none() {
        return None;
    }
    Some(
        equipped
            .slot
            .stuck(&crate::identify::display_name(world, item)),
    )
}

/// Marks `item`'s true type as known, announcing it the same way using one
/// yourself does. Watching a monster drink, read or put on what you threw at it
/// teaches you exactly as much as doing it would have.
fn identify_from_afar(world: &mut World, item: Entity) {
    let true_name = item_label(world, item);
    let potion = world.get::<Potion>(item).map(|p| p.effect);
    let scroll = world.get::<Scroll>(item).map(|s| s.effect);
    let wand = world.get::<Wand>(item).map(|w| w.effect);
    let ring = world.get::<Ring>(item).map(|r| r.effect);
    let mut known = world.resource_mut::<Identified>();
    let newly = match (potion, scroll, wand, ring) {
        (Some(e), _, _, _) => known.potions.insert(e),
        (_, Some(e), _, _) => known.scrolls.insert(e),
        (_, _, Some(e), _) => known.wands.insert(e),
        (_, _, _, Some(e)) => known.rings.insert(e),
        _ => false,
    };
    drop(known);
    if newly {
        world.resource_mut::<GameLog>().add(format!(
            "That was {} {true_name}!",
            crate::identify::article_for(&true_name)
        ));
    }
}

/// The creature standing on `pos` — anything that acts, friend or foe — never
/// counting `except`, the thrower whose own tile the item is leaving.
fn actor_at(world: &mut World, pos: Position, except: Entity) -> Option<Entity> {
    world
        .query_filtered::<(Entity, &Position), Or<(With<Mob>, With<Player>)>>()
        .iter(world)
        .find(|(e, p)| *e != except && **p == pos)
        .map(|(e, _)| e)
}

/// Traces a throw: the tiles the item crosses (the thrower's own excluded), the
/// tile it comes to rest on, and every creature it ran into, in the order it
/// reached them.
///
/// A wall always stops it short of the aimed spot. So does the first creature in
/// the way — which is the whole point of aiming past one — unless the thing in
/// flight is [`Piercing`], in which case it runs the line to its end and the
/// list comes back with everyone standing in it.
fn flight_path(
    world: &mut World,
    thrower: Entity,
    item: Entity,
    from: Position,
    to: Position,
) -> (Vec<(u16, u16)>, Position, Vec<Entity>) {
    let map = world.resource::<Map>().clone();
    let piercing = world.get::<Piercing>(item).is_some();
    let mut cells = Vec::new();
    let mut landing = from;
    let mut victims = Vec::new();
    for pos in get_line(from, to) {
        if pos == from {
            continue;
        }
        if map.blocks(pos.x, pos.y) {
            break;
        }
        cells.push((pos.x, pos.y));
        landing = pos;
        if let Some(victim) = actor_at(world, pos, thrower) {
            victims.push(victim);
            if !piercing {
                break;
            }
        }
    }
    (cells, landing, victims)
}

/// What a hurled object does on impact, or `None` if it is not the sort of thing
/// that hurts anyone: only an item carrying [`ThrownDamage`] rolls at all, so a
/// wand or a suit of armour just bounces off and falls.
///
/// The roll is `1d[thrown damage]`, plus the item's own enchantment, plus every
/// [`ThrowBonus`] the *thrower* is wearing — a ring of dexterity, the plus on the
/// bow in their hand. Three things bend it:
///
/// * **A launcher doubles the die.** A missile carrying [`LaunchedBy`] asks
///   whether its thrower has the effect it answers to; an arrow lobbed by hand
///   rolls `1d4`, the same arrow loosed from a bow rolls `1d8`. The bow is not
///   consulted — only the effect is, so a monster that picked one up shoots just
///   as well as you do.
/// * **A [`Projectile`] ignores armour.** A point already in the air does not
///   care what you are wearing.
/// * **Anything else is still blunted by it** — by the armour *plus* only, never
///   the die, exactly as a trap is.
fn roll_throw_damage(
    world: &mut World,
    thrower: Entity,
    item: Entity,
    target: Entity,
) -> Option<i32> {
    let mut die = world.get::<ThrownDamage>(item)?.0;
    if let Some(&LaunchedBy(launcher)) = world.get::<LaunchedBy>(item) {
        if launcher.probe(world, thrower) {
            die *= LAUNCHER_DIE_MULTIPLIER;
        }
    }
    if die < 1 {
        return Some(0);
    }
    let bonus = world.get::<PowerBonus>(item).map(|b| b.0).unwrap_or(0)
        + equipped_total::<ThrowBonus>(world, thrower);
    let roll = world.resource_mut::<GameRng>().0.gen_range(1..=die) + bonus;
    let soak = match world.get::<Projectile>(item) {
        Some(_) => 0,
        None => total_armor_plus(world, target),
    };
    Some((roll - soak).max(0))
}

/// Lays a thrown item down on the floor where it stopped, ready to be picked up
/// again.
fn land_item(world: &mut World, item: Entity, at: Position) {
    world.entity_mut(item).insert(at);
}

/// The single missile a throw actually looses.
///
/// A quiver does not leave your hand when you shoot it: it gives up one arrow
/// and stays exactly where it was in the pack, one lighter — `slot` is the row
/// it came from, so it goes back there instead of to the bottom of the list.
/// Anything that is not a stack is thrown whole and comes back unchanged.
///
/// Called with `item` already lifted out of the pack, which is how both the
/// engine's Throw action and a scripted throw hand an item over.
pub fn draw_one(world: &mut World, thrower: Entity, item: Entity, slot: Option<usize>) -> Entity {
    let Some(count) = world.get::<Stack>(item).map(|s| s.count).filter(|&c| c > 1) else {
        return item;
    };
    let Some(one) = crate::catalog::split_one(world, item) else {
        return item;
    };
    if let Some(mut stack) = world.get_mut::<Stack>(item) {
        stack.count = count - 1;
    }
    if let Some(mut pack) = world.get_mut::<Backpack>(thrower) {
        let at = slot.unwrap_or(pack.items.len()).min(pack.items.len());
        pack.items.insert(at, item);
    }
    one
}

/// Takes `item` into `carrier`'s pack, and returns what was taken as it reads in
/// a sentence — `"a dagger"`, `"7 arrows"` — or `None` if there was no pack to
/// put it in.
///
/// Ammunition merges. A bundle off the floor tops up the quivers already in the
/// pack rather than claiming a fresh inventory letter for every arrow, and only
/// what is left over after they are all full to [`STACK_LIMIT`] takes a slot of
/// its own. Everything else claims its own slot, exactly as it always has.
pub fn stow(world: &mut World, carrier: Entity, item: Entity) -> Option<String> {
    let Some(mut left) = world.get::<Stack>(item).map(|s| s.count) else {
        let label = crate::identify::with_article(world, item);
        world.get_mut::<Backpack>(carrier)?.items.push(item);
        world.entity_mut(item).remove::<Position>();
        return Some(label);
    };

    // Every quiver of the same thing that still has room, in pack order.
    let name = world.get::<Name>(item)?.what.clone();
    let quivers: Vec<Entity> = world
        .get::<Backpack>(carrier)?
        .items
        .iter()
        .copied()
        .filter(|&e| {
            e != item
                && world.get::<Name>(e).is_some_and(|n| n.what == name)
                && world.get::<Stack>(e).is_some_and(|s| s.count < STACK_LIMIT)
        })
        .collect();

    let taking = left;
    for quiver in quivers {
        if left == 0 {
            break;
        }
        let Some(mut stack) = world.get_mut::<Stack>(quiver) else {
            continue;
        };
        let moved = left.min(STACK_LIMIT - stack.count);
        stack.count += moved;
        left -= moved;
    }

    match left {
        // Every last one went into a quiver: the pile has nothing left to be.
        0 => {
            world.entity_mut(item).despawn();
        }
        // The overflow takes a slot of its own rather than being left behind.
        _ => {
            if let Some(mut stack) = world.get_mut::<Stack>(item) {
                stack.count = left;
            }
            world.get_mut::<Backpack>(carrier)?.items.push(item);
            world.entity_mut(item).remove::<Position>();
        }
    }
    Some(crate::identify::counted(&name, taking))
}

/// A thrown wand bursts where it lands, spending every charge it had left in one
/// go.
///
/// * An **attack** wand throws the wide, hot grenade — `d4` a charge at
///   [`GRENADE_RADIUS`], armour-ignoring, elemental where the wand is.
/// * The **wand of light** throws the same grenade (`d4` a charge, wide) but
///   [`dazzle`]s every creature it catches instead of carrying an element.
/// * A **utility** wand throws a smaller [`BLAST_RADIUS`] blast that deals *no*
///   damage — the payload is the effect, worked on every creature caught.
/// * The **wand of nothing** just makes confetti.
fn resolve_wand_throw(
    world: &mut World,
    item: Entity,
    landing: Position,
    effect: WandEffect,
    seen_name: &str,
) {
    let charges = world
        .get::<Battery>(item)
        .map(|b| b.charges as i32)
        .unwrap_or(0)
        .max(0);

    match effect {
        WandEffect::Nothing => {
            world.resource_mut::<GameLog>().add(format!(
                "The {seen_name} bursts in a shower of colourful confetti. That's it. That's the whole spell."
            ));
            if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                for (i, (dx, dy)) in [
                    (0i32, 0i32),
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                ]
                .into_iter()
                .enumerate()
                {
                    if let Some((x, y)) =
                        crate::particles::on_map(landing.x as i32 + dx, landing.y as i32 + dy)
                    {
                        let color = if i % 2 == 0 {
                            Color::Magenta
                        } else {
                            Color::Cyan
                        };
                        fx.blip(x, y, '*', color);
                    }
                }
            }
            return;
        }
        _ => {}
    }

    let is_attack = is_attack_wand(effect);
    let is_light = effect == WandEffect::Light;
    // The wand of light throws the same wide, hot grenade an attack wand does —
    // it just blinds instead of burning through armour.
    let grenade = is_attack || is_light;
    let (radius, sides) = if grenade {
        (GRENADE_RADIUS, GRENADE_DIE_PER_CHARGE)
    } else {
        (BLAST_RADIUS, EFFECT_DIE_PER_CHARGE)
    };
    // Attack wands and the light wand deal damage; the utility wands' blasts are
    // pure delivery — the effect is the whole payload.
    let damage = if grenade {
        roll_dice(world, charges, sides)
    } else {
        0
    };
    let element = if is_attack { Element::of(effect) } else { None };

    world.resource_mut::<GameLog>().add(format!(
        "The {seen_name} shatters, and {charges} charges' worth of magic gets out at once!"
    ));

    let caught = elemental_blast(
        world,
        landing,
        radius,
        damage,
        element,
        blast_palette(effect),
    );

    if is_attack {
        return;
    }
    for entity in caught {
        // Only creatures answer to a wand's effect — a scroll lying in the blast
        // is not "confused". And an earlier victim may already have been
        // teleported clear or replaced outright.
        let is_creature =
            world.get::<Mob>(entity).is_some() || world.get::<Player>(entity).is_some();
        if !is_creature || world.get::<Position>(entity).is_none() {
            continue;
        }
        if is_light {
            dazzle(world, entity);
        } else {
            apply_thrown_wand_effect(world, entity, effect);
        }
    }
}

/// The per-creature half of a thrown utility wand: the same change a zap would
/// have made, applied to everyone the blast reached.
fn apply_thrown_wand_effect(world: &mut World, entity: Entity, effect: WandEffect) {
    match effect {
        WandEffect::Polymorph => polymorph_entity(world, entity),
        WandEffect::HasteMonster => shift_entity_speed(world, entity, true),
        WandEffect::SlowMonster => shift_entity_speed(world, entity, false),
        WandEffect::TeleportAway => teleport_entity_away(world, entity),
        WandEffect::TeleportTo => teleport_entity_to_self(world, entity),
        WandEffect::Cancellation => cancel_entity(world, entity),
        _ => {}
    }
}

/// The schedule step that resolves everything hurled this turn.
pub fn throw_system(world: &mut World) {
    let throws = std::mem::take(&mut world.resource_mut::<ThrowQueue>().throws);
    for throw in throws {
        resolve_throw(world, throw);
    }
}

/// One thrown item, from the thrower's hand to whatever it finds along its line.
/// A potion shatters over its target and is drunk by it; a scroll is read aloud
/// by anything literate enough and otherwise flutters to the floor; everything
/// else simply arrives — hurting what it hits only if it carries
/// [`ThrownDamage`], and staying with a creature that knows what to do with it
/// ([`ItemUser`]).
///
/// Everything except a [`Piercing`] weapon resolves on the *first* creature in
/// the way and goes no further, whatever tile it was aimed at.
fn resolve_throw(world: &mut World, throw: WantsToThrow) {
    let WantsToThrow {
        thrower,
        item,
        target,
    } = throw;
    let Some(&origin) = world.get::<Position>(thrower) else {
        return;
    };

    // Gear leaves the hand the moment it is thrown, taking its bonuses with it.
    force_unequip(world, item);
    sync_equipment_effects(world, thrower);

    let seen_name = crate::identify::display_name(world, item);
    let announcement = if world.get::<Player>(thrower).is_some() {
        format!("You throw the {seen_name}.")
    } else {
        format!("The {} throws the {seen_name}.", item_label(world, thrower))
    };
    world.resource_mut::<GameLog>().add(announcement);

    let (cells, landing, victims) = flight_path(world, thrower, item, origin, target);
    let victim = victims.first().copied();
    if let Some((glyph, color)) = world.get::<Renderable>(item).map(|r| (r.glyph, r.color)) {
        if let Some(mut fx) = world.get_resource_mut::<Particles>() {
            fx.hurl(&cells, glyph, color);
        }
    }

    // A potion is glass: it breaks on whatever it reaches, and whoever wears it
    // gets the dose.
    if let Some(effect) = world.get::<Potion>(item).map(|p| p.effect) {
        match victim {
            Some(v) => {
                let victim_name = item_label(world, v);
                world.resource_mut::<GameLog>().add(format!(
                    "The {seen_name} bursts over the {victim_name}, which splutters and swallows a mouthful!"
                ));
                // A dose that plainly did something names the potion for you; one
                // that fizzled keeps its secret.
                if apply_potion_effect(world, v, effect) {
                    identify_from_afar(world, item);
                }
            }
            None => {
                world
                    .resource_mut::<GameLog>()
                    .add(format!("The {seen_name} shatters on the floor."));
            }
        }
        world.entity_mut(item).despawn();
        return;
    }

    // A scroll only means something to a creature that can read it. Anything
    // else it bounces off, and it can be picked up again.
    if let Some(effect) = world.get::<Scroll>(item).map(|s| s.effect) {
        match victim.filter(|&v| world.get::<ItemUser>(v).is_some()) {
            Some(reader) => {
                let who = item_label(world, reader);
                world.resource_mut::<GameLog>().add(format!(
                    "The {who} unrolls the {seen_name} and reads it aloud!"
                ));
                apply_scroll_effect(world, reader, effect);
                // The words were spoken out loud, in front of you: whatever the
                // scroll was, it is no longer a mystery.
                identify_from_afar(world, item);
                world.entity_mut(item).despawn();
            }
            None => land_item(world, item, landing),
        }
        return;
    }

    // A wand is a stick with its magic held inside it. Hurl it instead of
    // zapping it and everything it was saving comes out at once, where it lands
    // — all its remaining charges spent in a single burst. It does not care
    // whose idea it was, so mind how close you are standing.
    //
    // But the glass only breaks on *impact*: the wand has to hit a creature or a
    // wall, or fly its full leash, before it goes off. Lobbed gently into open
    // floor, it just clatters down with its charges — and its secret — intact.
    if let Some(effect) = world.get::<Wand>(item).map(|w| w.effect) {
        let hit_creature = victim.is_some();
        let hit_wall = landing != target;
        let range_flown = (landing.x as i32 - origin.x as i32)
            .abs()
            .max((landing.y as i32 - origin.y as i32).abs());
        let spent_its_leash = range_flown >= THROW_RANGE;

        if hit_creature || hit_wall || spent_its_leash {
            resolve_wand_throw(world, item, landing, effect, &seen_name);
            identify_from_afar(world, item);
            world.entity_mut(item).despawn();
        } else {
            world.resource_mut::<GameLog>().add(format!(
                "The {seen_name} clatters to the floor, its magic still bottled up."
            ));
            land_item(world, item, landing);
        }
        return;
    }

    // Everything else flies as a missile. A dagger or a spear spends itself on
    // everyone standing in the line; anything else has exactly one victim, or
    // none.
    let Some(victim) = victim else {
        land_item(world, item, landing);
        return;
    };

    for &hit in &victims {
        let hit_name = item_label(world, hit);
        let msg = match roll_throw_damage(world, thrower, item, hit) {
            Some(damage) if damage > 0 => {
                let at = world.get::<Position>(hit).copied().unwrap_or(landing);
                apply_damage(world, hit, damage);
                if let Some(mut fx) = world.get_resource_mut::<Particles>() {
                    fx.hit_spark(at.x, at.y);
                }
                format!("The {seen_name} hits the {hit_name} for {damage} damage.")
            }
            // A weapon whose roll the armour ate.
            Some(_) => format!("The {seen_name} glances off the {hit_name}."),
            // Not a thing that hurts anyone: it simply arrives.
            None => format!("The {seen_name} bounces off the {hit_name}."),
        };
        world.resource_mut::<GameLog>().add(msg);
    }

    // A missile built for the flight is spent on what it found: an arrow snaps,
    // a spear is left where it stuck. Nothing catches one, and there is nothing
    // left on the floor to collect.
    if world.get::<Projectile>(item).is_some() {
        world.entity_mut(item).despawn();
        return;
    }

    // A creature the throw just killed keeps nothing; the reaper will lay the
    // rest of its gear out beside this.
    let victim_name = item_label(world, victim);
    let slain = world.get::<Fighter>(victim).is_some_and(|f| f.hp <= 0);
    let takes_it =
        !slain && world.get::<ItemUser>(victim).is_some() && world.get::<Equipped>(item).is_some();
    if takes_it && equip_silently(world, victim, item) {
        let slot = world.get::<Equipped>(item).map(|e| e.slot);
        let verb = match slot {
            Some(Slot::Hand) => "snatches it up and wields it",
            Some(Slot::Body) => "pulls it on",
            _ => "slips it on",
        };
        world
            .resource_mut::<GameLog>()
            .add(format!("The {victim_name} {verb}!"));
        identify_from_afar(world, item);
        return;
    }
    land_item(world, item, landing);
}

pub fn item_system(world: &mut World) {
    let mut use_queue = world.resource_mut::<UseQueue>();
    let uses = std::mem::take(&mut use_queue.uses);
    drop(use_queue);

    for item_use in uses {
        // What the player sees right now (appearance if unidentified, true
        // name otherwise) and the item's true name, captured before any
        // despawn below could make `item_use.item` unqueryable.
        let seen_name = crate::identify::display_name(world, item_use.item);
        let true_name = item_label(world, item_use.item);

        // We store the "work to be done" here
        let mut potion_effect: Option<PotionEffect> = None;
        let mut wand_effect: Option<WandEffect> = None;
        let mut scroll_effect: Option<ScrollEffect> = None;
        let mut is_equipment = false;
        let mut destroy_item = false;
        let mut return_to_inventory = false;

        {
            let mut item_entity = world.entity_mut(item_use.item);

            // Check for Potion
            if let Some(p) = item_entity.get::<Potion>() {
                potion_effect = Some(p.effect);
            }

            // Check for Wand
            if let Some(w) = item_entity.get::<Wand>() {
                wand_effect = Some(w.effect);
            }

            // Check for Scroll
            if let Some(s) = item_entity.get::<Scroll>() {
                scroll_effect = Some(s.effect);
            }

            // Equipment: using it toggles the equipped state (handled below).
            // Weapon, armour or ring — the slot on the component says which, and
            // nothing here needs to.
            if item_entity.get::<crate::equipment::Equipped>().is_some() {
                is_equipment = true;
            }

            // Handle Wands / Battery logic
            if let Some(mut battery) = item_entity.get_mut::<Battery>() {
                battery.charges -= 1;
                if battery.charges <= 0 {
                    destroy_item = true;
                } else {
                    // Item survives! We need to put it back in the user's bag.
                    return_to_inventory = true;
                }
            }

            // Handle basic consumables
            if let Some(_consume) = item_entity.get::<Consume>() {
                destroy_item = true;
            }
        } // Drop the entity_mut borrow so we can freely use the world again

        // 0. Equipment toggle — these items always go back in the pack.
        if is_equipment {
            toggle_equipped(world, item_use.user, item_use.item);
            return_to_inventory = true;
        }

        // Anything the game doesn't know how to "use" is handed straight back
        // rather than vanishing into limbo.
        if !destroy_item
            && !return_to_inventory
            && potion_effect.is_none()
            && wand_effect.is_none()
            && scroll_effect.is_none()
        {
            let name = crate::identify::with_the(&item_label(world, item_use.item));
            world
                .resource_mut::<GameLog>()
                .add(format!("You can't use {name} right now."));
            return_to_inventory = true;
        }

        // 1. Manage the item's physical existence
        if return_to_inventory {
            if let Some(mut backpack) = world.get_mut::<Backpack>(item_use.user) {
                if let Some(idx) = item_use.slot_idx {
                    // Put it back in its exact slot (clamp if inventory shifted somehow)
                    let insert_pos = std::cmp::min(idx, backpack.items.len());
                    backpack.items.insert(insert_pos, item_use.item);
                } else {
                    backpack.items.push(item_use.item); // Fallback
                }
            }
        }

        if destroy_item {
            let is_wand = world.get::<Wand>(item_use.item).is_some();
            let is_potion = world.get::<Potion>(item_use.item).is_some();
            let is_scroll = world.get::<Scroll>(item_use.item).is_some();

            let mut log = world.resource_mut::<GameLog>();
            if is_wand {
                log.add(format!("The {seen_name} crumbles to dust!"));
            } else if is_potion {
                log.add(format!("You drink the {seen_name}."));
            } else if is_scroll {
                log.add(format!("You read the {seen_name}."));
            } else {
                log.add("The item turns to dust!".to_string());
            }

            world.entity_mut(item_use.item).despawn();
        } else if wand_effect.is_some() {
            // Wands survive a zap (until their battery runs dry, handled
            // above), so the "you use it" beat lives here instead.
            world
                .resource_mut::<GameLog>()
                .add(format!("You zap the {seen_name}."));
        }

        // 2. Dispatch to specialized functions. Using a potion, scroll or
        // wand always identifies its true type — every roguelike's
        // use-to-identify convention (rings identify on wear instead, inside
        // `toggle_puton`).
        if let Some(eff) = potion_effect {
            apply_potion_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().potions.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }

        if let Some(eff) = wand_effect {
            apply_wand_effect(world, item_use.user, item_use.target, eff);
            if world.resource_mut::<Identified>().wands.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }

        if let Some(eff) = scroll_effect {
            apply_scroll_effect(world, item_use.user, eff);
            if world.resource_mut::<Identified>().scrolls.insert(eff) {
                world.resource_mut::<GameLog>().add(format!(
                    "That was {} {true_name}!",
                    crate::identify::article_for(&true_name)
                ));
            }
        }
    }
}
