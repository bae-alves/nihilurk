//! Loading the `bad-apple` reel off disk and turning it into something the
//! frame loop can replay without doing any work of its own.
//!
//! The whole file is parsed once, at startup, into a `Vec<Frame>` of
//! pre-resolved ink cells: coordinate, glyph and colour already computed. This
//! is the point of the module. If the frame loop split strings or matched
//! characters while the clock was running, the flamegraph would be a picture of
//! `str::split` and the RSS graph would be tracking the parser's garbage, and
//! the one thing this rig exists to measure -- what [`models::Particles`] costs
//! -- would be buried under it. After [`Reel::load`] returns, replaying a frame
//! is a slice walk and nothing else.

use std::fmt;
use std::fs;
use std::path::Path;

use crossterm::style::Color;

/// Frame geometry, as the reel on disk is authored. Checked on load rather
/// than assumed: a reel of the wrong shape is a bad measurement, not a crash
/// halfway through the run.
pub const FRAME_WIDTH: usize = 80;
pub const FRAME_HEIGHT: usize = 20;

/// What the frames are joined by in the file.
const SEPARATOR: &str = "\n---FRAME---\n";

/// The ASCII ramp the reel is rendered in, darkest first. A glyph's index here
/// is its brightness, which is all the colour mapping needs.
const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// One lit cell of one frame: everything [`models::Particles::blip`] needs,
/// resolved ahead of time.
#[derive(Clone, Copy)]
pub struct Ink {
    pub x: u16,
    pub y: u16,
    pub glyph: char,
    pub color: Color,
}

/// One frame, as the cells that are actually lit. Blank cells are dropped
/// here and never reach the particle layer, so the reel's own contrast becomes
/// the rig's load curve: a frame that is mostly silhouette spawns half what a
/// mostly-filled frame does, and the RSS graph visibly breathes with the video.
#[derive(Clone)]
pub struct Frame {
    pub ink: Vec<Ink>,
}

/// Every frame of the reel, in order.
pub struct Reel {
    pub frames: Vec<Frame>,
}

/// Why a reel could not be loaded. Worth distinguishing: "no such file" is a
/// typo in a path, "wrong shape" is a reel that would silently produce
/// meaningless numbers.
#[derive(Debug)]
pub enum ReelError {
    Io(std::io::Error),
    Empty,
    BadShape {
        frame: usize,
        rows: usize,
        widest: usize,
    },
}

impl fmt::Display for ReelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReelError::Io(e) => write!(f, "cannot read reel: {e}"),
            ReelError::Empty => write!(f, "reel contains no frames"),
            ReelError::BadShape {
                frame,
                rows,
                widest,
            } => write!(
                f,
                "frame {frame} is {rows}x{widest}, expected {FRAME_HEIGHT}x{FRAME_WIDTH}"
            ),
        }
    }
}

impl std::error::Error for ReelError {}

impl Reel {
    /// Read and parse a reel. `y_offset` is added to every cell's row, which is
    /// how a 20-row reel gets centred in nihilurk's 22-row map without the frame
    /// loop having to think about it.
    pub fn load(path: &Path, y_offset: u16) -> Result<Self, ReelError> {
        let raw = fs::read_to_string(path).map_err(ReelError::Io)?;
        let frames = raw
            .split(SEPARATOR)
            .enumerate()
            .map(|(i, chunk)| Frame::parse(i, chunk, y_offset))
            .collect::<Result<Vec<_>, _>>()?;
        if frames.is_empty() {
            return Err(ReelError::Empty);
        }
        Ok(Self { frames })
    }

    /// The frame at `index`, wrapping so the reel loops forever.
    pub fn frame(&self, index: usize) -> &Frame {
        &self.frames[index % self.frames.len()]
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Lit cells in the densest frame -- the worst case the run will hit, and
    /// so the number worth printing next to a peak particle count.
    pub fn peak_ink(&self) -> usize {
        self.frames.iter().map(|f| f.ink.len()).max().unwrap_or(0)
    }

    /// Mean lit cells per frame, for the headless summary.
    pub fn mean_ink(&self) -> f64 {
        if self.frames.is_empty() {
            return 0.0;
        }
        let total: usize = self.frames.iter().map(|f| f.ink.len()).sum();
        total as f64 / self.frames.len() as f64
    }
}

impl Frame {
    fn parse(index: usize, chunk: &str, y_offset: u16) -> Result<Self, ReelError> {
        // A trailing separator leaves an empty final chunk; a reel authored
        // with CRLF leaves a stray '\r' on every row. Neither is worth
        // rejecting the file over.
        let rows: Vec<&str> = chunk
            .trim_end_matches('\n')
            .split('\n')
            .map(|r| r.trim_end_matches('\r'))
            .collect();
        let widest = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        if rows.len() != FRAME_HEIGHT || widest > FRAME_WIDTH {
            return Err(ReelError::BadShape {
                frame: index,
                rows: rows.len(),
                widest,
            });
        }

        let mut ink = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            for (x, glyph) in row.chars().enumerate() {
                let Some(color) = shade(glyph) else {
                    continue;
                };
                ink.push(Ink {
                    x: x as u16,
                    y: y as u16 + y_offset,
                    glyph,
                    color,
                });
            }
        }
        // `push` grows by doubling, so a frame can end up carrying half its
        // own length again in slack. One frame's worth is nothing; a
        // full-length reel is several thousand of these vectors, and the slack
        // is tens of megabytes that sit in `reel at rest` and in RSS for the
        // whole run, pretending to be something the particle layer did.
        ink.shrink_to_fit();
        Ok(Self { ink })
    }
}

/// The colour a ramp glyph burns in, or `None` for a cell dark enough to leave
/// unlit. Four greys is all a black-and-white reel can use, and matching the
/// palette nihilurk itself draws in keeps the canvas honest -- this is the same
/// `crossterm::style::Color` the game hands its own particles.
fn shade(glyph: char) -> Option<Color> {
    let brightness = RAMP.iter().position(|&c| c == glyph)?;
    match brightness {
        0 => None,
        1..=3 => Some(Color::DarkGrey),
        4..=6 => Some(Color::Grey),
        _ => Some(Color::White),
    }
}
