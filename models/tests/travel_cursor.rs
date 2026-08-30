use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

#[test]
fn open_seeds_the_cursor_then_close_clears_it() {
    let mut tc = TravelCursor::default();
    assert!(!tc.active);

    tc.open(4, 7);
    assert!(tc.active && tc.x == 4 && tc.y == 7 && tc.blink_on);

    tc.close();
    assert!(!tc.active);
}

#[test]
fn tile_is_revealed_follows_the_players_viewshed() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);

    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(&mut w);

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let ppos = *w.get::<Position>(player).unwrap();

    // Some tile the player has not seen yet — a fresh floor is never fully lit.
    let (ux, uy) = {
        let vs = w.get::<Viewshed>(player).unwrap();
        let i = (0..MAP_TILE_COUNT)
            .find(|&i| !vs.revealed_tiles.contains(i))
            .expect("a fresh floor should have unseen tiles");
        ((i % MAP_WIDTH as usize) as u16, (i / MAP_WIDTH as usize) as u16)
    };

    assert!(tile_is_revealed(&mut w, ppos.x, ppos.y), "own tile is revealed");
    assert!(!tile_is_revealed(&mut w, ux, uy), "unseen tile is not revealed");
    assert!(!tile_is_revealed(&mut w, MAP_WIDTH + 5, 0), "out of bounds is not revealed");
}

const DIRS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
];

/// A fully-revealed fresh floor with monsters and items removed.
fn fresh_mapped_floor(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
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

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    {
        let mut vs = w.get_mut::<Viewshed>(player).unwrap();
        for i in 0..MAP_TILE_COUNT {
            vs.revealed_tiles.insert(i);
        }
    }
    (w, player)
}

#[test]
fn nearest_reachable_returns_a_reachable_walkable_tile() {
    for seed in [1u64, 7, 42, 2112, 55555] {
        let (mut w, _player) = fresh_mapped_floor(seed);
        let map = w.resource::<Map>().clone();

        // A walkable target hands itself straight back.
        let stairs = stair_location(&map, true).unwrap();
        assert_eq!(nearest_reachable(&mut w, stairs), Some(stairs), "seed {seed}");

        // A wall target is redirected to a tile you can actually stand on.
        let wall = (0..MAP_TILE_COUNT)
            .map(|i| ((i % MAP_WIDTH as usize) as u16, (i / MAP_WIDTH as usize) as u16))
            .find(|&(x, y)| {
                map.blocks(x, y)
                    && DIRS.iter().any(|&(dx, dy)| {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        nx >= 0
                            && ny >= 0
                            && nx < MAP_WIDTH as i32
                            && ny < MAP_HEIGHT as i32
                            && !map.blocks(nx as u16, ny as u16)
                    })
            })
            .expect("every floor has a wall beside open ground");
        let goal = nearest_reachable(&mut w, wall).expect("seed has a reachable tile");
        assert!(!map.blocks(goal.0, goal.1), "seed {seed}: routed onto a wall {goal:?}");
    }
}
