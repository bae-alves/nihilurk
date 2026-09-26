//! The special levels: which floors can be one, and that each kind is the
//! shape it promises.
//!
//! [`map_at`] rebuilds the `Map` alone — all a layout question needs.
//! [`world_at`] walks a real run down to a floor, population and arrival
//! lines included, only where a test needs to see what stands on it.

use bevy_ecs::prelude::*;
use models::constants::map::SPECIAL_LEVEL_MIN_DEPTH;
use models::constants::population::START_CLEARING_RADIUS;
use models::*;
use std::collections::{HashSet, VecDeque};

/// Every kind the roll can produce. A kind added to the enum and not here
/// still turns up in the game; it just skips the sweeps below.
const KINDS: [SpecialLevel; 6] = [
    SpecialLevel::Battlefield,
    SpecialLevel::Labyrinth,
    SpecialLevel::Vault,
    SpecialLevel::BeeWorld,
    SpecialLevel::Castle,
    SpecialLevel::Island,
];

/// How many seeds the sweeps walk. Enough for the rarest (1%) kind to turn up
/// a few dozen times across depths 6–12.
const SEEDS: u64 = 600;

/// How many floors of one kind an invariant is checked against.
const SAMPLES: usize = 12;

fn map_at(seed: u64, depth: u8) -> Map {
    let mut w = World::new();
    regenerate_map(&mut w, seed, depth);
    w.remove_resource::<Map>().unwrap()
}

fn world_at(seed: u64, depth: u8) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    for _ in 1..depth {
        let (dx, dy) = find(w.resource::<Map>(), TileType::Downstairs);
        let player = w.query_filtered::<Entity, With<Player>>().single(&w);
        w.get_mut::<Position>(player).unwrap().x = dx;
        w.get_mut::<Position>(player).unwrap().y = dy;
        assert!(change_level(&mut w, true));
    }
    w
}

/// Up to [`SAMPLES`] `(seed, depth)` floors of `kind`, in sweep order.
fn samples(kind: SpecialLevel) -> Vec<(u64, u8, Map)> {
    let mut found = Vec::new();
    for seed in 0..SEEDS {
        for depth in SPECIAL_LEVEL_MIN_DEPTH..FINAL_DEPTH {
            let map = map_at(seed, depth);
            if map.level == Some(kind) {
                found.push((seed, depth, map));
                if found.len() == SAMPLES {
                    return found;
                }
            }
        }
    }
    assert!(
        !found.is_empty(),
        "{kind:?} never turned up across the sweep"
    );
    found
}

fn coord(i: usize) -> (u16, u16) {
    (
        (i % MAP_WIDTH as usize) as u16,
        (i / MAP_WIDTH as usize) as u16,
    )
}

fn find(map: &Map, want: TileType) -> (u16, u16) {
    coord(
        map.tiles
            .iter()
            .position(|&t| t == want)
            .unwrap_or_else(|| panic!("no {want:?} on the floor")),
    )
}

/// Every tile a walker who cannot swim could stand on.
fn standable(map: &Map) -> HashSet<(u16, u16)> {
    (0..map.tiles.len())
        .map(coord)
        .filter(|&(x, y)| map.walkable(x, y, false))
        .collect()
}

