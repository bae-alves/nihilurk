//! The terminal grid roog paints onto, and the diff that gets it there.
//!
//! One crate, so there is **one** `Screen` in the workspace. `engine` paints
//! the game into it; `perf` measures what a frame of that costs. Both are
//! looking at the same type, which is the whole reason this is a crate rather
//! than a module: the rig used to carry a hand-maintained copy, nothing
//! checked the two stayed in step, and a copy that drifts reports numbers for
//! a renderer the game no longer has.
//!
//! What lives here is the *grid*: its size, its double buffer, the cell-diff
//! that turns a painted frame into escape sequences, and the map/screen
//! coordinate split the screen shake rides on. What does not live here is any
//! knowledge of the game — no map, no HUD, no monsters. `view` knows how big a
//! frame is and how to get one onto a terminal; what goes in it is `engine`'s
//! business, and what goes in it for a measurement is `perf`'s.
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

use std::io::Write;

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
};

use models::MAP_HEIGHT;

/// Screen dimensions. The classic 80x25: row 0 is the status line, rows 1..=22
/// hold the map, and rows 22..25 hold the message log.
pub const SCREEN_W: u16 = 80;
pub const SCREEN_H: u16 = 25;

/// The screen row map row 0 paints on — row 0 being the status line.
///
/// Every map-space painter folds this in, which is why `render`'s map layers
/// pass a raw map coordinate instead of each carrying its own `y + 1`.
pub const MAP_TOP: u16 = 1;

/// A screen cell: glyph, foreground colour, background colour.
pub type Cell = (char, Color, Color);
pub const BLANK_CELL: Cell = (' ', Color::Reset, Color::Reset);

/// Double-buffered character grid. `render` paints the whole frame into `cur`;
/// `flush` then emits terminal commands only for the cells that differ from the
/// previously displayed frame, so a typical turn writes a few dozen cells
/// instead of repainting all 2000.
pub struct Screen {
    cur: Vec<Cell>,
    prev: Vec<Cell>,
    /// Force a full repaint on the next flush (first frame, or the centering
    /// offset changed and stale cells would otherwise be left behind).
    pub dirty_all: bool,
    last_offset: (u16, u16),
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
    pub fn new() -> Self {
        let len = (SCREEN_W * SCREEN_H) as usize;
        Self {
            cur: vec![BLANK_CELL; len],
            prev: vec![BLANK_CELL; len],
            dirty_all: true,
            last_offset: (0, 0),
            map_shift: (0, 0),
        }
    }

    pub fn clear(&mut self) {
        for c in &mut self.cur {
            *c = BLANK_CELL;
        }
    }

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

    pub fn puts(&mut self, x: u16, y: u16, s: &str, color: Color) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as u16, y, ch, color);
        }
    }

    pub fn hline(&mut self, x: u16, y: u16, ch: char, n: u16, color: Color) {
        for i in 0..n {
            self.put(x + i, y, ch, color);
        }
    }

    pub fn flush<W: Write>(&mut self, out: &mut W, offset: (u16, u16)) -> std::io::Result<usize> {
        if offset != self.last_offset {
            self.dirty_all = true;
            self.last_offset = offset;
        }

        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        let mut drawn = 0usize;
        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W {
                let idx = (y * SCREEN_W + x) as usize;
                if !self.dirty_all && self.cur[idx] == self.prev[idx] {
                    continue;
                }
                let (ch, fg, bg) = self.cur[idx];
                if cur_fg != Some(fg) {
                    queue!(out, SetForegroundColor(fg))?;
                    cur_fg = Some(fg);
                }
                if cur_bg != Some(bg) {
                    queue!(out, SetBackgroundColor(bg))?;
                    cur_bg = Some(bg);
                }
                queue!(out, MoveTo(offset.0 + x, offset.1 + y), Print(ch))?;
                drawn += 1;
            }
        }
        out.flush()?;

        std::mem::swap(&mut self.cur, &mut self.prev);
        self.dirty_all = false;
        Ok(drawn)
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}
