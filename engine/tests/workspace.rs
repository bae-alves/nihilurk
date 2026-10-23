//! One check on the workspace manifest itself: that every crate in the tree is a
//! member.

use std::path::{Path, PathBuf};

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
         add them to `members`"
    );
}

/// Every crate that goes to crates.io: the members.
fn published(root: &Path) -> Vec<(String, String)> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("the workspace manifest is readable");
    array(&manifest, "members")
        .into_iter()
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
         name them `nihilurk` or `nihilurk-<thing>`; see docs/explanation/adr-0002-two-names-per-crate.md"
    );
}

/// A path dependency with no version is rejected at publish time,
/// after every crate before it in the order has already gone out.
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
        for line in text.lines().filter(|l| l.contains("path = \"../")) {
            if !line.contains("version = ") {
                missing.push(format!(
                    "{dir}/ has a path dependency with no version: {line}"
                ));
            }
        }
        let tests = root.join(&dir).join("tests");
        let mut dirs = vec![tests.clone()];
        while let Some(at) = dirs.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                let Ok(src) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let leaves = src.contains("CARGO_MANIFEST_DIR")
                    && (src.contains(".parent()")
                        || src.contains("/../")
                        || src.contains(".join(\"..\")"));
                if !leaves {
                    continue;
                }
                let file = path
                    .strip_prefix(&tests)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if at != tests {
                    missing.push(format!(
                        "{dir}/tests/{file} reads outside its crate inside a shared test \
                         binary; make it a file of its own under `tests/` and `exclude` it"
                    ));
                } else if !text.contains(&format!("\"tests/{file}\"")) {
                    missing.push(format!(
                        "{dir}/tests/{file} reads outside its crate but is not in `exclude`"
                    ));
                }
            }
        }
    }

    assert!(missing.is_empty(), "not ready for crates.io: {missing:#?}");
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
