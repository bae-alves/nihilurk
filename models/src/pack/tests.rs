use super::*;

/// Every mode, so a mode added without a row here fails to compile.
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

#[test]
fn every_mode_names_itself_and_says_why_it_is_empty() {
    for mode in ALL {
        assert!(mode.title().starts_with(' '), "{mode:?}");
        assert!(mode.nothing_line().ends_with('.'), "{mode:?}");
    }
}

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
