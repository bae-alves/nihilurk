//! `-fast`: a six-floor dungeon. The Element sits on floor six, and the
//! content of each floor is drawn as if the run were the usual thirteen long.

mod common;

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
fn content_depth_stretches_the_short_run_over_the_long_one() {
    let fast = Floors::new(6);
    let seen: Vec<u8> = (1..=6).map(|d| fast.content_depth(d)).collect();
    assert!(seen.windows(2).all(|p| p[0] < p[1]), "{seen:?}");
    assert_eq!(seen.last().copied(), Some(Floors::default().last));

    let full = Floors::default();
    assert!((1..=13).all(|d| full.content_depth(d) == d));
}

#[test]
fn floor_six_holds_the_element_and_has_no_down_stair() {
    let mut w = fast_world(7);
    let p = w.query_filtered::<Entity, With<Player>>().single(&w);
    while w.resource::<Depth>().what < 6 {
        let down = w
            .resource::<Map>()
            .tiles
            .iter()
            .position(|&t| t == TileType::Downstairs)
            .expect("every floor above the last has a down-stair");
        w.get_mut::<Position>(p).unwrap().x = (down % MAP_WIDTH as usize) as u16;
        w.get_mut::<Position>(p).unwrap().y = (down / MAP_WIDTH as usize) as u16;
        assert!(change_level(&mut w, true));
    }
    assert!(!w.resource::<Map>().tiles.contains(&TileType::Downstairs));
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

#[test]
fn a_save_remembers_the_run_is_fast() {
    let save = common::SaveFile::new("fast-floors");
    let mut w = fast_world(7);
    save_game(&mut w, save.path()).unwrap();

    let mut loaded = World::new();
    loaded.init_resource::<GameLog>();
    load_game(&mut loaded, save.path()).unwrap();
    assert_eq!(loaded.resource::<Floors>().last, 6);
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

#[test]
fn a_save_remembers_the_run_is_endless() {
    let save = common::SaveFile::new("endless-flag");
    let mut w = fast_world(7);
    w.insert_resource(Endless { enabled: true });
    save_game(&mut w, save.path()).unwrap();

    let mut loaded = World::new();
    loaded.init_resource::<GameLog>();
    load_game(&mut loaded, save.path()).unwrap();
    assert!(loaded.resource::<Endless>().enabled);
}
