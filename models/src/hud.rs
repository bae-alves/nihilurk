//! Presentation helpers shared by the renderer and the input handler so both
//! agree on exactly what the message log is showing this frame.

use crossterm::style::Color;

use crate::components::{LogCategory, LogEntry};

// Message-log sizing (rows shown, wrap widths). Defined and documented in
// `constants.rs`; re-exported so `hud::LOG_LINES` etc. keep resolving.
pub use crate::constants::hud::{LOG_LINES, LOG_MORE_WIDTH, LOG_WIDTH};

impl LogCategory {
    /// The colour this category alone implies. [`LogCategory::Pride`] is the
    /// one exception — [`log_paint`] stripes it instead of using this.
    fn color(self) -> Color {
        match self {
            LogCategory::Plain => Color::White,
            LogCategory::TrickShot | LogCategory::Combo | LogCategory::Dazzle => Color::Magenta,
            LogCategory::Curse => Color::DarkRed,
            LogCategory::Wounded => Color::Red,
            LogCategory::Haste => Color::Cyan,
            LogCategory::Slowed => Color::DarkCyan,
            LogCategory::Thrown => Color::Yellow,
            LogCategory::Pride => Color::White,
        }
    }
}

/// How one log message is painted.
///
/// Nearly everything is one colour for the whole message. `Striped` is the
/// exception the log has exactly one of: a line that is worth a rainbow gets a
/// colour per character, cycled — red through purple and straight back to red,
/// so no two neighbouring letters ever share a stripe.
pub enum LogPaint {
    Solid(Color),
    Striped(&'static [Color]),
}

impl LogPaint {
    /// The colour for character `i` of the message.
    pub fn color_at(&self, i: usize) -> Color {
        match self {
            LogPaint::Solid(c) => *c,
            LogPaint::Striped(stripes) => stripes[i % stripes.len()],
        }
    }
}

/// The one log line in the game that comes out in colours rather than a colour.
/// See [`crate::score`], which writes it on the tenth combo or so.
pub fn pride_line() -> &'static str {
    strings::with_pride()
}

/// How to paint one log message: solid, off its own [`LogCategory`], except
/// [`LogCategory::Pride`] which stripes with `stripes` — whatever flag the
/// run is flying (see [`crate::pride::stripes`]).
pub fn log_paint(message: &LogEntry, stripes: &'static [Color]) -> LogPaint {
    match message.category {
        LogCategory::Pride => LogPaint::Striped(stripes),
        other => LogPaint::Solid(other.color()),
    }
}

/// Packs `messages` into at most `max_lines` lines no wider than `width`, each
/// line left as the list of messages on it, in order. A message is never split:
/// one that doesn't fit on the current line starts the next, and one longer than
/// `width` gets its own overflowing line.
///
/// The lines come back as segments rather than strings because each message
/// keeps its own colour (see [`log_paint`]). Displayed, the segments on a line
/// are joined with a single space — so a segment's column is the widths of the
/// segments before it, plus one space each.
///
/// Returns the packed lines and how many messages they cover.
pub fn pack_line_segments(
    messages: &[LogEntry],
    width: usize,
    max_lines: usize,
) -> (Vec<Vec<LogEntry>>, usize) {
    let mut lines: Vec<Vec<LogEntry>> = Vec::new();
    let mut cur: Vec<LogEntry> = Vec::new();
    let mut cur_len = 0usize;
    let mut consumed = 0usize;

    for msg in messages {
        let msg_len = msg.chars().count();
        let would_be = match cur.is_empty() {
            true => msg_len,
            false => cur_len + 1 + msg_len,
        };

        if !cur.is_empty() && would_be > width {
            lines.push(std::mem::take(&mut cur));
            cur_len = 0;
            if lines.len() == max_lines {
                return (lines, consumed);
            }
        }

        if !cur.is_empty() {
            cur_len += 1;
        }
        cur.push(msg.clone());
        cur_len += msg_len;
        consumed += 1;
    }

    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    (lines, consumed)
}

/// What the message log should display: the packed lines — each still split
/// into the messages on it, so each can be painted in its own colour — how many
/// unread messages they cover, and whether a `--MORE--` prompt is required
/// because more messages are queued than fit.
pub fn log_view(unread: &[LogEntry]) -> (Vec<Vec<LogEntry>>, usize, bool) {
    let (lines, consumed) = pack_line_segments(unread, LOG_WIDTH, LOG_LINES);
    if consumed >= unread.len() {
        return (lines, consumed, false);
    }
    let (lines, consumed) = pack_line_segments(unread, LOG_MORE_WIDTH, LOG_LINES);
    (lines, consumed, true)
}
