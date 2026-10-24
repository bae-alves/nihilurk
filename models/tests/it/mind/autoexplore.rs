//! `o`'s pathfinding: the nearest frontier or item `explore_step` walks
//! toward, the staircase beeline, the items it leaves behind, and the
//! refusals — a known trap, a fully-explored floor — that end a walk before
//! it starts.

use std::collections::{HashSet, VecDeque};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

/// Eight-way neighbour offsets, matching both the player's moves and the
/// auto-explore pathfinder.
const DIRS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// A bare world with a generated first floor, its visibility resolved once, and
/// every monster and floor item removed so the walk is over pure terrain.
fn fresh_floor(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && (e.contains::<Mob>() || e.contains::<Item>()))
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    resolve_visibility(&mut w);
    (w, player)
}

/// Runs the visibility system in isolation (the engine schedule lives in the
/// binary crate, so the test drives the one system it needs directly).
fn resolve_visibility(w: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(w);
}

/// Every walkable tile reachable from the player over an 8-connected path of
/// walkable tiles.
fn reachable_walkable(w: &mut World, player: Entity) -> HashSet<(u16, u16)> {
    let start = {
        let p = w.get::<Position>(player).unwrap();
        (p.x, p.y)
    };
    let map = w.resource::<Map>().clone();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);
    while let Some((cx, cy)) = queue.pop_front() {
        for (dx, dy) in DIRS {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            if map.blocks(nx, ny) || !seen.insert((nx, ny)) {
                continue;
            }
            queue.push_back((nx, ny));
        }
    }
    seen
}

#[test]
fn auto_explore_reveals_every_reachable_tile_then_stops() {
    for seed in [1u64, 7, 42, 99, 2112, 55555] {
        let (mut w, player) = fresh_floor(seed);

        let mut steps = 0;
        while let Some((dx, dy)) = explore_step(&mut w) {
            assert!(
                dx.abs() <= 1 && dy.abs() <= 1 && (dx != 0 || dy != 0),
                "seed {seed}: bad step"
            );

            let mut pos = w.get_mut::<Position>(player).unwrap();
            pos.x = (pos.x as i16 + dx) as u16;
            pos.y = (pos.y as i16 + dy) as u16;
            assert!(
                !w.resource::<Map>().blocks(
                    w.get::<Position>(player).unwrap().x,
                    w.get::<Position>(player).unwrap().y
                ),
                "seed {seed}: stepped into a wall"
            );
            w.get_mut::<Viewshed>(player).unwrap().dirty = true;
            resolve_visibility(&mut w);

            steps += 1;
            const NEVER: u32 = 20_000;
            assert!(steps < NEVER, "seed {seed}: auto-explore never terminated");
        }

        let reachable = reachable_walkable(&mut w, player);
        let revealed = w.get::<Viewshed>(player).unwrap().revealed_tiles.clone();
        for (x, y) in &reachable {
            assert!(
                revealed.contains(tile_index(*x, *y)),
                "seed {seed}: ({x},{y}) is reachable but auto-explore left it unseen"
            );
        }
        assert!(steps > 0, "seed {seed}: made no progress on a fresh floor");
    }
}

#[test]
fn stair_location_points_at_the_right_tiles() {
    let (w, _player) = fresh_floor(2112);
    let map = w.resource::<Map>();
    let down = stair_location(map, true).expect("a downstairs");
    let up = stair_location(map, false).expect("an upstairs");
    assert_eq!(map.tile(down.0, down.1), TileType::Downstairs);
    assert_eq!(map.tile(up.0, up.1), TileType::Upstairs);
}

/// A known trap sitting on the travel target itself is refused rather than
/// walked onto — the player can still step there by hand, but auto-travel
/// won't do it for them.
#[test]
fn travel_step_wont_walk_onto_a_known_trap() {
    let (mut w, player) = fresh_floor(2112);
    {
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for i in 0..MAP_TILE_COUNT {
            vs.revealed_tiles.insert(i);
        }
    }
    let (px, py) = {
        let p = w.get::<Position>(player).unwrap();
        (p.x, p.y)
    };
    let map = w.resource::<Map>().clone();
    let target = DIRS
        .iter()
        .map(|&(dx, dy)| ((px as i32 + dx) as u16, (py as i32 + dy) as u16))
        .find(|&(x, y)| !map.blocks(x, y))
        .expect("a player never stands fully walled in");

    w.spawn((
        Position {
            x: target.0,
            y: target.1,
        },
        Trap {
            row: TrapDef::lookup("dart trap").unwrap(),
            reveal: TrapReveal::Sight,
            revealed: true,
        },
    ));

    assert!(
        travel_step(&mut w, target).is_none(),
        "auto-travel should refuse to step onto a known trap"
    );
}

