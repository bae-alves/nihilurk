//! The 16-colour terminal palette, plus `Reset` for "the terminal's default".
//!
//! The game's own type, so `models` names no terminal library. The variant
//! names match crossterm's; the engine's `term` module maps each to its
//! escape code. Saves hold the palette as one byte each (see `saveload`).

/// A foreground or background colour.
#[allow(missing_docs)] // the names are the palette
#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum Color {
    Reset,
    Black,
    DarkGrey,
    Red,
    DarkRed,
    Green,
    DarkGreen,
    Yellow,
    DarkYellow,
    Blue,
    DarkBlue,
    Magenta,
    DarkMagenta,
    Cyan,
    DarkCyan,
    White,
    Grey,
}
