//! Does roog run on this machine, and how well.
//!
//! This is the only module in the pipeline that turns measurements into a
//! judgement, and it exists so that there is exactly one place where the
//! judgement lives. The shell half writes raw numbers and never grades them;
//! the dashboard, the report and CI all come here. A band that drifted between
//! the script and the report would mean two different answers to the one
//! question the rig is for.
//!
//! # What is being judged
//!
//! The game, under the game's own load -- a real floor, animated by the
//! batches roog actually queues, one per turn. Not Bad Apple. The reel is two
//! to three orders of magnitude past anything the game produces, and a machine
//! that cannot keep up with it may still play roog perfectly well; grading on
//! the reel would fail every row that matters and tell you nothing. The reel
//! run is the ceiling, reported beside the verdict and never gated on. See
//! `docs/explanation/cross-platform-testing.md`.
//!
//! # Why frame time and not fps
//!
//! The rig paces itself to the target frame rate, so a machine with headroom
//! and a machine with none both report ~30 fps -- the first sleeps out the rest
//! of the budget and the second does not. Achieved fps only falls once the
//! machine is *already* losing, which makes it a lagging indicator of the thing
//! being asked. Mean frame time as a share of the budget says how close to the
//! edge a row is while it is still comfortably on the right side of it.

use crate::results::{Load, Run, Status};

/// Where a row landed.
///
/// The order is the severity order, so `max()` over a set of rows is the worst
/// of them -- which is what a CI exit code wants.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Band {
    /// Room to spare: the frame is done in under half the budget and nothing
    /// was dropped. roog will feel the same here as on a workstation.
    Plays,
    /// Keeps up. Inside the budget, with the occasional dropped frame -- which
    /// on an emulated row is as likely to be the emulator as the machine.
    Playable,
    /// Visibly struggling. Over budget on the mean, or dropping frames often
    /// enough to see. It runs; you would not want to play it.
    Janky,
    /// Cannot hold the frame rate at all. The animation will crawl.
    Unplayable,
    /// It did not run. The container failed, was killed, or was never given a
    /// binary. On a memory-capped row this is usually the OOM killer, and that
    /// is a finding, not an error in the pipeline.
    DoesNotRun,
    /// Nothing was measured for this row yet.
    Unknown,
}

impl Band {
    pub fn label(self) -> &'static str {
        match self {
            Band::Plays => "plays",
            Band::Playable => "playable",
            Band::Janky => "janky",
            Band::Unplayable => "unplayable",
            Band::DoesNotRun => "does not run",
            Band::Unknown => "-",
        }
    }

    /// One line on why, for the report. Deliberately about the machine rather
    /// than about the numbers: the numbers are printed beside it already.
    pub fn gloss(self) -> &'static str {
        match self {
            Band::Plays => "comfortable; the frame is done with the budget to spare",
            Band::Playable => "keeps up; roog is playable on this hardware",
            Band::Janky => "over budget or dropping frames -- it runs, but it shows",
            Band::Unplayable => "cannot hold the frame rate; the animation crawls",
            Band::DoesNotRun => "the binary did not complete a run on this machine",
            Band::Unknown => "not measured",
        }
    }

    /// Whether a row in this band should fail a pipeline run.
    ///
    /// `Janky` deliberately does not. Half the matrix is hardware roog is
    /// *expected* to be slow on -- a Pi Zero emulated at 0.4 of a core is not
    /// a machine anyone promised a smooth 30 fps -- and a gate that went red
    /// on it would be turned off within a week. What must never happen is a
    /// target that stops running at all.
    pub fn is_failure(self) -> bool {
        matches!(self, Band::Unplayable | Band::DoesNotRun)
    }
}

/// Share of the budget below which a row is comfortable rather than merely
/// keeping up. Half is generous on purpose: the remaining half is what the
/// game has left for a turn that does real work -- pathfinding a room full of
/// monsters, generating the next floor -- on a machine whose spare capacity
/// this rig never measures.
const COMFORTABLE: f64 = 0.5;

/// Dropped-frame rate a row is allowed before it is called janky. One frame in
/// two hundred is invisible; it is also roughly the rate qemu-user contributes
/// on its own, so anything tighter would be grading the emulator.
const DROPS_TOLERATED: f64 = 0.005;

/// Dropped-frame rate past which the machine is not holding the frame rate at
/// all, regardless of what the mean says. A row can have a flattering mean and
/// still be unplayable if the work arrives in clumps, which is exactly what a
/// starved container does.
const DROPS_HOPELESS: f64 = 0.10;

/// Share of the row's memory cap past which the run is worth flagging even
/// though it passed. Nothing is failed on this -- the run completed, so the
/// memory was there -- but a row at 90% of a Pi Zero's RAM is one dungeon
/// level away from not completing.
const MEMORY_TIGHT: f64 = 0.90;

/// Grade one run.
///
/// Only [`Load::Game`] runs are graded. Handing this a reel run returns
/// [`Band::Unknown`] rather than a band, because a reel verdict would be read
/// as a claim about the game and it is not one.
pub fn grade(run: &Run) -> Band {
    if run.load != Load::Game {
        return Band::Unknown;
    }
    if run.status != Status::Ok {
        return Band::DoesNotRun;
    }
    // A run that produced no frames printed a report without measuring
    // anything -- it started and died. Not a fast machine; no machine.
    if run.frames == 0 {
        return Band::DoesNotRun;
    }
    let factor = run.load_factor();
    let drop_rate = run.dropped as f64 / run.frames as f64;
    if factor > 1.0 || drop_rate > DROPS_HOPELESS {
        return unplayable_or_janky(factor, drop_rate);
    }
    if factor > COMFORTABLE || drop_rate > DROPS_TOLERATED {
        return Band::Playable;
    }
    Band::Plays
}

