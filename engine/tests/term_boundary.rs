//! Keeps crossterm in `engine/src/term.rs` and out of `models`.
//!
//! The game draws and reads input through one file, so a second backend has
//! one file to replace and `models` and the rest of `engine` can run with no
//! terminal at all. The compiler would not notice a `use crossterm::...` that
//! crept back in; this does.

use std::path::{Path, PathBuf};

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

fn mentions(text: &str, comment: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with(comment))
        .filter(|(_, line)| line.contains("crossterm"))
        .map(|(n, _)| n + 1)
        .collect()
}

// ponytail: line scan, not syn; a crossterm path in a string literal trips it
#[test]
fn only_term_rs_names_crossterm() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let leaks: Vec<String> = files
        .iter()
        .filter(|f| f.file_name().is_some_and(|n| n != "term.rs"))
        .flat_map(|f| {
            let text = std::fs::read_to_string(f).expect("source is readable");
            mentions(&text, "//")
                .into_iter()
                .map(|n| format!("{}:{n}", f.strip_prefix(&src).unwrap().display()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(leaks.is_empty(), "crossterm outside term.rs: {leaks:?}");
}

#[test]
fn models_does_not_depend_on_crossterm() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/Cargo.toml");
    let text = std::fs::read_to_string(manifest).expect("models/Cargo.toml is readable");
    assert_eq!(mentions(&text, "#"), Vec::<usize>::new());
}
