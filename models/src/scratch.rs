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
    /// binary; the process id keeps runs apart. Name the module in it. Anything
    /// a killed run with the same id left at the path is cleared first.
    pub(crate) fn new(tag: &str) -> Self {
        let scratch =
            Self(std::env::temp_dir().join(format!("nihilurk-unit-{}-{tag}", std::process::id())));
        scratch.clear();
        scratch
    }

    fn clear(&self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(with_suffix(&self.0, ".tmp"));
        let _ = std::fs::remove_file(with_suffix(&self.0, ".lock"));
        let _ = std::fs::remove_dir_all(&self.0);
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
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_removes_the_path_its_tmp_and_lock_and_a_directory_tree() {
        let file = Scratch::new("scratch-drop-file");
        let tmp = with_suffix(file.path(), ".tmp");
        let lock = with_suffix(file.path(), ".lock");
        std::fs::write(&file, b"x").unwrap();
        std::fs::write(&tmp, b"x").unwrap();
        std::fs::write(&lock, b"x").unwrap();
        let dir = Scratch::new("scratch-drop-dir");
        let nested = dir.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("f"), b"x").unwrap();
        let (file_path, dir_path) = (file.path().to_path_buf(), dir.path().to_path_buf());

        drop(file);
        drop(dir);

        for gone in [file_path, tmp, lock, dir_path] {
            assert!(!gone.exists(), "{gone:?} outlived its Scratch");
        }
    }

    #[test]
    fn a_new_scratch_clears_what_a_dead_run_left_at_its_path() {
        let path = std::env::temp_dir().join(format!(
            "nihilurk-unit-{}-scratch-stale",
            std::process::id()
        ));
        std::fs::write(&path, b"stale").unwrap();
        std::fs::write(with_suffix(&path, ".lock"), b"stale").unwrap();

        let scratch = Scratch::new("scratch-stale");

        assert!(!scratch.path().exists());
        assert!(!with_suffix(scratch.path(), ".lock").exists());
    }
}
