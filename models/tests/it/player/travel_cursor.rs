//! The `O` travel cursor: opening and closing it, that it only follows the
//! player's own revealed tiles, and that it resolves to the nearest reachable
//! walkable tile when the one picked isn't itself one.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

#[test]
fn tile_is_revealed_follows_the_players_viewshed() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2112)));
    w.insert_resource(RngSeed(2112));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(&mut w);

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let ppos = *w.get::<Position>(player).unwrap();

    let (ux, uy) = {
        let vs = w.get::<Viewshed>(player).unwrap();
        let i = (0..MAP_TILE_COUNT)
            .find(|&i| !vs.revealed_tiles.contains(i))
            .expect("a fresh floor should have unseen tiles");
        (
            (i % MAP_WIDTH as usize) as u16,
            (i / MAP_WIDTH as usize) as u16,
        )
    };

    assert!(
        tile_is_revealed(&mut w, ppos.x, ppos.y),
        "own tile is revealed"
    );
    assert!(
        !tile_is_revealed(&mut w, ux, uy),
        "unseen tile is not revealed"
    );
    assert!(
        !tile_is_revealed(&mut w, MAP_WIDTH + 5, 0),
        "out of bounds is not revealed"
    );
}

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

/// A fully-revealed fresh floor with monsters and items removed.
fn fresh_mapped_floor(seed: u64) -> (World, Entity) {
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

        let stairs = stair_location(&map, true).unwrap();
        assert_eq!(
            nearest_reachable(&mut w, stairs),
            Some(stairs),
            "seed {seed}"
        );

        let wall = (0..MAP_TILE_COUNT)
            .map(|i| {
                (
                    (i % MAP_WIDTH as usize) as u16,
                    (i / MAP_WIDTH as usize) as u16,
                )
            })
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
        assert!(
            !map.blocks(goal.0, goal.1),
            "seed {seed}: routed onto a wall {goal:?}"
        );
    }
}

#[test]
fn jump_stairs_lands_on_the_staircase_only_once_revealed() {
    let (mut w, player) = fresh_mapped_floor(42);
    let stairs = stair_location(w.resource::<Map>(), true).unwrap();
    assert_eq!(
        cursor_jump(&mut w, (0, 0), CursorJump::DownStairs, true),
        Some(stairs)
    );

    w.get_mut::<Viewshed>(player)
        .unwrap()
        .revealed_tiles
        .clear();
    assert_eq!(
        cursor_jump(&mut w, (0, 0), CursorJump::DownStairs, true),
        None
    );
}

#[test]
fn jump_items_cycles_nearest_first_both_ways() {
    let (mut w, player) = fresh_mapped_floor(42);
    let p = *w.get::<Position>(player).unwrap();
    for dx in [3u16, 1, 2] {
        w.spawn((
            Item,
            Position {
                x: p.x + dx,
                y: p.y,
            },
        ));
    }
    w.get_mut::<Viewshed>(player).unwrap().visible_tiles =
        (1..=3).map(|dx| (p.x + dx, p.y)).collect();
    let mut at = (p.x, p.y);
    let mut seen = vec![];
    for _ in 0..4 {
        at = cursor_jump(&mut w, at, CursorJump::Items, true).unwrap();
        seen.push(at.0 - p.x);
    }
    assert_eq!(seen, [1, 2, 3, 1]);

    let back = cursor_jump(&mut w, at, CursorJump::Items, false).unwrap();
    assert_eq!(back.0 - p.x, 3);
}

#[test]
fn jump_monsters_skips_unseen_and_hidden() {
    const MOB: Mob = Mob {
        movement_type: MovementType::Static,
    };
    let (mut w, player) = fresh_mapped_floor(42);
    let p = *w.get::<Position>(player).unwrap();
    let at = |dx| Position {
        x: p.x + dx,
        y: p.y,
    };
    w.spawn((MOB, at(2)));
    w.spawn((MOB, at(4)));
    w.spawn((MOB, Hidden, at(1)));
    w.get_mut::<Viewshed>(player).unwrap().visible_tiles = vec![(p.x + 1, p.y), (p.x + 2, p.y)];

    let first = cursor_jump(&mut w, (p.x, p.y), CursorJump::Monsters, true).unwrap();
    assert_eq!(first, (p.x + 2, p.y));
    assert_eq!(
        cursor_jump(&mut w, first, CursorJump::Monsters, true),
        Some(first)
    );
}

/// An item the map does not draw, hidden or out of view, is no target.
#[test]
fn jump_items_skips_what_the_map_does_not_draw() {
    let (mut w, player) = fresh_mapped_floor(42);
    let p = *w.get::<Position>(player).unwrap();
    let at = |dx| Position {
        x: p.x + dx,
        y: p.y,
    };
    w.spawn((Item, at(1)));
    w.spawn((Item, Hidden, at(2)));
    w.spawn((Item, at(3)));
    w.get_mut::<Viewshed>(player).unwrap().visible_tiles =
        (1..=2).map(|dx| (p.x + dx, p.y)).collect();

    let first = cursor_jump(&mut w, (p.x, p.y), CursorJump::Items, true).unwrap();
    assert_eq!(first, (p.x + 1, p.y));
    assert_eq!(
        cursor_jump(&mut w, first, CursorJump::Items, true),
        Some(first)
    );
}
