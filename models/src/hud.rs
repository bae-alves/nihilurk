//! Presentation helpers shared by the renderer and the input handler so both
//! agree on exactly what the message log is showing this frame.

/// Rows available to the message log.
pub const LOG_LINES: usize = 3;
/// Column budget for a normal packed log line.
pub const LOG_WIDTH: usize = 80;
/// Narrower budget for the last line when a `--MORE--` prompt has to fit.
pub const LOG_MORE_WIDTH: usize = 56;

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

        if cur.is_empty() {
            cur.push_str(msg);
            cur_len = msg_len;
        } else {
            cur.push(' ');
            cur.push_str(msg);
            cur_len += 1 + msg_len;
        }
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
