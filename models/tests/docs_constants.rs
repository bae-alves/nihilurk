//! Keeps `docs/reference/constants.md` in step with `constants.rs`.
//!
//! The page is a map with one table row per `pub mod`. The pre-commit hook asks
//! for an edit when a constants module changes; this fails the suite when a
//! module has no row, or a row names a module that is gone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn modules(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|l| l.strip_prefix("pub mod "))
        .filter_map(|l| l.strip_suffix(" {"))
        .map(str::to_string)
        .collect()
}

fn rows(page: &str) -> BTreeSet<String> {
    page.lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split_once('`'))
        .map(|(name, _)| name.to_string())
        .filter(|name| !name.contains("::"))
        .collect()
}

fn drift(source: &str, page: &str) -> Vec<String> {
    let (have, said) = (modules(source), rows(page));
    let missing = have.difference(&said).map(|m| format!("no row for `{m}`"));
    let stale = said
        .difference(&have)
        .map(|m| format!("row for `{m}`, which is not a module"));
    missing.chain(stale).collect()
}

#[test]
fn every_constants_module_has_a_row_and_every_row_a_module() {
    let read = |p: &str| std::fs::read_to_string(root().join(p)).expect("file is readable");
    let found = drift(
        &read("models/src/constants.rs"),
        &read("docs/reference/constants.md"),
    );
    assert!(
        found.is_empty(),
        "constants.md has drifted:\n{}",
        found.join("\n")
    );
}

#[test]
fn a_missing_and_a_stale_row_are_both_caught() {
    let source = "pub mod combat {\n    pub mod inner {\n}\npub mod shake {\n}\n";
    let page = "| `constants::` | Holds |\n|---|---|\n| `combat` | x |\n| `gone` | y |\n";
    assert_eq!(
        drift(source, page),
        [
            "no row for `shake`",
            "row for `gone`, which is not a module"
        ]
    );
}
