//! The save file: what survives a round trip, and what deliberately does not.

mod common;

use bevy_ecs::prelude::*;
use models::constants::player::START_MAGIC;
use models::*;

#[test]
fn round_trip() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w.insert_resource(RngSeed(1));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);
    // Pretend we walked down to floor 4. A floor's layout is a pure function of
    // (seed, depth), so moving the depth marker means rebuilding the map to
    // match — otherwise this world is floor 1 wearing a floor-4 label, and the
    // tile comparison at the end of the test is meaningless.
    w.resource_mut::<Depth>().what = 4;
    regenerate_map(&mut w, 1, 4);
    w.resource_mut::<Identified>()
        .potions
        .insert(PotionEffect::Healing);
    let n0 = w.iter_entities().count();
    let save = common::SaveFile::new("roundtrip");
    let p = save.path();
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
    assert!(
        w2.resource::<Identified>()
            .potions
            .contains(&PotionEffect::Healing)
    );
    assert_eq!(
        w.resource::<ItemAppearances>().potions,
        w2.resource::<ItemAppearances>().potions,
    );
    // The starting kit — ring mail, mace, bow, arrows, healing potion — round-trips.
    let packed: Vec<Entity> = {
        let mut q = w2.query_filtered::<&Backpack, With<Player>>();
        q.single(&w2).items.clone()
    };
    assert_eq!(packed.len(), 5);
    assert!(packed.iter().any(|&it| w2.get::<ArmorDie>(it).is_some()));

    // The player's magic pool survives the round trip.
    let magic = w2.query_filtered::<&Magic, With<Player>>().single(&w2);
    assert_eq!((magic.points, magic.max_points), (START_MAGIC, START_MAGIC));

    // Equipment / scroll / ring components survive the round trip.
    let mut w3 = World::new();
    w3.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(3)));
    w3.insert_resource(RngSeed(3));
    w3.init_resource::<GameLog>();
    w3.insert_resource(PlayerName { what: "Y".into() });
    initialize_world(&mut w3);
    // Strip the floor's own spawned loot/monsters and the player's starting kit,
    // so this round trip only sees the gear the test itself places below.
    {
        let hero = w3.query_filtered::<Entity, With<Player>>().single(&w3);
        let kit = std::mem::take(&mut w3.get_mut::<Backpack>(hero).unwrap().items);
        for item in kit {
            w3.despawn(item);
        }
    }
    let strays: Vec<Entity> = w3
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && e.contains::<Position>())
        .map(|e| e.id())
        .collect();
    for e in strays {
        w3.despawn(e);
    }
    let pos = Position { x: 5, y: 5 };
    let vorpal_sword = spawn_weapon(&mut w3, "long sword", pos);
    w3.entity_mut(vorpal_sword).insert(Vorpal {
        bane: "dragon".into(),
    });
    let cursed_armor = spawn_armor(&mut w3, "plate mail", pos);
    w3.entity_mut(cursed_armor).insert(Curse);
    spawn_scroll(&mut w3, ScrollEffect::MagicMapping, pos);
    spawn_ring(&mut w3, RingEffect::Regeneration, pos);
    spawn_element_of_yoord(&mut w3, pos);
    let save3 = common::SaveFile::new("gear");
    let p3 = save3.path();
    save_game(&mut w3, p3).unwrap();

    let mut w4 = World::new();
    w4.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(9)));
    w4.insert_resource(RngSeed(9));
    w4.init_resource::<GameLog>();
    w4.insert_resource(PlayerName { what: "Z".into() });
    load_game(&mut w4, p3).unwrap();
    // The dice come off the catalog rows, not off a number copied into this
    // test: what is being checked is that the save carried them, not what the
    // balance happens to be this week.
    let sword_die = WEAPONS
        .iter()
        .find(|d| d.name == "long sword")
        .unwrap()
        .power_die;
    let plate_die = ARMORS
        .iter()
        .find(|d| d.name == "plate mail")
        .unwrap()
        .armor_die;
    assert_eq!(
        w4.query::<&PowerDie>()
            .iter(&w4)
            .map(|m| m.0)
            .collect::<Vec<_>>(),
        vec![sword_die],
    );
    assert_eq!(
        w4.query::<&ArmorDie>()
            .iter(&w4)
            .map(|m| m.0)
            .collect::<Vec<_>>(),
        vec![plate_die]
    );
    assert_eq!(
        w4.query::<&Scroll>()
            .iter(&w4)
            .map(|s| s.effect)
            .collect::<Vec<_>>(),
        vec![ScrollEffect::MagicMapping],
    );
    assert_eq!(
        w4.query::<&Ring>()
            .iter(&w4)
            .map(|r| r.effect)
            .collect::<Vec<_>>(),
        vec![RingEffect::Regeneration],
    );
    assert_eq!(w4.query::<&Amulet>().iter(&w4).count(), 1);
    // The curse tag rides along, so cursed gear stays cursed after a reload.
    assert_eq!(w4.query::<&Curse>().iter(&w4).count(), 1);
    // A vorpalized weapon keeps its edge — and its bane — through a reload.
    assert_eq!(
        w4.query::<&Vorpal>()
            .iter(&w4)
            .map(|v| v.bane.clone())
            .collect::<Vec<_>>(),
        vec!["dragon".to_string()],
    );

    // An ordinary save is not clear data.
    assert!(clear_data(p).unwrap().is_none());

    // Map regenerated from (seed, depth) matches the original tile-for-tile.
    assert_eq!(w.resource::<Map>().tiles, w2.resource::<Map>().tiles);
    assert!(w2.resource::<Map>().tiles.contains(&TileType::Wall));
}

