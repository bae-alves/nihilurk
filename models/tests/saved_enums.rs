//! Pins the variant order of every enum a save can contain.
//!
//! postcard writes an enum as its variant's position, so reordering variants
//! (or inserting one anywhere but the end) silently changes what old saves
//! mean. `tests/golden/save.bin` only sees the variants its fixture uses; this
//! reads the source and sees them all.
//!
//! A failure means a saved enum differs from `tests/golden/saved_enums.txt`.
//! Appended a variant at the end, or added or removed a saved enum? Regenerate
//! with `NIHILURK_REGEN_ENUMS=1 cargo test -p nihilurk-models --test
//! saved_enums` and bump `SAVE_VERSION` if a removal or reorder was on purpose.
//! Anything else moved a variant: put it back.

use std::path::{Path, PathBuf};

use syn::{Attribute, Item};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/saved_enums.txt");

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

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

fn derives_serialize(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("derive")
            && a.meta
                .require_list()
                .is_ok_and(|l| l.tokens.to_string().contains("Serialize"))
    })
}

fn saved_enums(file: &str, body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut items: Vec<Item> = syn::parse_file(body).expect("source parses").items;
    while let Some(item) = items.pop() {
        match item {
            Item::Enum(e) if derives_serialize(&e.attrs) => {
                let variants: Vec<String> =
                    e.variants.iter().map(|v| v.ident.to_string()).collect();
                found.push(format!("{file}::{} = {}", e.ident, variants.join(",")));
            }
            Item::Mod(m) => items.extend(m.content.into_iter().flat_map(|(_, i)| i)),
            _ => {}
        }
    }
    found
}

fn current() -> Vec<String> {
    let mut files = Vec::new();
    rust_files(&src(), &mut files);
    let mut lines: Vec<String> = files
        .iter()
        .flat_map(|path| {
            let name = path.strip_prefix(src()).unwrap().display().to_string();
            let body = std::fs::read_to_string(path).expect("source is readable");
            saved_enums(&name, &body)
        })
        .collect();
    lines.sort();
    lines
}

#[test]
fn saved_enums_keep_their_variant_order() {
    let now = current().join("\n") + "\n";
    if std::env::var_os("NIHILURK_REGEN_ENUMS").is_some() {
        std::fs::write(GOLDEN, &now).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(GOLDEN).unwrap_or_default();
    let differs: Vec<String> = now
        .lines()
        .filter(|l| !golden.lines().any(|g| g == *l))
        .map(|l| format!("+ {l}"))
        .chain(
            golden
                .lines()
                .filter(|g| !now.lines().any(|l| l == *g))
                .map(|g| format!("- {g}")),
        )
        .collect();
    assert!(
        differs.is_empty(),
        "a saved enum changed shape (see the file header):\n{}",
        differs.join("\n")
    );
}

#[test]
fn a_reordered_variant_is_a_difference() {
    let before = saved_enums("a.rs", "#[derive(Serialize)] enum E { A, B, C }");
    let swapped = saved_enums("a.rs", "#[derive(Serialize)] enum E { A, C, B }");
    let plain = saved_enums("a.rs", "#[derive(Debug)] enum E { A }");
    assert_eq!(before, ["a.rs::E = A,B,C"]);
    assert_ne!(before, swapped);
    assert!(plain.is_empty());
}
