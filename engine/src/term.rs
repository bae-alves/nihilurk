//! The terminal, and the only file that names crossterm.
//!
//! What the player does arrives here as crossterm events and leaves as the
//! game's own [`Event`]: key presses and Left or Right mouse downs, nothing
//! else. Releases, repeats, resizes, mouse moves, other buttons and keys the
//! game has no name for never get past [`read`], so nothing above this file
//! has to ask what kind of event it holds. A click leaves in grid
//! coordinates: the centering offset is taken off here, and one outside the
//! frame is dropped.
//!
//! What the game draws goes out through [`Term::present`], which turns a
//! [`Screen`]'s changed cells into escape sequences.

use crate::input::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};
use crate::screen::{SCREEN_H, SCREEN_W, Screen};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event as ct, execute, queue,
    style::{Color as CtColor, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode, size,
    },
};
use models::Color;
use std::cell::Cell;
use std::io::{BufWriter, Stdout, Write, stdout};
use std::time::Duration;

thread_local! {
    /// Where the frame on screen starts, for turning a click into a grid cell.
    /// Set by [`Term::present`]: a click lands on the frame the player sees.
    static FRAME_AT: Cell<(u16, u16)> = const { Cell::new((0, 0)) };
}

/// Puts the terminal into raw, alternate-screen mode for the run and takes it
/// back out on drop. [`install_panic_hook`] leaves the terminal sane even if
/// the drop never gets to run because unwinding is disabled.
pub struct TerminalGuard;

impl TerminalGuard {
    /// Enters raw mode and the alternate screen with the cursor hidden, and
    /// mouse capture too when `mouse` is set. Fails if the terminal refuses
    /// either; nothing is left half-set.
    pub fn new(mouse: bool) -> std::io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        if mouse {
            execute!(stdout(), ct::EnableMouseCapture)?;
        }
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            stdout(),
            ct::DisableMouseCapture,
            Show,
            LeaveAlternateScreen
        );
        let _ = disable_raw_mode();
    }
}

/// Makes a panic put the terminal back before its message prints.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = execute!(
            std::io::stderr(),
            ct::DisableMouseCapture,
            LeaveAlternateScreen,
            Show
        );
        let _ = disable_raw_mode();
        original_hook(panic_info);
    }));
}

/// Prints `text` to the terminal in `color`, then resets the colour.
pub fn print_colored(color: Color, text: &str) -> std::io::Result<()> {
    execute!(
        stdout(),
        SetForegroundColor(to_ct(color)),
        Print(text),
        ResetColor
    )
}

/// The terminal's colour for a palette colour.
fn to_ct(c: Color) -> CtColor {
    match c {
        Color::Reset => CtColor::Reset,
        Color::Black => CtColor::Black,
        Color::DarkGrey => CtColor::DarkGrey,
        Color::Red => CtColor::Red,
        Color::DarkRed => CtColor::DarkRed,
        Color::Green => CtColor::Green,
        Color::DarkGreen => CtColor::DarkGreen,
        Color::Yellow => CtColor::Yellow,
        Color::DarkYellow => CtColor::DarkYellow,
        Color::Blue => CtColor::Blue,
        Color::DarkBlue => CtColor::DarkBlue,
        Color::Magenta => CtColor::Magenta,
        Color::DarkMagenta => CtColor::DarkMagenta,
        Color::Cyan => CtColor::Cyan,
        Color::DarkCyan => CtColor::DarkCyan,
        Color::White => CtColor::White,
        Color::Grey => CtColor::Grey,
    }
}

/// Where frames go: stdout, optionally centred in a terminal larger than the
/// 80x25 grid.
// ponytail: concrete Term; becomes a Frontend trait when a second backend lands
pub struct Term {
    out: BufWriter<Stdout>,
    centered: bool,
}

impl Term {
    /// A terminal that centres the frame when `centered` is set (`-c`).
    pub fn new(centered: bool) -> Self {
        Self {
            out: BufWriter::with_capacity(32 * 1024, stdout()),
            centered,
        }
    }

    fn offset(&self) -> (u16, u16) {
        if !self.centered {
            return (0, 0);
        }
        let (width, height) = size().unwrap_or((SCREEN_W, SCREEN_H));
        (
            width.saturating_sub(SCREEN_W) / 2,
            height.saturating_sub(SCREEN_H) / 2,
        )
    }

    /// Draws the cells of `screen` that changed (all of them when the centring
    /// offset moved) and marks the frame as shown.
    pub fn present(&mut self, screen: &mut Screen) -> std::io::Result<()> {
        let offset = self.offset();
        if offset != FRAME_AT.get() {
            screen.dirty_all = true;
        }
        FRAME_AT.set(offset);

        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        for (x, y, (ch, fg, bg)) in screen.changed() {
            if cur_fg != Some(fg) {
                queue!(self.out, SetForegroundColor(to_ct(fg)))?;
                cur_fg = Some(fg);
            }
            if cur_bg != Some(bg) {
                queue!(self.out, SetBackgroundColor(to_ct(bg)))?;
                cur_bg = Some(bg);
            }
            queue!(self.out, MoveTo(offset.0 + x, offset.1 + y), Print(ch))?;
        }
        self.out.flush()?;
        screen.shown();
        Ok(())
    }
}

