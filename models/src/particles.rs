//! A tiny ASCII/CP437 particle layer for instant, throwaway visual feedback:
//! the spark when you land a blow, the streak of a wand bolt racing down its
//! line, the blossoming fireball of a fire/cold blast.
//!
//! It is deliberately *not* an ECS system. Particles never touch the turn
//! schedule, are never saved, and never affect gameplay — they are pure
//! decoration. Gameplay code just drops requests into the [`Particles`] resource
//! (`hit_spark`, `beam`, `explosion`); once the turn has resolved the engine
//! notices [`Particles::pending`] and plays the whole batch out over a handful
//! of ~33 ms frames, re-rendering the map each frame, before blocking for the
//! next key. A keypress skips straight to the end.
//!
//! This mirrors how NetHack's `tmp_at()` and DCSS's `bolt::animate()` freeze
//! input for the length of a transient effect, only much smaller.

use bevy_ecs::prelude::*;
use crossterm::style::Color;

use crate::map::{MAP_HEIGHT, MAP_WIDTH};

/// One transient mote, living in map-tile coordinates.
pub struct Particle {
    pub x: u16,
    pub y: u16,
    /// Milliseconds to wait, after the batch is spawned, before this mote starts
    /// drawing. Lets a single batch ripple outward one ring at a time (a beam
    /// racing to its target, a blast expanding from its core).
    pub delay_ms: f32,
    /// Milliseconds the mote stays on screen once its delay has elapsed.
    pub lifetime_ms: f32,
    /// Milliseconds elapsed since the batch was spawned.
    pub age_ms: f32,
    /// `(glyph, colour)` keyframes; the mote steps through them evenly across its
    /// lifetime, so a spark can decay `‼ → * → +` and a flame can cycle through
    /// fire colours. Never empty.
    pub frames: Vec<(char, Color)>,
}

impl Particle {
    /// Whether the mote has been born and is not yet dead.
    fn visible(&self) -> bool {
        self.age_ms >= self.delay_ms && self.age_ms < self.delay_ms + self.lifetime_ms
    }

    /// Age past which the mote can be culled.
    fn finished(&self) -> bool {
        self.age_ms >= self.delay_ms + self.lifetime_ms
    }

    /// The `(glyph, colour)` to paint this frame, or `None` if the mote is still
    /// waiting out its delay or has already expired.
    pub fn current(&self) -> Option<(char, Color)> {
        if !self.visible() {
            return None;
        }
        let t = ((self.age_ms - self.delay_ms) / self.lifetime_ms).clamp(0.0, 0.999);
        let idx = (t * self.frames.len() as f32) as usize;
        Some(self.frames[idx.min(self.frames.len() - 1)])
    }
}

/// The effect layer. Systems push requests in during a turn; the engine drains
/// it afterwards.
#[derive(Resource, Default)]
pub struct Particles {
    pub live: Vec<Particle>,
    /// Raised whenever a batch is queued, so the engine knows a turn produced an
    /// animation worth playing out.
    pub pending: bool,
}

impl Particles {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, p: Particle) {
        self.live.push(p);
        self.pending = true;
    }

    /// A short-lived mote of your choosing on a single tile.
    pub fn blip(&mut self, x: u16, y: u16, glyph: char, color: Color) {
        self.push(Particle {
            x,
            y,
            delay_ms: 0.0,
            lifetime_ms: 130.0,
            age_ms: 0.0,
            frames: vec![(glyph, color)],
        });
    }

    /// The impact spark for a landed melee (or thrown) hit: a bright pop that
    /// decays to an ember over ~180 ms.
    pub fn hit_spark(&mut self, x: u16, y: u16) {
        self.push(Particle {
            x,
            y,
            delay_ms: 0.0,
            lifetime_ms: 190.0,
            age_ms: 0.0,
            frames: vec![
                ('‼', Color::White),
                ('*', Color::Yellow),
                ('+', Color::DarkYellow),
                ('·', Color::DarkRed),
            ],
        });
    }

    /// A wand bolt: a directional streak (`- | \ /`) that races cell by cell
    /// from the caster to the point of impact, each cell flaring white then
    /// settling to `color` then guttering out. `pts` is the traversed line in
    /// map coordinates, caster's own tile excluded.
    pub fn beam(&mut self, pts: &[(u16, u16)], color: Color) {
        const TRAVEL_MS_PER_CELL: f32 = 12.0;
        for (i, &(x, y)) in pts.iter().enumerate() {
            let glyph = beam_glyph(pts, i);
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                // Cells nearer the caster linger a touch longer, leaving a tail.
                lifetime_ms: 150.0 + (pts.len() - i) as f32 * 10.0,
                age_ms: 0.0,
                frames: vec![
                    (glyph, Color::White),
                    (glyph, color),
                    ('·', color),
                ],
            });
        }
    }

    /// A DCSS-style area blast. `cells` is `(x, y, distance_from_centre)` for
    /// every tile the blast covers (already LOS-checked by the caller); the ring
    /// of flame expands outward from the core and every cell cycles through the
    /// element's colours before fading. `fire` picks the fire palette, otherwise
    /// the frost one.
    pub fn explosion(&mut self, cells: &[(u16, u16, f32)], fire: bool) {
        const RIPPLE_MS_PER_TILE: f32 = 24.0;
        let frames: [(char, Color); 5] = if fire {
            [
                ('#', Color::White),
                ('@', Color::Yellow),
                ('*', Color::DarkYellow),
                ('+', Color::Red),
                ('·', Color::DarkRed),
            ]
        } else {
            [
                ('*', Color::White),
                ('+', Color::Cyan),
                ('+', Color::Blue),
                (':', Color::DarkBlue),
                ('·', Color::DarkCyan),
            ]
        };
        for &(x, y, dist) in cells {
            self.push(Particle {
                x,
                y,
                delay_ms: dist * RIPPLE_MS_PER_TILE,
                lifetime_ms: 280.0,
                age_ms: 0.0,
                frames: frames.to_vec(),
            });
        }
    }

    /// Advance every mote by `dt_ms` and cull the dead ones.
    pub fn advance(&mut self, dt_ms: f32) {
        for p in &mut self.live {
            p.age_ms += dt_ms;
        }
        self.live.retain(|p| !p.finished());
    }

    /// Whether any mote is still alive (including ones still waiting out a delay).
    pub fn any_alive(&self) -> bool {
        !self.live.is_empty()
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.pending = false;
    }
}

/// The NetHack-style beam glyph for segment `i` of a traced line: `-`
/// horizontal, `|` vertical, `\` and `/` for the two diagonals (screen space, so
/// y grows downward). Direction is taken from the neighbouring points.
fn beam_glyph(pts: &[(u16, u16)], i: usize) -> char {
    if pts.len() < 2 {
        return '*';
    }
    let (ax, ay) = pts[i.saturating_sub(1).min(pts.len() - 2)];
    let (bx, by) = pts[(i + 1).min(pts.len() - 1)];
    let dx = bx as i32 - ax as i32;
    let dy = by as i32 - ay as i32;
    match (dx.signum(), dy.signum()) {
        (_, 0) => '-',
        (0, _) => '|',
        (a, b) if a == b => '\\',
        _ => '/',
    }
}

/// Clamp helper: keep an animation tile on the map before it is queued.
pub fn on_map(x: i32, y: i32) -> Option<(u16, u16)> {
    if x >= 0 && y >= 0 && (x as u16) < MAP_WIDTH && (y as u16) < MAP_HEIGHT {
        Some((x as u16, y as u16))
    } else {
        None
    }
}
