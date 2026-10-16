//! The grid nihilurk paints onto, and the diff that says what changed.
//!
//! What lives here is the *grid*: its size, its double buffer, the cell-diff
//! ([`Screen::changed`]), and the map/screen coordinate split the screen shake
//! rides on. What does not live here is any knowledge of the game -- no map,
//! no HUD, no monsters -- or of any terminal. `view` decides what goes in a
//! frame; this module knows how big a frame is and which cells differ from
//! the last one shown; `term` turns those cells into escape sequences.
//!
//! # The two coordinate systems
//!
//! [`Screen::put`] and friends take *screen* coordinates: the status line, the
//! message log and the pack overlay paint in those and never move.
//! [`Screen::put_map`] and friends take *map* coordinates, run them through
//! [`Screen::map_cell`], and are the only things the screen shake can displace.
//!
//! `map_cell` returning `None` means "not drawn". It never clamps: a tile the
//! shake pushes off an edge is dropped rather than piled onto the opposite
//! column, and nothing is rendered to fill the gap. The shake moves the map
//! inside a fixed frame; it does not change the resolution.

use models::{Color, MAP_HEIGHT};

pub use crate::constants::screen::{MAP_TOP, SCREEN_H, SCREEN_W};

/// A screen cell: glyph, foreground colour, background colour.
pub type Cell = (char, Color, Color);
/// An empty cell: a space in the terminal's default colours.
pub const BLANK_CELL: Cell = (' ', Color::Reset, Color::Reset);

/// Double-buffered character grid. `render` paints the whole frame into `cur`;
/// `Term::present` then draws only the cells that differ from the
/// previously displayed frame, so a typical turn writes a few dozen cells
/// instead of repainting all 2000.
pub struct Screen {
    /// The frame being built, row-major, `SCREEN_W` cells per row.
    cur: Vec<Cell>,
    /// The frame the terminal is showing, for diffing against `cur`.
    prev: Vec<Cell>,
    /// Force a full repaint on the next present (first frame, or the centering
    /// offset changed and stale cells would otherwise be left behind).
    pub dirty_all: bool,
    /// Where the screen shake has thrown the map this frame, in whole cells
    /// right and down. Read only by the map-space painters below, so the
    /// status line, the message log and the pack overlay stay nailed down
    /// while the floor rocks.
    ///
    /// This is *not* a second viewport. The grid is 80x25 whatever this says,
    /// and a tile the displacement pushes past the edge of the map rows is
    /// dropped by `map_cell` rather than drawn somewhere else — the game
    /// renders no more of the map mid-shake than it does at rest, and no less
    /// of anything else.
    pub map_shift: (i16, i16),
}

impl Screen {
    /// A blank grid of `SCREEN_W` x `SCREEN_H` cells, due a full repaint on
    /// the first present.
    pub fn new() -> Self {
        let len = (SCREEN_W * SCREEN_H) as usize;
        Self {
            cur: vec![BLANK_CELL; len],
            prev: vec![BLANK_CELL; len],
            dirty_all: true,
            map_shift: (0, 0),
        }
    }

    /// Blanks every cell of the frame being built. The displayed frame is untouched.
    pub fn clear(&mut self) {
        for c in &mut self.cur {
            *c = BLANK_CELL;
        }
    }

