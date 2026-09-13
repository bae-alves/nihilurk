//! The rig's window onto the real renderer, plus the sink it flushes into.
//!
//! [`Screen`] is not defined here and never was again: it is `view::Screen`,
//! the same type `engine` paints the game with. This module re-exports it so
//! `crate::screen::Screen` keeps resolving, and adds the one thing the game has
//! no use for — a [`Sink`] that counts bytes instead of writing them to a
//! terminal.
//!
//! **This used to be a copy**, and the copy is the reason `view` is a crate.
//! `engine` was a binary with no library target, so the rig could not depend on
//! it and reimplemented the grid instead; nothing checked the two stayed in
//! step, and a rig measuring a renderer the game no longer had would have gone
//! on reporting numbers with a straight face. There is one definition now, so
//! there is nothing left to drift.

use std::io::Write;

pub use view::{SCREEN_H, SCREEN_W, Screen};

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
