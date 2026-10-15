//! A creature's name in a message comes from `helpers::item_label`, never
//! from reading [`Name`] directly.
//!
//! A twice-polymorphed creature is called a chimera, a typhon or an echidna
//! while its `Name` still says what it was (see `effects::chimeric_form`), so
//! a message that reads `Name` raw names the wrong thing. This test does not
//! judge a read; it pins how many there are in each file, so a new one cannot
//! arrive without somebody looking at it. If it fails, ask whether the read
//! ends up in a message. If so, use `item_label` (or `FormMarks` in a
//! system's own query). If not, update the count and say why below.

use std::path::{Path, PathBuf};

/// `(file under models/src, raw Name reads there, what they are for)`.
const RAW_READS: &[(&str, usize, &str)] = &[
    (
        "visibility.rs",
        5,
        "the sighting line (reads the form first) and three gear names",
    ),
    (
        "monsters.rs",
        4,
        "two renames at spawn, the slime split and `species_of` looking up the true species",
    ),
    (
        "helpers.rs",
        4,
        "`item_label` itself, and three `is_some` guards in front of it",
    ),
    (
        "items/wands.rs",
        3,
        "renaming a spoiled scroll, rune and potion: items, not creatures",
    ),
    (
        "items/throwing.rs",
        4,
        "the name of a thing being thrown: an item; `put_back` matching a thawed arrow to its quiver",
    ),
    ("identify.rs", 2, "an item's display name"),
    (
        "combat.rs",
        2,
        "`entity_name` (reads the form first), and the vorpal bane match, which is about species",
    ),
    ("traps.rs", 1, "a guard in front of `item_label`"),
    ("saveload.rs", 1, "writing the name to disk"),
    ("catalog.rs", 1, "an item's name"),
    ("bones.rs", 1, "an item's name"),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("src/ is readable").flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn raw_reads(src: &str) -> usize {
    ["get::<Name>", "get_mut::<Name>", "&Name", "&mut Name"]
        .iter()
        .map(|needle| src.matches(needle).count())
        .sum()
}

#[test]
fn no_raw_name_read_arrives_unreviewed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);

    for file in files {
        let rel = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let found = raw_reads(&std::fs::read_to_string(&file).unwrap());
        let allowed = RAW_READS
            .iter()
            .find(|(f, _, _)| *f == rel)
            .map_or(0, |(_, n, _)| *n);
        assert_eq!(
            found, allowed,
            "{rel} reads `Name` raw {found} time(s), reviewed: {allowed}. A creature's name \
             in a message must come from `helpers::item_label`, which reads chimeric forms."
        );
    }
}
