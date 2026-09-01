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
    w.resource_mut::<Depth>().what = 4;
    w.resource_mut::<Identified>().potions.insert(PotionEffect::Healing);
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
    assert_eq!(w2.resource::<Depth>().what, 4);
    // Identification knowledge and this run's item appearances survive too.
    assert!(w2.resource::<Identified>().potions.contains(&PotionEffect::Healing));
    assert_eq!(
        w.resource::<ItemAppearances>().potions,
        w2.resource::<ItemAppearances>().potions,
    );
    let mut q = w2.query_filtered::<&Backpack, With<Player>>();
    assert_eq!(q.single(&w2).items.len(), 1);
    assert!(w2.get::<Wand>(q.single(&w2).items[0]).is_some());

    // The player's magic pool survives the round trip.
    let magic = w2.query_filtered::<&Magic, With<Player>>().single(&w2);
    assert_eq!((magic.points, magic.max_points), (4, 4));

    // Equipment / scroll / ring components survive the round trip.
    let mut w3 = World::new();
    w3.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w3.insert_resource(RngSeed(3));
    w3.init_resource::<GameLog>();
    w3.insert_resource(PlayerName { what: "Y".into() });
    initialize_world(&mut w3);
    // Strip the floor's own spawned loot/monsters so this round trip only sees
    // the gear the test itself places below.
    let strays: Vec<Entity> = w3
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && e.contains::<Position>())
        .map(|e| e.id())
        .collect();
    for e in strays {
        w3.despawn(e);
    }
    let pos = Position { x: 5, y: 5 };
    let vorpal_sword = w3.spawn(WeaponsBundle::long_sword(pos)).id();
    w3.entity_mut(vorpal_sword).insert(Vorpal { bane: "dragon".into() });
    let cursed_armor = w3.spawn(ArmorBundle::plate_mail(pos)).id();
    w3.entity_mut(cursed_armor).insert(Curse);
    w3.spawn(ScrollBundle::magic_mapping(pos));
    w3.spawn(RingBundle::new(RingEffect::Regeneration, pos));
    w3.spawn(AmuletBundle::element_of_yoord(pos));
    let path3 = std::env::temp_dir().join("roog_test_gear.sav");
    let p3 = path3.to_str().unwrap();
    save_game(&mut w3, p3).unwrap();

    let mut w4 = World::new();
    w4.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(9)));
    w4.insert_resource(RngSeed(9));
    w4.init_resource::<GameLog>();
    w4.insert_resource(PlayerName { what: "Z".into() });
    load_game(&mut w4, p3).unwrap();
    assert_eq!(
        w4.query::<&Wield>().iter(&w4).map(|w| w.pow_increase).collect::<Vec<_>>(),
        vec![8],
    );
    assert_eq!(w4.query::<&Wear>().iter(&w4).map(|w| w.arm_increase).collect::<Vec<_>>(), vec![9]);
    assert_eq!(
        w4.query::<&Scroll>().iter(&w4).map(|s| s.effect).collect::<Vec<_>>(),
        vec![ScrollEffect::MagicMapping],
    );
    assert_eq!(
        w4.query::<&PutOn>().iter(&w4).map(|p| p.effect).collect::<Vec<_>>(),
        vec![RingEffect::Regeneration],
    );
    assert_eq!(w4.query::<&Amulet>().iter(&w4).count(), 1);
    // The curse tag rides along, so cursed gear stays cursed after a reload.
    assert_eq!(w4.query::<&Curse>().iter(&w4).count(), 1);
    // A vorpalized weapon keeps its edge — and its bane — through a reload.
    assert_eq!(
        w4.query::<&Vorpal>().iter(&w4).map(|v| v.bane.clone()).collect::<Vec<_>>(),
        vec!["dragon".to_string()],
    );

    // An ordinary save is not clear data.
    assert!(clear_data(p).unwrap().is_none());

    // Map regenerated from the seed matches the original tile-for-tile.
    assert_eq!(w.resource::<Map>().tiles, w2.resource::<Map>().tiles);
    assert!(w2
        .resource::<Map>()
        .tiles
        .iter()
        .any(|&t| t == TileType::Wall));
}

#[test]
fn a_won_run_saves_as_clear_data() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(5)));
    w.insert_resource(RngSeed(5));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName { what: "VICTOR".into() });
    initialize_world(&mut w);
    w.resource_mut::<Ending>().player_won = true;

    let path = std::env::temp_dir().join("roog_clear.sav");
    let p = path.to_str().unwrap();
    save_game(&mut w, p).unwrap();

    let clear = clear_data(p).unwrap().expect("recognised as clear data");
    assert_eq!(clear.player_name, "VICTOR");
    let _ = std::fs::remove_file(p);
}
