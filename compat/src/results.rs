//! What came back from a matrix run, read off disk.
//!
//! The shell half of the pipeline writes two kinds of file into
//! `target/compat/` and this module reads them. Nothing here talks to Docker,
//! and nothing here runs anything.
//!
//! The split is also what makes the report work after the fact. A matrix run
//! that finished yesterday, or one that ran on a build machine and was copied
//! here as a tarball, reads exactly the same as one still going.
//!
//! # The two files
//!
//! `results.tsv`     one line per checked row: whether it has a shell and
//!                   whether the binary started.
//! `footprint.tsv`   one line per built target: how big the binary came out.
//!
//! Both are append-only and tab-separated, which is what lets the report
//! re-read them while they are still being written. A half-written final line
//! parses as garbage and is dropped; the next read gets it whole.

use std::fs;
use std::path::{Path, PathBuf};

/// How one linux row's check ended.
///
/// This is the whole verdict. There is no performance band here on purpose --
/// nihilurk's compat claim is "if it builds, has a shell, and is std, it can run
/// the game", not a measurement of how fast it does so. See
/// `docs/explanation/cross-platform-testing.md`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Built, the image has a shell, and `engine -content` started and
    /// printed something.
    Ok,
    /// Built, but the image has no usable shell -- nihilurk cannot run
    /// without one, whatever the binary does.
    NoShell,
    /// Had a shell, but the binary did not start, exited non-zero, or
    /// printed nothing.
    Failed,
    /// Never attempted: `--targets` excluded it, or no binary was built for
    /// it in the first place.
    Skipped,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s {
            "ok" => Status::Ok,
            "no-shell" => Status::NoShell,
            "skipped" => Status::Skipped,
            _ => Status::Failed,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::NoShell => "no-shell",
            Status::Failed => "failed",
            Status::Skipped => "skipped",
        }
    }

    /// What to print in the verdict column.
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "runs",
            Status::NoShell => "no shell",
            Status::Failed => "does not run",
            Status::Skipped => "-",
        }
    }

    /// Whether a row in this state should fail the gate.
    ///
    /// `Skipped` does not: compat/ genuinely has nothing to say about a row
    /// `--targets` excluded, and that is not the same claim `Failed` and
    /// `NoShell` make.
    pub fn is_failure(self) -> bool {
        matches!(self, Status::NoShell | Status::Failed)
    }
}

/// One checked row.
#[derive(Clone, Debug)]
pub struct Check {
    pub id: String,
    pub target: String,
    pub status: Status,
}

/// How big the binary came out for one target.
#[derive(Clone, Copy, Debug, Default)]
pub struct Footprint {
    /// `engine`, `--release`: the thing that ships, and the only binary
    /// built now that there is no rig for it to carry alongside.
    pub game: u64,
}

/// Everything on disk from one matrix run.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub checks: Vec<Check>,
    pub footprints: Vec<(String, Footprint)>,
    pub dir: PathBuf,
}

impl Results {
    /// Read everything in `dir`. A missing directory is not an error: it is
    /// what "the matrix has not been checked yet" looks like, and the report
    /// says so rather than refusing to run.
    pub fn load(dir: &Path) -> Self {
        Self {
            checks: read_checks(&dir.join("results.tsv")),
            footprints: read_footprints(&dir.join("footprint.tsv")),
            dir: dir.to_path_buf(),
        }
    }

    pub fn check(&self, id: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.id == id)
    }

    pub fn footprint(&self, id: &str) -> Option<Footprint> {
        self.footprints
            .iter()
            .find(|(fid, _)| fid == id)
            .map(|(_, f)| *f)
    }

    pub fn is_empty(&self) -> bool {
        self.checks.is_empty() && self.footprints.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Readers
// ---------------------------------------------------------------------------

/// Split a line the way every file here is written, dropping comments and
/// blanks. Returns `None` for a line that is not data, which is how a
/// half-written trailing line gets skipped until it is complete.
fn fields(line: &str) -> Option<Vec<&str>> {
    let line = line.trim_end();
    if line.trim_start().starts_with('#') || line.trim().is_empty() {
        return None;
    }
    Some(line.split('\t').collect())
}

fn read_checks(path: &Path) -> Vec<Check> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_check).collect()
}

fn parse_check(line: &str) -> Option<Check> {
    let f = fields(line)?;
    if f.len() < 3 {
        return None;
    }
    Some(Check {
        id: f[0].to_string(),
        target: f[1].to_string(),
        status: Status::parse(f[2]),
    })
}

fn read_footprints(path: &Path) -> Vec<(String, Footprint)> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_footprint).collect()
}

fn parse_footprint(line: &str) -> Option<(String, Footprint)> {
    let f = fields(line)?;
    if f.len() < 3 {
        return None;
    }
    Some((
        f[0].to_string(),
        Footprint {
            game: f[2].parse().unwrap_or(0),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_results_line_round_trips() {
        let run = parse_check("pi-zero\tarmv7-unknown-linux-musleabihf\tok").expect("well-formed");
        assert_eq!(run.id, "pi-zero");
        assert_eq!(run.status, Status::Ok);
        assert!(!run.status.is_failure());
    }

    #[test]
    fn comments_blanks_and_half_written_lines_are_dropped_not_guessed_at() {
        // The report re-reads this file while the runner is appending to it,
        // so a truncated final line is normal and must not parse.
        assert!(parse_check("# id\ttarget\tstatus").is_none());
        assert!(parse_check("").is_none());
        assert!(parse_check("pi-zero\tarmv7").is_none());
    }

    #[test]
    fn no_shell_and_failed_are_failures_but_skipped_is_not() {
        assert!(Status::parse("no-shell").is_failure());
        assert!(Status::parse("failed").is_failure());
        assert!(!Status::parse("skipped").is_failure());
        assert!(!Status::parse("ok").is_failure());
    }

    #[test]
    fn a_missing_directory_reads_as_empty_not_a_panic() {
        let results = Results::load(Path::new("/nonexistent/compat/dir"));
        assert!(results.is_empty());
    }

    #[test]
    fn a_footprint_line_round_trips() {
        let (id, f) = parse_footprint("cloud\tx86_64-unknown-linux-musl\t1800000\tnote").unwrap();
        assert_eq!(id, "cloud");
        assert_eq!(f.game, 1_800_000);
    }
}
