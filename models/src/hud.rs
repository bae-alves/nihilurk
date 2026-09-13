//! Presentation helpers shared by the renderer and the input handler so both
//! agree on exactly what the message log is showing this frame.

use crossterm::style::Color;

// Message-log sizing (rows shown, wrap widths). Defined and documented in
// `constants.rs`; re-exported so `hud::LOG_LINES` etc. keep resolving.
pub use crate::constants::hud::{LOG_LINES, LOG_MORE_WIDTH, LOG_WIDTH};

/// Sparingly colours one log *message* — with two exceptions (a trick shot and
/// a combo, both magenta) only when it reads as happening *to the player*
/// (contains "you"), and only for a handful of categories worth
/// calling out: a curse taking hold (dark red), a dazzle (magenta), the
/// low-HP warning (red), the player's own speed shifting (cyan hasted, dark
/// cyan slowed), or the player throwing/firing something (yellow, to make it
/// read as juicier than an ordinary log line). Everything else stays the
/// plain log colour.
///
/// This is asked per message, never per painted line. Several messages share a
/// line ([`pack_line_segments`]) and each keeps its own colour — one shouting
/// message must not repaint the sentences that happen to sit beside it.
pub fn log_line_color(line: &str) -> Color {
    let lower = line.to_ascii_lowercase();
    // The two lines that shout before the "you" gate below. A trap going off
    // because something *shot* it is the player's doing whether or not the
    // sentence says so; and a combo's "With style." is the game applauding the
    // player in three words, none of which is "you".
    if lower.contains("trick shot") || lower.contains("with style") {
        return Color::Magenta;
    }
    if !lower.contains("you") {
        return Color::White;
    }
    if lower.contains("curse") {
        return Color::DarkRed;
    }
    if lower.contains("dazzl") {
        return Color::Magenta;
    }
    if lower.contains("wounded") {
        return Color::Red;
    }
    // The haste/slow messages ("the world lurches into slow motion around
    // you", "your limbs turn to lead", and the two "already as
    // quick/sluggish as you can be" refusals) are matched on their distinct
    // halves rather than a generic "fast"/"slow" — the haste line's own text
    // ironically contains "slow motion".
    if lower.contains("quick") || lower.contains("slow motion around you") {
        return Color::Cyan;
    }
    if lower.contains("sluggish") || lower.contains("limbs turn to lead") {
        return Color::DarkCyan;
    }
    if lower.contains("you throw") || lower.contains("you fire") {
        return Color::Yellow;
    }
    Color::White
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
pub const PRIDE_LINE: &str = "With pride.";

/// How to paint one log message: [`log_line_color`] for all but the one that
/// earns the flag, which is striped with `stripes` — whatever flag the run is
/// flying (see [`crate::pride::stripes`]).
pub fn log_paint(message: &str, stripes: &'static [Color]) -> LogPaint {
    if message.contains(PRIDE_LINE) {
        return LogPaint::Striped(stripes);
    }
    LogPaint::Solid(log_line_color(message))
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
    messages: &[String],
    width: usize,
    max_lines: usize,
) -> (Vec<Vec<String>>, usize) {
    let mut lines: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
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
pub fn log_view(unread: &[String]) -> (Vec<Vec<String>>, usize, bool) {
    let (lines, consumed) = pack_line_segments(unread, LOG_WIDTH, LOG_LINES);
    if consumed >= unread.len() {
        return (lines, consumed, false);
    }
    let (lines, consumed) = pack_line_segments(unread, LOG_MORE_WIDTH, LOG_LINES);
    (lines, consumed, true)
}
