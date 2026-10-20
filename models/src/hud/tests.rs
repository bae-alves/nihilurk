use super::*;

fn text_of(lines: &[Vec<LogEntry>]) -> String {
    lines
        .iter()
        .flatten()
        .map(|e| e.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
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
        let (lines, _, more) = log_view(&unread, LOG_WIDTH);
        seen.push(text_of(&lines));
        if !more {
            unread.clear();
            break;
        }
        acknowledge(&mut unread, LOG_WIDTH);
        assert!(!unread.is_empty(), "the rest of the message vanished");
        assert!(unread[0].category == LogCategory::Curse);
    }
    assert!(unread.is_empty(), "the pager never finished");
    assert!(seen.len() > 1, "the fixture fits on one page");
    assert_eq!(seen.join(" "), long, "words were lost or repeated");
}
