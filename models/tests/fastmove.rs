//! NetHack-style fast movement: the planner that decides between a straight run
//! and a beeline to a feature, plus the straight-run stop conditions.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

fn resolve_visibility(w: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(w);
}

/// A world with a real first floor, then everything wiped and a single clean
/// rectangular room carved in, with the player parked in the middle. Fully
/// controlled: no stray monsters, items, corridors or stairs unless the test
/// adds them.
///
/// Room spans x 10..=24, y 5..=13; player at (17, 9).
fn arena() -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
    w.init_resource::<FastMove>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);

    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && (e.contains::<Mob>() || e.contains::<Item>()))
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }

    {
        let mut map = w.resource_mut::<Map>();
        for t in map.tiles.iter_mut() {
            *t = TileType::Wall;
        }
        for y in 5..=13u16 {
            for x in 10..=24u16 {
                map.tiles[tile_index(x, y)] = TileType::Room;
            }
        }
    }

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    {
        let mut p = w.get_mut::<Position>(player).unwrap();
        p.x = 17;
        p.y = 9;
    }
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);
    (w, player)
}

fn set_tile(w: &mut World, x: u16, y: u16, t: TileType) {
    w.resource_mut::<Map>().tiles[tile_index(x, y)] = t;
}

#[test]
fn running_is_refused_while_a_creature_is_in_view() {
    let (mut w, player) = arena();
    w.spawn((
        Name { what: "orc".into() },
        Mob { movement_type: MovementType::Static },
        Position { x: 19, y: 9 },
        Faction::Monster,
    ));
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);

    assert!(matches!(fast_move_plan(&mut w, 1, 0), FastMovePlan::MonsterInSight));
}

#[test]
fn open_ground_with_nothing_ahead_is_a_straight_run() {
    let (mut w, _player) = arena();
    assert!(matches!(fast_move_plan(&mut w, -1, 0), FastMovePlan::Straight));
    assert!(matches!(fast_move_plan(&mut w, 0, 1), FastMovePlan::Straight));
}

#[test]
fn facing_a_wall_with_nothing_ahead_is_blocked() {
    let (mut w, player) = arena();
    {
        let mut p = w.get_mut::<Position>(player).unwrap();
        p.x = 10; // hard against the west wall (x = 9)
    }
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);

    assert!(matches!(fast_move_plan(&mut w, -1, 0), FastMovePlan::Blocked));
}

#[test]
fn a_door_in_view_that_way_is_a_beeline() {
    let (mut w, player) = arena();
    set_tile(&mut w, 22, 9, TileType::Door); // due east of the player
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);

    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Travel(tile) => assert_eq!(tile, (22, 9)),
        other => panic!("expected a beeline to the door, got {:?}", plan_name(&other)),
    }
    // ...but not when running the other way.
    assert!(matches!(fast_move_plan(&mut w, -1, 0), FastMovePlan::Straight));
}

#[test]
fn stairs_beat_doors_beat_items() {
    let (mut w, player) = arena();
    set_tile(&mut w, 23, 9, TileType::Door); // east
    set_tile(&mut w, 21, 11, TileType::Downstairs); // east-ish (within the cone)
    w.spawn((
        Name { what: "gold".into() },
        Item,
        Position { x: 19, y: 9 },
    ));
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);

    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Travel(tile) => assert_eq!(tile, (21, 11), "stairs win"),
        other => panic!("expected the stairs, got {:?}", plan_name(&other)),
    }

    // Drop the stairs back to plain floor: the door is now the pick.
    set_tile(&mut w, 21, 11, TileType::Room);
    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Travel(tile) => assert_eq!(tile, (23, 9), "door beats the item"),
        other => panic!("expected the door, got {:?}", plan_name(&other)),
    }

    // Door gone too: fall back to the item.
    set_tile(&mut w, 23, 9, TileType::Room);
    match fast_move_plan(&mut w, 1, 0) {
        FastMovePlan::Travel(tile) => assert_eq!(tile, (19, 9), "item is all that's left"),
        other => panic!("expected the item, got {:?}", plan_name(&other)),
    }
}

#[test]
fn straight_step_stops_at_a_wall() {
    let (mut w, player) = arena();
    w.resource_mut::<FastMove>().start(1, 0, None);

    // Mid-room: keeps going east.
    assert_eq!(straight_step(&mut w), Some((1, 0)));

    // On the east edge (x = 24), the next tile east is wall.
    w.get_mut::<Position>(player).unwrap().x = 24;
    assert_eq!(straight_step(&mut w), None);
}

#[test]
fn straight_run_halts_on_a_door_and_at_a_corridor_branch() {
    let (mut w, player) = arena();

    // Wipe to a bare horizontal corridor y = 9, x 10..=24, with one side
    // passage at (17, 10) and a door at (20, 9).
    {
        let mut map = w.resource_mut::<Map>();
        for t in map.tiles.iter_mut() {
            *t = TileType::Wall;
        }
        for x in 10..=24u16 {
            map.tiles[tile_index(x, 9)] = TileType::Passage;
        }
        map.tiles[tile_index(17, 10)] = TileType::Passage;
        map.tiles[tile_index(20, 9)] = TileType::Door;
    }
    w.resource_mut::<FastMove>().start(1, 0, None);

    // Plain corridor tile: run on.
    w.get_mut::<Position>(player).unwrap().x = 15;
    w.get_mut::<Position>(player).unwrap().y = 9;
    assert!(!straight_stop_here(&mut w));

    // The tile with a side passage: stop.
    w.get_mut::<Position>(player).unwrap().x = 17;
    assert!(straight_stop_here(&mut w));

    // The door: stop.
    w.get_mut::<Position>(player).unwrap().x = 20;
    assert!(straight_stop_here(&mut w));
}

/// Tiny helper so panics on the `Travel` matches print something legible.
fn plan_name(p: &FastMovePlan) -> &'static str {
    match p {
        FastMovePlan::MonsterInSight => "MonsterInSight",
        FastMovePlan::Blocked => "Blocked",
        FastMovePlan::Straight => "Straight",
        FastMovePlan::Travel(_) => "Travel",
    }
}
