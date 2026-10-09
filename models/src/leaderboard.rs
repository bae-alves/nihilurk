//! The internal leaderboard: a postcard blob at [`path`], in [`crate::data_dir`],
//! versioned like a save, updated the moment a run ends and read back by
//! `-scores` without starting a game.
//!
//! Capped at [`LEADERBOARD_STORE_LIMIT`] entries so the file stays exactly
//! what a leaderboard should be: tiny, forever — a run that doesn't make the
//! cut is simply not written.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How a run ended. Distinguishes the two ways a run can be lost by whether
/// the Element of Yoord was in the pack ([`crate::map::holding_element_of_yoord`])
/// — a death on the way back out cost more than one on the way down.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The Element of Yoord was carried up the last stair.
    Win,
    /// Died with the Element of Yoord in the pack, on the way back out.
    LoseAscent,
    /// Died without it, on the way down.
    LoseDescent,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Outcome::Win => "WIN",
            Outcome::LoseAscent => "LOSE (Asc.)",
            Outcome::LoseDescent => "LOSE (Desc.)",
        })
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    name: String,
    outcome: Outcome,
    score: i64,
    /// Seconds since the Unix epoch, UTC. Formatted for display by
    /// [`format_timestamp`] rather than stored pre-formatted, so the file
    /// keeps the one number and the rendering can change without touching it.
    epoch_secs: u64,
}

/// Where the leaderboard lives: `leaderboard.sav` in [`crate::data_dir`], next
/// to the saves.
pub fn path() -> PathBuf {
    crate::data_dir().join("leaderboard.sav")
}

/// The leaderboard file's format version, written ahead of the entries. A file
/// under any other version is refused, never overwritten, so a format change
/// can't wipe the board.
const LEADERBOARD_VERSION: u16 = 1;

/// How many rows the file keeps, and `-scores` prints. A run that doesn't
/// beat the lowest of these is dropped on write rather than kept and hidden,
/// so the file never grows past this many entries.
pub const LEADERBOARD_STORE_LIMIT: usize = 10;

/// Records this run's outcome, keeping only the top [`LEADERBOARD_STORE_LIMIT`]
/// scores ever seen. Best-effort: a leaderboard nobody can write to (a
/// read-only directory) shouldn't stop the player's run from ending, so a
/// caller that cares can log the `Err` rather than propagate it.
pub fn record(name: &str, outcome: Outcome, score: i64) -> std::io::Result<()> {
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    record_to(path(), name, outcome, score, epoch_secs)
}

fn record_to(
    path: impl AsRef<Path>,
    name: &str,
    outcome: Outcome,
    score: i64,
    epoch_secs: u64,
) -> std::io::Result<()> {
    let path = path.as_ref();
    let _lock = lock_beside(path)?;
    let mut entries = all(path)?;
    entries.push(Entry {
        name: name.to_string(),
        outcome,
        score,
        epoch_secs,
    });
    entries.sort_by_key(|e| std::cmp::Reverse(e.score));
    entries.truncate(LEADERBOARD_STORE_LIMIT);

    let bytes = postcard::to_allocvec(&(LEADERBOARD_VERSION, &entries))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    crate::saveload::write_atomically(path, |w| w.write_all(&bytes))
}

/// Takes the leaderboard's lock, `<path>.lock`, and holds it until the file
/// handed back is dropped. Two runs ending at once in the same data dir take
/// turns at the read-modify-write in [`record_to`] instead of one dropping
/// the other's score. Readers don't need it: a write lands by rename, whole.
fn lock_beside(path: &Path) -> std::io::Result<std::fs::File> {
    crate::saveload::make_parent_dir(path)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(crate::saveload::with_suffix(path, ".lock"))?;
    lock.lock()?;
    Ok(lock)
}

/// Every entry the file holds, already in score order. Empty if nothing has
/// been recorded yet. A file that won't parse, or carries another
/// [`LEADERBOARD_VERSION`], is an `InvalidData` error: the scores in it are
/// still someone's, so nothing here starts a fresh board over them.
fn all(path: impl AsRef<Path>) -> std::io::Result<Vec<Entry>> {
    let bytes = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        read => read?,
    };
    match postcard::from_bytes::<(u16, Vec<Entry>)>(&bytes) {
        Ok((LEADERBOARD_VERSION, entries)) => Ok(entries),
        _ => Err(std::io::ErrorKind::InvalidData.into()),
    }
}

