//! The four special rooms: layout invariants ([`Map::special`] vs
//! [`Map::dark`]) and what each kind actually fills its tiles with.
//!
//! [`map_at`] is the cheap probe — [`regenerate_map`] rebuilds only the
//! `Map` resource, no population, which is all a layout question needs.
//! [`world_at`] pays for the full floor (population included) only where a
//! test actually needs to see what got spawned.

use bevy_ecs::prelude::*;
use models::*;

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
        let down = w
            .resource::<Map>()
            .tiles
            .iter()
            .position(|&t| t == TileType::Downstairs)
            .expect("every floor above the last has a down-stair");
        let player = w.query_filtered::<Entity, With<Player>>().single(&w);
        let (dx, dy) = (
            (down % MAP_WIDTH as usize) as u16,
            (down / MAP_WIDTH as usize) as u16,
        );
        w.get_mut::<Position>(player).unwrap().x = dx;
        w.get_mut::<Position>(player).unwrap().y = dy;
        assert!(change_level(&mut w, true));
    }
    w
}

/// The first `(seed, depth)` in the sweep whose `Map::special` holds `kind`.
fn first_with(kind: SpecialRoom) -> (u64, u8) {
    for seed in 0..500u64 {
        for depth in 1..=6u8 {
            if map_at(seed, depth).special.contains(&Some(kind)) {
                return (seed, depth);
            }
        }
    }
    panic!("{kind:?} never turned up across the sweep");
}

#[test]
fn every_special_room_kind_turns_up_and_none_are_also_dark() {
    let (mut hoard, mut zoo, mut hive, mut red) = (false, false, false, false);
    for seed in 0..500u64 {
        for depth in 1..=6u8 {
            let map = map_at(seed, depth);
            for (idx, kind) in map.special.iter().enumerate() {
                let Some(kind) = kind else { continue };
                assert!(
                    !map.dark.contains(idx),
                    "seed {seed} depth {depth}: a {kind:?} tile is also dark"
                );
                match kind {
                    SpecialRoom::DragonHoard => hoard = true,
                    SpecialRoom::MonsterZoo => zoo = true,
                    SpecialRoom::TreasureHive => hive = true,
                    SpecialRoom::RedRoom => red = true,
                }
            }
        }
    }
    assert!(
        hoard && zoo && hive && red,
        "not every kind turned up across the sweep: hoard={hoard} zoo={zoo} hive={hive} red={red}"
    );
}

/// A staircase never stands in a special room: a hoard, a zoo, a hive or a
/// red room is somewhere the player walks into, never somewhere they arrive.
/// Rooms never touch, so the eight tiles around a stair are its own room's
/// floor or that room's walls and doors.
#[test]
fn no_special_room_holds_a_stair() {
    for seed in 0..500u64 {
        for depth in 1..=6u8 {
            let map = map_at(seed, depth);
            for (i, _) in map
                .tiles
                .iter()
                .enumerate()
                .filter(|(_, t)| matches!(t, TileType::Upstairs | TileType::Downstairs))
            {
                let (x, y) = (
                    (i % MAP_WIDTH as usize) as u16,
                    (i / MAP_WIDTH as usize) as u16,
                );
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = ((x as i32 + dx) as u16, (y as i32 + dy) as u16);
                        assert_eq!(
                            map.special_kind(nx, ny),
                            None,
                            "seed {seed} depth {depth}: a stair at ({x},{y}) in a special room"
                        );
                    }
                }
            }
        }
    }
}

/// Every tile a `kind` room owns, as `(x, y)`.
fn tiles_of(map: &Map, kind: SpecialRoom) -> Vec<(u16, u16)> {
    map.special
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == Some(kind))
        .map(|(i, _)| {
            (
                (i % MAP_WIDTH as usize) as u16,
                (i / MAP_WIDTH as usize) as u16,
            )
        })
        .collect()
}