/// A known trap sitting on the only tile bordering unexplored ground takes it
/// out of consideration as a frontier: auto-explore refuses to walk onto it
/// rather than treating it like any other open tile.
#[test]
fn explore_step_wont_walk_onto_a_known_trap() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let existing_traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();
    for e in existing_traps {
        w.despawn(e);
    }

    {
        let mut map = w.resource_mut::<Map>();
        for t in map.tiles.iter_mut() {
            *t = TileType::Wall;
        }
        for x in 0..=6u16 {
            map.tiles[tile_index(x, 5)] = TileType::Passage;
        }
    }

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    {
        let mut p = w.get_mut::<Position>(player).unwrap();
        p.x = 0;
        p.y = 5;
    }
    {
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for x in 0..=3u16 {
            for y in 4..=6u16 {
                vs.revealed_tiles.insert(tile_index(x, y));
            }
        }
    }

    w.spawn((
        Position { x: 3, y: 5 },
        Trap {
            row: TrapDef::lookup("dart trap").unwrap(),
            reveal: TrapReveal::Sight,
            revealed: true,
        },
    ));

    assert!(
        explore_step(&mut w).is_none(),
        "the only frontier tile is a known trap; explore should refuse it"
    );
}

/// A generated first floor walled up whole, then opened again where a test
/// says, with nothing on it but the player and an empty pack: no monster, no
/// trap, no floor item the generator placed.
fn walled_floor() -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.init_resource::<AutoExplore>();
    w.init_resource::<AutoPickup>();
    w.init_resource::<LeftBehind>();
    w.init_resource::<PlayerActionQueue>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let clutter: Vec<Entity> = w
        .iter_entities()
        .filter(|e| {
            !e.contains::<Player>()
                && (e.contains::<Trap>() || e.contains::<Item>() || e.contains::<Mob>())
        })
        .map(|e| e.id())
        .collect();
    for e in clutter {
        w.despawn(e);
    }

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    w.get_mut::<Backpack>(player).unwrap().items.clear();
    for t in w.resource_mut::<Map>().tiles.iter_mut() {
        *t = TileType::Wall;
    }
    (w, player)
}

/// Opens `tiles` as `kind` and reveals them and the ring of tiles around them.
fn open(w: &mut World, player: Entity, kind: TileType, tiles: &[(u16, u16)]) {
    for &(x, y) in tiles {
        w.resource_mut::<Map>().tiles[tile_index(x, y)] = kind;
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for (dx, dy) in DIRS.iter().chain(&[(0, 0)]) {
            vs.revealed_tiles
                .insert(tile_index((x as i32 + dx) as u16, (y as i32 + dy) as u16));
        }
    }
}

/// Hides `tiles` again, so the open tiles beside them are frontiers.
fn unseen(w: &mut World, player: Entity, tiles: &[(u16, u16)]) {
    let mut vs = w.get_mut::<Viewshed>(player).unwrap();
    for &(x, y) in tiles {
        vs.revealed_tiles.set(tile_index(x, y), false);
    }
}

/// A corridor on row 10 from `west` to `east`, both ends frontiers, with the
/// player at `at`.
fn corridor(west: u16, east: u16, at: u16) -> (World, Entity) {
    let (mut w, player) = walled_floor();
    let tiles: Vec<(u16, u16)> = (west..=east).map(|x| (x, 10)).collect();
    open(&mut w, player, TileType::Passage, &tiles);
    unseen(&mut w, player, &[(west - 1, 10), (east + 1, 10)]);
    *w.get_mut::<Position>(player).unwrap() = Position { x: at, y: 10 };
    (w, player)
}

/// A room (x 3..=7, rows 9..=11), its door at (8, 10), and a corridor east
/// from it to a frontier at (20, 10). The player stands at the room's east
/// edge, (7, 10); a spotted scroll lies at its west edge, (3, 10), six hops
/// away from the corridor's first tile, where the frontier is eleven.
fn room_with_a_scroll() -> (World, Entity, Entity) {
    let (mut w, player) = walled_floor();
    let room: Vec<(u16, u16)> = (3..=7)
        .flat_map(|x| (9..=11).map(move |y| (x, y)))
        .collect();
    open(&mut w, player, TileType::Room, &room);
    open(&mut w, player, TileType::Door, &[(8, 10)]);
    let hall: Vec<(u16, u16)> = (9..=20).map(|x| (x, 10)).collect();
    open(&mut w, player, TileType::Passage, &hall);
    unseen(&mut w, player, &[(21, 10)]);
    *w.get_mut::<Position>(player).unwrap() = Position { x: 7, y: 10 };
    let scroll = spawn_named(&mut w, "scroll of identify", Position { x: 3, y: 10 }).unwrap();
    w.entity_mut(scroll).remove::<Hidden>();
    (w, player, scroll)
}

