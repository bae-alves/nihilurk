//! What each pack menu shows.
//!
//! Two layers. The first tests give a real player a real pack, so the filters
//! are answered by items the catalog built rather than by a bare entity, and
//! the rows come back as the *backpack indices* the renderer and the cursor
//! both key off. The tests after `ALL` cover the table itself, one mode at a
//! time: titles, verbs, and what a mode admits from an entity with no marks
//! at all.

use crate::common::fixtures::{spawn_armor, spawn_named, spawn_ring, spawn_weapon};
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
        spawn_named(&mut w, "potion of healing", nowhere).unwrap(),
        spawn_named(&mut w, "scroll of magic mapping", nowhere).unwrap(),
        spawn_named(&mut w, "wand of fire", nowhere).unwrap(),
        spawn_weapon(&mut w, "dagger", nowhere),
        spawn_armor(&mut w, "ring mail", nowhere),
        spawn_ring(&mut w, "ring of protection", nowhere),
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
    let all = vec![0, 1, 2, 3, 4, 5];
    assert_eq!(pack_rows(&mut w, PackMode::Browse), all);
    assert_eq!(pack_rows(&mut w, PackMode::Use), all);
    assert_eq!(pack_rows(&mut w, PackMode::Drop), all);
    assert_eq!(pack_rows(&mut w, PackMode::Quaff), vec![0]);
    assert_eq!(pack_rows(&mut w, PackMode::Read), vec![1]);
    assert_eq!(pack_rows(&mut w, PackMode::Zap), vec![2]);
    assert_eq!(pack_rows(&mut w, PackMode::Equip), vec![3, 4, 5]);
    assert_eq!(pack_rows(&mut w, PackMode::Wield), vec![3]);
    assert_eq!(pack_rows(&mut w, PackMode::Wear), vec![4]);
    assert_eq!(pack_rows(&mut w, PackMode::PutOn), vec![5]);
}

/// Every mode, for the table tests below to walk. An array's length cannot
/// catch a new mode, so `all_lists_every_mode` does.
const ALL: [PackMode; 11] = [
    PackMode::Browse,
    PackMode::Use,
    PackMode::Throw,
    PackMode::Drop,
    PackMode::Equip,
    PackMode::Quaff,
    PackMode::Read,
    PackMode::Zap,
    PackMode::Wield,
    PackMode::Wear,
    PackMode::PutOn,
];

#[test]
fn browsing_asks_which_verb_and_every_other_mode_already_knows() {
    assert_eq!(PackMode::Browse.action(), None);
    for mode in ALL.into_iter().filter(|&m| m != PackMode::Browse) {
        assert!(mode.action().is_some(), "{mode:?} has no verb of its own");
    }
    assert_eq!(PackMode::Drop.action(), Some(ItemAction::Drop));
    assert_eq!(PackMode::Throw.action(), Some(ItemAction::Throw));
}

#[test]
fn the_browse_modal_offers_each_verb_exactly_once() {
    for action in [
        ItemAction::Use,
        ItemAction::Throw,
        ItemAction::Drop,
        ItemAction::Info,
    ] {
        assert_eq!(
            ItemAction::MENU.iter().filter(|&&a| a == action).count(),
            1,
            "{action:?}"
        );
    }
    assert_eq!(ItemAction::at(0), ItemAction::Use);
    assert_eq!(
        ItemAction::at(99),
        ItemAction::Info,
        "clamped, not panicking"
    );
}
