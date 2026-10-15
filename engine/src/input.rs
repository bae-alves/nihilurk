//! The one door input comes through, so a run can be written down and played
//! back.
//!
//! Every `poll` and `read` in the engine goes through here. Live, they pass
//! to crossterm and log each key press and mouse click that the game acts on.
//! In replay, they answer from a log instead. Each logged event carries a
//! *stamp*: how many calls into this module had been made when the event first
//! showed up (a `poll` that said yes, or a `read` that had to wait for it).
//! Replay says yes to a `poll` on exactly the same call, so an auto-walk that
//! a keypress cut short is cut short at the same step.
//!
//! Only presses and clicks are logged. Releases, resizes and mouse moves never
//! change the game, and logging them would make the stream depend on the
//! terminal.

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{ErrorKind, Write};
use std::time::Duration;

/// How long replay holds on each key the log hands back from a blocking read,
/// so a watcher can follow the run.
const REPLAY_KEY_MS: u64 = 120;

#[derive(Default)]
struct Stream {
    calls: u64,
    pending: Option<Event>,
    recorder: Option<Box<dyn Write>>,
    replay: Option<VecDeque<(u64, Event)>>,
    ended: bool,
}

thread_local! {
    static STREAM: RefCell<Stream> = RefCell::new(Stream::default());
}

/// Starts logging every event the game acts on to `out`, one line each.
pub fn record_to(out: Box<dyn Write>) {
    STREAM.with(|s| s.borrow_mut().recorder = Some(out));
}

/// Answers every later `poll` and `read` from `events` instead of the terminal.
pub fn replay_from(events: Vec<(u64, Event)>) {
    STREAM.with(|s| s.borrow_mut().replay = Some(events.into()));
}

fn recordable(event: &Event) -> bool {
    match event {
        Event::Key(k) => k.kind == KeyEventKind::Press,
        Event::Mouse(m) => matches!(
            m.kind,
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right)
        ),
        _ => false,
    }
}

/// Logs `event` with `stamp` and holds it for the next `poll` or `read`.
fn arrive(event: Event, stamp: u64) {
    STREAM.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(out) = s.recorder.as_mut() {
            let _ = writeln!(out, "{}", encode(stamp, &event));
            let _ = out.flush();
        }
        s.pending = Some(event);
    });
}

fn calls_so_far() -> u64 {
    STREAM.with(|s| s.borrow().calls)
}

/// Holds `event` for the next call, stamped as that call would have stamped
/// it, so looking ahead leaves the log as it would be without the look.
/// Whether it was one worth holding.
fn hold_ahead(event: Event) -> bool {
    if !recordable(&event) {
        return false;
    }
    arrive(event, calls_so_far() + 1);
    true
}