/// Takes one step the way a key press does: planned, queued, applied.
fn walk(w: &mut World, dx: i16, dy: i16) {
    assert!(queue_step(w, dx, dy), "the step ({dx}, {dy}) was refused");
    player_action_system(w);
}

/// `A` off: the walk stops noticing loot and goes back to exploring. The item
/// is still there, still spotted — it is simply no longer a destination.
#[test]
fn the_pickup_toggle_calls_off_the_beeline() {
    let (mut w, _) = corridor(3, 12, 10);
    w.spawn((Position { x: 9, y: 10 }, Item));
    assert_eq!(
        explore_step(&mut w),
        Some((-1, 0)),
        "the detour is the default"
    );

    w.resource_mut::<AutoPickup>().enabled = false;
    assert_eq!(
        explore_step(&mut w),
        Some((1, 0)),
        "with pick-up off the walk should head for the nearer east end"
    );
}

/// A full pack calls the beeline off by itself. Without this the walk marches
/// to an item it cannot lift, is halted by "Your pack is full.", and does the
/// same thing again on the next `o` — the floor never gets explored.
#[test]
fn a_full_pack_calls_off_the_beeline_without_being_told_to() {
    let (mut w, player) = corridor(3, 12, 10);
    w.spawn((Position { x: 9, y: 10 }, Item));

    let junk: Vec<Entity> = (0..models::constants::items::PACK_CAPACITY)
        .map(|_| w.spawn(Item).id())
        .collect();
    w.get_mut::<Backpack>(player).unwrap().items = junk;

    assert_eq!(
        explore_step(&mut w),
        Some((1, 0)),
        "with nowhere to put it, the walk should explore instead of fetching"
    );
}

/// What the player drops, the walk leaves where it lies. Without this the
/// next `o` picks it straight back up.
#[test]
fn a_dropped_item_is_left_behind() {
    let (mut w, player) = corridor(3, 14, 9);
    let scroll = spawn_named(&mut w, "scroll of identify", Position { x: 9, y: 10 }).unwrap();
    pick_up_here(&mut w, player, 9, 10);
    assert!(queue_drop(&mut w, player, scroll));
    player_action_system(&mut w);
    *w.get_mut::<Position>(player).unwrap() = Position { x: 10, y: 10 };

    assert_eq!(
        explore_step(&mut w),
        Some((1, 0)),
        "the dropped scroll is one hop west, but the walk should explore east"
    );
}

/// Picking a dropped item back up forgets the drop: should it reach the floor
/// again some other way, the walk wants it again.
#[test]
fn picking_a_dropped_item_back_up_forgets_the_drop() {
    let (mut w, player) = corridor(3, 14, 9);
    let scroll = spawn_named(&mut w, "scroll of identify", Position { x: 9, y: 10 }).unwrap();
    pick_up_here(&mut w, player, 9, 10);
    assert!(queue_drop(&mut w, player, scroll));
    player_action_system(&mut w);
    pick_up_here(&mut w, player, 9, 10);

    assert!(!w.resource::<LeftBehind>().holds(scroll));
}

/// Walking out of a room past a spotted item you did not take: the walk
/// leaves it there too.
#[test]
fn an_item_seen_in_a_room_you_walk_out_of_is_left_behind() {
    let (mut w, _, _) = room_with_a_scroll();
    walk(&mut w, 1, 0);
    walk(&mut w, 1, 0);
    assert_eq!(
        explore_step(&mut w),
        Some((1, 0)),
        "the scroll was left in the room behind; the walk should explore east"
    );
}

/// Auto-explore's own steps out of a room leave nothing behind: the walk
/// passing an item on the way to a nearer frontier has not turned it down.
#[test]
fn leaving_a_room_by_auto_explore_leaves_nothing_behind() {
    let (mut w, _, _) = room_with_a_scroll();
    w.resource_mut::<AutoExplore>().start(None);
    walk(&mut w, 1, 0);
    walk(&mut w, 1, 0);
    assert_eq!(
        explore_step(&mut w),
        Some((-1, 0)),
        "the walk left the room itself; it should go back for the scroll"
    );
}

/// The doorway is not out: a step into it and back leaves nothing behind.
#[test]
fn a_step_into_the_doorway_and_back_leaves_nothing_behind() {
    let (mut w, _, _) = room_with_a_scroll();
    walk(&mut w, 1, 0);
    walk(&mut w, -1, 0);
    walk(&mut w, 1, 0);
    walk(&mut w, -1, 0);
    assert_eq!(explore_step(&mut w), Some((-1, 0)));
}
