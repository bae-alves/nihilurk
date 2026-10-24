//! `-fast`: a six-floor dungeon. The Element sits on floor six, and the
//! content of each floor is drawn as if the run were the usual thirteen long.

use bevy_ecs::prelude::*;
use models::*;

fn fast_world(seed: u64) -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
    w.insert_resource(RngSeed(seed));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    w.insert_resource(Floors::new(6));
    initialize_world(&mut w);
    w
}

#[test]
fn floors_four_and_five_are_always_special_and_the_rest_never() {
    for seed in 1..=20 {
        let mut w = fast_world(seed);
        for depth in 1..=6 {
            w.resource_mut::<Depth>().what = depth;
            create_map(&mut w);
            let special = w.resource::<Map>().level.is_some();
            assert_eq!(
                special,
                depth == 4 || depth == 5,
                "seed {seed} floor {depth}"
            );
        }
    }
}

/// Items on the floor outside any special room: a hoard is the same size
/// whatever the run's length and would drown the budget this compares.
fn floor_items(w: &mut World) -> usize {
    let special = w.resource::<Map>().special.clone();
    w.query_filtered::<&Position, With<Item>>()
        .iter(w)
        .filter(|p| special[tile_index(p.x, p.y)].is_none())
        .count()
}

#[test]
fn a_fast_floor_holds_far_more_items() {
    let (mut fast, mut full) = (0, 0);
    for seed in 1..=30 {
        fast += floor_items(&mut fast_world(seed));
        let mut w = World::new();
        w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(seed)));
        w.insert_resource(RngSeed(seed));
        w.init_resource::<GameLog>();
        w.insert_resource(PlayerName { what: "T".into() });
        initialize_world(&mut w);
        full += floor_items(&mut w);
    }
    assert!(fast > full, "fast {fast} vs full {full}");
}
