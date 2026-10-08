//! The BALANCE thermometer: one letter lit for the player's alignment, and
//! BROKEN in the colour that went too far once the spirits turn.

use crossterm::style::Color;
use models::balance_gauge;

fn word(g: &[(char, Color)]) -> String {
    g.iter().map(|&(c, _)| c).collect()
}

fn lit(g: &[(char, Color)]) -> Vec<(usize, Color)> {
    g.iter()
        .enumerate()
        .filter(|(_, (_, c))| *c != Color::White)
        .map(|(i, &(_, c))| (i, c))
        .collect()
}

#[test]
fn centred_and_at_peace_shows_nothing() {
    assert!(balance_gauge(0, false).is_none());
}

#[test]
fn one_letter_is_lit_for_the_level_and_the_rest_stay_white() {
    for (alignment, index, color) in [
        (-3, 0, Color::Red),
        (-2, 1, Color::Red),
        (-1, 2, Color::Red),
        (1, 4, Color::Cyan),
        (2, 5, Color::Cyan),
        (3, 6, Color::Cyan),
    ] {
        let g = balance_gauge(alignment, false).unwrap();
        assert_eq!(word(&g), "BALANCE");
        assert_eq!(lit(&g), vec![(index, color)], "alignment {alignment}");
    }
}

#[test]
fn a_broken_balance_is_the_word_broken_in_the_side_that_went_too_far() {
    for (alignment, color) in [(-3, Color::Red), (-1, Color::Red), (3, Color::Cyan)] {
        let g = balance_gauge(alignment, true).unwrap();
        assert_eq!(word(&g), "BROKEN");
        assert!(g.iter().all(|&(_, c)| c == color), "alignment {alignment}");
    }
}

#[test]
fn a_balance_broken_by_a_wound_with_no_side_is_white() {
    let g = balance_gauge(0, true).unwrap();
    assert_eq!(word(&g), "BROKEN");
    assert!(g.iter().all(|&(_, c)| c == Color::White));
}
