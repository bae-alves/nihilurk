//! One check on the workspace manifest itself: that a bare `cargo test` still
//! covers everything that ships.
//!
//! `default-members` is a list of what to *include*, and what it is really for
//! here is excluding one crate: `compat` is a test rig that pulls in ratatui,
//! which the shipped binary is not allowed to depend on. Written as an
//! include-list, it only stays correct if whoever adds the next game crate
//! remembers to add it in two places.
//!
//! Nothing would have complained if they didn't. A new crate missing from
//! `default-members` is not an error — it builds fine, it tests fine when you
//! name it, and it silently stops being part of `cargo test` at the workspace
//! root. Two lists that have to agree, and no third thing checking that they
//! do. This is the third thing.
//!
//! Cargo has no `default-exclude`, so the list stays as it is and the intent
//! is asserted here instead.

use std::path::{Path, PathBuf};

/// The one crate that is deliberately outside a bare `cargo test`. Reaching
/// it is always explicit: `-p nihilurk-compat` or `compat_test.sh`.
const RIGS: [&str; 1] = ["compat"];

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
fn a_bare_cargo_test_covers_every_crate_but_the_rig() {
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
        "`default-members` should exclude exactly the test rig.\n\
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

#[test]
fn the_docs_crate_map_names_every_crate() {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    let map =
        std::fs::read_to_string(root.join("docs/README.md")).expect("docs/README.md is readable");
    let (_, map) = map
        .split_once("\nThe crates\n")
        .expect("docs/README.md has a `The crates` section");
    let map = map
        .split_once("\nThe rule that keeps this true")
        .map_or(map, |s| s.0);

    let missing: Vec<String> = array(&manifest, "members")
        .into_iter()
        .filter(|name| !map.contains(&format!("  {name}/")))
        .collect();

    assert!(
        missing.is_empty(),
        "crates missing from the `The crates` map in docs/README.md: {missing:?}"
    );
}

/// Every crate that goes to crates.io: the members that are not rigs.
fn published(root: &Path) -> Vec<(String, String)> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    array(&manifest, "members")
        .into_iter()
        .filter(|dir| !RIGS.contains(&dir.as_str()))
        .map(|dir| {
            let text = std::fs::read_to_string(root.join(&dir).join("Cargo.toml"))
                .unwrap_or_else(|_| panic!("{dir}/Cargo.toml is readable"));
            (dir, text)
        })
        .collect()
}