/// Every tile that walker reaches from the up-stair, by the two rules every
/// step obeys: [`Map::walkable`] and [`Map::diagonal_step_ok`].
fn reachable(map: &Map) -> HashSet<(u16, u16)> {
    let start = find(map, TileType::Upstairs);
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if map.walkable(nx, ny, false)
                    && map.diagonal_step_ok(x, y, nx, ny)
                    && seen.insert((nx, ny))
                {
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    seen
}

#[test]
fn special_levels_only_replace_floors_six_through_twelve() {
    for seed in 0..SEEDS {
        for depth in 1..=FINAL_DEPTH {
            if let Some(kind) = map_at(seed, depth).level {
                assert!(
                    (SPECIAL_LEVEL_MIN_DEPTH..FINAL_DEPTH).contains(&depth),
                    "seed {seed}: a {kind:?} on depth {depth}"
                );
            }
        }
    }
}

#[test]
fn a_special_level_holds_no_special_rooms() {
    for kind in KINDS {
        for (seed, depth, map) in samples(kind) {
            assert!(
                map.special.iter().all(Option::is_none),
                "seed {seed} depth {depth}: a {kind:?} rolled a special room"
            );
        }
    }
}

#[test]
fn every_floor_tile_of_every_kind_is_reachable_from_the_up_stair() {
    for kind in KINDS {
        for (seed, depth, map) in samples(kind) {
            let unreachable: Vec<_> = standable(&map)
                .difference(&reachable(&map))
                .copied()
                .collect();
            assert!(
                unreachable.is_empty(),
                "seed {seed} depth {depth}: {kind:?} strands {unreachable:?}"
            );
            find(&map, TileType::Downstairs);
        }
    }
}

#[test]
fn a_battlefield_is_one_room_wall_to_wall() {
    for (seed, depth, map) in samples(SpecialLevel::Battlefield) {
        for y in 1..MAP_HEIGHT - 1 {
            for x in 1..MAP_WIDTH - 1 {
                assert!(
                    matches!(
                        map.tile(x, y),
                        TileType::Room | TileType::Upstairs | TileType::Downstairs
                    ),
                    "seed {seed} depth {depth}: ({x},{y}) is {:?}",
                    map.tile(x, y)
                );
            }
        }
    }
}

#[test]
fn a_labyrinth_is_all_passage_and_loops_back_on_itself() {
    for (seed, depth, map) in samples(SpecialLevel::Labyrinth) {
        let open = standable(&map);
        assert!(
            !map.tiles.contains(&TileType::Room),
            "seed {seed} depth {depth}: a labyrinth has room floor"
        );
        // A maze with no loops is a tree, and a tree has one edge fewer than
        // it has tiles. One more edge than that is a loop somewhere.
        let edges = open
            .iter()
            .filter(|&&(x, y)| open.contains(&(x + 1, y)))
            .count()
            + open
                .iter()
                .filter(|&&(x, y)| open.contains(&(x, y + 1)))
                .count();
        assert!(
            edges >= open.len(),
            "seed {seed} depth {depth}: {edges} edges over {} tiles is a perfect maze",
            open.len()
        );
    }
}

#[test]
fn walking_into_a_labyrinth_says_so() {
    let (seed, depth, _) = samples(SpecialLevel::Labyrinth).remove(0);
    let w = world_at(seed, depth);
    assert!(
        w.resource::<GameLog>()
            .history
            .iter()
            .any(|l| l == strings::labyrinth_arrival()),
        "seed {seed} depth {depth}: no labyrinth line in the log"
    );
}

/// Which room every room-floor tile belongs to: floods of Room and stair
/// tiles, 8-connected the way sight and diagonal steps are. A door is not
/// room floor, so it is where one room stops and the next begins.
fn room_ids(map: &Map) -> std::collections::HashMap<(u16, u16), usize> {
    let floor = |x: u16, y: u16| {
        matches!(
            map.tile(x, y),
            TileType::Room | TileType::Upstairs | TileType::Downstairs
        )
    };
    let mut ids = std::collections::HashMap::new();
    for start in (0..map.tiles.len()).map(coord) {
        if !floor(start.0, start.1) || ids.contains_key(&start) {
            continue;
        }
        let id = ids.len();
        let mut stack = vec![start];
        ids.insert(start, id);
        while let Some((x, y)) = stack.pop() {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    let n = (nx as u16, ny as u16);
                    if floor(n.0, n.1) && !ids.contains_key(&n) {
                        ids.insert(n, id);
                        stack.push(n);
                    }
                }
            }
        }
    }
    ids
}

#[test]
fn a_vault_is_rooms_joined_by_single_doors() {
    for (seed, depth, map) in samples(SpecialLevel::Vault) {
        assert!(
            !map.tiles.contains(&TileType::Passage),
            "seed {seed} depth {depth}: a vault has a passage"
        );
        let ids = room_ids(&map);
        let mut joined = HashSet::new();
        for (x, y) in (0..map.tiles.len()).map(coord) {
            if map.tile(x, y) != TileType::Door {
                continue;
            }
            let across = [((x - 1, y), (x + 1, y)), ((x, y - 1), (x, y + 1))]
                .into_iter()
                .find_map(|(a, b)| Some((*ids.get(&a)?, *ids.get(&b)?)));
            let Some((a, b)) = across else {
                panic!(
                    "seed {seed} depth {depth}: door ({x},{y}) has no room on two opposite sides"
                );
            };
            assert_ne!(
                a, b,
                "seed {seed} depth {depth}: door ({x},{y}) opens a room onto itself"
            );
            assert!(
                joined.insert((a.min(b), a.max(b))),
                "seed {seed} depth {depth}: two doors join the same two rooms"
            );
        }
    }
}

