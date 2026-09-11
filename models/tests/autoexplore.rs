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
            assert!(
                steps < AUTO_EXPLORE_STEP_CAP,
                "seed {seed}: auto-explore never terminated"
            );
        }

        // Nothing reachable is left unmapped.
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

#[test]
fn travel_walks_the_player_onto_a_known_staircase() {
    for seed in [1u64, 7, 42, 2112, 55555] {
        let (mut w, player) = fresh_floor(seed);

        // Pretend the whole floor has been mapped so the staircase is "known".
        {
            let mut vs = w.get_mut::<Viewshed>(player).unwrap();
            for i in 0..MAP_TILE_COUNT {
                vs.revealed_tiles.insert(i);
            }
        }
        let target = stair_location(w.resource::<Map>(), true).unwrap();

        let mut steps = 0;
        loop {
            let here = {
                let p = w.get::<Position>(player).unwrap();
                (p.x, p.y)
            };
            if here == target {
                break;
            }
            let (dx, dy) = travel_step(&mut w, target)
                .unwrap_or_else(|| panic!("seed {seed}: lost the path to {target:?} at {here:?}"));
            let mut pos = w.get_mut::<Position>(player).unwrap();
            pos.x = (pos.x as i16 + dx) as u16;
            pos.y = (pos.y as i16 + dy) as u16;
            steps += 1;
            assert!(
                steps < AUTO_EXPLORE_STEP_CAP,
                "seed {seed}: travel never arrived"
            );
        }
        assert!(
            travel_step(&mut w, target).is_none(),
            "seed {seed}: still stepping after arrival"
        );
    }
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
    // Any open tile next to the player will do as the target.
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
            effect: TrapEffect::Dart,
            reveal: TrapReveal::Sight,
            revealed: true,
        },
    ));

    assert!(
        travel_step(&mut w, target).is_none(),
        "auto-travel should refuse to step onto a known trap"
    );
}

#[test]
fn explore_step_is_none_when_the_whole_map_is_known() {
    let (mut w, player) = fresh_floor(2112);
    {
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for i in 0..MAP_TILE_COUNT {
            vs.revealed_tiles.insert(i);
        }
    }
    assert!(explore_step(&mut w).is_none());
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

    // Only the trap this test plants should be in play.
    let existing_traps: Vec<Entity> = w.query_filtered::<Entity, With<Trap>>().iter(&w).collect();
    for e in existing_traps {
        w.despawn(e);
    }

    // A bare corridor, x 0..=6 at y = 5, walled everywhere else.
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
    // Reveal x = 0..=3 (passage row y=5, plus the walls hugging it at y=4/6,
    // exactly as the real visibility system would have when the player stood
    // there). x = 3 borders the unrevealed x = 4 and is the only tile with an
    // unseen neighbour — the lone frontier auto-explore would head for.
    {
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for x in 0..=3u16 {
            for y in 4..=6u16 {
                vs.revealed_tiles.insert(tile_index(x, y));
            }
        }
    }

    // A known trap parked right on that frontier tile.
    w.spawn((
        Position { x: 3, y: 5 },
        Trap {
            effect: TrapEffect::Dart,
            reveal: TrapReveal::Sight,
            revealed: true,
        },
    ));

    assert!(
        explore_step(&mut w).is_none(),
        "the only frontier tile is a known trap; explore should refuse it"
    );
}

#[test]
fn monster_in_sight_tracks_visible_mobs() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    // Clear the randomly-placed spawns so we control what is on screen.
    let mobs: Vec<Entity> = w.query_filtered::<Entity, With<Mob>>().iter(&w).collect();
    for e in mobs {
        w.despawn(e);
    }
    resolve_visibility(&mut w);
    assert!(
        !monster_in_sight(&mut w),
        "no mobs left, nothing should be in sight"
    );

    // Drop a visible monster right next to the player.
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let ppos = *w.get::<Position>(player).unwrap();
    w.spawn((
        Name { what: "orc".into() },
        Mob {
            movement_type: MovementType::Static,
        },
        Position {
            x: ppos.x + 1,
            y: ppos.y,
        },
        Faction::Monster,
    ));
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);
    assert!(
        monster_in_sight(&mut w),
        "an adjacent monster should be in sight"
    );
}
