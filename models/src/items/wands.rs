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
use std::collections::{HashSet, VecDeque};

use crate::components::*;
use crate::effects::*;
use crate::equipment::{equipped_items, sync_equipment_effects};
use crate::helpers::{
    apply_damage, clear_player_conditions, get_entities_at_position, get_line, item_label,
    monster_at, roll_dice,
};
use crate::map::{GameRng, MAP_HEIGHT, MAP_WIDTH, Map, TileType, tile_index};
use crate::monsters::{BESTIARY, spawn_monster};
use crate::particles::{BlastPalette, Particles};
use crate::traps::{Trap, random_open_tile};

use super::scrolls::teleport_reader;
use crate::constants::wands::{BLAST_RADIUS, DAMAGE_DICE, DAMAGE_SIDES};

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
pub(super) fn elemental_blast(
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

/// Dazzle — the wand of light's confusion. A monster is switched to a
/// random-walk ([`MovementType::Confused`]); the player picks up the [`Confused`]
/// condition, which rides along until they take a staircase or are cancelled.
pub(super) fn dazzle(world: &mut World, entity: Entity) {
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
pub(super) fn shift_entity_speed(world: &mut World, victim: Entity, faster: bool) {
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
pub(super) fn teleport_entity_away(world: &mut World, victim: Entity) {
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
    let spot =
        crate::helpers::free_adjacent_tile(world, user_pos).or_else(|| random_open_tile(world));
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
