//! What stands on a floor once it has been carved.
//!
//! [`super::generate`] decides the shape of a floor; this file decides how
//! crowded it is. Nothing here picks *which* creature, item or trap — those are
//! weighted draws over the content tables themselves ([`MonsterDef::pick`],
//! [`crate::spawn::roll_item`], [`TrapDef::pick`]). This is only how many, and
//! where.
//!
//! Every roll comes off the floor's own [`content_rng`] stream, never the
//! shared [`GameRng`](super::streams::GameRng). That is what keeps a floor's
//! contents a function of `(seed, depth, staircases taken)` rather than of the
//! player's blow-by-blow history — walk back up through floor 7 and the same
//! corridors come back stocked differently, but two runs on one seed that
//! descend in lockstep see the same everything.

use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;
use std::collections::HashSet;

use crate::catalog::spawn_element_of_yoord;
use crate::components::*;
use crate::constants::population::{
    CORRIDOR_LURKER_CHANCE, CORRIDOR_LURKER_MIN_DEPTH, HIDDEN_ITEM_CHANCE, ITEM_SLOTS_BASE,
    MONSTER_FILL_CHANCE_BASE, MONSTER_FILL_CHANCE_CAP, MONSTER_FILL_CHANCE_PER_TIER,
    MONSTER_SLOTS_BASE, TRAP_FILL_CHANCE_BASE, TRAP_FILL_CHANCE_CAP, TRAP_FILL_CHANCE_PER_TIER,
    TRAP_SLOTS_BASE,
};
use crate::constants::progression::DIFFICULTY_TIER_LAST_DEPTH;

/// How many times a placement will re-roll before giving the slot up.
///
/// Ten, not a hundred. A floor is a few hundred open tiles holding a dozen
/// things, so the first draw nearly always lands somewhere free and the budget
/// is never spent; the only floors that reach the end of it are ones so
/// crowded that the eleventh try would not have helped either. Spending a
/// hundred draws to find that out costs the same seeded RNG stream everything
/// else on the floor draws from, for a slot the dungeon is happy to skip.
const PLACEMENT_TRIES: usize = 10;
use crate::monsters::{MonsterDef, spawn_monster};
use crate::rect::Rect;
use crate::spawn::{roll_item, spawn_requested};

use super::generate::{corridor_centers, find_tile};
use super::levels::holding_element_of_yoord;
use super::streams::{RngSeed, content_rng};
use super::{FINAL_DEPTH, Map, TileType, random_point_in_room, tile_index};

/// Reserves a free tile in a random room other than the start room (index 0),
/// giving up after [`PLACEMENT_TRIES`]. Returns the tile it claimed in
/// `occupied`, or `None` — and `None` simply means that slot goes unfilled,
/// which is a floor with one fewer monster on it, not an error.
fn claim_random_spot(
    rooms: &[Rect],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
) -> Option<(u16, u16)> {
    for _ in 0..PLACEMENT_TRIES {
        let room_idx = rng.gen_range(1..rooms.len());
        let spot = random_point_in_room(&rooms[room_idx], rng);
        if occupied.insert(spot) {
            return Some(spot);
        }
    }
    None
}

/// On the deepest floor, replaces the down-stair with the Element of Yoord. A
/// no-op on every shallower floor.
fn place_element_of_yoord(world: &mut World, occupied: &mut HashSet<(u16, u16)>, depth: u8) {
    if depth < FINAL_DEPTH {
        return;
    }
    let Some((ex, ey)) = find_tile(&world.resource::<Map>().tiles, TileType::Downstairs) else {
        return;
    };
    world.resource_mut::<Map>().tiles[tile_index(ex, ey)] = TileType::Room;
    spawn_element_of_yoord(world, Position { x: ex, y: ey });
    occupied.insert((ex, ey));
}

/// One trap attempt: up to [`PLACEMENT_TRIES`] draws for a free room tile that
/// isn't the player's landing spot, then a random trap on it. A budget spent
/// without finding one is a trap the floor does without.
fn place_one_trap(
    world: &mut World,
    rooms: &[Rect],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    player_start: (u16, u16),
) {
    for _ in 0..PLACEMENT_TRIES {
        let room_idx = rng.gen_range(0..rooms.len());
        let (x, y) = random_point_in_room(&rooms[room_idx], rng);
        if (x, y) == player_start {
            continue;
        }
        if world.resource::<Map>().tiles[tile_index(x, y)] != TileType::Room {
            continue;
        }
        if occupied.insert((x, y)) {
            world.spawn(crate::TrapBundle::random(rng, depth, Position { x, y }));
            return;
        }
    }
}

/// Which floor-crowding tier `depth` falls in — `0` on the shallowest floors,
/// rising by one at each boundary in [`DIFFICULTY_TIER_LAST_DEPTH`]. The
/// monster, trap and item budgets all read this. `[3, 6, 9, 12]` gives five tiers:
/// depths 1-3, 4-6, 7-9, 10-12, and 13 on its own. (The damage traps scale on
/// their own coarser bands — `traps::trap_damage_tier`.)
pub fn difficulty_tier(depth: u8) -> u32 {
    DIFFICULTY_TIER_LAST_DEPTH
        .iter()
        .position(|&last| depth <= last)
        .unwrap_or(DIFFICULTY_TIER_LAST_DEPTH.len()) as u32
}

