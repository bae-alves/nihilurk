//! `keymap.txt`: Brogue-style key remapping, two characters per line.

use models::*;

#[test]
fn blank_lines_and_comments_are_skipped() {
    let map = parse_keymap("# dvorak\n\n  \nhd\n").unwrap();
    assert_eq!(map.apply('h'), 'd');
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