#[test]
fn a_vaults_stairs_sit_in_its_corner_rooms() {
    for (seed, depth, map) in samples(SpecialLevel::Vault) {
        let ids = room_ids(&map);
        // The room floor nearest each corner of the map is in that corner's room.
        let corner_rooms: HashSet<usize> = [
            (0, 0),
            (MAP_WIDTH, 0),
            (0, MAP_HEIGHT),
            (MAP_WIDTH, MAP_HEIGHT),
        ]
        .into_iter()
        .map(|(cx, cy): (u16, u16)| {
            let nearest = ids
                .keys()
                .min_by_key(|&&(x, y)| {
                    let (dx, dy) = (x as i32 - cx as i32, y as i32 - cy as i32);
                    dx * dx + dy * dy
                })
                .unwrap();
            ids[nearest]
        })
        .collect();
        for stair in [TileType::Upstairs, TileType::Downstairs] {
            assert!(
                corner_rooms.contains(&ids[&find(&map, stair)]),
                "seed {seed} depth {depth}: the {stair:?} is not in a corner room"
            );
        }
    }
}

#[test]
fn a_vault_is_white_and_the_bee_world_is_yellow() {
    use crossterm::style::Color;
    for (kind, color) in [
        (SpecialLevel::Vault, Color::White),
        (SpecialLevel::BeeWorld, Color::Yellow),
    ] {
        let (_, _, map) = samples(kind).remove(0);
        for want in [TileType::Room, TileType::Wall] {
            let (x, y) = find(&map, want);
            assert_eq!(map.special_tint(x, y), Some(color), "{kind:?} {want:?}");
        }
    }
}

#[test]
fn the_bee_world_is_one_cave_of_nothing_but_apis() {
    let (seed, depth, map) = samples(SpecialLevel::BeeWorld).remove(0);
    assert!(
        !map.tiles
            .iter()
            .any(|t| matches!(t, TileType::Door | TileType::Passage)),
        "seed {seed} depth {depth}: the bee world is one cave, with no doors or passages"
    );
    let mut w = world_at(seed, depth);
    let names: Vec<String> = w
        .query_filtered::<&Name, With<Mob>>()
        .iter(&w)
        .map(|n| n.what.clone())
        .collect();
    assert!(
        !names.is_empty(),
        "seed {seed} depth {depth}: an empty hive"
    );
    assert!(
        names.iter().all(|n| n == "apis"),
        "seed {seed} depth {depth}: not all apis: {names:?}"
    );
    let log = &w.resource::<GameLog>().history;
    assert!(log.iter().any(|l| l == strings::bee_world_arrival()));
}

#[test]
fn a_castle_is_a_keep_with_four_towers_and_a_dragon_inside() {
    let (seed, depth, _) = samples(SpecialLevel::Castle).remove(0);
    let mut w = world_at(seed, depth);
    let map = w.resource::<Map>().clone();
    let ids = room_ids(&map);
    let size = |id: usize| ids.values().filter(|&&i| i == id).count();

    let keep = w
        .query::<(&Name, &Position)>()
        .iter(&w)
        .filter(|(n, _)| n.what == "dragon")
        .filter_map(|(_, p)| ids.get(&(p.x, p.y)).copied())
        .find(|&id| size(id) == 7 * 7)
        .unwrap_or_else(|| panic!("seed {seed} depth {depth}: no dragon in a 7x7 keep"));

    // Every door out of the keep, and what lies on its far side.
    let mut towers = Vec::new();
    let mut gates = 0;
    for (&(x, y), _) in ids.iter().filter(|&(_, &id)| id == keep) {
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let door = ((x as i32 + dx) as u16, (y as i32 + dy) as u16);
            if map.tile(door.0, door.1) != TileType::Door {
                continue;
            }
            let beyond = ((door.0 as i32 + dx) as u16, (door.1 as i32 + dy) as u16);
            match ids.get(&beyond) {
                Some(&room) => towers.push(room),
                None => {
                    assert_eq!(map.tile(beyond.0, beyond.1), TileType::Passage);
                    gates += 1;
                }
            }
        }
    }
    towers.sort();
    let distinct: HashSet<usize> = towers.iter().copied().collect();
    assert_eq!(
        towers.len(),
        4,
        "seed {seed} depth {depth}: one door per tower"
    );
    assert_eq!(
        distinct.len(),
        4,
        "seed {seed} depth {depth}: four different towers"
    );
    assert!(
        towers.iter().all(|&t| size(t) == 5 * 5),
        "seed {seed} depth {depth}: every tower is 5x5"
    );
    assert_eq!(gates, 2, "seed {seed} depth {depth}: a gate on each side");

    assert_eq!(
        w.resource::<GameLog>().history.last().map(String::as_str),
        Some(strings::descend_stairs(depth).as_str()),
        "seed {seed} depth {depth}: a castle arrives without a word"
    );
}

