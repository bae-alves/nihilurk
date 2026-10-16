//! The one door input comes through, so a run can be written down and played
//! back.
//!
//! Every `poll` and `read` in the engine goes through here. Live, they pass
//! to [`crate::term`] and log each key press and mouse click that the game
//! acts on.
//! In replay, they answer from a log instead. Each logged event carries a
//! *stamp*: how many calls into this module had been made when the event first
//! showed up (a `poll` that said yes, or a `read` that had to wait for it).
//! Replay says yes to a `poll` on exactly the same call, so an auto-walk that
//! a keypress cut short is cut short at the same step.
//!
//! Only presses and clicks are logged. Releases, resizes and mouse moves never
//! change the game, and logging them would make the stream depend on the
//! terminal: [`crate::term`] drops them before an event reaches this module.
//!
//! The event types are the game's own, so nothing above the terminal names a
//! terminal library. Their names and shapes follow what `update` matches on.

use crate::term;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{ErrorKind, Write};
use std::time::Duration;

/// A key, as far as the game tells them apart.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key, as typed (shifted keys arrive already shifted).
    Char(char),
    /// A function key, `F(1)` to `F(12)` and beyond.
    F(u8),
    /// Backspace.
    Backspace,
    /// Enter or Return.
    Enter,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Tab.
    Tab,
    /// Shift+Tab.
    BackTab,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Escape.
    Esc,
}

/// The modifier keys held with an event. The bits are the ones recordings
/// hold: Shift 1, Control 2, Alt 4. Any other bit a log line carries is dropped.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// No modifier.
    #[cfg(test)]
    pub const NONE: Self = Self(0);
    /// Shift.
    pub const SHIFT: Self = Self(1);
    /// Control.
    pub const CONTROL: Self = Self(2);
    /// Alt.
    pub const ALT: Self = Self(4);

    /// Whether every modifier in `other` is held.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The modifiers as a recording writes them.
    pub fn bits(self) -> u8 {
        self.0
    }

    /// The modifiers in `bits`, ignoring any bit the game has no modifier for.
    pub fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0b111)
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A key press.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Which key.
    pub code: KeyCode,
    /// The modifiers held with it.
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    /// A press of `code` with `modifiers` held.
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
}

/// The mouse buttons the game acts on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// The left button.
    Left,
    /// The right button.
    Right,
}

/// A mouse button going down, at a cell of the terminal.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    /// Which button went down.
    pub button: MouseButton,
    /// The column, counted from 0.
    pub column: u16,
    /// The row, counted from 0.
    pub row: u16,
    /// The modifiers held with it.
    pub modifiers: KeyModifiers,
}

/// What the player did that the game acts on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// A key went down.
    Key(KeyEvent),
    /// A mouse button went down.
    Mouse(MouseEvent),
}

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
fn hold_ahead(event: Event) {
    arrive(event, calls_so_far() + 1);
}

/// Whether the next `poll` or `read` gets an event at once. Not a call itself:
/// the stamps stay as they were. Live, it drops what the terminal reports that
/// is no key or click and holds a key or click for the next call, which the
/// caller is about to make.
/// Replay always has the next event, or the Ctrl+C that ends the log.
pub fn waiting() -> std::io::Result<bool> {
    let ready = STREAM.with(|s| {
        let s = s.borrow();
        s.pending.is_some() || s.replay.is_some()
    });
    if ready {
        return Ok(true);
    }
    while term::poll(Duration::ZERO)? {
        if let Some(event) = term::read()? {
            hold_ahead(event);
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
        if !term::poll(left)? {
            return Ok(false);
        }
        if let Some(event) = term::read()? {
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
        if let Some(event) = term::read()? {
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

/// One log line for `event`, stamped with the call it arrived on.
fn encode(stamp: u64, event: &Event) -> String {
    match event {
        Event::Key(k) => format!("{stamp} k {} {}", encode_code(k.code), k.modifiers.bits()),
        Event::Mouse(m) => {
            let button = match m.button {
                MouseButton::Right => 'R',
                MouseButton::Left => 'L',
            };
            format!(
                "{stamp} m {button} {} {} {}",
                m.column,
                m.row,
                m.modifiers.bits()
            )
        }
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
            let mods = KeyModifiers::from_bits_truncate(mods.parse().ok()?);
            Event::Key(KeyEvent::new(decode_code(code)?, mods))
        }
        ("m", [button, column, row, mods]) => {
            let button = match *button {
                "L" => MouseButton::Left,
                "R" => MouseButton::Right,
                _ => return None,
            };
            Event::Mouse(MouseEvent {
                button,
                column: column.parse().ok()?,
                row: row.parse().ok()?,
                modifiers: KeyModifiers::from_bits_truncate(mods.parse().ok()?),
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
            button: MouseButton::Right,
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
            assert_eq!(decode(&line), Some((i as u64, *event)), "{line}");
        }
    }

    #[test]
    fn modifiers_and_both_buttons_survive_their_log_line() {
        let shifted = Event::Key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        let ctrl_alt = Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));
        let left = Event::Mouse(MouseEvent {
            button: MouseButton::Left,
            column: 3,
            row: 9,
            modifiers: KeyModifiers::NONE,
        });
        for event in [shifted, ctrl_alt, left, click()] {
            let line = encode(1, &event);
            assert_eq!(decode(&line), Some((1, event)), "{line}");
        }
    }

    #[test]
    fn a_modifier_bit_the_game_does_not_know_is_dropped_not_refused() {
        assert_eq!(decode("3 k c107 8"), Some((3, key('k'))));
        assert_eq!(decode("3 k c107 11"), decode("3 k c107 3"));
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
        hold_ahead(key('x'));
        assert_eq!(read().unwrap(), key('x'));
        assert_eq!(calls_so_far(), 5);
        let text = String::from_utf8(sink.0.borrow().clone()).unwrap();
        assert_eq!(text.lines().collect::<Vec<_>>(), ["5 k c120 0"]);
        reset();
    }
}