/// Whether the next `poll` or `read` gets an event at once. Not a call itself:
/// the stamps stay as they were. Live, it drops what `read` would skip and
/// holds a key or click for the next call, which the caller is about to make.
/// Replay always has the next event, or the Ctrl+C that ends the log.
pub fn waiting() -> std::io::Result<bool> {
    let ready = STREAM.with(|s| {
        let s = s.borrow();
        s.pending.is_some() || s.replay.is_some()
    });
    if ready {
        return Ok(true);
    }
    while crossterm::event::poll(Duration::ZERO)? {
        if hold_ahead(crossterm::event::read()?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether an event is waiting, giving the player up to `wait` to produce one.
/// Replay never asks the terminal: when the log has nothing for this call it
/// sleeps `wait` out, which keeps animations playing at their own pace.
pub fn poll(wait: Duration) -> std::io::Result<bool> {
    let from_log = STREAM.with(|s| {
        let mut s = s.borrow_mut();
        s.calls += 1;
        if s.pending.is_some() {
            return Some(true);
        }
        let calls = s.calls;
        let due = s
            .replay
            .as_ref()?
            .front()
            .is_some_and(|&(stamp, _)| stamp <= calls);
        if due {
            s.pending = s.replay.as_mut()?.pop_front().map(|(_, e)| e);
        }
        Some(due)
    });
    match from_log {
        Some(true) => return Ok(true),
        Some(false) => {
            std::thread::sleep(wait);
            return Ok(false);
        }
        None => {}
    }
    let started = std::time::Instant::now();
    loop {
        let left = wait.saturating_sub(started.elapsed());
        if !crossterm::event::poll(left)? {
            return Ok(false);
        }
        let event = crossterm::event::read()?;
        if recordable(&event) {
            arrive(event, calls_so_far());
            return Ok(true);
        }
    }
}

/// The next event, waiting for it if need be. In replay, the end of the log
/// reads once as Ctrl+C, which ends the run, and as `UnexpectedEof` after that.
pub fn read() -> std::io::Result<Event> {
    let (held, replaying) = STREAM.with(|s| {
        let mut s = s.borrow_mut();
        s.calls += 1;
        let mut held = s.pending.take();
        let replaying = s.replay.is_some();
        if held.is_none() {
            held = s
                .replay
                .as_mut()
                .and_then(|q| q.pop_front())
                .map(|(_, e)| e);
        }
        (held, replaying)
    });
    if replaying {
        let event = match held {
            Some(event) => event,
            None if !STREAM.with(|s| std::mem::replace(&mut s.borrow_mut().ended, true)) => {
                Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
            }
            None => return Err(std::io::Error::from(ErrorKind::UnexpectedEof)),
        };
        std::thread::sleep(Duration::from_millis(REPLAY_KEY_MS));
        return Ok(event);
    }
    if let Some(event) = held {
        return Ok(event);
    }
    loop {
        let event = crossterm::event::read()?;
        if recordable(&event) {
            arrive(event, calls_so_far());
            return Ok(STREAM.with(|s| s.borrow_mut().pending.take().expect("arrive set it")));
        }
    }
}

fn encode_code(code: KeyCode) -> String {
    match code {
        KeyCode::Char(c) => format!("c{}", c as u32),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    }
}

fn decode_code(text: &str) -> Option<KeyCode> {
    if let Some(n) = text.strip_prefix('c') {
        return char::from_u32(n.parse().ok()?).map(KeyCode::Char);
    }
    if let Some(n) = text.strip_prefix('F') {
        return n.parse().ok().map(KeyCode::F);
    }
    Some(match text {
        "Backspace" => KeyCode::Backspace,
        "Enter" => KeyCode::Enter,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Tab" => KeyCode::Tab,
        "BackTab" => KeyCode::BackTab,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Esc" => KeyCode::Esc,
        _ => return None,
    })
}

/// One log line for `event`, stamped with the call it arrived on. Only events
/// that [`recordable`] accepts have a line.
fn encode(stamp: u64, event: &Event) -> String {
    match event {
        Event::Key(k) => format!("{stamp} k {} {}", encode_code(k.code), k.modifiers.bits()),
        Event::Mouse(m) => {
            let button = match m.kind {
                MouseEventKind::Down(MouseButton::Right) => 'R',
                _ => 'L',
            };
            format!(
                "{stamp} m {button} {} {} {}",
                m.column,
                m.row,
                m.modifiers.bits()
            )
        }
        _ => String::new(),
    }
}

/// The event on one log line, or `None` for a line that is not one.
pub fn decode(line: &str) -> Option<(u64, Event)> {
    let mut parts = line.split(' ');
    let stamp = parts.next()?.parse().ok()?;
    let kind = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let event = match (kind, rest.as_slice()) {
        ("k", [code, mods]) => {
            let mods = KeyModifiers::from_bits(mods.parse().ok()?)?;
            Event::Key(KeyEvent::new(decode_code(code)?, mods))
        }
        ("m", [button, column, row, mods]) => {
            let button = match *button {
                "L" => MouseButton::Left,
                "R" => MouseButton::Right,
                _ => return None,
            };
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(button),
                column: column.parse().ok()?,
                row: row.parse().ok()?,
                modifiers: KeyModifiers::from_bits(mods.parse().ok()?)?,
            })
        }
        _ => return None,
    };
    Some((stamp, event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Sink(Rc<RefCell<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn click() -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: 12,
            row: 7,
            modifiers: KeyModifiers::SHIFT,
        })
    }

    fn reset() {
        STREAM.with(|s| *s.borrow_mut() = Stream::default());
    }

    #[test]
    fn every_recordable_event_survives_its_log_line() {
        let events = [
            key('h'),
            key('é'),
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::ALT)),
            Event::Key(KeyEvent::new(KeyCode::F(5), KeyModifiers::CONTROL)),
            click(),
        ];
        for (i, event) in events.iter().enumerate() {
            let line = encode(i as u64, event);
            assert_eq!(decode(&line), Some((i as u64, event.clone())), "{line}");
        }
    }

    #[test]
    fn a_garbled_line_is_no_event() {
        for line in [
            "",
            "x",
            "3 k",
            "3 k c104",
            "3 k Nope 0",
            "3 m Q 1 1 0",
            "a k c104 0",
        ] {
            assert_eq!(decode(line), None, "{line:?}");
        }
    }

    #[test]
    fn replay_says_yes_to_a_poll_on_the_call_that_said_yes_live() {
        reset();
        let log = vec![(2, key('a')), (4, key('b')), (7, key('c'))];
        replay_from(log);
        let mut seen = Vec::new();
        for _ in 0..5 {
            let yes = poll(Duration::ZERO).unwrap();
            seen.push(yes);
            if yes {
                read().unwrap();
            }
        }
        assert_eq!(seen, [false, true, true, false, true]);
        reset();
    }

    #[test]
    fn the_end_of_the_log_reads_as_ctrl_c_then_eof() {
        reset();
        replay_from(vec![]);
        let Event::Key(last) = read().unwrap() else {
            panic!("the log's end should read as a key");
        };
        assert_eq!(last.code, KeyCode::Char('c'));
        assert!(last.modifiers.contains(KeyModifiers::CONTROL));
        let err = read().unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
        reset();
    }

    #[test]
    fn an_event_arrives_on_the_call_that_found_it() {
        reset();
        let sink = Sink::default();
        record_to(Box::new(sink.clone()));
        arrive(key('x'), 4);
        arrive(click(), 4);
        let text = String::from_utf8(sink.0.borrow().clone()).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines, ["4 k c120 0", "4 m R 12 7 1"]);
        reset();
    }

    #[test]
    fn waiting_sees_a_key_a_poll_already_holds_and_is_no_call() {
        reset();
        STREAM.with(|s| s.borrow_mut().pending = Some(key('a')));
        assert!(waiting().unwrap());
        assert_eq!(calls_so_far(), 0);
        reset();
    }

    #[test]
    fn waiting_in_replay_is_yes_and_leaves_the_stamps_alone() {
        reset();
        replay_from(vec![(2, key('a')), (4, key('b')), (7, key('c'))]);
        let mut seen = Vec::new();
        for _ in 0..5 {
            assert!(waiting().unwrap());
            let yes = poll(Duration::ZERO).unwrap();
            seen.push(yes);
            if yes {
                read().unwrap();
            }
        }
        assert_eq!(seen, [false, true, true, false, true]);
        reset();
    }

    #[test]
    fn a_key_held_ahead_is_stamped_for_the_call_that_takes_it() {
        reset();
        let sink = Sink::default();
        record_to(Box::new(sink.clone()));
        STREAM.with(|s| s.borrow_mut().calls = 4);
        assert!(hold_ahead(key('x')));
        assert_eq!(read().unwrap(), key('x'));
        assert_eq!(calls_so_far(), 5);
        let text = String::from_utf8(sink.0.borrow().clone()).unwrap();
        assert_eq!(text.lines().collect::<Vec<_>>(), ["5 k c120 0"]);
        reset();
    }

    #[test]
    fn what_read_would_skip_is_not_held_ahead() {
        reset();
        let sink = Sink::default();
        record_to(Box::new(sink.clone()));
        let release = Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ));
        assert!(!hold_ahead(release));
        assert!(!hold_ahead(Event::Resize(80, 25)));
        assert!(STREAM.with(|s| s.borrow().pending.is_none()));
        assert!(sink.0.borrow().is_empty());
        reset();
    }
}