/// crates.io hands out names first come, first served, and `engine`,
/// `models` and `strings` went to other people long ago. A workspace crate
/// under a bare name cannot be published, and `engine` cannot go out until
/// the three crates it depends on have. So every crate that ships carries the
/// game's name; the directories keep their short names.
#[test]
fn every_published_crate_is_named_for_the_game() {
    let root = workspace_root();
    let wrong: Vec<String> = published(&root)
        .into_iter()
        .filter_map(|(dir, text)| {
            let name = text
                .lines()
                .find_map(|l| l.strip_prefix("name = \""))?
                .trim_end_matches('"')
                .to_string();
            let ok = name == "nihilurk" || name.starts_with("nihilurk-");
            (!ok).then(|| format!("{dir}/ is named `{name}`"))
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "crates that crates.io would refuse or that squat a generic name: {wrong:?}\n\
         name them `nihilurk` or `nihilurk-<thing>`; see docs/how-to/publish-to-crates-io.md"
    );
}

#[test]
fn every_published_crate_has_what_crates_io_asks_for() {
    let root = workspace_root();
    let mut missing = Vec::new();
    for (dir, text) in published(&root) {
        if text.contains("publish = false") {
            missing.push(format!("{dir}/ is marked `publish = false`"));
        }
        if !text.contains("description = ") {
            missing.push(format!("{dir}/ has no `description`"));
        }
        if !text.contains("repository.workspace = true") {
            missing.push(format!("{dir}/ does not inherit `repository`"));
        }
        if !text.contains("readme.workspace = true") {
            missing.push(format!("{dir}/ does not inherit `readme`"));
        }
        if !text.contains("\nrust-version = \"") {
            missing.push(format!("{dir}/ has no `rust-version`"));
        }
        if !text.contains("categories = [") {
            missing.push(format!("{dir}/ has no `categories`"));
        }
        if text.contains("keywords = [") {
            let keywords = array(&text, "keywords");
            if keywords.is_empty() || keywords.len() > 5 {
                missing.push(format!("{dir}/ needs 1 to 5 keywords, has {keywords:?}"));
            }
            for k in keywords {
                let ok = k.len() <= 20
                    && k.starts_with(|c: char| c.is_ascii_alphanumeric())
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_-+".contains(c));
                if !ok {
                    missing.push(format!("{dir}/ has a keyword crates.io refuses: `{k}`"));
                }
            }
        } else {
            missing.push(format!("{dir}/ has no `keywords`"));
        }
        // A path dependency with no version is rejected at publish time,
        // after every crate before it in the order has already gone out.
        for line in text.lines().filter(|l| l.contains("path = \"../")) {
            if !line.contains("version = ") {
                missing.push(format!(
                    "{dir}/ has a path dependency with no version: {line}"
                ));
            }
        }
        let tests = root.join(&dir).join("tests");
        for entry in std::fs::read_dir(&tests).into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let Ok(src) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let leaves = src.contains("CARGO_MANIFEST_DIR")
                && (src.contains(".parent()") || src.contains("/../"));
            if leaves && !text.contains(&format!("\"tests/{file}\"")) {
                missing.push(format!(
                    "{dir}/tests/{file} reads outside its crate but is not in `exclude`"
                ));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "not ready for crates.io: {missing:#?}\n\
         see docs/how-to/publish-to-crates-io.md"
    );
}

/// `cargo install nihilurk` installs every `[[bin]]` that does not need a
/// feature. The dispatcher execs a `nihilurk-<lang>` sitting next to it, and
/// an install from crates.io has no such file, so installed by default it is a
/// command that only ever errors. It has to sit behind a feature that packaging
/// asks for by name.
#[test]
fn the_dispatcher_is_not_installed_by_default() {
    let manifest = std::fs::read_to_string(workspace_root().join("engine/Cargo.toml"))
        .expect("engine/Cargo.toml is readable");
    let bin = manifest
        .split("[[bin]]")
        .find(|b| b.contains("name = \"nihilurk-dispatch\""))
        .expect("engine declares the nihilurk-dispatch bin");
    let bin = bin.split("\n[").next().expect("the table has a body");

    assert!(
        bin.contains("required-features = [\"dispatch\"]"),
        "nihilurk-dispatch must carry `required-features = [\"dispatch\"]`, or \
         `cargo install nihilurk` installs a command that cannot work.\n\
         release/package.sh and aur/PKGBUILD build it with `--features dispatch`."
    );
    assert!(
        !manifest.contains("default = [\"lang-en\", \"dispatch\"]"),
        "`dispatch` must not be a default feature"
    );
}

/// The minimum Rust is written in the manifest and again in prose that tells a
/// contributor what to install. Nothing connects the two, so a bump in the
/// manifest that misses the prose sends people to the wrong compiler.
#[test]
fn the_docs_name_the_real_minimum_rust() {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("engine/Cargo.toml"))
        .expect("engine/Cargo.toml is readable");
    let floor = manifest
        .lines()
        .find_map(|l| l.strip_prefix("rust-version = \""))
        .expect("engine declares a rust-version")
        .trim_end_matches('"');

    for file in ["README.md", "CONTRIBUTING.md"] {
        let text = std::fs::read_to_string(root.join(file)).expect("the file is readable");
        assert!(
            text.contains(&format!("Rust {floor}")),
            "{file} should say `Rust {floor}`, the `rust-version` in engine/Cargo.toml"
        );
    }
}

/// `release/bump.lua` moves all four published crates to one number, and every
/// `path` pin with them. Nothing else would notice if one crate or one pin were
/// edited by hand and fell behind, and the next release would publish a crate
/// that asks for a version of its neighbour that no longer matches.
#[test]
fn all_published_crates_share_one_version() {
    let root = workspace_root();
    let crates = published(&root);
    let version_of = |text: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix("version = \""))
            .map(|v| v.trim_end_matches('"').to_string())
            .expect("a [package] version")
    };
    let first = version_of(&crates[0].1);

    let mut wrong = Vec::new();
    for (dir, text) in &crates {
        let version = version_of(text);
        if version != first {
            wrong.push(format!("{dir}/ is {version}, the others are {first}"));
        }
        for line in text.lines().filter(|l| l.contains("path = \"../")) {
            let pin = line
                .split("version = \"")
                .nth(1)
                .and_then(|rest| rest.split('"').next());
            if pin != Some(first.as_str()) {
                wrong.push(format!(
                    "{dir}/ pins a neighbour at the wrong version: {line}"
                ));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "the published crates are not in lockstep: {wrong:#?}\n\
         `lua release/bump.lua` moves them together; see docs/how-to/cut-a-release.md"
    );
}