/// The `limit` highest scores ever recorded, highest first, each with a
/// "YYYY-MM-DD HH:MM" UTC rendering of when the run ended. Errs the way
/// [`all`] does.
pub fn top(limit: usize) -> std::io::Result<Vec<(String, Outcome, i64, String)>> {
    Ok(all(path())?
        .into_iter()
        .take(limit)
        .map(|e| (e.name, e.outcome, e.score, format_timestamp(e.epoch_secs)))
        .collect())
}

/// Hand-rolled rather than pulling in a date crate for one stamp: this is
/// Howard Hinnant's `civil_from_days`, the standard proleptic-Gregorian
/// algorithm, fed only ever-increasing `SystemTime::now()` values (always
/// well past the epoch, never before it).
fn format_timestamp(epoch_secs: u64) -> String {
    let days_since_epoch = (epoch_secs / 86_400) as i64;
    let secs_of_day = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days_since_epoch);
    let (hour, minute) = (secs_of_day / 3600, (secs_of_day % 3600) / 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "nihilurk-leaderboard-unit-{}-{tag}.sav",
                std::process::id()
            ))
            .to_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn a_recorded_run_round_trips_in_score_order() {
        let path = temp_path("roundtrip");
        let _ = std::fs::remove_file(&path);

        record_to(&path, "GUEST", Outcome::LoseDescent, 100, 1).unwrap();
        record_to(&path, "BAE", Outcome::Win, 4200, 2).unwrap();
        record_to(&path, "AL-OK", Outcome::LoseAscent, 900, 3).unwrap();

        let entries: Vec<_> = all(&path)
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.outcome, e.score))
            .collect();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            entries,
            vec![
                ("BAE".to_string(), Outcome::Win, 4200),
                ("AL-OK".to_string(), Outcome::LoseAscent, 900),
                ("GUEST".to_string(), Outcome::LoseDescent, 100),
            ]
        );
    }

    #[test]
    fn the_file_never_grows_past_the_store_limit() {
        let path = temp_path("cap");
        let _ = std::fs::remove_file(&path);

        for i in 0..(LEADERBOARD_STORE_LIMIT + 5) {
            record_to(&path, "P", Outcome::Win, i as i64, 0).unwrap();
        }

        let entries = all(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(entries.len(), LEADERBOARD_STORE_LIMIT);
        assert_eq!(entries.last().unwrap().score, 5);
    }

    #[test]
    fn a_missing_leaderboard_is_an_empty_one() {
        assert!(all(temp_path("missing")).unwrap().is_empty());
    }

    #[test]
    fn a_corrupt_leaderboard_is_refused_and_left_alone() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"\xff\xff\xff not a leaderboard").unwrap();

        assert!(all(&path).is_err());
        assert!(record_to(&path, "BAE", Outcome::Win, 1, 0).is_err());

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"\xff\xff\xff not a leaderboard"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_leaderboard_from_another_version_is_refused_and_left_alone() {
        let path = temp_path("version");
        let theirs: Vec<Entry> = Vec::new();
        let bytes = postcard::to_allocvec(&(LEADERBOARD_VERSION + 1, theirs)).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        assert!(record_to(&path, "BAE", Outcome::Win, 1, 0).is_err());

        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn runs_ending_at_once_all_make_the_board() {
        for round in 0..5 {
            let path = temp_path(&format!("race-{round}"));
            let _ = std::fs::remove_file(&path);
            let start = std::sync::Barrier::new(LEADERBOARD_STORE_LIMIT);

            std::thread::scope(|s| {
                for score in 0..LEADERBOARD_STORE_LIMIT as i64 {
                    let (path, start) = (&path, &start);
                    s.spawn(move || {
                        start.wait();
                        record_to(path, "P", Outcome::Win, score, 0).unwrap();
                    });
                }
            });

            let entries = all(&path).unwrap();
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(format!("{path}.lock"));
            assert_eq!(
                entries.len(),
                LEADERBOARD_STORE_LIMIT,
                "round {round} lost a score"
            );
        }
    }

    #[test]
    fn known_epoch_seconds_format_to_their_known_utc_date() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
        assert_eq!(format_timestamp(1_704_067_200), "2024-01-01 00:00");
        assert_eq!(format_timestamp(1_735_689_540), "2024-12-31 23:59");
    }
}
