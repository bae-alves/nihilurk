//! What each pack menu shows.
//!
//! Two layers. The first tests give a real player a real pack, so the filters
//! are answered by items the catalog built rather than by a bare entity, and
//! the rows come back as the *backpack indices* the renderer and the cursor
//! both key off. The tests after `ALL` cover the table itself, one mode at a
//! time: titles, verbs, and what a mode admits from an entity with no marks
//! at all.

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

#[test]
fn a_row_keeps_its_pack_letter_in_every_menu() {
    let mut w = carrying_one_of_everything();
    let ring_row = pack_rows(&mut w, PackMode::Browse)[5];
    assert_eq!(pack_rows(&mut w, PackMode::PutOn), vec![ring_row]);
}

#[test]
fn a_menu_with_nothing_in_it_comes_back_empty_rather_than_showing_the_wrong_thing() {
    let mut w = carrying_one_of_everything();
    let player = w.query_filtered::<Entity, With<Player>>().single(&w);
    let potion = w.get::<Backpack>(player).unwrap().items[0];
    w.get_mut::<Backpack>(player).unwrap().items = vec![potion];

    assert_eq!(pack_rows(&mut w, PackMode::Quaff), vec![0]);
    for empty in [
        PackMode::Read,
        PackMode::Zap,
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

/// `ALL` is the chain of modes below, in order and whole. Each arm names the
/// mode after it, and the match has no catch-all, so a new mode fails to
/// compile until it has an arm, and an arm nothing links to leaves `ALL` out
/// of step with the chain.
#[test]
fn all_lists_every_mode() {
    fn after(mode: PackMode) -> Option<PackMode> {
        match mode {
            PackMode::Browse => Some(PackMode::Use),
            PackMode::Use => Some(PackMode::Throw),
            PackMode::Throw => Some(PackMode::Drop),
            PackMode::Drop => Some(PackMode::Equip),
            PackMode::Equip => Some(PackMode::Quaff),
            PackMode::Quaff => Some(PackMode::Read),
            PackMode::Read => Some(PackMode::Zap),
            PackMode::Zap => Some(PackMode::Wield),
            PackMode::Wield => Some(PackMode::Wear),
            PackMode::Wear => Some(PackMode::PutOn),
            PackMode::PutOn => None,
        }
    }
    let mut chain = vec![ALL[0]];
    while let Some(next) = after(*chain.last().unwrap()) {
        chain.push(next);
    }
    assert_eq!(chain, ALL);
}

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

#[test]
fn every_mode_names_itself_and_says_why_it_is_empty() {
    for mode in ALL {
        assert!(mode.title().starts_with(' '), "{mode:?}");
        assert!(mode.nothing_line().ends_with('.'), "{mode:?}");
    }
}

/// A filtered mode admits an item by a component it carries: a potion to
/// quaff, a scroll to read, a weapon to wield. A bare entity carries none, so
/// only the modes that take anything let it in.
#[test]
fn an_item_with_none_of_the_marks_is_admitted_only_by_the_unfiltered_modes() {
    let world = World::new();
    let nothing_in_particular = Entity::from_raw(0);
    let unfiltered = [
        PackMode::Browse,
        PackMode::Use,
        PackMode::Throw,
        PackMode::Drop,
    ];
    for mode in ALL {
        assert_eq!(
            mode.admits(&world, nothing_in_particular),
            unfiltered.contains(&mode),
            "{mode:?}"
        );
    }
}
