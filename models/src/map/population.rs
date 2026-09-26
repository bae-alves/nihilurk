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

use crate::catalog::{COINS, PROGRESSION_ITEMS, spawn_element_of_yoord};
use crate::components::*;
use crate::constants::population::{
    CORRIDOR_LURKER_CHANCE, CORRIDOR_LURKER_MIN_DEPTH, HIDDEN_ITEM_CHANCE, ITEM_SLOTS_BASE,
    MONSTER_FILL_CHANCE_BASE, MONSTER_FILL_CHANCE_CAP, MONSTER_FILL_CHANCE_PER_TIER,
    MONSTER_SLOTS_BASE, START_CLEARING_RADIUS, TRAP_FILL_CHANCE_BASE, TRAP_FILL_CHANCE_CAP,
    TRAP_FILL_CHANCE_PER_TIER, TRAP_SLOTS_BASE,
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
use crate::monsters::{MonsterDef, spawn_monster_with_rng};
use crate::spawn::{roll_item, roll_one, spawn_named, spawn_requested};

use super::generate::{corridor_centers, find_tile, room_floor_tiles, tiles_of};
use super::levels::holding_element_of_yoord;
use super::special::castle_rooms;
use super::streams::{RngSeed, content_rng};
use super::{FINAL_DEPTH, Map, Rooms, SpecialLevel, SpecialRoom, TileType, tile_index};

/// Reserves a free tile in a random room other than the start room (index 0)
/// — or in the start room itself, on a floor that is one room and nothing
/// else — giving up after [`PLACEMENT_TRIES`]. Returns the tile it claimed in
/// `occupied`, or `None` — and `None` simply means that slot goes unfilled,
/// which is a floor with one fewer monster on it, not an error.
fn claim_random_spot(
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
) -> Option<(u16, u16)> {
    for _ in 0..PLACEMENT_TRIES {
        let room = match rooms.len() {
            1 => &rooms[0],
            n => &rooms[rng.gen_range(1..n)],
        };
        let spot = random_tile(room, rng);
        if occupied.insert(spot) {
            return Some(spot);
        }
    }
    None
}

/// On a floor that is one room and nothing else, splits the tiles within
/// [`START_CLEARING_RADIUS`] of where the player lands off into a start room
/// of their own — a pseudoroom with no walls, which [`claim_random_spot`]
/// then skips like any other floor's start room. Every other floor already
/// has one and comes back as it went in.
fn clear_the_landing(rooms: &[Vec<(u16, u16)>], landing: (u16, u16)) -> Rooms {
    let [only] = rooms else {
        return rooms.to_vec();
    };
    let near = |&&(x, y): &&(u16, u16)| {
        let (dx, dy) = (x as i32 - landing.0 as i32, y as i32 - landing.1 as i32);
        dx.abs().max(dy.abs()) <= START_CLEARING_RADIUS as i32
    };
    let (clearing, rest): (Vec<_>, Vec<_>) = only.iter().partition(near);
    [clearing, rest]
        .into_iter()
        .filter(|room| !room.is_empty())
        .collect()
}

/// A uniformly random floor tile of `room`. Every room a floor is built with
/// has at least one — see [`super::generate::build_floor`].
fn random_tile(room: &[(u16, u16)], rng: &mut ChaCha12Rng) -> (u16, u16) {
    room[rng.gen_range(0..room.len())]
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

/// One trap attempt: up to [`PLACEMENT_TRIES`] draws for a free floor tile that
/// isn't the player's landing spot and isn't orthogonally next to a door, then
/// a random trap on it. A budget spent without finding one is a trap the floor
/// does without.
///
/// The door rule keeps a doorway from being a coin flip: the tile just inside
/// one is the tile every route into the room has to cross, so a trap there is
/// unavoidable rather than merely unlucky. Orthogonal is the whole
/// neighbourhood that matters — [`Map::diagonal_step_ok`] already forbids
/// stepping diagonally off a door tile, so the first step out of any doorway
/// is one of these four.
fn place_one_trap(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    player_start: (u16, u16),
) {
    for _ in 0..PLACEMENT_TRIES {
        let room = &rooms[rng.gen_range(0..rooms.len())];
        let (x, y) = random_tile(room, rng);
        if (x, y) == player_start {
            continue;
        }
        let map = world.resource::<Map>();
        if [
            map.tile(x.saturating_sub(1), y),
            map.tile(x.saturating_add(1), y),
            map.tile(x, y.saturating_sub(1)),
            map.tile(x, y.saturating_add(1)),
        ]
        .into_iter()
        .any(|tile| tile == TileType::Door)
        {
            continue;
        }
        if occupied.insert((x, y)) {
            world.spawn(crate::TrapBundle::random(rng, depth, Position { x, y }));
            return;
        }
    }
}

/// What a floor is stocked with before any budget is spent: the depth's parity
/// coin — blue on the odd floors, red on the even ones — and, on the last
/// floor of each difficulty tier, one draw from [`PROGRESSION_ITEMS`].
///
/// Neither comes out of the item budget below. The budget is what a floor might
/// be worth; this is what it is worth at worst, so a run is never starved of
/// the magic it needs to cast with, nor of the permanent point of something
/// that pays for the tier it just survived.
///
/// The progression floors are [`DIFFICULTY_TIER_LAST_DEPTH`] itself, rather
/// than a list of their own: one reward for finishing a band, read off the
/// same array the band is defined by. (The deepest floor is its own tier and
/// has no last floor before the Element of Yoord, so it gets none.)
fn place_guaranteed(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
) {
    let parity = match depth % 2 {
        1 => "blue coin",
        _ => "red coin",
    };
    let progression = DIFFICULTY_TIER_LAST_DEPTH
        .contains(&depth)
        .then(|| PROGRESSION_ITEMS[rng.gen_range(0..PROGRESSION_ITEMS.len())]);
    for name in [Some(parity), progression].into_iter().flatten() {
        let Some((x, y)) = claim_random_spot(rooms, occupied, rng) else {
            continue;
        };
        spawn_named(world, name, Position { x, y }).expect("a guaranteed find is a catalog row");
    }
}

/// Fills every room [`build_tiles`] rolled a [`SpecialRoom`] kind for,
/// reading the kind straight off the [`Map`] resource at the room's first
/// floor tile. Every tile claimed here goes into `occupied` first, so the
/// ordinary monster/trap/item budgets below just see fewer free spots.
fn populate_special_rooms(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    tier: u32,
    pick_species: &impl Fn(&mut ChaCha12Rng, (u16, u16)) -> &'static MonsterDef,
) {
    for room in rooms.iter().skip(1) {
        let (x0, y0) = room[0];
        let Some(kind) = world.resource::<Map>().special_kind(x0, y0) else {
            continue;
        };
        let mut free = room.clone();

        match kind {
            SpecialRoom::DragonHoard => {
                let slots = (tier as usize + 1).min(free.len());
                for _ in 0..slots {
                    let i = rng.gen_range(0..free.len());
                    let (x, y) = free.remove(i);
                    occupied.insert((x, y));
                    spawn_monster_with_rng(
                        world,
                        MonsterDef::named("dragon"),
                        Position { x, y },
                        rng,
                    );
                }
                for (x, y) in free {
                    if occupied.insert((x, y)) {
                        roll_one(world, rng, depth, Position { x, y }, COINS);
                    }
                }
            }
            SpecialRoom::MonsterZoo => {
                for (x, y) in free {
                    if occupied.insert((x, y)) {
                        let def = pick_species(rng, (x, y));
                        spawn_monster_with_rng(world, def, Position { x, y }, rng);
                    }
                }
            }
            SpecialRoom::TreasureHive => {
                if !free.is_empty() {
                    let i = rng.gen_range(0..free.len());
                    let (x, y) = free.remove(i);
                    occupied.insert((x, y));
                    spawn_monster_with_rng(
                        world,
                        MonsterDef::named("apis"),
                        Position { x, y },
                        rng,
                    );
                }
                for (x, y) in free {
                    if occupied.insert((x, y)) {
                        roll_one(world, rng, depth, Position { x, y }, COINS);
                    }
                }
            }
            SpecialRoom::RedRoom => {
                let max_items = ITEM_SLOTS_BASE + tier as usize;
                for _ in 0..max_items {
                    if free.is_empty() {
                        break;
                    }
                    let i = rng.gen_range(0..free.len());
                    let (x, y) = free.remove(i);
                    if occupied.insert((x, y)) {
                        roll_item(world, rng, depth, Position { x, y });
                    }
                }
            }
        }
    }
}

/// Which floor-crowding tier `depth` falls in — `0` on the shallowest floors,
/// rising by one at each boundary in [`DIFFICULTY_TIER_LAST_DEPTH`]. The
/// monster, trap and item budgets all read this: the array's length is the
/// number of tiers, its values are where they break. (The damage traps scale
/// on their own coarser bands — `traps::trap_damage_tier`.)
pub fn difficulty_tier(depth: u8) -> u32 {
    DIFFICULTY_TIER_LAST_DEPTH
        .iter()
        .position(|&last| depth <= last)
        .unwrap_or(DIFFICULTY_TIER_LAST_DEPTH.len()) as u32
}

/// How many times over a floor runs the ordinary monster and item budgets:
/// once each on an ordinary floor, more on the special levels that promise a
/// crowd or a haul.
fn budget_runs(level: Option<SpecialLevel>, tier: u32) -> (usize, usize) {
    match level {
        Some(SpecialLevel::Battlefield) => (2, 2),
        Some(SpecialLevel::Vault) => (2, 3),
        // A bee world only rolls at tier 1 or deeper; the floor of one keeps a
        // `NIHILURK_LEVEL` bee world on a shallower floor from standing empty.
        Some(SpecialLevel::BeeWorld) => (tier.max(1) as usize, 3),
        Some(SpecialLevel::Labyrinth | SpecialLevel::Castle | SpecialLevel::Island) | None => {
            (1, 1)
        }
    }
}

/// One run of the monster budget over `rooms`: `MONSTER_SLOTS_BASE` slots at
/// the surface, one more per tier. The first slot always fills (no floor is
/// ever completely empty); every later slot fills with a probability that
/// itself climbs one step per tier (capped so a slot is never quite certain).
fn spawn_monster_budget(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    tier: u32,
    pick_species: &impl Fn(&mut ChaCha12Rng, (u16, u16)) -> &'static MonsterDef,
) {
    let max_monsters = MONSTER_SLOTS_BASE + tier as usize;
    let monster_chance = (MONSTER_FILL_CHANCE_BASE + MONSTER_FILL_CHANCE_PER_TIER * tier as f64)
        .min(MONSTER_FILL_CHANCE_CAP);
    for slot in 0..max_monsters {
        if slot > 0 && !rng.gen_bool(monster_chance) {
            continue;
        }
        let Some((x, y)) = claim_random_spot(rooms, occupied, rng) else {
            continue;
        };
        let def = pick_species(rng, (x, y));
        spawn_monster_with_rng(world, def, Position { x, y }, rng);
    }
}

/// One run of the item budget over `rooms`: `ITEM_SLOTS_BASE` attempts at the
/// surface, one more per tier (like the monster and trap budgets). Every
/// attempt that finds a free tile drops an item.
fn spawn_item_budget(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    tier: u32,
) {
    for _ in 0..ITEM_SLOTS_BASE + tier as usize {
        let Some((x, y)) = claim_random_spot(rooms, occupied, rng) else {
            continue;
        };
        roll_item(world, rng, depth, Position { x, y });
    }
}

/// A castle's own stock, on top of the floor's: a dragon in the keep, then
/// each of the five rooms stocked as though it were a whole floor.
fn populate_castle(
    world: &mut World,
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    tier: u32,
    pick_species: &impl Fn(&mut ChaCha12Rng, (u16, u16)) -> &'static MonsterDef,
) {
    let rooms: Rooms = castle_rooms()
        .iter()
        .map(|r| room_floor_tiles(r, &world.resource::<Map>().tiles))
        .collect();
    if let Some((x, y)) = claim_random_spot(&rooms[..1], occupied, rng) {
        spawn_monster_with_rng(world, MonsterDef::named("dragon"), Position { x, y }, rng);
    }
    for room in &rooms {
        let room = std::slice::from_ref(room);
        spawn_monster_budget(world, room, occupied, rng, tier, pick_species);
        spawn_item_budget(world, room, occupied, rng, depth, tier);
    }
}

/// Spawns the monsters and items for a freshly built floor. The staircases are
/// carved by [`build_tiles`]. Shared by [`initialize_world`] and [`change_level`].
pub(super) fn populate_level(
    world: &mut World,
    rooms: &[Vec<(u16, u16)>],
    player_start: (u16, u16),
) {
    let (player_x, player_y) = player_start;
    let rooms = &clear_the_landing(rooms, player_start);

    let mut occupied = HashSet::new();

    // The player's tile is already occupied.
    occupied.insert((player_x, player_y));

    // Rough danger tier: deeper floors unlock nastier letters.
    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);

    // Once the Element of Yoord is in the pack, the climb out lifts every depth
    // gate: each floor draws from the whole bestiary, so a dragon can be waiting
    // on floor 1. A bee world is apis and nothing else, Element or no, and a
    // tile of deep water only ever gets a swimmer. `pick_species` routes every
    // spawn below through the right draw for the tile it is filling.
    let anything_goes = holding_element_of_yoord(world);
    let level = world.resource::<Map>().level;
    let tiles = world.resource::<Map>().tiles.clone();
    let pick_species = |rng: &mut ChaCha12Rng, (x, y): (u16, u16)| {
        if tiles[tile_index(x, y)] == TileType::Water {
            return MonsterDef::pick_aquatic(rng);
        }
        match (level == Some(SpecialLevel::BeeWorld), anything_goes) {
            (true, _) => MonsterDef::named("apis"),
            (false, true) => MonsterDef::pick_any(rng),
            (false, false) => MonsterDef::pick(depth, rng),
        }
    };

    // Everything below draws from this floor's own stream, never the shared
    // `GameRng` — see [`content_rng`]. The stream is keyed off the staircase
    // count, so a repeat visit re-stocks the same layout; but nothing the
    // player did *on* a floor (fighting, looting) can reach into how the next
    // one is built.
    let seed = world.resource::<RngSeed>().0;
    let changes = world.get_resource::<FloorChanges>().map_or(0, |c| c.count);
    let mut rng = content_rng(seed, depth, changes);

    // What the floor owes the player before it owes them anything else.
    place_guaranteed(world, rooms, &mut occupied, &mut rng, depth);

    // Both the monster and trap budgets step up in depth bands drawn from
    // `DIFFICULTY_TIER_LAST_DEPTH` (see `difficulty_tier`). Each tier grants
    // one more spawn slot and widens the odds that a given slot actually
    // fills, so the dungeon gets more crowded and more dangerous the deeper
    // you go.
    let tier = difficulty_tier(depth);

    // A special room fills itself completely before the ordinary budgets
    // below get a turn at its tiles — every tile it claims goes into
    // `occupied` first, so the loops that follow just see fewer free spots
    // to draw from, the same as any other crowded floor.
    populate_special_rooms(
        world,
        rooms,
        &mut occupied,
        &mut rng,
        depth,
        tier,
        &pick_species,
    );

    // A special level runs the ordinary budgets more than once over. On an
    // island the monsters have the sea to spread into as well as the shore;
    // everything else keeps to the land.
    let (monster_runs, item_runs) = budget_runs(level, tier);
    let monster_rooms = match level {
        Some(SpecialLevel::Island) => {
            let sea = tiles_of(&tiles, TileType::Water);
            clear_the_landing(
                &[rooms.concat().into_iter().chain(sea).collect()],
                player_start,
            )
        }
        _ => rooms.to_vec(),
    };
    for _ in 0..monster_runs {
        spawn_monster_budget(
            world,
            &monster_rooms,
            &mut occupied,
            &mut rng,
            tier,
            &pick_species,
        );
    }

    // From `CORRIDOR_LURKER_MIN_DEPTH` on, every corridor also has a small
    // chance (`CORRIDOR_LURKER_CHANCE`) of hiding a lurker dead centre — right
    // where an unwary traveller runs into it.
    if depth >= CORRIDOR_LURKER_MIN_DEPTH {
        let centers = corridor_centers(&world.resource::<Map>().tiles);
        for (cx, cy) in centers {
            if !rng.gen_bool(CORRIDOR_LURKER_CHANCE) {
                continue;
            }
            if occupied.insert((cx, cy)) {
                let def = pick_species(&mut rng, (cx, cy));
                spawn_monster_with_rng(world, def, Position { x: cx, y: cy }, &mut rng);
            }
        }
    }

    for _ in 0..item_runs {
        spawn_item_budget(world, rooms, &mut occupied, &mut rng, depth, tier);
    }
    if level == Some(SpecialLevel::Castle) {
        populate_castle(world, &mut occupied, &mut rng, depth, tier, &pick_species);
    }

    // A floor hides an extra item in plain sight at `HIDDEN_ITEM_CHANCE` —
    // every floor, at the rate it currently sits at. It draws nothing and is
    // never announced until a ring of perception turns it up, a detection
    // finds it (`items::potions::detect_item`), or the player walks straight
    // onto it ("Hey! There's something here!").
    if rng.gen_bool(HIDDEN_ITEM_CHANCE) {
        if let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) {
            let item = roll_item(world, &mut rng, depth, Position { x, y });
            world.entity_mut(item).insert((Hidden, Invisible));
        }
    }

    place_element_of_yoord(world, &mut occupied, depth);

    // Traps: placed after the stairs, monsters and loot, before the hero drops
    // in. Like the monster budget, the trap budget steps up a tier at a time —
    // `TRAP_SLOTS_BASE` slots at the surface, one more per `tier` — and each
    // slot's chance of producing a trap climbs the same way, so the deep
    // floors bristle with them and the first floors rarely hold more than one.
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

    // Anything the stocking left in deep water — a swimmer's gear, dropped at
    // its feet — goes down before the player arrives to see it go.
    crate::items::sink_items(world);
}
