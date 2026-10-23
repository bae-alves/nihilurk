//! The one door input comes through.
//!
//! Every `poll` and `read` in the engine goes through here and passes to
//! [`crate::term`], which drops releases, resizes and mouse moves before an
//! event reaches this module: only presses and clicks arrive.
//!
//! The event types are the game's own, so nothing above the terminal names a
//! terminal library. Their names and shapes follow what `update` matches on.

use crate::term;
use std::cell::RefCell;
use std::time::Duration;

/// A key, as far as the game tells them apart.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

/// The modifier keys held with an event: Shift 1, Control 2, Alt 4.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// The left button.
    Left,
    /// The right button.
    Right,
}

/// A mouse button going down, at a cell of the terminal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A key went down.
    Key(KeyEvent),
    /// A mouse button went down.
    Mouse(MouseEvent),
}

#[derive(Default)]
struct Stream {
    pending: Option<Event>,
}

thread_local! {
    static STREAM: RefCell<Stream> = RefCell::new(Stream::default());
}

fn hold(event: Event) {
    STREAM.with(|s| s.borrow_mut().pending = Some(event));
}

/// Whether the next `poll` or `read` gets an event at once. Drops what the
/// terminal reports that is no key or click and holds a key or click for the
/// next call, which the caller is about to make.
pub fn waiting() -> std::io::Result<bool> {
    if STREAM.with(|s| s.borrow().pending.is_some()) {
        return Ok(true);
    }
    while term::poll(Duration::ZERO)? {
        if let Some(event) = term::read()? {
            hold(event);
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether an event is waiting, giving the player up to `wait` to produce one.
pub fn poll(wait: Duration) -> std::io::Result<bool> {
    if STREAM.with(|s| s.borrow().pending.is_some()) {
        return Ok(true);
    }
    let started = std::time::Instant::now();
    loop {
        let left = wait.saturating_sub(started.elapsed());
        if !term::poll(left)? {
            return Ok(false);
        }
        if let Some(event) = term::read()? {
            hold(event);
            return Ok(true);
        }
    }
}

/// The next event, waiting for it if need be.
pub fn read() -> std::io::Result<Event> {
    if let Some(event) = STREAM.with(|s| s.borrow_mut().pending.take()) {
        return Ok(event);
    }
    loop {
        if let Some(event) = term::read()? {
            return Ok(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_sees_a_key_a_poll_already_holds() {
        hold(Event::Key(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
        )));
        assert!(waiting().unwrap());
        assert!(poll(Duration::ZERO).unwrap());
        assert!(matches!(read().unwrap(), Event::Key(_)));
    }
}