    /// Writes `ch` at `(x, y)` in `color` over a default background. A cell off
    /// the grid is ignored.
    #[inline]
    pub fn put(&mut self, x: u16, y: u16, ch: char, color: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize] = (ch, color, Color::Reset);
        }
    }

    /// Sets only the foreground colour of a cell, leaving its glyph untouched.
    /// Used by the blood overlay to redden a tile in place.
    #[inline]
    pub fn set_fg(&mut self, x: u16, y: u16, fg: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize].1 = fg;
        }
    }

    /// Sets only the background colour of a cell, leaving its glyph and
    /// foreground untouched. Used by the travel cursor to highlight a tile.
    #[inline]
    pub fn set_bg(&mut self, x: u16, y: u16, bg: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize].2 = bg;
        }
    }

    /// The cell at `(x, y)` in the frame being built, or [`BLANK_CELL`] off the grid.
    #[inline]
    pub fn get(&self, x: u16, y: u16) -> Cell {
        if x >= SCREEN_W || y >= SCREEN_H {
            return BLANK_CELL;
        }
        self.cur[(y * SCREEN_W + x) as usize]
    }

    /// The screen cell a map tile lands on, with [`Screen::map_shift`] folded
    /// in — or `None` when the shake has thrown that tile clean out of the map
    /// viewport.
    ///
    /// The clip is against the map's own rows, not the whole grid, and that is
    /// the whole safety property of the shake: a displaced map can never smear
    /// a tile up into the status line or down into the message log, and a
    /// tile pushed off the left or right edge is not drawn at all rather than
    /// wrapping onto the next row. Off-viewport is off — nothing is rendered
    /// to fill the gap it leaves, which is why the shake reads as the map
    /// moving inside a fixed frame rather than as the frame resizing.
    #[inline]
    pub fn map_cell(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        let sx = x as i32 + self.map_shift.0 as i32;
        let sy = y as i32 + MAP_TOP as i32 + self.map_shift.1 as i32;
        if sx < 0 || sx >= SCREEN_W as i32 {
            return None;
        }
        if sy < MAP_TOP as i32 || sy >= (MAP_TOP + MAP_HEIGHT) as i32 {
            return None;
        }
        Some((sx as u16, sy as u16))
    }

    /// [`Screen::put`], in map coordinates.
    #[inline]
    pub fn put_map(&mut self, x: u16, y: u16, ch: char, color: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.put(sx, sy, ch, color);
        }
    }

    /// [`Screen::set_fg`], in map coordinates.
    #[inline]
    pub fn fg_map(&mut self, x: u16, y: u16, fg: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.set_fg(sx, sy, fg);
        }
    }

    /// [`Screen::set_bg`], in map coordinates.
    #[inline]
    pub fn bg_map(&mut self, x: u16, y: u16, bg: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.set_bg(sx, sy, bg);
        }
    }

    /// [`Screen::get`], in map coordinates. A tile the shake has pushed out of
    /// the viewport reads as blank, which is what the targeting beam's
    /// recolour wants: there is nothing under it to preserve.
    #[inline]
    pub fn get_map(&self, x: u16, y: u16) -> Cell {
        match self.map_cell(x, y) {
            Some((sx, sy)) => self.get(sx, sy),
            None => BLANK_CELL,
        }
    }

    /// Writes `s` left to right from `(x, y)`, one cell per char. Anything past
    /// the right edge is dropped.
    pub fn puts(&mut self, x: u16, y: u16, s: &str, color: Color) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as u16, y, ch, color);
        }
    }

    /// Writes `ch` in `n` cells running right from `(x, y)`.
    pub fn hline(&mut self, x: u16, y: u16, ch: char, n: u16, color: Color) {
        for i in 0..n {
            self.put(x + i, y, ch, color);
        }
    }

    /// The cells that differ from the frame being shown, row by row (all of
    /// them when `dirty_all` is set), as `(x, y, cell)`.
    pub fn changed(&self) -> impl Iterator<Item = (u16, u16, Cell)> + '_ {
        let all = self.dirty_all;
        (0..SCREEN_H)
            .flat_map(|y| (0..SCREEN_W).map(move |x| (x, y)))
            .filter_map(move |(x, y)| {
                let idx = (y * SCREEN_W + x) as usize;
                (all || self.cur[idx] != self.prev[idx]).then(|| (x, y, self.cur[idx]))
            })
    }

    /// Marks the frame being built as the one the terminal now shows.
    pub fn shown(&mut self) {
        std::mem::swap(&mut self.cur, &mut self.prev);
        self.dirty_all = false;
    }

    /// The frame the terminal is showing, as plain text: one line per row,
    /// trailing blanks trimmed, colours dropped. Reads `prev`, since
    /// [`Screen::shown`] swaps the shown frame there.
    pub fn shown_text(&self) -> String {
        self.prev
            .chunks(SCREEN_W as usize)
            .map(|row| {
                let line: String = row.iter().map(|c| c.0).collect();
                line.trim_end().to_string() + "\n"
            })
            .collect()
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_screen_changes_everywhere_until_it_is_shown() {
        let mut s = Screen::new();
        assert_eq!(s.changed().count(), (SCREEN_W * SCREEN_H) as usize);
        s.shown();
        assert_eq!(s.changed().count(), 0);
    }

    #[test]
    fn only_a_cell_that_differs_from_the_shown_frame_changes() {
        let mut s = Screen::new();
        s.shown();
        s.put(3, 2, 'x', Color::Red);
        assert_eq!(
            s.changed().collect::<Vec<_>>(),
            [(3, 2, ('x', Color::Red, Color::Reset))]
        );
        s.shown();
        s.clear();
        s.put(3, 2, 'x', Color::Red);
        assert_eq!(s.changed().count(), 0, "unchanged since it was shown");
    }

    #[test]
    fn shown_text_reads_the_frame_last_shown() {
        let mut s = Screen::new();
        s.puts(1, 0, "hi", Color::White);
        s.shown();
        s.clear();
        assert!(s.shown_text().starts_with(" hi\n"));
    }
}
