//! What each pack menu shows.
//!
//! `models/src/pack.rs`'s own unit tests cover the table — titles, verbs, and
//! which component each mode asks for. This file covers the part they can't: a
//! real player carrying a real pack, so the filters are answered by items the
//! catalog built rather than by a bare entity, and the rows come back as the
//! *backpack indices* the renderer and the cursor both key off.

use bevy_ecs::prelude::*;
use models::*;

/// A player with a known pack: a potion, a scroll, a wand, a dagger, some
/// ring mail and a ring — one of everything the menus sort by, in that order,
/// so the expected rows can be written as literal letters.
fn carrying_one_of_everything() -> World {
    let mut w = World::new();
    w.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(4)));
    w.insert_resource(RngSeed(4));
    w.init_resource::<GameLog>();
    w.insert_resource(PlayerName {
        what: "TESTER".into(),
    });
    initialize_world(&mut w);

    let nowhere = Position { x: 0, y: 0 };
    let pack = vec![
        spawn_potion(&mut w, PotionEffect::Healing, nowhere),
        spawn_scroll(&mut w, ScrollEffect::MagicMapping, nowhere),
        spawn_wand(&mut w, WandEffect::Fire, nowhere),
        spawn_weapon(&mut w, "dagger", nowhere),
        spawn_armor(&mut w, "ring mail", nowhere),
        spawn_ring(&mut w, RingEffect::Protection, nowhere),
    ];
    for &item in &pack {
        w.entity_mut(item).remove::<Position>();
    }

    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    w.get_mut::<Backpack>(player).unwrap().items = pack;
    w
}

#[test]
fn each_menu_shows_exactly_what_its_verb_can_act_on() {
    let mut w = carrying_one_of_everything();
    // Rows are pack positions: 0 potion, 1 scroll, 2 wand, 3 dagger,
    // 4 ring mail, 5 ring.
    let all = vec![0, 1, 2, 3, 4, 5];
    assert_eq!(pack_rows(&mut w, PackMode::Browse), all);
    assert_eq!(pack_rows(&mut w, PackMode::Use), all);
    assert_eq!(pack_rows(&mut w, PackMode::Drop), all);
    assert_eq!(pack_rows(&mut w, PackMode::Quaff), vec![0]);
    assert_eq!(pack_rows(&mut w, PackMode::Read), vec![1]);
    assert_eq!(pack_rows(&mut w, PackMode::Equip), vec![3, 4, 5]);
    assert_eq!(pack_rows(&mut w, PackMode::Wield), vec![3]);
    assert_eq!(pack_rows(&mut w, PackMode::Wear), vec![4]);
    assert_eq!(pack_rows(&mut w, PackMode::PutOn), vec![5]);
}

#[test]
fn a_row_keeps_its_pack_letter_in_every_menu() {
    // The property the whole "rows are backpack indices" design exists for: the
    // ring is `f` in the pack, so it is `f` in the put-on menu too, even though
    // it is the only row there. (`f` is index 5 — a) through f).)
    let mut w = carrying_one_of_everything();
    let ring_row = pack_rows(&mut w, PackMode::Browse)[5];
    assert_eq!(pack_rows(&mut w, PackMode::PutOn), vec![ring_row]);
}

#[test]
fn a_menu_with_nothing_in_it_comes_back_empty_rather_than_showing_the_wrong_thing() {
    // What the engine turns into "You have nothing to read." instead of opening
    // a box the player has to close again.
    let mut w = carrying_one_of_everything();
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let potion = w.get::<Backpack>(player).unwrap().items[0];
    w.get_mut::<Backpack>(player).unwrap().items = vec![potion];

    assert_eq!(pack_rows(&mut w, PackMode::Quaff), vec![0]);
    for empty in [
        PackMode::Read,
        PackMode::Equip,
        PackMode::Wield,
        PackMode::Wear,
        PackMode::PutOn,
    ] {
        assert!(
            pack_rows(&mut w, empty).is_empty(),
            "{empty:?} found something to show in a pack holding one potion"
        );
    }
}

#[test]
fn worn_gear_still_shows_up_because_that_is_how_it_comes_off() {
    let mut w = carrying_one_of_everything();
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let mail = w.get::<Backpack>(player).unwrap().items[4];

    equip_silently(&mut w, player, mail);

    assert_eq!(
        pack_rows(&mut w, PackMode::Wear),
        vec![4],
        "a suit already on your back must stay on the wear menu, or you can never take it off"
    );
}
