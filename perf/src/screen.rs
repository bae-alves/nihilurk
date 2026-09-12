//! A copy of the game's double-buffered terminal grid, so the rig can measure
//! what a redraw costs.
//!
//! # This is a deliberate copy, and it can drift
//!
//! The original is `Screen` in `engine/src/view.rs`. `engine` is a binary
//! crate, its `Screen::put` / `clear` / `flush` are private to that module, and
//! this rig depends on `models` only -- so the alternative to copying was to
//! give `engine` a library target and depend on it. That was weighed and
//! declined: the game crates stay untouched.
//!
//! The cost of that decision is that nothing checks these two stay the same.
//! There is no compiler error, no failing test, and no warning if
//! `engine/src/view.rs` changes and this file does not -- the rig will simply
//! go on reporting numbers for a renderer the game no longer has. **If you
//! touch `Screen` in `engine/src/view.rs`, mirror it here.** What has to match
//! for the measurement to mean anything:
//!
//!   * `SCREEN_W` x `SCREEN_H` and the `(char, fg, bg)` cell shape
//!   * the double buffer, and the swap at the end of `flush`
//!   * the skip-unchanged-cells test in `flush` -- the diff is the thing being
//!     measured, and a copy that repainted everything would overstate the cost
//!     several-fold
//!   * emitting a colour command only when that colour actually changes, since
//!     that is most of what lands in the byte count
//!   * `dirty_all` forcing one full repaint after an offset change
//!
//! What deliberately differs: nothing here draws a map, a status line or a
//! message log. The rig paints reel cells instead. The *painting* is not what
//! this measures -- the diff and the escape-sequence generation are, and those
//! do not care what put the glyphs there.

use std::io::Write;

use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
};

/// The classic 80x25, as `engine/src/view.rs` declares it: row 0 is the status
/// line, rows 1..=22 the map, 22..25 the log. The rig paints the reel into the
/// map rows and leaves the rest blank, which is what makes a frame's changed
/// cell count realistic rather than a full-screen repaint every time.
pub const SCREEN_W: u16 = 80;
pub const SCREEN_H: u16 = 25;

/// A screen cell: glyph, foreground colour, background colour.
type Cell = (char, Color, Color);
const BLANK_CELL: Cell = (' ', Color::Reset, Color::Reset);

/// Double-buffered character grid. `put` paints into `cur`; `flush` emits
/// terminal commands only for the cells that differ from the previously
/// displayed frame.
pub struct Screen {
    cur: Vec<Cell>,
    prev: Vec<Cell>,
    /// Force a full repaint on the next flush (first frame, or the centering
    /// offset changed and stale cells would otherwise be left behind).
    dirty_all: bool,
    last_offset: (u16, u16),
}

impl Screen {
    pub fn new() -> Self {
        let len = (SCREEN_W * SCREEN_H) as usize;
        Self {
            cur: vec![BLANK_CELL; len],
            prev: vec![BLANK_CELL; len],
            dirty_all: true,
            last_offset: (0, 0),
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

    /// Emit the cells that changed, and return how many that was.
    ///
    /// The count is the one intentional difference from the original's
    /// signature, and it is free: `flush` is already walking exactly those
    /// cells. Measuring it with a separate diff pass over all 2000 would have
    /// added several percent to the thing being measured. "Cells changed" is
    /// what explains the byte count printed beside it.
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

/// Where a headless flush writes.
///
/// The game flushes into a `BufWriter<Stdout>`: escape sequences are formatted
/// into a buffer, and one write syscall per frame hands it to the terminal.
/// This models the buffer half and stops there. The syscall is left out on
/// purpose -- what it costs is the terminal emulator's business, it varies by
/// an order of magnitude between them, and it would make the measurement a
/// benchmark of whatever happened to be attached to stdout.
///
/// So the bytes are really formatted and really copied, the capacity is reused
/// between frames so the allocation counters stay clean, and `len` afterwards
/// is exactly what the game would have handed the terminal.
pub struct Sink {
    buf: Vec<u8>,
    pub total_bytes: u64,
    pub last_bytes: usize,
}

impl Sink {
    pub fn new() -> Self {
        Self {
            // One frame of full repaint is ~2000 cells of MoveTo + Print; this
            // is comfortably past that, so the buffer never grows mid-run.
            buf: Vec::with_capacity(64 * 1024),
            total_bytes: 0,
            last_bytes: 0,
        }
    }

    /// Call once per frame, after `Screen::flush`.
    pub fn take_frame(&mut self) {
        self.last_bytes = self.buf.len();
        self.total_bytes += self.buf.len() as u64;
        self.buf.clear();
    }
}

impl Default for Sink {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    // `Screen::flush` calls this at the end of every frame. There is nothing
    // downstream to push to -- the frame's bytes are counted by `take_frame`.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
