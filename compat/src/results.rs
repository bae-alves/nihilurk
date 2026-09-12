//! What came back from a matrix run, read off disk.
//!
//! The shell half of the pipeline writes three kinds of file into
//! `target/compat/` and this module reads them. Nothing here talks to Docker,
//! and nothing here runs anything: a dashboard that shelled out to `docker
//! stats` while drawing would be competing with the container it is measuring
//! for the CPU it is measuring.
//!
//! The split is also what makes the report work after the fact. A matrix run
//! that finished yesterday, or one that ran on a build machine and was copied
//! here as a tarball, reads exactly the same as one still going.
//!
//! # The three files
//!
//! `results.tsv`     one line per finished run: the numbers `roog-perf`
//!                   printed, scraped out of its report.
//! `stats-<id>-<load>.tsv`
//!                   one line per second while that container was alive:
//!                   what Docker said it was using.
//! `footprint.tsv`   one line per built target: how big the binary came out.
//!
//! All three are append-only and tab-separated, which is what lets the
//! dashboard re-read them while they are still being written. A half-written
//! final line parses as garbage and is dropped; the next refresh gets it whole.

use std::fs;
use std::path::{Path, PathBuf};

/// Which load a run was driven by.
///
/// The distinction is the whole argument of the compat rig, so it is a type
/// rather than a string. [`Load::Game`] is the gate -- it answers "does roog
/// run here" -- and [`Load::Reel`] is the ceiling, which answers "how much
/// harder could you push this machine before the particle layer gives out".
/// A row is judged on the first and informed by the second; see
/// [`crate::verdict`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Load {
    /// A real dungeon floor, animated by the batches the game actually queues.
    /// This is roog running.
    Game,
    /// Bad Apple: ~800 motes a frame, two to three orders of magnitude past
    /// anything the game asks for. Nothing is gated on it.
    Reel,
}

impl Load {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "game" => Some(Load::Game),
            "reel" => Some(Load::Reel),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Load::Game => "game",
            Load::Reel => "reel",
        }
    }

    /// What the row is actually claiming, spelled out for a report header.
    pub fn what(self) -> &'static str {
        match self {
            Load::Game => "roog, played",
            Load::Reel => "Bad Apple, the ceiling",
        }
    }
}

/// How a run ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// The binary ran to the end and printed a report.
    Ok,
    /// The container exited non-zero, or printed nothing parseable. On a
    /// memory-capped row this is usually the OOM killer, which is a result:
    /// the machine this row stands for cannot hold roog.
    Failed,
    /// Killed by the runner's own clock. Not a crash -- the machine is simply
    /// too slow to finish the run in the time allowed.
    Timeout,
    /// Never attempted, and nothing stands in for it: no shell in the row's
    /// image, no qemu interpreter to fetch, `--targets` excluded it, or no
    /// binary was built. There is genuinely nothing to say about this row.
    Skipped,
    /// Deliberately not executed because the build already said enough: an
    /// emulated row, by default (see `stress_test_matrix.sh`'s
    /// `--exec-emulated`). Distinct from `Skipped` -- this is a claim, not an
    /// absence of one, and it must read as one.
    BuildOnly,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s {
            "ok" => Status::Ok,
            "timeout" => Status::Timeout,
            "skipped" => Status::Skipped,
            "build-only" => Status::BuildOnly,
            _ => Status::Failed,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Failed => "failed",
            Status::Timeout => "timeout",
            Status::Skipped => "skipped",
            Status::BuildOnly => "build-only",
        }
    }
}

/// One finished run of one binary on one row of the matrix.
///
/// Every field but `id`, `load` and `status` is what `roog-perf` measured
/// *inside* the container, scraped from its report. Nothing is recomputed
/// here -- if a number looks wrong, it is wrong in the report too, which is
/// the only place it can be chased.
#[derive(Clone, Debug)]
pub struct Run {
    pub id: String,
    pub target: String,
    pub load: Load,
    pub status: Status,
    pub frames: u64,
    /// Mean frame time in milliseconds. The number the verdict turns on.
    pub mean_ms: f64,
    pub p99_ms: f64,
    /// Frames that overran the budget. On a slow machine this rises before
    /// the mean does, because one long frame is absorbed by the mean and is
    /// not absorbed by the person looking at the screen.
    pub dropped: u64,
    /// Frames per second the run actually achieved, paced.
    pub fps: f64,
    /// Peak RSS of the whole process, in bytes, as measured inside the
    /// container. Compare against the row's memory cap, not against the host.
    pub peak_rss: u64,
    /// CPU percentage as the process saw itself, where 100 is one core.
    pub cpu_pct: f64,
    /// Wall clock the container was alive, measured by the runner outside it.
    /// Larger than the run itself by the container's startup, which on an
    /// emulated row is not small.
    pub wall_s: f64,
    /// Frame budget the run was paced to, in milliseconds. Carried per run
    /// rather than assumed, because `--fps` is a flag.
    pub budget_ms: f64,
}