/// Two rooms that share a wall and a door, the way a vault's cells and a
/// castle's towers do: the light of one stops at the doorway, showing the
/// first step into the next room and no more of it.
#[test]
fn a_lit_room_does_not_light_the_room_behind_its_door() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    for y in 2..=6 {
        for x in (2..=6).chain(8..=12) {
            map.tiles[tile_index(x, y)] = TileType::Room;
        }
    }
    map.tiles[tile_index(7, 4)] = TileType::Door;

    let mut w = World::new();
    w.init_resource::<GameLog>();
    w.insert_resource(map);
    w.spawn((
        Player,
        Position { x: 3, y: 4 },
        Viewshed {
            visible_tiles: Vec::new(),
            revealed_tiles: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
            range: 12,
            dirty: true,
        },
    ));
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(&mut w);

    let seen = w
        .query::<&Viewshed>()
        .single(&w)
        .visible_tiles
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    assert!(seen.contains(&(7, 4)), "the door itself is in view");
    assert!(seen.contains(&(8, 4)), "and the step just past it");
    assert!(
        !seen.contains(&(11, 4)),
        "but the room behind it stays dark"
    );
}

#[test]
fn water_is_floor_to_a_swimmer_and_a_wall_to_everyone_else() {
    let mut map = Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Water;
    assert!(
        !map.walkable(5, 5, false),
        "a walker cannot stand in deep water"
    );
    assert!(map.walkable(5, 5, true), "a swimmer can");
    assert!(
        !map.blocks(5, 5),
        "and nothing about water stops sight or a shot"
    );
}

