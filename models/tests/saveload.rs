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
    // Gear a monster spawned wearing has no `Position` of its own, so it needs
    // naming here as well or it outlives the monster and lands in the save.
    let strays: Vec<Entity> = w3
        .iter_entities()
        .filter(|e| {
            !e.contains::<Player>() && (e.contains::<Position>() || e.contains::<Equipped>())
        })
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

/// Gear stays on across a save. What is wielded and worn when the file is
/// written is wielded and worn when it is read back, by the same hands — and
/// what the gear lends its wearer (a ring's granted effect, an armour's die)
/// comes back with it, because the save leaves loaned effects out of every
/// entity's set and load re-lends them.
#[test]
fn equipped_gear_stays_on_across_a_save() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(11)));
    w.insert_resource(RngSeed(11));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "WEARER".into(),
    });
    initialize_world(&mut w);
    // Strip the floor's spawned loot and monsters so this round trip sees only
    // the gear the test itself places. The player's starting kit stays; it is
    // equipped (ring mail, mace) and part of the claim under test.
    let hero = w.query_filtered::<Entity, With<Player>>().single(&w);
    let strays: Vec<Entity> = w
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && e.contains::<Position>())
        .map(|e| e.id())
        .collect();
    for e in strays {
        w.despawn(e);
    }
    let here = *w.get::<Position>(hero).unwrap();
    // A ring of perception, put on the proper way: its SeesInvisible is on
    // loan from gear, which is exactly the effect a reload must re-lend.
    let ring = spawn_ring(&mut w, RingEffect::Perception, here);
    assert!(toggle_equipped(&mut w, hero, ring));
    assert!(w.get::<SeesInvisible>(hero).is_some());
    // A monster holds gear of its own — the save must not strip it from the
    // orc's hand any more than from the player's.
    let dagger = spawn_weapon(&mut w, "dagger", here);
    let orc = spawn_monster(&mut w, MonsterDef::named("orc"), here);
    assert!(equip_silently(&mut w, orc, dagger));

    let before = loadout(&w, hero);
    let save = common::SaveFile::new("worn");
    let p = save.path();
    save_game(&mut w, p).unwrap();
    // Saving is read-only: it did not undress anyone to write the file.
    assert_eq!(w.get::<Equipped>(ring).map(|e| e.by), Some(Some(hero)));
    assert_eq!(w.get::<Equipped>(dagger).map(|e| e.by), Some(Some(orc)));

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(12)));
    w2.insert_resource(RngSeed(12));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, p).unwrap();

    let hero2 = w2.query_filtered::<Entity, With<Player>>().single(&w2);
    let orc2 = w2.query_filtered::<Entity, With<Mob>>().single(&w2);
    // The player's hand, body and finger all come back occupied.
    assert!(equipped_in(&w2, hero2, Slot::Hand).is_some());
    assert!(equipped_in(&w2, hero2, Slot::Body).is_some());
    let worn = equipped_in(&w2, hero2, Slot::Finger).expect("the ring stayed on");
    assert_eq!(
        w2.get::<Ring>(worn).map(|r| r.effect),
        Some(RingEffect::Perception)
    );
    // What the ring lent is re-lent: the wearer sees invisible again without
    // touching the slot.
    assert!(
        w2.get::<SeesInvisible>(hero2).is_some(),
        "the ring's loaned effect did not come back with the ring"
    );
    // The numbers the HUD reads are the numbers they were.
    assert_eq!(loadout(&w2, hero2), before);
    // And the orc still holds what it caught.
    let held = equipped_in(&w2, orc2, Slot::Hand).expect("the orc kept its dagger");
    assert_eq!(
        w2.get::<Name>(held).map(|n| n.what.as_str()),
        Some("dagger")
    );
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

/// Two buffs nothing lends: the violet charge a scroll of monster confusion
/// leaves on the reader's hands, and Bide's coiled blow. Neither comes off a
/// ring or a floor, so neither has an owner to re-lend it on load — the ledger
/// is the only thing that remembers them, and the save writes the ledger.
/// Attach either with a bare `insert` and it goes dark the next time the run
/// is opened.
#[test]
fn a_charged_touch_and_a_coiled_bide_survive_a_save() {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(7)));
    w.insert_resource(RngSeed(7));
    w.init_resource::<GameLog>();
    w.init_resource::<UseQueue>();
    w.init_resource::<AttackQueue>();
    w.init_resource::<SpellQueue>();
    w.init_resource::<Ending>();
    w.insert_resource(PlayerName {
        what: "CHARGED".into(),
    });
    initialize_world(&mut w);
    let hero = w.query_filtered::<Entity, With<Player>>().single(&w);
    let here = *w.get::<Position>(hero).unwrap();

    // Read the scroll the way the pack screen reads one.
    let scroll = spawn_scroll(&mut w, ScrollEffect::MonsterConfusion, here);
    w.entity_mut(scroll).remove::<Position>();
    w.resource_mut::<UseQueue>().uses.push(WantsToUse {
        user: hero,
        item: scroll,
        target: None,
        slot_idx: None,
    });
    item_system(&mut w);
    // And cast Bide the way the spells menu casts one.
    w.resource_mut::<SpellQueue>().spells.push(WantsToCast {
        user: hero,
        effect: SpellEffect::Bide,
        target: here,
    });
    spell_system(&mut w);
    assert!(
        w.get::<ConfusingTouch>(hero).is_some() && w.get::<Bided>(hero).is_some(),
        "the fixture did not actually charge the player, so this test proves nothing"
    );

    let save = common::SaveFile::new("buffs");
    let p = save.path();
    save_game(&mut w, p).unwrap();

    let mut w2 = World::new();
    w2.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(8)));
    w2.insert_resource(RngSeed(8));
    w2.init_resource::<GameLog>();
    w2.insert_resource(PlayerName { what: "X".into() });
    load_game(&mut w2, p).unwrap();

    let hero2 = w2.query_filtered::<Entity, With<Player>>().single(&w2);
    assert!(
        w2.get::<ConfusingTouch>(hero2).is_some(),
        "the charge on the reader's hands did not survive the save"
    );
    assert!(
        w2.get::<Bided>(hero2).is_some(),
        "Bide's coiled blow did not survive the save"
    );
}