impl Run {
    /// Frame time as a share of the budget. `1.0` is exactly keeping up.
    pub fn load_factor(&self) -> f64 {
        self.mean_ms / self.budget_ms.max(f64::EPSILON)
    }
}

/// One sample of what Docker said a container was using.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub elapsed_s: f64,
    /// CPU as Docker reports it: a percentage where 100 is one core, so a
    /// two-core row can legitimately read 200.
    pub cpu_pct: f64,
    pub mem_bytes: u64,
    /// The row's `--memory` cap, echoed on every sample so the dashboard can
    /// draw a gauge without going back to the matrix.
    pub mem_limit: u64,
}

/// The per-container time series for one run.
#[derive(Clone, Debug, Default)]
pub struct Series {
    pub samples: Vec<Sample>,
}

impl Series {
    pub fn peak_cpu(&self) -> f64 {
        self.samples.iter().map(|s| s.cpu_pct).fold(0.0, f64::max)
    }

    pub fn peak_mem(&self) -> u64 {
        self.samples.iter().map(|s| s.mem_bytes).max().unwrap_or(0)
    }

    /// The row's memory cap. Taken as the largest any sample reported rather
    /// than off the last one: the cap does not change during a run, so the
    /// maximum is the cap, and reading it that way means a single bad sample
    /// cannot report a container that had no memory at all.
    pub fn limit(&self) -> u64 {
        self.samples.iter().map(|s| s.mem_limit).max().unwrap_or(0)
    }

    /// The last sample that was actually recorded.
    pub fn last(&self) -> Option<Sample> {
        self.samples.last().copied()
    }

    /// Sparkline fodder, in hundredths of a percent.
    ///
    /// Not whole percent: a paced game run on a machine with headroom sits at
    /// about 1% of a core, and truncating that to an integer leaves a graph of
    /// zeroes with the occasional 1 in it. The sparkline scales to its own
    /// maximum, so the unit does not matter to the reader -- only the
    /// resolution does.
    pub fn cpu_line(&self) -> Vec<u64> {
        self.samples
            .iter()
            .map(|s| (s.cpu_pct * 100.0) as u64)
            .collect()
    }

    pub fn mem_line(&self) -> Vec<u64> {
        self.samples.iter().map(|s| s.mem_bytes).collect()
    }
}

/// How big the binaries came out for one target.
#[derive(Clone, Copy, Debug, Default)]
pub struct Footprint {
    /// `engine`, `--release`: the thing that ships.
    pub game: u64,
    /// `roog-perf`: the rig, which is not shipped and is here only because
    /// it is what the container runs.
    pub rig: u64,
}

/// Everything on disk from one matrix run.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub runs: Vec<Run>,
    pub series: Vec<(String, Load, Series)>,
    pub footprints: Vec<(String, Footprint)>,
    pub dir: PathBuf,
}

impl Results {
    /// Read everything in `dir`. A missing directory is not an error: it is
    /// what "the matrix has not been run yet" looks like, and the dashboard
    /// says so rather than refusing to start.
    pub fn load(dir: &Path) -> Self {
        Self {
            runs: read_runs(&dir.join("results.tsv")),
            series: read_all_series(dir),
            footprints: read_footprints(&dir.join("footprint.tsv")),
            dir: dir.to_path_buf(),
        }
    }

    /// The run for one row under one load, if it finished.
    pub fn run(&self, id: &str, load: Load) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id && r.load == load)
    }

    pub fn series_for(&self, id: &str, load: Load) -> Option<&Series> {
        self.series
            .iter()
            .find(|(sid, sload, _)| sid == id && *sload == load)
            .map(|(_, _, s)| s)
    }

    pub fn footprint(&self, id: &str) -> Option<Footprint> {
        self.footprints
            .iter()
            .find(|(fid, _)| fid == id)
            .map(|(_, f)| *f)
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty() && self.footprints.is_empty()
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

fn read_runs(path: &Path) -> Vec<Run> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_run).collect()
}

