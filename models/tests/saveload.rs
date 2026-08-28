use bevy_ecs::prelude::*;
use models::*;

#[test]
fn round_trip() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w.insert_resource(RngSeed(1));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName { what: "TESTER".into() });
    initialize_world(&mut w);
    let n0 = w.iter_entities().count();
    let path = std::env::temp_dir().join("roog_test.sav");
    let p = path.to_str().unwrap();
    save_game(&mut w, p).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(2)));
    w2.insert_resource(RngSeed(2));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, p).unwrap();
    assert_eq!(n0, w2.iter_entities().count());
    assert_eq!(w2.resource::<RngSeed>().0, 1);
    // RNG state resumes: next draws match the original world's next draws.
    use rand::Rng;
    let a: u64 = w.resource_mut::<GameRng>().0.r#gen();
    let b: u64 = w2.resource_mut::<GameRng>().0.r#gen();
    assert_eq!(a, b);
    assert_eq!(w2.resource::<PlayerName>().what, "TESTER");
    let mut q = w2.query_filtered::<&Backpack, With<Player>>();
    assert_eq!(q.single(&w2).items.len(), 1);
    assert!(w2.get::<Wand>(q.single(&w2).items[0]).is_some());

    // Map regenerated from the seed matches the original tile-for-tile.
    assert_eq!(w.resource::<Map>().tiles, w2.resource::<Map>().tiles);
    assert!(w2
        .resource::<Map>()
        .tiles
        .iter()
        .any(|&t| t == TileType::Wall));
}
