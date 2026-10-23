//! `keymap.txt`: Brogue-style key remapping, two characters per line.

use models::*;

#[test]
fn unlisted_keys_map_to_themselves() {
    let map = parse_keymap("hj").unwrap();
    assert_eq!(map.apply('z'), 'z');
    assert_eq!(map.apply('j'), 'j');
}

#[test]
fn a_swap_does_not_chain() {
    let map = parse_keymap("hj\njh\n").unwrap();
    assert_eq!((map.apply('h'), map.apply('j')), ('j', 'h'));
}

#[test]
fn blank_lines_and_comments_are_skipped() {
    let map = parse_keymap("# dvorak\n\n  \nhd\n").unwrap();
    assert_eq!(map.apply('h'), 'd');
}

#[test]
fn a_line_of_the_wrong_length_names_its_number() {
    let err = parse_keymap("hj\nabc\n").unwrap_err();
    assert!(err.starts_with("line 2:"), "{err}");
}

#[test]
fn a_space_cannot_be_a_key() {
    assert!(parse_keymap("h \n").is_err());
    assert!(parse_keymap(" h\n").is_err());
}

#[test]
fn a_key_listed_twice_is_refused() {
    let err = parse_keymap("hj\nhk\n").unwrap_err();
    assert!(err.starts_with("line 2:"), "{err}");
}

#[test]
fn show_gives_the_key_that_produces_a_command() {
    let map = parse_keymap("hj\njh\nkl\n").unwrap();
    assert_eq!(map.show('j'), 'h');
    assert_eq!(map.show('l'), 'k');
    assert_eq!(map.show('z'), 'z');
}

#[test]
fn a_missing_file_is_an_error_only_when_asked_for() {
    let path = std::path::Path::new("/nonexistent/keymap.txt");
    assert!(load_keymap(path).is_err());
    assert!(load_keymap_if_present(path).unwrap().is_empty());
}

#[test]
fn shift_follows_a_lowercase_mapping() {
    let map = parse_keymap("hl\n").unwrap();
    assert_eq!(map.apply('H'), 'L');
    assert_eq!(map.apply('K'), 'K');
}

#[test]
fn an_explicit_uppercase_line_wins_over_the_shift_rule() {
    let map = parse_keymap("hl\nHJ\n").unwrap();
    assert_eq!(map.apply('H'), 'J');
}

#[test]
fn show_follows_shift_too() {
    let map = parse_keymap("hl\n").unwrap();
    assert_eq!(map.show('L'), 'H');
}