/// Spawns the monsters and items for a freshly built floor. The staircases are
/// carved by [`build_tiles`]. Shared by [`initialize_world`] and [`change_level`].
pub(super) fn populate_level(world: &mut World, rooms: &[Rect], player_start: (u16, u16)) {
    let (player_x, player_y) = player_start;

    let mut occupied = HashSet::new();

    // The player's tile is already occupied.
    occupied.insert((player_x, player_y));

    // Rough danger tier: deeper floors unlock nastier letters.
    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);

    // Once the Element of Yoord is in the pack, the climb out lifts every depth
    // gate: each floor draws from the whole bestiary, so a dragon can be waiting
    // on floor 1. `pick_species` routes every spawn below through the right draw.
    let anything_goes = holding_element_of_yoord(world);
    let pick_species = |rng: &mut ChaCha12Rng| match anything_goes {
        true => MonsterDef::pick_any(rng),
        false => MonsterDef::pick(depth, rng),
    };

    // Everything below draws from this floor's own stream, never the shared
    // `GameRng` — see [`content_rng`]. The stream is keyed off the staircase
    // count, so a repeat visit re-stocks the same layout; but nothing the
    // player did *on* a floor (fighting, looting) can reach into how the next
    // one is built.
    let seed = world.resource::<RngSeed>().0;
    let changes = world.get_resource::<FloorChanges>().map_or(0, |c| c.count);
    let mut rng = content_rng(seed, depth, changes);

    // Both the monster and trap budgets step up in five depth bands (1-3, 4-6,
    // 7-9, 10-12, and 13 alone — see `difficulty_tier`). Each tier grants one
    // more spawn slot and widens the odds that a given slot actually fills, so
    // the dungeon gets more crowded and more dangerous the deeper you go.
    let tier = difficulty_tier(depth);

    // Monsters: three slots at the surface, +1 per tier. The first slot always
    // fills (no floor is ever completely empty); every later slot fills with a
    // probability that itself climbs one step per tier (capped so a slot is
    // never quite certain).
    let max_monsters = MONSTER_SLOTS_BASE + tier as usize;
    let monster_chance = (MONSTER_FILL_CHANCE_BASE + MONSTER_FILL_CHANCE_PER_TIER * tier as f64)
        .min(MONSTER_FILL_CHANCE_CAP);
    for slot in 0..max_monsters {
        if slot > 0 && !rng.gen_bool(monster_chance) {
            continue;
        }
        let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) else {
            continue;
        };
        let def = pick_species(&mut rng);
        spawn_monster(world, def, Position { x, y });
    }

    // From depth 7 on, every corridor also has a small (5%) chance of hiding a
    // lurker dead centre — right where an unwary traveller runs into it.
    if depth >= CORRIDOR_LURKER_MIN_DEPTH {
        let centers = corridor_centers(&world.resource::<Map>().tiles);
        for (cx, cy) in centers {
            if !rng.gen_bool(CORRIDOR_LURKER_CHANCE) {
                continue;
            }
            if occupied.insert((cx, cy)) {
                let def = pick_species(&mut rng);
                spawn_monster(world, def, Position { x: cx, y: cy });
            }
        }
    }

    // Items: three attempts at the surface, +1 per tier (like the monster and
    // trap budgets). Every attempt that finds a free tile drops an item.
    let max_items = ITEM_SLOTS_BASE + tier as usize;
    for _ in 0..max_items {
        let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) else {
            continue;
        };
        roll_item(world, &mut rng, depth, Position { x, y });
    }

    // Some floors hide an extra item in plain sight (`HIDDEN_ITEM_CHANCE`): it
    // draws nothing and is never announced until a ring of perception turns it
    // up or the player walks straight onto it ("Hey! There's something here!").
    if rng.gen_bool(HIDDEN_ITEM_CHANCE) {
        if let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) {
            let item = roll_item(world, &mut rng, depth, Position { x, y });
            world.entity_mut(item).insert((Hidden, Invisible));
        }
    }

    place_element_of_yoord(world, &mut occupied, depth);

    // Traps: placed after the stairs, monsters and loot, before the hero drops
    // in. Like the monster budget, the trap budget steps up a tier at a time —
    // four slots at the surface, +1 per `tier` — and each slot's chance of
    // producing a trap climbs the same way, so the deep floors bristle with them
    // and the first floors rarely hold more than one.
    let max_traps = TRAP_SLOTS_BASE + tier as usize;
    let trap_chance =
        (TRAP_FILL_CHANCE_BASE + TRAP_FILL_CHANCE_PER_TIER * tier as f64).min(TRAP_FILL_CHANCE_CAP);
    for _ in 0..max_traps {
        if !rng.gen_bool(trap_chance) {
            continue;
        }
        place_one_trap(world, rooms, &mut occupied, &mut rng, depth, player_start);
    }

    // Last of all, whatever the content author asked for on the command line.
    spawn_requested(
        world,
        Position {
            x: player_x,
            y: player_y,
        },
        &mut occupied,
    );
}
