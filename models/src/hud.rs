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

/// Greedily packs `messages` into at most `max_lines` lines no wider than
/// `width`, joining consecutive messages with a single space. A message is never
/// split: if it doesn't fit on the current line it starts the next one (and a
/// message longer than `width` simply occupies its own overflowing line).
///
/// Returns the packed lines and how many messages they cover.
///
/// A convenience over [`pack_line_segments`] for callers that only want the
/// text: it joins each line's messages back together with the space they are
/// displayed with. The renderer wants the segments instead, because each
/// message keeps its own colour.
pub fn pack_messages(messages: &[String], width: usize, max_lines: usize) -> (Vec<String>, usize) {
    let (lines, consumed) = pack_line_segments(messages, width, max_lines);
    (lines.iter().map(|line| line.join(" ")).collect(), consumed)
}

/// The same packing, with each line left as the list of messages on it, in
/// order. Displayed they are joined with a single space — so a segment's column
/// is the widths of the segments before it, plus one space each.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn short_messages_share_one_line() {
        let (lines, consumed) = pack_messages(&v(&["You hit the orc.", "It dies."]), 80, 3);
        assert_eq!(lines, vec!["You hit the orc. It dies."]);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn wraps_whole_message_never_splits_it() {
        let (lines, consumed) = pack_messages(&v(&["aaaaaa", "bbbbbb", "cccccc"]), 13, 3);
        // "aaaaaa bbbbbb" == 13 fits; "cccccc" would push to 20 -> next line, intact.
        assert_eq!(lines, vec!["aaaaaa bbbbbb", "cccccc"]);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn stops_at_max_lines_and_reports_consumed() {
        let msgs = v(&["one", "two", "three", "four", "five", "six", "seven"]);
        let (lines, consumed) = pack_messages(&msgs, 3, 3);
        assert_eq!(lines, vec!["one", "two", "three"]);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn log_view_flags_more_when_queue_overflows() {
        let long = "x".repeat(50);
        let msgs = vec![long.clone(), long.clone(), long.clone(), long.clone()];
        let (lines, consumed, more) = log_view(&msgs);
        assert!(more);
        assert!(consumed < msgs.len());
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn log_view_no_more_when_everything_fits() {
        let (_lines, consumed, more) = log_view(&v(&["a", "b", "c"]));
        assert!(!more);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn colors_only_apply_to_lines_about_the_player() {
        assert_eq!(
            log_line_color("The goblin is dazzled!"),
            Color::White,
            "no \"you\" in it, so it stays plain even though it's a dazzle line"
        );
        assert_eq!(
            log_line_color("The rat is cursed!"),
            Color::White,
            "no \"you\" in it, so it stays plain even though it's a curse line"
        );
    }

    #[test]
    fn curse_dazzle_and_low_hp_lines_get_their_colours() {
        assert_eq!(
            log_line_color("The ring welds itself to your grip! It is cursed!"),
            Color::DarkRed
        );
        assert_eq!(
            log_line_color("The flash leaves you reeling — you are dazzled!"),
            Color::Magenta
        );
        assert_eq!(log_line_color("You are badly wounded!"), Color::Red);
    }

    #[test]
    fn a_shouting_message_never_repaints_the_ones_beside_it() {
        // The bug this guards: a line is several messages, and colouring used
        // to be decided for the whole painted row. One combo would turn every
        // sentence sharing its row magenta.
        let (lines, _) =
            pack_line_segments(&v(&["You hit the orc for 3 damage.", "With style."]), 80, 3);
        assert_eq!(lines.len(), 1, "they share a row");
        let colors: Vec<Color> = lines[0].iter().map(|m| log_line_color(m)).collect();
        assert_eq!(colors, vec![Color::White, Color::Magenta]);
    }

    #[test]
    fn the_proud_line_comes_out_in_stripes_that_never_repeat_side_by_side() {
        let flag = crate::pride::PrideFlag::default_flag().stripes;
        let paint = log_paint(PRIDE_LINE, flag);
        let painted: Vec<Color> = (0..PRIDE_LINE.len()).map(|i| paint.color_at(i)).collect();
        assert_eq!(painted[0], flag[0], "opens on red");
        assert_eq!(
            painted[6], flag[0],
            "and wraps from purple straight back to it"
        );
        assert!(
            painted.windows(2).all(|w| w[0] != w[1]),
            "no two neighbouring letters share a stripe"
        );
    }

    #[test]
    fn a_combo_shouts_in_magenta_with_no_you_in_it() {
        assert_eq!(log_line_color("With style."), Color::Magenta);
    }

    #[test]
    fn a_trick_shot_shouts_in_magenta_with_no_you_in_it() {
        // The one line coloured without the player being named in it: it is
        // their shot either way.
        assert_eq!(log_line_color("BAM! Trick shot!"), Color::Magenta);
        assert_eq!(log_line_color("WHY! Trick shot!"), Color::Magenta);
    }

    #[test]
    fn the_haste_lines_beat_the_word_slow_in_their_own_text() {
        // The haste message ironically contains "slow motion" — it must still
        // read as the fast colour, not the slow one.
        assert_eq!(
            log_line_color("The world lurches into slow motion around you."),
            Color::Cyan
        );
        assert_eq!(
            log_line_color("You are already as quick as you can be."),
            Color::Cyan
        );
        assert_eq!(log_line_color("Your limbs turn to lead."), Color::DarkCyan);
        assert_eq!(
            log_line_color("You are already as sluggish as you can be."),
            Color::DarkCyan
        );
    }

    #[test]
    fn the_players_own_throw_is_yellow() {
        assert_eq!(log_line_color("You throw the dagger."), Color::Yellow);
        assert_eq!(log_line_color("You fire an arrow."), Color::Yellow);
        assert_eq!(
            log_line_color("The orc throws a dagger."),
            Color::White,
            "not the player's own throw"
        );
    }
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
