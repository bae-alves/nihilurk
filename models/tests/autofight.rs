use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use models::*;

/// Eight-way neighbour offsets, matching the player's moves and the pathfinder.
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

/// A world with a generated first floor and every monster / floor item removed,
/// so the test controls exactly what is on the map. Visibility is resolved once.
fn fresh_floor(seed: u64) -> (World, Entity) {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
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

fn resolve_visibility(w: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(visibility_system);
    schedule.run(w);
}

fn player_xy(w: &mut World, player: Entity) -> (u16, u16) {
    let p = w.get::<Position>(player).unwrap();
    (p.x, p.y)
}

/// Spawn a bare hostile at `(x, y)` with the given HP. No `Hidden`, so it counts
/// as "in sight" until the visibility system says otherwise.
fn spawn_enemy(w: &mut World, x: u16, y: u16, hp: i32) -> Entity {
    w.spawn((
        Name {
            what: "dummy".into(),
        },
        Mob {
            movement_type: MovementType::Static,
        },
        Position { x, y },
        Fighter {
            hp,
            max_hp: hp,
            armor: 0,
            power: 1,
            max_power: 1,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Faction::Monster,
        Blood,
    ))
    .id()
}

fn set_player_hp(w: &mut World, player: Entity, hp: i32) {
    w.get_mut::<Fighter>(player).unwrap().hp = hp;
}

#[test]
fn player_too_injured_matches_the_quarter_hp_rule() {
    let (mut w, player) = fresh_floor(2112);
    let max = w.get::<Fighter>(player).unwrap().max_hp; // 12

    set_player_hp(&mut w, player, max);
    assert!(!player_too_injured(&mut w), "full HP is fine");

    set_player_hp(&mut w, player, max / 4 + 1); // 4
    assert!(!player_too_injured(&mut w), "just above a quarter is fine");

    set_player_hp(&mut w, player, max / 4); // 3, i.e. exactly 25%
    assert!(
        player_too_injured(&mut w),
        "exactly a quarter is too injured"
    );

    set_player_hp(&mut w, player, 1);
    assert!(player_too_injured(&mut w), "one HP is too injured");
}

#[test]
fn target_is_the_lowest_hp_foe_when_none_are_adjacent() {
    let (mut w, player) = fresh_floor(2112);
    let (px, py) = player_xy(&mut w, player);

    spawn_enemy(&mut w, px + 2, py, 5);
    let weakest = spawn_enemy(&mut w, px + 3, py, 2);
    spawn_enemy(&mut w, px + 4, py, 8);

    assert_eq!(auto_fight_target(&mut w), Some(weakest));
}

#[test]
fn an_adjacent_foe_is_finished_before_chasing_a_weaker_distant_one() {
    let (mut w, player) = fresh_floor(2112);
    let (px, py) = player_xy(&mut w, player);

    let adjacent = spawn_enemy(&mut w, px + 1, py, 8);
    spawn_enemy(&mut w, px + 3, py, 1); // weaker, but two tiles further out

    assert_eq!(
        auto_fight_target(&mut w),
        Some(adjacent),
        "should not turn its back on an adjacent enemy"
    );
}

#[test]
fn hidden_foes_are_not_targeted() {
    let (mut w, player) = fresh_floor(2112);

    // Far opposite corner from any starting room: out of view, so the
    // visibility pass stamps it Hidden.
    spawn_enemy(&mut w, MAP_WIDTH - 2, MAP_HEIGHT - 2, 1);
    w.get_mut::<Viewshed>(player).unwrap().dirty = true;
    resolve_visibility(&mut w);

    assert_eq!(auto_fight_target(&mut w), None);
}

#[test]
fn fight_step_steps_straight_onto_an_adjacent_target() {
    let (mut w, player) = fresh_floor(2112);
    let (px, py) = player_xy(&mut w, player);

    let target = spawn_enemy(&mut w, px + 1, py + 1, 3);
    assert_eq!(fight_step(&mut w, target), Some((1, 1)));

    let target2 = spawn_enemy(&mut w, px - 1, py, 3);
    assert_eq!(fight_step(&mut w, target2), Some((-1, 0)));
}

#[test]
fn tab_closes_on_and_kills_a_distant_foe() {
    for seed in [1u64, 7, 42, 2112, 55555] {
        let (mut w, player) = fresh_floor(seed);

        // Pretend the whole floor is mapped so the pathfinder can see a route.
        {
            let mut vs = w.get_mut::<Viewshed>(player).unwrap();
            for i in 0..MAP_TILE_COUNT {
                vs.revealed_tiles.insert(i);
            }
        }

        // A walkable tile several steps away from the player.
        let start = player_xy(&mut w, player);
        let spot = far_reachable_tile(&mut w, start, 5)
            .unwrap_or_else(|| panic!("seed {seed}: nowhere to put a foe"));
        let foe = spawn_enemy(&mut w, spot.0, spot.1, 1); // 1 HP: one solid hit ends it
        w.get_mut::<Viewshed>(player).unwrap().dirty = true;
        resolve_visibility(&mut w);

        let mut steps = 0;
        while let Some(target) = auto_fight_target(&mut w) {
            assert_eq!(target, foe, "seed {seed}: only one foe exists");

            let (dx, dy) = fight_step(&mut w, target)
                .unwrap_or_else(|| panic!("seed {seed}: lost the path to the foe"));
            assert!(
                dx.abs() <= 1 && dy.abs() <= 1 && (dx != 0 || dy != 0),
                "seed {seed}: bad step"
            );

            let (px, py) = player_xy(&mut w, player);
            let (nx, ny) = ((px as i16 + dx) as u16, (py as i16 + dy) as u16);
            let foe_pos = *w.get::<Position>(foe).unwrap();

            if (nx, ny) == (foe_pos.x, foe_pos.y) {
                resolve_attack(&mut w, player, foe); // a step onto the foe is a strike
            } else {
                assert!(
                    !w.resource::<Map>().blocks(nx, ny),
                    "seed {seed}: stepped into a wall"
                );
                let mut pos = w.get_mut::<Position>(player).unwrap();
                pos.x = nx;
                pos.y = ny;
                w.get_mut::<Viewshed>(player).unwrap().dirty = true;
                resolve_visibility(&mut w);
            }

            steps += 1;
            assert!(steps < 500, "seed {seed}: auto-fight never reached the foe");
        }

        assert!(
            w.get_entity(foe).is_none(),
            "seed {seed}: the foe should be dead"
        );
        assert!(steps > 0, "seed {seed}: made no moves");
    }
}

/// BFS over walkable tiles from `start`; returns the first tile found at path
/// distance `>= min_dist`, or the farthest tile if none is that far.
fn far_reachable_tile(w: &mut World, start: (u16, u16), min_dist: u32) -> Option<(u16, u16)> {
    let map = w.resource::<Map>().clone();
    let mut seen = vec![start];
    let mut queue = VecDeque::new();
    queue.push_back((start, 0u32));
    let mut farthest = None;
    while let Some(((cx, cy), d)) = queue.pop_front() {
        if (cx, cy) != start {
            farthest = Some((cx, cy));
            if d >= min_dist {
                return Some((cx, cy));
            }
        }
        for (dx, dy) in DIRS {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            if map.blocks(nx, ny) || seen.contains(&(nx, ny)) {
                continue;
            }
            seen.push((nx, ny));
            queue.push_back(((nx, ny), d + 1));
        }
    }
    farthest
}
