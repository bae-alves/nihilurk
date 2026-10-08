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

/// How many `Mob`s, `Item`s and `Pickup`s (coins) stand on `(x, y)`.
fn count_at(w: &mut World, x: u16, y: u16) -> (usize, usize, usize) {
    let mobs = w
        .query_filtered::<&Position, With<Mob>>()
        .iter(w)
        .filter(|p| p.x == x && p.y == y)
        .count();
    let items = w
        .query_filtered::<&Position, With<Item>>()
        .iter(w)
        .filter(|p| p.x == x && p.y == y)
        .count();
    let coins = w
        .query_filtered::<&Position, With<Pickup>>()
        .iter(w)
        .filter(|p| p.x == x && p.y == y)
        .count();
    (mobs, items, coins)
}

fn is_dragon(w: &mut World, x: u16, y: u16) -> bool {
    name_at_mob(w, x, y).as_deref() == Some("dragon")
}

fn name_at_mob(w: &mut World, x: u16, y: u16) -> Option<String> {
    w.query_filtered::<(&Name, &Position), With<Mob>>()
        .iter(w)
        .find(|(_, p)| p.x == x && p.y == y)
        .map(|(n, _)| n.what.clone())
}

#[test]
fn a_dragon_hoard_has_the_right_dragon_count_and_exceptional_gear_under_each() {
    let (seed, depth) = first_with(SpecialRoom::DragonHoard);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::DragonHoard);
    assert!(!tiles.is_empty());

    let expected_dragons = (difficulty_tier(depth) as usize + 1).min(tiles.len());
    let mut dragons = 0;
    for (x, y) in tiles {
        let (mobs, items, _) = count_at(&mut w, x, y);
        assert_eq!(items, 1, "tile ({x},{y}) in the hoard should hold one item");
        if is_dragon(&mut w, x, y) {
            dragons += 1;
            assert_eq!(mobs, 1);
            let bonus = w
                .query_filtered::<(
                    &Position,
                    Option<&PowerBonus>,
                    Option<&ArmorBonus>,
                    Option<&ThrowBonus>,
                    Option<&MaxHpBonus>,
                ), With<Item>>()
                .iter(&w)
                .find(|(p, ..)| p.x == x && p.y == y)
                .map(|(_, p, a, t, h)| {
                    p.map_or(0, |b| b.0)
                        + a.map_or(0, |b| b.0)
                        + t.map_or(0, |b| b.0)
                        + h.map_or(0, |b| b.0)
                })
                .unwrap();
            assert!(bonus >= 1, "dragon at ({x},{y}) stands on a plain item");
        } else {
            assert_eq!(mobs, 0);
        }
    }
    assert_eq!(
        dragons, expected_dragons,
        "seed {seed} depth {depth}: wrong dragon count for its tier"
    );
}

#[test]
fn a_monster_zoo_holds_a_monster_on_every_tile_and_a_floors_item_budget() {
    let (seed, depth) = first_with(SpecialRoom::MonsterZoo);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::MonsterZoo);
    assert!(!tiles.is_empty());

    let mut items = 0;
    for &(x, y) in &tiles {
        let (m, i, _) = count_at(&mut w, x, y);
        assert_eq!(m, 1, "seed {seed} depth {depth}: zoo tile ({x},{y})");
        items += i;
    }
    let budget = models::constants::population::ITEM_SLOTS_BASE + difficulty_tier(depth) as usize;
    assert_eq!(items, budget.min(tiles.len()));
}

#[test]
fn a_treasure_hive_is_all_bees_on_coins() {
    let (seed, depth) = first_with(SpecialRoom::TreasureHive);
    let mut w = world_at(seed, depth);
    let tiles = tiles_of(w.resource::<Map>(), SpecialRoom::TreasureHive);
    assert!(!tiles.is_empty());

    for (x, y) in tiles {
        assert_eq!(
            name_at_mob(&mut w, x, y).as_deref(),
            Some("apis"),
            "seed {seed} depth {depth}: hive tile ({x},{y}) has no bee"
        );
        assert_eq!(
            count_at(&mut w, x, y).2,
            1,
            "hive tile ({x},{y}) has no coin"
        );
    }
}

// ---------------------------------------------------------------------------
// Map::special_tint and the entry line, as pure functions on a hand-built Map
// ---------------------------------------------------------------------------

