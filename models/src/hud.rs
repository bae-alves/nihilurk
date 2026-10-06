//! Presentation helpers shared by the renderer and the input handler so both
//! agree on exactly what the message log is showing this frame.

use crossterm::style::Color;

use crate::components::{LogCategory, LogEntry};

// Message-log sizing (rows shown, wrap widths). Defined and documented in
// `constants.rs`; re-exported so `hud::LOG_LINES` etc. keep resolving.
pub use crate::constants::hud::{BAR_X, LOG_LINES, LOG_MORE_WIDTH, LOG_WIDTH, LOG_X};

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
            LogCategory::Ghost => Color::DarkGrey,
            LogCategory::Faerie => Color::Magenta,
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
    /// One colour for the whole message.
    Solid(Color),
    /// A colour per character, cycled.
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
/// See [`crate::score`], which writes it at
/// [`COMBO_PRIDE_CHANCE`](crate::constants::score::COMBO_PRIDE_CHANCE) odds.
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

/// What [`pack_line_segments`] fit on the screen, and how much of the
/// unread queue that used up.
pub struct Packed {
    /// The lines, each as the pieces of messages on it, in order.
    pub lines: Vec<Vec<LogEntry>>,
    /// Messages shown in full, from the front of the queue.
    pub consumed: usize,
    /// Words already shown of the message after those, which did not fit
    /// whole. `0` when nothing is split.
    pub partial_words: usize,
}

/// Packs `messages` into at most `max_lines` lines, each no wider than `width`
/// (the last one no wider than `last_width`, which leaves room for the
/// `--MORE--` prompt).
///
/// Messages share a line, joined by a single space. One that does not fit the
/// rest of its line wraps at a word boundary onto the next, so a long message
/// is split across lines and never run off the edge; a word wider than a whole
/// line gets its own line and overflows. Each piece keeps the message's
/// [`LogCategory`], so each is painted in the message's colour (see
/// [`log_paint`]).
///
/// When the lines run out mid-message, `consumed` stops before it and
/// `partial_words` says how far into it the screen got — [`acknowledge`] cuts
/// exactly that much, so no word is lost between pages.
pub fn pack_line_segments(
    messages: &[LogEntry],
    width: usize,
    last_width: usize,
    max_lines: usize,
) -> Packed {
    let mut lines: Vec<Vec<LogEntry>> = Vec::new();
    let mut cur: Vec<LogEntry> = Vec::new();
    let mut cur_len = 0usize;

    for (mi, msg) in messages.iter().enumerate() {
        let mut piece: Vec<&str> = Vec::new();
        for (wi, word) in msg.split_whitespace().enumerate() {
            let word_len = word.chars().count();
            let limit = match lines.len() + 1 == max_lines {
                true => last_width,
                false => width,
            };
            if cur_len > 0 && cur_len + 1 + word_len > limit {
                if !piece.is_empty() {
                    cur.push(LogEntry::tagged(piece.join(" "), msg.category));
                    piece.clear();
                }
                lines.push(std::mem::take(&mut cur));
                cur_len = 0;
                if lines.len() == max_lines {
                    return Packed {
                        lines,
                        consumed: mi,
                        partial_words: wi,
                    };
                }
            }
            cur_len += match cur_len {
                0 => word_len,
                _ => 1 + word_len,
            };
            piece.push(word);
        }
        if !piece.is_empty() {
            cur.push(LogEntry::tagged(piece.join(" "), msg.category));
        }
    }

    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    Packed {
        lines,
        consumed: messages.len(),
        partial_words: 0,
    }
}

/// The packing the log shows for `unread`, and whether it needs a `--MORE--`
/// prompt (more is queued than fits). When it does, the last line is
/// narrowed to leave room for the prompt.
fn log_pack(unread: &[LogEntry]) -> (Packed, bool) {
    let packed = pack_line_segments(unread, LOG_WIDTH, LOG_WIDTH, LOG_LINES);
    if packed.consumed >= unread.len() {
        return (packed, false);
    }
    (
        pack_line_segments(unread, LOG_WIDTH, LOG_MORE_WIDTH, LOG_LINES),
        true,
    )
}

/// What the message log should display: the packed lines — each still split
/// into the pieces on it, so each can be painted in its own colour — how many
/// unread messages they cover in full, and whether a `--MORE--` prompt is
/// required because more messages are queued than fit.
pub fn log_view(unread: &[LogEntry]) -> (Vec<Vec<LogEntry>>, usize, bool) {
    let (packed, more) = log_pack(unread);
    (packed.lines, packed.consumed, more)
}

/// The player has read this page: drop what [`log_view`] showed. Messages
/// shown in full go; a message cut mid-way keeps only the words not yet shown.
pub fn acknowledge(unread: &mut Vec<LogEntry>) {
    let (packed, _) = log_pack(unread);
    unread.drain(0..packed.consumed);
    if packed.partial_words > 0 {
        if let Some(first) = unread.first_mut() {
            let rest: Vec<&str> = first
                .split_whitespace()
                .skip(packed.partial_words)
                .collect();
            first.text = rest.join(" ");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(lines: &[Vec<LogEntry>]) -> String {
        lines
            .iter()
            .flatten()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A message wider than the log wraps onto the next line instead of
    /// running off the edge, loses no word, and keeps its category.
    #[test]
    fn a_long_message_wraps_onto_the_next_line() {
        let words: Vec<String> = (0..12).map(|i| format!("word{i:02}")).collect();
        let long = words.join(" ");
        assert!(long.len() > LOG_WIDTH && long.len() < 2 * LOG_WIDTH - 10);
        let unread = vec![LogEntry::tagged(long.clone(), LogCategory::Wounded)];

        let (lines, consumed, more) = log_view(&unread);

        assert_eq!(lines.len(), 2, "it should take both lines");
        assert!(lines.iter().flatten().all(|e| e.text.len() <= LOG_WIDTH));
        assert!(
            lines
                .iter()
                .flatten()
                .all(|e| e.category == LogCategory::Wounded)
        );
        assert_eq!(text_of(&lines), long);
        assert_eq!((consumed, more), (1, false));
    }

    /// A message too long for what is left on the screen is split across
    /// pages: acknowledging keeps the unshown rest as the next unread, so
    /// no word is ever lost to the pager.
    #[test]
    fn acknowledging_keeps_the_unshown_rest_of_a_split_message() {
        let words: Vec<String> = (0..40).map(|i| format!("word{i:02}")).collect();
        let long = words.join(" ");
        let mut unread = vec![LogEntry::tagged(long.clone(), LogCategory::Curse)];

        let mut seen: Vec<String> = Vec::new();
        for _ in 0..10 {
            let (lines, _, more) = log_view(&unread);
            seen.push(text_of(&lines));
            if !more {
                unread.clear();
                break;
            }
            acknowledge(&mut unread);
            assert!(!unread.is_empty(), "the rest of the message vanished");
            assert!(unread[0].category == LogCategory::Curse);
        }
        assert!(unread.is_empty(), "the pager never finished");
        assert!(seen.len() > 1, "the fixture fits on one page");
        assert_eq!(seen.join(" "), long, "words were lost or repeated");
    }

    /// Short messages still share a line, as before.
    #[test]
    fn short_messages_still_share_a_line() {
        let unread = vec![LogEntry::plain("You hit."), LogEntry::plain("It dies.")];
        let (lines, consumed, more) = log_view(&unread);
        assert_eq!(
            (lines.len(), lines[0].len(), consumed, more),
            (1, 2, 2, false)
        );
    }
}
