//! State of the run as a whole: the message log, the depth, floor changes, the kill
//! tally, and the Dungeon Lord.

use super::*;

// ===========================================================================
// Run state (resources)
// ===========================================================================

/// Why a log line is painted the way it is — decided once, by whoever writes
/// the message, and carried on [`LogEntry`] from then on. Guessing a line's
/// category back out of its rendered text would break the moment a message's
/// wording changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogCategory {
    /// No special paint: the default colour.
    #[default]
    Plain,
    /// A trick shot's shout — the ordinary one and the ULTIMATE one alike.
    TrickShot,
    /// A combo kill's "With style.", and the ring of adornment's own line of
    /// the same six words — the same feat, so the same colour.
    Combo,
    /// A combo kill lucky enough to earn the flag instead — striped, not
    /// solid; see `hud::log_paint`.
    Pride,
    /// A curse taking hold or letting go.
    Curse,
    /// A dazzle landing on the player.
    Dazzle,
    /// The player crossing the low-HP threshold.
    Wounded,
    /// The player's own tempo shifting faster.
    Haste,
    /// The player's own tempo shifting slower.
    Slowed,
    /// The player throwing or firing something.
    Thrown,
    /// A faerie shapeshifter's reveal, the dog's death (see
    /// `crate::combat`'s `reveal_faerie`). Pink.
    Faerie,
    /// A special or dark room's entry line, painted in that room's wall colour.
    Room(Color),
}

/// One line for the message log: its text, and the [`LogCategory`] it was
/// written with. Derefs to `str` and compares equal to one, so most existing
/// callers that only care about the text (a test's `.contains(...)`) never
/// have to know this wraps anything.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The words.
    pub text: String,
    /// How it is painted.
    pub category: LogCategory,
}

impl LogEntry {
    /// A line with no special paint ([`LogCategory::Plain`]).
    pub fn plain<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            category: LogCategory::Plain,
        }
    }

    /// A line painted as `category`.
    ///
    /// ```
    /// use models::{LogCategory, LogEntry};
    ///
    /// let line = LogEntry::tagged("Bang!", LogCategory::TrickShot);
    /// assert!(line == *"Bang!");
    /// assert_eq!(line.category, LogCategory::TrickShot);
    /// ```
    pub fn tagged<S: Into<String>>(text: S, category: LogCategory) -> Self {
        Self {
            text: text.into(),
            category,
        }
    }
}

impl std::ops::Deref for LogEntry {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl PartialEq<str> for LogEntry {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

/// The message log: everything that has happened (`history`, capped at [`LOG_HISTORY_CAP`]) and
/// everything the player has not yet acknowledged with `--MORE--` (`unread`).
/// `history` is plain text — nothing ever colours the scrollback — while
/// `unread` is what's actually painted, so it keeps each line's [`LogCategory`].
#[derive(Resource)]
pub struct GameLog {
    /// Everything that has happened, as plain text.
    pub history: Vec<String>,
    /// What the player has not yet acknowledged, with each line's category.
    pub unread: Vec<LogEntry>, // The queue of messages waiting for a --MORE-- acknowledgment
}

impl Default for GameLog {
    fn default() -> Self {
        let unread = vec![LogEntry::plain(strings::welcome_new_run())];
        Self {
            history: Vec::new(),
            unread,
        }
    }
}

impl GameLog {
    /// A plain (white) log line — everything that isn't one of the handful of
    /// categories [`GameLog::add_colored`] exists for.
    pub fn add<S: Into<String>>(&mut self, message: S) {
        self.add_colored(message, LogCategory::Plain);
    }

    /// A log line tagged with the category that decides how it's painted —
    /// set here, at the message's origin, never guessed at later from its text.
    pub fn add_colored<S: Into<String>>(&mut self, message: S, category: LogCategory) {
        let msg = message.into();
        self.history.push(msg.clone());
        self.unread.push(LogEntry::tagged(msg, category));

        if self.history.len() > LOG_HISTORY_CAP {
            self.history.remove(0);
        }
    }

    /// Drops every unread line since the first unread trick-shot shout except
    /// the shouts themselves, from the history too: a combo the shot earned
    /// says it all, and the deaths, the clatter and the coins between are noise.
    pub fn fold_into_trick_shot(&mut self) {
        let Some(start) = self
            .unread
            .iter()
            .position(|e| e.category == LogCategory::TrickShot)
        else {
            return;
        };
        let since = &self.unread[start + 1..];
        let from = self.history.len().saturating_sub(since.len());
        if self.history[from..]
            .iter()
            .eq(since.iter().map(|e| &e.text))
        {
            let shouts = since
                .iter()
                .filter(|e| e.category == LogCategory::TrickShot)
                .map(|e| e.text.clone())
                .collect::<Vec<_>>();
            self.history.truncate(from);
            self.history.extend(shouts);
        }
        let mut at = 0;
        self.unread.retain(|e| {
            at += 1;
            at <= start + 1 || e.category == LogCategory::TrickShot
        });
    }
}

/// The current dungeon floor, 1-based.
#[derive(Resource)]
pub struct Depth {
    /// The floor number, counted down from the top.
    pub what: u8,
}

/// How many times the player has moved between floors this run — every
/// staircase, portal and trapdoor bumps it by one. It salts
/// [`crate::map::content_rng`], so a floor's *layout* is still a pure function
/// of `(seed, depth)` but its *contents* are re-rolled every time it is built:
/// climb back up and the corridors you remember are stocked with different
/// monsters and loot. Saved, so a reload lands on the same re-roll.
#[derive(Resource, Default)]
pub struct FloorChanges {
    /// Staircases, portals and trapdoors taken so far this run.
    pub count: u32,
}

/// Every corpse this run has produced, counted under the name it died with —
/// the same words the log used, so a disguise is tallied as
/// what the player saw. Bumped wherever the score pays for a corpse, so a
/// helper's death is no kill. Saved, and read by the run dump.
#[derive(Resource, Default)]
pub struct Kills {
    /// How many of each, by name.
    pub by_name: std::collections::BTreeMap<String, u32>,
}

/// Tracks how long the player has lingered on one dungeon level. Every turn adds
/// one; every level change resets it to zero. When it reaches
/// [`crate::map::DUNGEON_LORD_PATIENCE`] the Dungeon Lord opens a portal under
/// the player's feet and shunts them to the next level. Transient, never saved.
#[derive(Resource, Default)]
pub struct DungeonLord {
    /// Turns spent on this floor since arriving.
    pub idle_turns: u32,
}
