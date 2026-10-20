//! The internal leaderboards, one per [`Board`]: a postcard blob at [`path`], in [`crate::data_dir`],
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
    /// The [`crate::conduct::Conduct`]s kept, a bit each. Only a win keeps any.
    conducts: u16,
}

/// An [`Entry`] as version 1 of the file wrote it, before conducts. Read, never
/// written: the first write after a read turns the whole board into version 2.
#[derive(Serialize, Deserialize)]
struct EntryV1 {
    name: String,
    outcome: Outcome,
    score: i64,
    epoch_secs: u64,
}

impl From<EntryV1> for Entry {
    fn from(old: EntryV1) -> Self {
        Entry {
            name: old.name,
            outcome: old.outcome,
            score: old.score,
            epoch_secs: old.epoch_secs,
            conducts: 0,
        }
    }
}

/// Which board a run is scored on. Each has its own file, so a six-floor run
/// is never ranked against a thirteen-floor one, nor an endless run against either.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Board {
    /// The ordinary run.
    Normal,
    /// `-fast`.
    Fast,
    /// `-endless`. The CLI refuses it together with `-fast`.
    Endless,
}

impl Board {
    /// The board for a run with these modes on.
    pub fn pick(endless: bool, fast: bool) -> Self {
        match (endless, fast) {
            (true, _) => Self::Endless,
            (false, true) => Self::Fast,
            (false, false) => Self::Normal,
        }
    }

    /// The board the run in `world` is scored on.
    pub fn of(world: &bevy_ecs::world::World) -> Self {
        let endless = world
            .get_resource::<crate::Endless>()
            .is_some_and(|e| e.enabled);
        Self::pick(endless, crate::Floors::of(world).last < crate::FINAL_DEPTH)
    }
}

/// Where a board lives in [`crate::data_dir`], next to the saves:
/// `leaderboard.sav`, `leaderboard-fast.sav` or `leaderboard-endless.sav`.
pub fn path(board: Board) -> PathBuf {
    crate::data_dir().join(match board {
        Board::Normal => "leaderboard.sav",
        Board::Fast => "leaderboard-fast.sav",
        Board::Endless => "leaderboard-endless.sav",
    })
}

/// The leaderboard file's format version, written ahead of the entries. A file
/// under any other version is refused, never overwritten, so a format change
/// can't wipe the board.
const LEADERBOARD_VERSION: u16 = 2;

/// The version before conducts. Still read; see [`EntryV1`].
const LEADERBOARD_V1: u16 = 1;

/// How many rows the file keeps, and `-scores` prints. A run that doesn't
/// beat the lowest of these is dropped on write rather than kept and hidden,
/// so the file never grows past this many entries.
pub const LEADERBOARD_STORE_LIMIT: usize = 10;

/// Records this run's outcome, keeping only the top [`LEADERBOARD_STORE_LIMIT`]
/// scores ever seen. Best-effort: a leaderboard nobody can write to (a
/// read-only directory) shouldn't stop the player's run from ending, so a
/// caller that cares can log the `Err` rather than propagate it.
pub fn record(
    board: Board,
    name: &str,
    outcome: Outcome,
    score: i64,
    conducts: u16,
) -> std::io::Result<()> {
    record_to(
        path(board),
        name,
        outcome,
        score,
        conducts,
        now_epoch_secs(),
    )
}

/// Seconds since the Unix epoch, UTC; 0 on a clock set before it.
pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn record_to(
    path: impl AsRef<Path>,
    name: &str,
    outcome: Outcome,
    score: i64,
    conducts: u16,
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
        conducts,
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
/// [`LEADERBOARD_VERSION`] (or the one before it, which is upgraded), is an `InvalidData` error: the scores in it are
/// still someone's, so nothing here starts a fresh board over them.
fn all(path: impl AsRef<Path>) -> std::io::Result<Vec<Entry>> {
    let bytes = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        read => read?,
    };
    let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidData);
    let (version, _) = postcard::take_from_bytes::<u16>(&bytes).map_err(|_| invalid())?;
    let mut entries = match version {
        LEADERBOARD_VERSION => postcard::from_bytes::<(u16, Vec<Entry>)>(&bytes)
            .map(|(_, entries)| entries)
            .map_err(|_| invalid()),
        LEADERBOARD_V1 => postcard::from_bytes::<(u16, Vec<EntryV1>)>(&bytes)
            .map(|(_, old)| old.into_iter().map(Entry::from).collect())
            .map_err(|_| invalid()),
        _ => Err(invalid()),
    }?;
    for entry in &mut entries {
        entry.name = crate::clean_name(&entry.name);
    }
    Ok(entries)
}

/// One leaderboard row, ready to print.
pub struct Standing {
    /// The name the run was played under.
    pub name: String,
    /// How the run ended.
    pub outcome: Outcome,
    /// The final score.
    pub score: i64,
    /// "YYYY-MM-DD HH:MM", UTC.
    pub when: String,
    /// The conducts kept, a bit each ([`crate::conduct::Conduct`]).
    pub conducts: u16,
}

/// The `limit` highest scores ever recorded, highest first, each with a
/// "YYYY-MM-DD HH:MM" UTC rendering of when the run ended. Errs the way
/// [`all`] does.
pub fn top(board: Board, limit: usize) -> std::io::Result<Vec<Standing>> {
    Ok(all(path(board))?
        .into_iter()
        .take(limit)
        .map(|e| Standing {
            name: e.name,
            outcome: e.outcome,
            score: e.score,
            when: format_timestamp(e.epoch_secs),
            conducts: e.conducts,
        })
        .collect())
}

/// Hand-rolled rather than pulling in a date crate for one stamp: this is
/// Howard Hinnant's `civil_from_days`, the standard proleptic-Gregorian
/// algorithm, fed only ever-increasing `SystemTime::now()` values (always
/// well past the epoch, never before it).
pub fn format_timestamp(epoch_secs: u64) -> String {
    let (year, month, day, hour, minute, _) = utc(epoch_secs);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// `YYYYMMDD-HHMMSS`, UTC: [`format_timestamp`] with the seconds kept and
/// nothing a file name can trip on, for the run dump's file name.
pub fn file_stamp(epoch_secs: u64) -> String {
    let (year, month, day, hour, minute, second) = utc(epoch_secs);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

fn utc(epoch_secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let secs_of_day = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days((epoch_secs / 86_400) as i64);
    (
        year,
        month,
        day,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    )
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
mod tests;
