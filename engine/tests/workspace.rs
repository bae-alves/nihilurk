//! One check on the workspace manifest itself: that a bare `cargo test` still
//! covers everything that ships.
//!
//! `default-members` is a list of what to *include*, and what it is really for
//! here is excluding two crates: `perf` and `compat` are test rigs that pull in
//! ratatui, sysinfo and criterion, none of which the shipped binary is allowed
//! to depend on. Written as an include-list, it says four names to mean "not
//! those two", and it only stays correct if whoever adds the fifth game crate
//! remembers to add it in two places.
//!
//! Nothing would have complained if they didn't. A new crate missing from
//! `default-members` is not an error — it builds fine, it tests fine when you
//! name it, and it silently stops being part of `cargo test` at the workspace
//! root. That is the same shape of bug as the renderer the perf rig used to
//! keep its own copy of: two lists that have to agree, and no third thing
//! checking that they do. This is the third thing.
//!
//! Cargo has no `default-exclude`, so the list stays as it is and the intent
//! is asserted here instead.

use std::path::{Path, PathBuf};

/// The two crates that are deliberately outside a bare `cargo test`. Reaching
/// one is always explicit: `-p nihilurk-perf`, `-p nihilurk-compat`, `perf_test.sh` or
/// `compat_test.sh`.
const RIGS: [&str; 2] = ["compat", "perf"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine/ has a parent")
        .to_path_buf()
}

/// The string entries of the first `key = [...]` array in the `[workspace]`
/// table. A hand-rolled read rather than a toml crate: this runs in the game's
/// own crate, and nihilurk does not take a dependency to read six lines of its own
/// manifest.
fn array(manifest: &str, key: &str) -> Vec<String> {
    let after = manifest
        .split_once(&format!("{key} = ["))
        .unwrap_or_else(|| panic!("no `{key} = [` in the workspace manifest"))
        .1;
    let body = after.split_once(']').expect("the array is closed").0;
    body.split(',')
        .map(|entry| entry.trim().trim_matches('"').to_string())
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[test]
fn a_bare_cargo_test_covers_every_crate_but_the_two_rigs() {
    let manifest = std::fs::read_to_string(workspace_root().join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    let members = array(&manifest, "members");
    let default = array(&manifest, "default-members");

    let mut excluded: Vec<&String> = members.iter().filter(|m| !default.contains(m)).collect();
    excluded.sort();
    let expected: Vec<&str> = RIGS.to_vec();
    let excluded: Vec<&str> = excluded.iter().map(|m| m.as_str()).collect();

    assert_eq!(
        excluded, expected,
        "`default-members` should exclude exactly the two test rigs.\n\
         members:         {members:?}\n\
         default-members: {default:?}\n\
         If you added a crate, add it to `default-members` too, or a bare \
         `cargo test` will quietly stop covering it. If you added a rig, add \
         its name to RIGS in this test."
    );
}

#[test]
fn every_crate_in_the_tree_is_a_workspace_member() {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    let members = array(&manifest, "members");

    let mut unlisted: Vec<String> = std::fs::read_dir(&root)
        .expect("the workspace root is readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        // `target/` holds build output, and a dot-directory is not a crate.
        .filter(|name| name != "target" && !name.starts_with('.'))
        .filter(|name| root.join(name).join("Cargo.toml").is_file())
        .filter(|name| !members.contains(name))
        .collect();
    unlisted.sort();

    assert!(
        unlisted.is_empty(),
        "crate directories that are not workspace members: {unlisted:?}\n\
         add them to `members` (and to `default-members` unless they are rigs)"
    );
}