fn blank_map() -> Map {
    Map {
        tiles: vec![TileType::Wall; MAP_TILE_COUNT],
        dark: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
        inert_doors: fixedbitset::FixedBitSet::with_capacity(MAP_TILE_COUNT),
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
fn the_entry_line_fires_once_on_the_threshold_in_the_wall_colour() {
    let mut map = blank_map();
    for x in 5..=7 {
        map.tiles[tile_index(x, 5)] = TileType::Room;
    }
    map.special[tile_index(5, 5)] = Some(SpecialRoom::RedRoom);
    map.special[tile_index(6, 5)] = Some(SpecialRoom::MonsterZoo);
    map.dark.insert(tile_index(7, 5));

    let (_, red) = special_room_entry_message(&map, (0, 0), (5, 5)).unwrap();
    assert_eq!(
        Some(red),
        map.special_tint(5, 4),
        "red room line wears its wall"
    );
    assert!(
        special_room_entry_message(&map, (5, 5), (5, 5)).is_none(),
        "standing still on the same tile is not a fresh entry"
    );
    let (_, zoo) = special_room_entry_message(&map, (0, 0), (6, 5)).unwrap();
    assert_eq!(Some(zoo), map.special_tint(6, 5), "the zoo has a line too");
    let (_, dark) = special_room_entry_message(&map, (0, 0), (7, 5)).unwrap();
    assert_eq!(
        dark,
        tile_appearance(TileType::Wall).1,
        "dark room wears the plain wall"
    );
    assert!(
        special_room_entry_message(&map, (7, 5), (7, 5)).is_none(),
        "a step inside the dark room stays silent"
    );
}

#[test]
fn only_the_red_room_walls_are_undiggable() {
    let mut map = blank_map();
    map.tiles[tile_index(5, 5)] = TileType::Room;
    map.special[tile_index(5, 5)] = Some(SpecialRoom::RedRoom);
    map.tiles[tile_index(20, 5)] = TileType::Room;
    map.special[tile_index(20, 5)] = Some(SpecialRoom::MonsterZoo);

    assert!(!map.diggable(4, 5), "a wall bounding a red room");
    assert!(!map.diggable(4, 4), "its corner too");
    assert!(map.diggable(19, 5), "a zoo's wall is plain rock");
    assert!(map.diggable(40, 10), "ordinary rock");
    assert!(!map.diggable(0, 10), "the outer wall");
    assert!(!map.diggable(MAP_WIDTH - 1, 10), "the outer wall");
}

#[test]
fn a_wand_of_digging_leaves_a_red_rooms_walls_alone() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w.insert_resource(RngSeed(3));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.insert_resource(PlayerName { what: "X".into() });
    initialize_world(&mut w);
    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    {
        let mut map = w.resource_mut::<Map>();
        map.tiles.fill(TileType::Wall);
        map.special.fill(None);
        map.tiles[tile_index(10, 10)] = TileType::Room;
        map.tiles[tile_index(13, 10)] = TileType::Room;
        map.special[tile_index(13, 10)] = Some(SpecialRoom::RedRoom);
    }
    w.get_mut::<Position>(p).unwrap().x = 10;
    w.get_mut::<Position>(p).unwrap().y = 10;
    let wand = spawn_wand(&mut w, WandEffect::Digging, Position { x: 0, y: 0 });
    w.entity_mut(wand).remove::<Position>();
    w.get_mut::<Backpack>(p).unwrap().items.push(wand);
    let slot = w.get::<Backpack>(p).unwrap().items.len() - 1;
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: p,
        item: wand,
        target: Some(Position { x: 11, y: 10 }),
        slot_idx: Some(slot),
    });
    item_system(&mut w);

    let map = w.resource::<Map>();
    assert_ne!(map.tile(11, 10), TileType::Wall, "plain rock is dug");
    assert_eq!(
        map.tile(12, 10),
        TileType::Wall,
        "the red room's wall holds"
    );
}

#[test]
fn a_monster_can_spawn_standing_on_an_item() {
    for seed in 0..200u64 {
        let mut w = world_at(seed, 1);
        let items: Vec<(u16, u16)> = w
            .query_filtered::<&Position, With<Item>>()
            .iter(&w)
            .map(|p| (p.x, p.y))
            .collect();
        let shared = w
            .query_filtered::<&Position, With<Mob>>()
            .iter(&w)
            .any(|p| items.contains(&(p.x, p.y)));
        if shared {
            return;
        }
    }
    panic!("no monster ever spawned on an item across the sweep");
}