/// The name standing on `(x, y)`, if anything is.
fn name_at(w: &mut World, x: u16, y: u16) -> Option<String> {
    w.query::<(&Name, &Position)>()
        .iter(w)
        .find(|(_, p)| p.x == x && p.y == y)
        .map(|(n, _)| n.what.clone())
}

#[test]
fn a_dragon_hoard_is_completely_full_with_the_right_dragon_count() {
    let (seed, depth) = first_with(SpecialRoom::DragonHoard);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::DragonHoard);
    assert!(!tiles.is_empty());

    let expected_dragons = (difficulty_tier(depth) as usize + 1).min(tiles.len());
    let mut dragons = 0;
    for (x, y) in tiles {
        let name = name_at(&mut w, x, y);
        assert!(name.is_some(), "tile ({x},{y}) in the hoard is empty");
        if name.as_deref() == Some("dragon") {
            dragons += 1;
        }
    }
    assert_eq!(
        dragons, expected_dragons,
        "seed {seed} depth {depth}: wrong dragon count for its tier"
    );
}

#[test]
fn a_monster_zoo_has_no_empty_tile() {
    let (seed, depth) = first_with(SpecialRoom::MonsterZoo);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::MonsterZoo);
    assert!(!tiles.is_empty());

    for (x, y) in tiles {
        assert!(
            name_at(&mut w, x, y).is_some(),
            "seed {seed} depth {depth}: tile ({x},{y}) in the zoo is empty"
        );
    }
}

#[test]
fn a_treasure_hive_holds_exactly_one_apis() {
    let (seed, depth) = first_with(SpecialRoom::TreasureHive);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::TreasureHive);
    assert!(!tiles.is_empty());

    let mut apis_count = 0;
    for (x, y) in tiles {
        let name = name_at(&mut w, x, y);
        assert!(name.is_some(), "tile ({x},{y}) in the hive is empty");
        if name.as_deref() == Some("apis") {
            apis_count += 1;
        }
    }
    assert_eq!(
        apis_count, 1,
        "seed {seed} depth {depth}: a treasure hive should hold exactly one apis"
    );
}

// ---------------------------------------------------------------------------
// Map::special_tint and the entry line, as pure functions on a hand-built Map
// ---------------------------------------------------------------------------

fn blank_map() -> Map {
    Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        special: vec![None; MAP_TILE_COUNT],
        level: None,
    }
}

#[test]
fn special_tint_covers_the_floor_and_its_bounding_wall_only() {
    let mut map = blank_map();
    map.tiles[tile_index(5, 5)] = TileType::Room;
    map.tiles[tile_index(4, 5)] = TileType::Wall;
    map.special[tile_index(5, 5)] = Some(SpecialRoom::RedRoom);

    assert_eq!(map.special_tint(5, 5), Some(crossterm::style::Color::Red));
    assert_eq!(
        map.special_tint(4, 5),
        Some(crossterm::style::Color::Red),
        "the wall bounding a red room's floor tints the same colour"
    );
    assert_eq!(
        map.special_tint(10, 10),
        None,
        "a tile nowhere near a special room stays untinted"
    );
}

#[test]
fn the_entry_line_fires_once_on_the_threshold_and_never_for_a_zoo() {
    let mut map = blank_map();
    map.tiles[tile_index(5, 5)] = TileType::Room;
    map.tiles[tile_index(6, 5)] = TileType::Room;
    map.special[tile_index(5, 5)] = Some(SpecialRoom::RedRoom);
    map.special[tile_index(6, 5)] = Some(SpecialRoom::MonsterZoo);

    assert!(special_room_entry_message(&map, (0, 0), (5, 5)).is_some());
    assert!(
        special_room_entry_message(&map, (5, 5), (5, 5)).is_none(),
        "standing still on the same tile is not a fresh entry"
    );
    assert!(
        special_room_entry_message(&map, (0, 0), (6, 5)).is_none(),
        "the monster zoo was never given a line"
    );
}
