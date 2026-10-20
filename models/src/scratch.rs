//! A path in the temp directory for a unit test that touches the disk, gone
//! when the test ends.
//!
//! The unit-test twin of `tests/it/common::SaveFile`, which the unit tests
//! cannot reach. The same two faults led to both. A fixed name in the shared
//! temp directory lets two runs at once see each other's bytes, so the name
//! carries the process id. Cleanup by hand is skipped by the first failed
//! assert, and a stale file left behind is how a test starts passing for the
//! wrong reason. So cleanup lives in `Drop`, which a panic runs too.

use std::path::{Path, PathBuf};

use crate::saveload::with_suffix;

/// A scratch path that removes itself, and whatever the code under test left
/// beside it, when it drops.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    /// `tag` has to be unique across the crate's unit tests, which run as one
    /// binary; the process id keeps runs apart. Name the module in it.
    pub(crate) fn new(tag: &str) -> Self {
        Self(std::env::temp_dir().join(format!("nihilurk-unit-{}-{tag}", std::process::id())))
    }

    /// The path, for a test that builds on it or prints it.
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// The `.tmp` that `write_atomically` writes first and the `.lock` the
/// leaderboard takes go with the path, and a path used as a directory goes
/// whole.
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(with_suffix(&self.0, ".tmp"));
        let _ = std::fs::remove_file(with_suffix(&self.0, ".lock"));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