#[test]
fn a_won_run_saves_as_clear_data() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(5)));
    w.insert_resource(RngSeed(5));
    w.init_resource::<GameLog>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "VICTOR".into(),
    });
    initialize_world(&mut w);
    w.resource_mut::<Ending>().player_won = true;

    let save = common::SaveFile::new("clear");
    let p = save.path();
    save_game(&mut w, p).unwrap();

    let clear = clear_data(p).unwrap().expect("recognised as clear data");
    assert_eq!(clear.player_name, "VICTOR");
}

/// The save carries no cosmetic state at all, and that is a decision rather
/// than an oversight: none of it is gameplay, none of it is replayed, and the
/// three map-sized overlays alone would outweigh the rest of the file.
///
/// The invariant a change could quietly break is "a reloaded floor is the
/// floor you left, scrubbed of the mess you made on it" — so this stains,
/// marks and smokes a tile, round-trips, and asks for all three back empty.
#[test]
fn the_mess_a_fight_leaves_behind_is_not_in_the_save() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(77)));
    w.insert_resource(RngSeed(77));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "MESSY".into(),
    });
    initialize_world(&mut w);

    let here = {
        let p = w.query_filtered::<Entity, With<Player>>().single(&w);
        *w.get::<Position>(p).unwrap()
    };
    w.resource_mut::<BloodStains>().stain(here.x, here.y);
    w.resource_mut::<Corpses>().mark(here.x, here.y);
    w.resource_mut::<Smoke>().puff(here.x, here.y, 4);
    assert!(
        w.resource::<BloodStains>().is_bloody(here.x, here.y)
            && w.resource::<Corpses>().has(here.x, here.y)
            && w.resource::<Smoke>().is_smoky(here.x, here.y),
        "the fixture did not actually dirty the tile, so this test proves nothing"
    );

    let save = common::SaveFile::new("juice");
    let p = save.path();
    save_game(&mut w, p).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(1)));
    w2.insert_resource(RngSeed(1));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, p).unwrap();

    assert!(
        !w2.resource::<BloodStains>().is_bloody(here.x, here.y),
        "blood was carried across a reload"
    );
    assert!(
        !w2.resource::<Corpses>().has(here.x, here.y),
        "a corpse mark was carried across a reload"
    );
    assert!(
        !w2.resource::<Smoke>().is_smoky(here.x, here.y),
        "smoke was carried across a reload"
    );
}
