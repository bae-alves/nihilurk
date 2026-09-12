//! Presentation helpers shared by the renderer and the input handler so both
//! agree on exactly what the message log is showing this frame.

use crossterm::style::Color;

// Message-log sizing (rows shown, wrap widths). Defined and documented in
// `constants.rs`; re-exported so `hud::LOG_LINES` etc. keep resolving.
pub use crate::constants::hud::{LOG_LINES, LOG_MORE_WIDTH, LOG_WIDTH};

/// Sparingly colours a packed log line — with one exception (a trick shot,
/// magenta) only when it reads as happening *to the player* (contains "you"),
/// and only for a handful of categories worth
/// calling out: a curse taking hold (dark red), a dazzle (magenta), the
/// low-HP warning (red), the player's own speed shifting (cyan hasted, dark
/// cyan slowed), or the player throwing/firing something (yellow, to make it
/// read as juicier than an ordinary log line). Everything else stays the
/// plain log colour. A packed line can join several original messages (see
/// [`pack_messages`]); if any of them matches, the whole line takes that
/// colour.
pub fn log_line_color(line: &str) -> Color {
    let lower = line.to_ascii_lowercase();
    // The one line that shouts before the "you" gate below: a trap going off
    // because something *shot* it is the player's doing whether or not the
    // sentence says so, and it is worth seeing from across the room.
    if lower.contains("trick shot") {
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

/// Greedily packs `messages` into at most `max_lines` lines no wider than
/// `width`, joining consecutive messages with a single space. A message is never
/// split: if it doesn't fit on the current line it starts the next one (and a
/// message longer than `width` simply occupies its own overflowing line).
///
/// Returns the packed lines and how many messages they cover.
pub fn pack_messages(messages: &[String], width: usize, max_lines: usize) -> (Vec<String>, usize) {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;
    let mut consumed = 0usize;

    for msg in messages {
        let msg_len = msg.chars().count();
        let would_be = if cur.is_empty() {
            msg_len
        } else {
            cur_len + 1 + msg_len
        };

        if !cur.is_empty() && would_be > width {
            lines.push(std::mem::take(&mut cur));
            cur_len = 0;
            if lines.len() == max_lines {
                return (lines, consumed);
            }
        }

        if !cur.is_empty() {
            cur.push(' ');
            cur_len += 1;
        }
        cur.push_str(msg);
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

/// What the message log should display: the packed lines, how many unread
/// messages they cover, and whether a `--MORE--` prompt is required because more
/// messages are queued than fit.
pub fn log_view(unread: &[String]) -> (Vec<String>, usize, bool) {
    let (lines, consumed) = pack_messages(unread, LOG_WIDTH, LOG_LINES);
    if consumed >= unread.len() {
        return (lines, consumed, false);
    }
    let (lines, consumed) = pack_messages(unread, LOG_MORE_WIDTH, LOG_LINES);
    (lines, consumed, true)
}