/// Split the losing half. Over budget on the mean *and* dropping frames in
/// clumps is a machine that has stopped keeping up; one or the other is a
/// machine that is merely bad at it.
fn unplayable_or_janky(factor: f64, drop_rate: f64) -> Band {
    if factor > 2.0 {
        return Band::Unplayable;
    }
    if drop_rate > DROPS_HOPELESS && factor > 1.0 {
        return Band::Unplayable;
    }
    Band::Janky
}

/// Whether a run came close enough to its container's memory cap to be worth
/// a word in the report. `limit` of zero means the row was not capped.
pub fn memory_is_tight(peak_rss: u64, limit: u64) -> bool {
    if limit == 0 {
        return false;
    }
    peak_rss as f64 / limit as f64 > MEMORY_TIGHT
}

/// The worst band across a set of runs -- what a whole matrix run comes to.
pub fn overall(runs: &[Run]) -> Band {
    runs.iter()
        .filter(|r| r.load == Load::Game)
        .map(grade)
        .max()
        .unwrap_or(Band::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(load: Load, status: Status, mean_ms: f64, dropped: u64) -> Run {
        Run {
            id: "test".into(),
            target: "x86_64-unknown-linux-musl".into(),
            load,
            status,
            frames: 450,
            mean_ms,
            p99_ms: mean_ms * 2.0,
            dropped,
            fps: 30.0,
            peak_rss: 8 * 1024 * 1024,
            cpu_pct: 40.0,
            wall_s: 15.0,
            budget_ms: 1000.0 / 30.0,
        }
    }

    #[test]
    fn a_desktop_class_row_plays() {
        assert_eq!(grade(&run(Load::Game, Status::Ok, 0.4, 0)), Band::Plays);
    }

    #[test]
    fn inside_the_budget_but_not_comfortable_is_playable() {
        assert_eq!(grade(&run(Load::Game, Status::Ok, 20.0, 0)), Band::Playable);
    }

    #[test]
    fn a_handful_of_drops_costs_the_comfortable_band_but_not_more() {
        // 3/450 is over the tolerated rate and nowhere near hopeless.
        assert_eq!(grade(&run(Load::Game, Status::Ok, 1.0, 3)), Band::Playable);
    }

    #[test]
    fn over_budget_is_janky_and_far_over_is_unplayable() {
        assert_eq!(grade(&run(Load::Game, Status::Ok, 40.0, 0)), Band::Janky);
        assert_eq!(
            grade(&run(Load::Game, Status::Ok, 90.0, 0)),
            Band::Unplayable
        );
    }

    #[test]
    fn clumped_work_is_unplayable_even_when_the_mean_only_just_loses() {
        // A starved container: the mean is barely over, but a fifth of the
        // frames missed the budget outright. The mean is flattering here and
        // the drop rate is the honest number.
        assert_eq!(
            grade(&run(Load::Game, Status::Ok, 35.0, 90)),
            Band::Unplayable
        );
    }

    #[test]
    fn a_failed_or_empty_run_does_not_run_rather_than_scoring_zero() {
        assert_eq!(
            grade(&run(Load::Game, Status::Failed, 0.0, 0)),
            Band::DoesNotRun
        );
        assert_eq!(
            grade(&run(Load::Game, Status::Timeout, 0.0, 0)),
            Band::DoesNotRun
        );
        let mut empty = run(Load::Game, Status::Ok, 0.0, 0);
        empty.frames = 0;
        assert_eq!(grade(&empty), Band::DoesNotRun);
    }

    #[test]
    fn the_reel_is_never_graded_because_nothing_is_gated_on_it() {
        // Bad Apple at 800 motes a frame will lose on a Pi, and that is not a
        // statement about whether roog runs there.
        assert_eq!(
            grade(&run(Load::Reel, Status::Ok, 400.0, 400)),
            Band::Unknown
        );
    }

    #[test]
    fn only_the_bands_that_mean_it_stopped_working_fail_a_run() {
        assert!(!Band::Plays.is_failure());
        assert!(!Band::Playable.is_failure());
        // The Pi rows live here and a red gate would get the pipeline muted.
        assert!(!Band::Janky.is_failure());
        assert!(Band::Unplayable.is_failure());
        assert!(Band::DoesNotRun.is_failure());
    }

    #[test]
    fn overall_is_the_worst_row_and_ignores_the_reel() {
        let runs = vec![
            run(Load::Game, Status::Ok, 0.4, 0),
            run(Load::Game, Status::Ok, 40.0, 0),
            run(Load::Reel, Status::Failed, 0.0, 0),
        ];
        assert_eq!(overall(&runs), Band::Janky);
    }

    #[test]
    fn memory_is_only_tight_against_a_cap_that_exists() {
        assert!(memory_is_tight(480 * 1024 * 1024, 512 * 1024 * 1024));
        assert!(!memory_is_tight(8 * 1024 * 1024, 512 * 1024 * 1024));
        assert!(!memory_is_tight(u64::MAX, 0));
    }
}