/// The grid cell under terminal cell `(column, row)` when the frame starts at
/// `offset`, or `None` above or left of the frame.
fn grid_cell(column: u16, row: u16, offset: (u16, u16)) -> Option<(u16, u16)> {
    Some((column.checked_sub(offset.0)?, row.checked_sub(offset.1)?))
}

/// Whether the terminal has an event for [`read`], waiting up to `wait` for one.
pub fn poll(wait: Duration) -> std::io::Result<bool> {
    ct::poll(wait)
}

/// The next terminal event, waiting for it if need be. `None` for one the game
/// does not act on.
pub fn read() -> std::io::Result<Option<Event>> {
    Ok(translate(ct::read()?))
}

fn translate(event: ct::Event) -> Option<Event> {
    match event {
        ct::Event::Key(k) if k.kind == ct::KeyEventKind::Press => Some(Event::Key(KeyEvent::new(
            key_code(k.code)?,
            KeyModifiers::from_bits_truncate(k.modifiers.bits()),
        ))),
        ct::Event::Mouse(m) => {
            let button = match m.kind {
                ct::MouseEventKind::Down(ct::MouseButton::Left) => MouseButton::Left,
                ct::MouseEventKind::Down(ct::MouseButton::Right) => MouseButton::Right,
                _ => return None,
            };
            let (column, row) = grid_cell(m.column, m.row, FRAME_AT.get())?;
            Some(Event::Mouse(MouseEvent {
                button,
                column,
                row,
                modifiers: KeyModifiers::from_bits_truncate(m.modifiers.bits()),
            }))
        }
        _ => None,
    }
}

fn key_code(code: ct::KeyCode) -> Option<KeyCode> {
    Some(match code {
        ct::KeyCode::Char(c) => KeyCode::Char(c),
        ct::KeyCode::F(n) => KeyCode::F(n),
        ct::KeyCode::Backspace => KeyCode::Backspace,
        ct::KeyCode::Enter => KeyCode::Enter,
        ct::KeyCode::Left => KeyCode::Left,
        ct::KeyCode::Right => KeyCode::Right,
        ct::KeyCode::Up => KeyCode::Up,
        ct::KeyCode::Down => KeyCode::Down,
        ct::KeyCode::Home => KeyCode::Home,
        ct::KeyCode::End => KeyCode::End,
        ct::KeyCode::PageUp => KeyCode::PageUp,
        ct::KeyCode::PageDown => KeyCode::PageDown,
        ct::KeyCode::Tab => KeyCode::Tab,
        ct::KeyCode::BackTab => KeyCode::BackTab,
        ct::KeyCode::Delete => KeyCode::Delete,
        ct::KeyCode::Insert => KeyCode::Insert,
        ct::KeyCode::Esc => KeyCode::Esc,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(code: ct::KeyCode, kind: ct::KeyEventKind) -> ct::Event {
        ct::Event::Key(ct::KeyEvent::new_with_kind(
            code,
            ct::KeyModifiers::NONE,
            kind,
        ))
    }

    fn mouse_event(kind: ct::MouseEventKind) -> ct::Event {
        ct::Event::Mouse(ct::MouseEvent {
            kind,
            column: 4,
            row: 2,
            modifiers: ct::KeyModifiers::SHIFT | ct::KeyModifiers::SUPER,
        })
    }

    #[test]
    fn only_a_press_is_a_key() {
        let press = key_event(ct::KeyCode::Char('x'), ct::KeyEventKind::Press);
        let plain_x = Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(translate(press), Some(plain_x));
        for kind in [ct::KeyEventKind::Release, ct::KeyEventKind::Repeat] {
            assert_eq!(translate(key_event(ct::KeyCode::Char('x'), kind)), None);
        }
    }

    #[test]
    fn a_key_the_game_has_no_name_for_is_dropped() {
        let caps = key_event(ct::KeyCode::CapsLock, ct::KeyEventKind::Press);
        assert_eq!(translate(caps), None);
    }

    #[test]
    fn left_and_right_downs_are_clicks_and_nothing_else_is() {
        let down = |b| mouse_event(ct::MouseEventKind::Down(b));
        let shift_only = KeyModifiers::SHIFT;
        let click = |button| {
            Some(Event::Mouse(MouseEvent {
                button,
                column: 4,
                row: 2,
                modifiers: shift_only,
            }))
        };
        assert_eq!(
            translate(down(ct::MouseButton::Left)),
            click(MouseButton::Left)
        );
        assert_eq!(
            translate(down(ct::MouseButton::Right)),
            click(MouseButton::Right)
        );
        assert_eq!(translate(down(ct::MouseButton::Middle)), None);
        assert_eq!(
            translate(mouse_event(ct::MouseEventKind::Up(ct::MouseButton::Left))),
            None
        );
        assert_eq!(translate(mouse_event(ct::MouseEventKind::Moved)), None);
        assert_eq!(translate(ct::Event::Resize(80, 25)), None);
    }
}