fn parse_run(line: &str) -> Option<Run> {
    let f = fields(line)?;
    if f.len() < 14 {
        return None;
    }
    Some(Run {
        id: f[0].to_string(),
        target: f[1].to_string(),
        load: Load::parse(f[2])?,
        status: Status::parse(f[3]),
        frames: f[4].parse().unwrap_or(0),
        mean_ms: f[5].parse().unwrap_or(0.0),
        p99_ms: f[6].parse().unwrap_or(0.0),
        dropped: f[7].parse().unwrap_or(0),
        fps: f[8].parse().unwrap_or(0.0),
        peak_rss: f[9].parse().unwrap_or(0),
        cpu_pct: f[10].parse().unwrap_or(0.0),
        wall_s: f[11].parse().unwrap_or(0.0),
        budget_ms: match f[12].parse().unwrap_or(0.0) {
            // A zero budget would make every load factor infinite. It can only
            // come from a malformed line, so fall back to the 30 fps default
            // rather than poisoning the arithmetic downstream.
            b if b > 0.0 => b,
            _ => 1000.0 / 30.0,
        },
        // Field 13 is the reel note, which the dashboard does not read.
    })
}

/// Find every `stats-<id>-<load>.tsv` in the directory and read it.
///
/// Discovered rather than looked up by row, so a stats file left behind by a
/// row that has since been removed from the matrix still shows up instead of
/// being silently ignored.
fn read_all_series(dir: &Path) -> Vec<(String, Load, Series)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name
            .strip_prefix("stats-")
            .and_then(|n| n.strip_suffix(".tsv"))
        else {
            continue;
        };
        // `<id>-<load>`, and an id may contain hyphens (`pi-zero`), so the
        // split is from the right.
        let Some((id, load)) = stem.rsplit_once('-') else {
            continue;
        };
        let Some(load) = Load::parse(load) else {
            continue;
        };
        out.push((id.to_string(), load, read_series(&entry.path())));
    }
    out
}

fn read_series(path: &Path) -> Series {
    let Ok(text) = fs::read_to_string(path) else {
        return Series::default();
    };
    Series {
        samples: text.lines().filter_map(parse_sample).collect(),
    }
}

fn parse_sample(line: &str) -> Option<Sample> {
    let f = fields(line)?;
    if f.len() < 4 {
        return None;
    }
    Some(Sample {
        elapsed_s: f[0].parse().ok()?,
        cpu_pct: f[1].parse().ok()?,
        mem_bytes: f[2].parse().ok()?,
        mem_limit: f[3].parse().ok()?,
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
    if f.len() < 4 {
        return None;
    }
    Some((
        f[0].to_string(),
        Footprint {
            game: f[2].parse().unwrap_or(0),
            rig: f[3].parse().unwrap_or(0),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "pi-zero\tarmv7-unknown-linux-musleabihf\tgame\tok\t450\t12.500\t28.100\t3\t29.4\t9437184\t61.2\t21.8\t33.33\t-";

    #[test]
    fn a_results_line_round_trips_into_the_numbers_the_verdict_reads() {
        let run = parse_run(LINE).expect("well-formed line");
        assert_eq!(run.id, "pi-zero");
        assert_eq!(run.load, Load::Game);
        assert_eq!(run.status, Status::Ok);
        assert_eq!(run.frames, 450);
        assert_eq!(run.dropped, 3);
        assert!((run.mean_ms - 12.5).abs() < 1e-9);
        assert!((run.load_factor() - 12.5 / 33.33).abs() < 1e-6);
    }

    #[test]
    fn comments_blanks_and_half_written_lines_are_dropped_not_guessed_at() {
        // The dashboard re-reads these files while the runner is appending to
        // them, so a truncated final line is normal and must not parse.
        assert!(parse_run("# id\ttarget\tload").is_none());
        assert!(parse_run("").is_none());
        assert!(parse_run("pi-zero\tarmv7\tgame\tok\t450").is_none());
    }

    #[test]
    fn a_zero_budget_never_becomes_an_infinite_load_factor() {
        let line = LINE.replace("\t33.33\t", "\t0\t");
        let run = parse_run(&line).expect("well-formed line");
        assert!(run.load_factor().is_finite());
    }

    #[test]
    fn a_stats_file_name_splits_on_the_last_hyphen_so_hyphenated_ids_survive() {
        // `pi-zero` is the row this would otherwise break on.
        let stem = "pi-zero-game";
        let (id, load) = stem.rsplit_once('-').expect("a load suffix");
        assert_eq!(id, "pi-zero");
        assert_eq!(Load::parse(load), Some(Load::Game));
    }

    #[test]
    fn a_missing_directory_reads_as_an_empty_run_not_a_panic() {
        let results = Results::load(Path::new("/nonexistent/compat/dir"));
        assert!(results.is_empty());
    }
}
