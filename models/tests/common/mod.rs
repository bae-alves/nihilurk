//! Shared scaffolding for the integration tests.
//!
//! Cargo compiles every *file* under `tests/` as its own test binary, and
//! `tests/common/` is the one directory it treats as a plain module instead —
//! so this is where a helper goes when it must not become a test target of its
//! own.

use std::path::PathBuf;

/// A save file with a path nothing else will pick, that deletes itself.
///
/// Two things used to be wrong with the eleven save round-trip tests. They
/// named fixed files in the shared temp directory (`nihilurk_test.sav`,
/// `nihilurk_melee_cap.sav`, …), so two `cargo test` runs at once — or a CI matrix
/// sharing one `/tmp` — could write the same path from different tests and see
/// each other's bytes. And cleanup was by hand, which meant two of them never
/// did it: a stale save in `/tmp` is how a test starts passing for the wrong
/// reason the next time the save format changes.
///
/// The process id in the name fixes the first, and `Drop` fixes the second by
/// making it impossible to forget.
pub struct SaveFile(PathBuf);

impl SaveFile {
    /// `tag` only has to be unique within one test binary; the pid separates
    /// the binaries and the runs.
    pub fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!("nihilurk-{}-{tag}.sav", std::process::id())))
    }

    /// The path, as `save_game` and `load_game` want it.
    pub fn path(&self) -> &str {
        self.0.to_str().expect("the temp directory is valid utf-8")
    }
}

impl Drop for SaveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
