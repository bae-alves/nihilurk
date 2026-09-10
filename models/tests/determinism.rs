//! The seed contract.
//!
//! A run's dungeon must be reproducible, and a floor's *layout* must depend on
//! nothing but the seed and the depth. That is what lets a save rebuild its
//! floor from two numbers, and what stops a new row in a content table from
//! quietly rearranging everybody's dungeon.
//!
//! A floor's *contents* are looser: they depend on the seed, the depth, and the
//! staircase count (`FloorChanges`), so a repeat visit re-stocks the same
//! layout with different things. What they must still *not* depend on is the
//! shared `GameRng` — the blow-by-blow of the run — because that would tie a
//! floor to how a fight went rather than to a clean count.

use bevy_ecs::prelude::*;
use models::*;

/// A cheap FNV-1a over the tile layout — enough to tell two floors apart.
fn map_hash(w: &World) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for t in w.resource::<Map>().tiles.iter() {
        h ^= *t as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn new_run(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    w
}

fn descend(w: &mut World) {
    let player = w.query_filtered::<Entity, With<Player>>().single(w);
    let down = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Downstairs)
        .expect("every floor above the last has a down-stair");
    w.get_mut::<Position>(player).unwrap().x = (down % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(player).unwrap().y = (down / MAP_WIDTH as usize) as u16;
    assert!(change_level(w, true));
}

/// Everything visible on a floor, sorted so entity iteration order cannot make
/// two identical dungeons look different.
fn contents(w: &mut World) -> Vec<String> {
    let mut v: Vec<String> = w
        .query::<(&Name, &Position)>()
        .iter(w)
        .map(|(n, p)| format!("{}@{},{}", n.what, p.x, p.y))
        .collect();
    v.sort();
    v
}

#[test]
fn the_same_seed_generates_the_same_game() {
    for seed in [1u64, 42, 7777, 123_456_789] {
        let (mut a, mut b) = (new_run(seed), new_run(seed));
        for depth in 1..=4 {
            assert_eq!(
                map_hash(&a),
                map_hash(&b),
                "seed {seed}, floor {depth}: maps differ"
            );
            assert_eq!(
                contents(&mut a),
                contents(&mut b),
                "seed {seed}, floor {depth}"
            );
            descend(&mut a);
            descend(&mut b);
        }
    }
}

#[test]
fn different_seeds_generate_different_games() {
    let (mut a, mut b) = (new_run(1), new_run(2));
    assert_ne!(map_hash(&a), map_hash(&b));
    assert_ne!(contents(&mut a), contents(&mut b));
}

/// The important one. A floor's shape comes from `floor_rng(seed, depth)` and
/// from nothing else, so no amount of drawing on the shared `GameRng` — loot
/// rolls, monster placement, combat, a new row in a content table — can move a
/// wall. If this test fails, adding content has started rearranging the
/// dungeon, and every seed anyone has written down is worthless.
#[test]
fn a_floors_layout_depends_only_on_seed_and_depth() {
    for seed in [1u64, 42, 7777] {
        for depth in 1..=FINAL_DEPTH {
            let mut a = World::new();
            regenerate_map(&mut a, seed, depth);
            // The deepest floor has its down-stair carved back to plain floor to
            // make room for the relic. Both level generation and the save loader
            // do it; a bare `regenerate_map` does not, so do it here too.
            if depth >= FINAL_DEPTH {
                let mut map = a.resource_mut::<Map>();
                if let Some(i) = map.tiles.iter().position(|&t| t == TileType::Downstairs) {
                    map.tiles[i] = TileType::Room;
                }
            }

            // The same floor, reached the long way: a live run that has spent
            // its shared RNG stream on several floors of loot and monsters.
            let mut b = new_run(seed);
            for _ in 1..depth {
                descend(&mut b);
            }

            assert_eq!(
                map_hash(&a),
                map_hash(&b),
                "seed {seed}, floor {depth}: the played floor is not the rebuilt one"
            );
        }
    }
}

/// The other half of the contract. A floor's *contents* are drawn from
/// `content_rng(seed, depth, floor_changes)`, so they cannot depend on the
/// player's blow-by-blow history: two runs on one seed that spend wildly
/// different amounts of the shared `GameRng` on floor 1 — but take the same
/// number of staircases — must still walk into the same floor 2.
///
/// If this fails, something inside floor generation has started drawing from
/// the shared stream again.
#[test]
fn a_floors_contents_ignore_the_shared_rng_stream() {
    for seed in [1u64, 42, 7777] {
        let mut quiet = new_run(seed);

        // The same run, after burning a great deal of the shared stream on
        // floor 1 — the stand-in for a player who fought their way through it.
        let mut busy = new_run(seed);
        {
            use rand::Rng;
            let mut g = busy.resource_mut::<GameRng>();
            for _ in 0..10_000 {
                let _: u64 = g.0.r#gen();
            }
        }

        for depth in 2..=6 {
            descend(&mut quiet);
            descend(&mut busy);
            assert_eq!(
                map_hash(&quiet),
                map_hash(&busy),
                "seed {seed}, floor {depth}: layout moved with the shared stream"
            );
            assert_eq!(
                contents(&mut quiet),
                contents(&mut busy),
                "seed {seed}, floor {depth}: contents moved with the shared stream"
            );
        }
    }
}

/// A floor reached after a save and reload is the floor it would have been.
#[test]
fn reloading_mid_run_does_not_shift_the_next_floor() {
    let path = std::env::temp_dir().join("roog_reload_shift.sav");
    let path = path.to_str().unwrap();

    let mut straight = new_run(42);
    descend(&mut straight);

    let mut interrupted = new_run(42);
    save_game(&mut interrupted, path).unwrap();
    let mut reloaded = World::new();
    reloaded.init_resource::<GameLog>();
    load_game(&mut reloaded, path).unwrap();
    descend(&mut reloaded);

    assert_eq!(
        map_hash(&straight),
        map_hash(&reloaded),
        "reload moved the walls"
    );
    assert_eq!(
        contents(&mut straight),
        contents(&mut reloaded),
        "reload moved the contents"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn every_floor_of_a_run_is_a_different_place() {
    let mut seen = std::collections::HashSet::new();
    let mut w = new_run(42);
    for depth in 1..=8 {
        assert!(
            seen.insert(map_hash(&w)),
            "floor {depth} repeats an earlier layout"
        );
        descend(&mut w);
    }
}

/// Saving on floor 3 and loading must give back floor 3, not floor 1. The map
/// is not in the save file; it is rebuilt from `(seed, depth)`.
#[test]
fn a_reloaded_save_comes_back_to_the_floor_it_was_written_on() {
    let path = std::env::temp_dir().join("roog_determinism_test.sav");
    let path = path.to_str().unwrap();

    let mut w = new_run(42);
    descend(&mut w);
    descend(&mut w);
    assert_eq!(w.resource::<Depth>().what, 3);
    let live = map_hash(&w);
    save_game(&mut w, path).unwrap();

    let mut reloaded = World::new();
    reloaded.init_resource::<GameLog>();
    load_game(&mut reloaded, path).unwrap();

    assert_eq!(reloaded.resource::<Depth>().what, 3);
    assert_eq!(map_hash(&reloaded), live, "reload rebuilt the wrong floor");
    let _ = std::fs::remove_file(path);
}

/// Climbing back up returns you to the floor you left, walls and all. This is
/// the whole back half of a run: you take the Element of Yoord from floor 13
/// and carry it out through floors you have already walked.
#[test]
fn going_back_up_returns_you_to_the_same_floor() {
    let mut w = new_run(7);
    let floor1 = map_hash(&w);
    descend(&mut w);
    let floor2 = map_hash(&w);
    assert_ne!(floor1, floor2);

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);

    // The staircases only invert for someone carrying the relic.
    let relic = spawn_named(&mut w, ELEMENT_OF_YOORD, Position { x: 0, y: 0 }).unwrap();
    w.entity_mut(relic).remove::<Position>();
    w.get_mut::<Backpack>(player).unwrap().items.push(relic);

    let up = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Upstairs)
        .expect("floor 2 has an up-stair");
    w.get_mut::<Position>(player).unwrap().x = (up % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(player).unwrap().y = (up / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, false));

    assert_eq!(w.resource::<Depth>().what, 1);
    assert_eq!(
        map_hash(&w),
        floor1,
        "floor 1 was rebuilt as a different place"
    );
}

/// Every monster and floor item on the current floor, sorted — the player and
/// anything in a pack left out, so only the floor's own stock is compared.
fn floor_stock(w: &mut World) -> Vec<String> {
    let mut v: Vec<String> = w
        .query_filtered::<(&Name, &Position), Without<Player>>()
        .iter(w)
        .map(|(n, p)| format!("{}@{},{}", n.what, p.x, p.y))
        .collect();
    v.sort();
    v
}

/// The layout comes back identical on a repeat visit — but the contents do not.
/// Walk 1 -> 2 -> 1 and the corridors of floor 1 are the same corridors,
/// re-stocked with different monsters and loot.
#[test]
fn a_repeat_visit_keeps_the_layout_but_rerolls_the_contents() {
    let mut w = new_run(7);
    let layout_first = map_hash(&w);
    let stock_first = floor_stock(&mut w);

    descend(&mut w); // 1 -> 2

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let relic = spawn_named(&mut w, ELEMENT_OF_YOORD, Position { x: 0, y: 0 }).unwrap();
    w.entity_mut(relic).remove::<Position>();
    w.get_mut::<Backpack>(player).unwrap().items.push(relic);

    let up = w
        .resource::<Map>()
        .tiles
        .iter()
        .position(|&t| t == TileType::Upstairs)
        .unwrap();
    w.get_mut::<Position>(player).unwrap().x = (up % MAP_WIDTH as usize) as u16;
    w.get_mut::<Position>(player).unwrap().y = (up / MAP_WIDTH as usize) as u16;
    assert!(change_level(&mut w, false)); // 2 -> 1

    assert_eq!(w.resource::<Depth>().what, 1);
    assert_eq!(
        map_hash(&w),
        layout_first,
        "the layout of floor 1 moved between visits"
    );
    assert_ne!(
        stock_first,
        floor_stock(&mut w),
        "floor 1 came back with exactly the same things in exactly the same places"
    );
}