#[test]
fn an_island_is_one_shore_in_deep_water() {
    for (seed, depth, map) in samples(SpecialLevel::Island) {
        assert!(
            map.tiles.contains(&TileType::Water),
            "seed {seed} depth {depth}: an island with no sea"
        );
        // Every tile off the island is water, bar the map's own rim.
        for y in 1..MAP_HEIGHT - 1 {
            for x in 1..MAP_WIDTH - 1 {
                assert_ne!(
                    map.tile(x, y),
                    TileType::Wall,
                    "seed {seed} depth {depth}: rock at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn only_a_swimmer_starts_in_the_water_and_every_swimmer_does() {
    for (seed, depth, _) in samples(SpecialLevel::Island).into_iter().take(3) {
        let mut w = world_at(seed, depth);
        let map = w.resource::<Map>().clone();
        let mobs: Vec<(String, Position, bool)> = w
            .query_filtered::<(&Name, &Position, Option<&Swims>), With<Mob>>()
            .iter(&w)
            .map(|(n, p, s)| (n.what.clone(), *p, s.is_some()))
            .collect();
        assert!(
            mobs.iter().any(|(_, _, swims)| *swims),
            "seed {seed} depth {depth}: nothing swims around the island"
        );
        for (name, p, swims) in mobs {
            assert_eq!(
                map.tile(p.x, p.y) == TileType::Water,
                swims,
                "seed {seed} depth {depth}: a {name} at ({},{})",
                p.x,
                p.y
            );
        }
    }
}

#[test]
fn the_whole_island_is_in_view_from_its_up_stair() {
    let (seed, depth, _) = samples(SpecialLevel::Island).remove(0);
    let mut w = world_at(seed, depth);
    let mut s = Schedule::default();
    s.add_systems(visibility_system);
    s.run(&mut w);
    let map = w.resource::<Map>().clone();
    let seen: HashSet<(u16, u16)> = w
        .query_filtered::<&Viewshed, With<Player>>()
        .single(&w)
        .visible_tiles
        .iter()
        .copied()
        .collect();
    for (x, y) in (0..map.tiles.len()).map(coord) {
        if map.tile(x, y) != TileType::Wall {
            assert!(
                seen.contains(&(x, y)),
                "seed {seed} depth {depth}: ({x},{y}) is out of sight"
            );
        }
    }
}

/// The floors that are one room and nothing else still give the player a
/// start room: nothing is stocked within [`START_CLEARING_RADIUS`] of where
/// they land, water included.
#[test]
fn a_one_room_floor_lands_the_player_in_a_clearing() {
    for kind in [
        SpecialLevel::Battlefield,
        SpecialLevel::Labyrinth,
        SpecialLevel::BeeWorld,
        SpecialLevel::Island,
    ] {
        for (seed, depth, _) in samples(kind).into_iter().take(3) {
            let mut w = world_at(seed, depth);
            let me = *w.query_filtered::<&Position, With<Player>>().single(&w);
            let crowding: Vec<(u16, u16)> = w
                .query_filtered::<&Position, With<Mob>>()
                .iter(&w)
                .filter(|p| chebyshev(**p, me) <= START_CLEARING_RADIUS as i32)
                .map(|p| (p.x, p.y))
                .collect();
            assert!(
                crowding.is_empty(),
                "seed {seed} depth {depth}: a {kind:?} stocked {crowding:?} beside the player"
            );
        }
    }
}

/// Nothing an island is stocked with starts out in the sea — the arrows an
/// ichthyocentaur carries sink before the player ever arrives to see them.
#[test]
fn no_item_starts_out_in_the_water() {
    for (seed, depth, _) in samples(SpecialLevel::Island).into_iter().take(6) {
        let mut w = world_at(seed, depth);
        let map = w.resource::<Map>().clone();
        let wet: Vec<(u16, u16)> = w
            .query_filtered::<&Position, With<Item>>()
            .iter(&w)
            .filter(|p| map.tile(p.x, p.y) == TileType::Water)
            .map(|p| (p.x, p.y))
            .collect();
        assert!(
            wet.is_empty(),
            "seed {seed} depth {depth}: items afloat at {wet:?}"
        );
    }
}

/// A floor of dry land with two tiles of deep water, (5,5) and (6,5), and a
/// player standing beside them who can see both.
fn shore() -> World {
    let mut map = Map {
        tiles: vec![TileType::Room; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    };
    map.tiles[tile_index(5, 5)] = TileType::Water;
    map.tiles[tile_index(6, 5)] = TileType::Water;
    let mut w = World::new();
    w.init_resource::<GameLog>();
    w.insert_resource(map);
    w.spawn((
        Player,
        Position { x: 5, y: 6 },
        Backpack { items: Vec::new() },
        Viewshed {
            visible_tiles: vec![(5, 5), (6, 5), (5, 7)],
            revealed_tiles: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
            range: 12,
            dirty: false,
        },
    ));
    w
}

fn sink(w: &mut World) {
    let mut s = Schedule::default();
    s.add_systems(sink_system);
    s.run(w);
}

/// Whatever lands in deep water sinks with a splash, however it got there,
/// and nothing on dry land beside it does.
#[test]
fn an_item_in_the_water_sinks_with_a_splash() {
    let mut w = shore();
    let sunk = spawn_weapon(&mut w, "mace", Position { x: 5, y: 5 });
    let dry = spawn_weapon(&mut w, "mace", Position { x: 5, y: 7 });
    sink(&mut w);

    assert!(w.get_entity(sunk).is_none(), "the mace went down");
    assert!(
        w.get_entity(dry).is_some(),
        "the mace on the shore stays put"
    );
    let log = &w.resource::<GameLog>().history;
    assert_eq!(
        log.len(),
        1,
        "one splash, for the one thing that sank: {log:?}"
    );
}

/// The Element of Yoord will not drown: it comes up into the player's hands,
/// and everything else they carried — worn gear included — is gone to make
/// room for it.
#[test]
fn the_element_leaps_from_the_water_and_takes_the_whole_pack() {
    let mut w = shore();
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let mace = spawn_weapon(&mut w, "mace", Position { x: 0, y: 0 });
    let spare = spawn_weapon(&mut w, "dagger", Position { x: 0, y: 0 });
    for item in [mace, spare] {
        w.entity_mut(item).remove::<Position>();
        w.get_mut::<Backpack>(player).unwrap().items.push(item);
    }
    equip_silently(&mut w, player, mace);
    let element = spawn_element_of_yoord(&mut w, Position { x: 6, y: 5 });
    sink(&mut w);

    assert_eq!(
        w.get::<Backpack>(player).unwrap().items,
        vec![element],
        "the Element, and only the Element, is in the pack"
    );
    assert!(w.get::<Position>(element).is_none(), "and off the floor");
    assert!(
        w.get_entity(mace).is_none() && w.get_entity(spare).is_none(),
        "the worn mace and the spare dagger both burned away"
    );
}
