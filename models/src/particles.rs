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

/// How long an explosion's ripple takes to cross one tile of radius. Above the
/// ~33ms frame period so the ring visibly steps outward ring by ring instead
/// of flashing all at once; shared with [`Particles::secondary_burst`] so a
/// target's cosmetic echo is timed to land after the primary ripple reaches it.
const RIPPLE_MS_PER_TILE: f32 = 40.0;

/// The colour family an area blast burns in. Each maps to a five-keyframe
/// glyph/colour cycle every cell in the blast steps through as it fades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlastPalette {
    /// Wand of fire — white-hot core to dark-red embers.
    Fire,
    /// Wand of cold — white flash to deep frost blue.
    Frost,
    /// Wand of lightning — a yellow-white crackle.
    Spark,
    /// Wand of magic missile — clean cyan arcana.
    Arcane,
    /// Wand of striking — a colourless concussive thump.
    Force,
    /// Wand of drain life — sickly magenta bleeding to dark red.
    Drain,
    /// Wand of light, thrown — a blinding gold-white flash.
    Dazzle,
    /// Polymorph / haste / slow / teleport — unstable green warp light.
    Warp,
    /// Wand of cancellation — a dead grey wave.
    Void,
}

impl BlastPalette {
    fn frames(self) -> [(char, Color); 5] {
        use Color::*;
        match self {
            BlastPalette::Fire => [
                ('#', White),
                ('@', Yellow),
                ('*', DarkYellow),
                ('+', Red),
                ('·', DarkRed),
            ],
            BlastPalette::Frost => [
                ('*', White),
                ('+', Cyan),
                ('+', Blue),
                (':', DarkBlue),
                ('·', DarkCyan),
            ],
            BlastPalette::Spark => [
                ('#', White),
                ('*', Yellow),
                ('+', Yellow),
                ('/', DarkYellow),
                ('·', DarkGrey),
            ],
            BlastPalette::Arcane => [
                ('*', White),
                ('+', Cyan),
                ('*', Cyan),
                (':', DarkCyan),
                ('·', DarkCyan),
            ],
            BlastPalette::Force => [
                ('#', White),
                ('*', Grey),
                ('+', Grey),
                (':', DarkGrey),
                ('·', DarkGrey),
            ],
            BlastPalette::Drain => [
                ('*', White),
                ('+', Magenta),
                ('*', DarkMagenta),
                (':', DarkMagenta),
                ('·', DarkRed),
            ],
            BlastPalette::Dazzle => [
                ('*', White),
                ('#', White),
                ('@', Yellow),
                ('+', White),
                ('·', DarkYellow),
            ],
            BlastPalette::Warp => [
                ('*', White),
                ('+', Green),
                ('*', Green),
                (':', DarkGreen),
                ('·', DarkGreen),
            ],
            BlastPalette::Void => [
                ('#', White),
                ('*', Grey),
                ('+', DarkGrey),
                (':', DarkGrey),
                ('·', DarkGrey),
            ],
        }
    }
}

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

/// Global multiplier on every animation frame's on-screen hold time —
/// particles and the magic-mapping reveal wipe alike. `1.0` is the default
/// pacing; the escape hatch for a terminal whose redraw can't keep up (raise
/// it) or that renders the default pacing too slowly to feel snappy (lower
/// it). Set once at startup from the `-anim-rate` CLI flag; never changes
/// mid-run.
#[derive(Resource, Clone, Copy)]
pub struct AnimRate(pub f32);

impl Default for AnimRate {
    fn default() -> Self {
        Self(1.0)
    }
}

impl AnimRate {
    /// Scales a base frame duration (ms) by this rate, floored at 1ms so a
    /// pathological rate can never freeze the animation loop outright.
    pub fn scale(self, base_ms: u64) -> u64 {
        ((base_ms as f32) * self.0).max(1.0) as u64
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

    /// The instant a spray of blood physically lands — the wound tile itself,
    /// or wherever a droplet's streak comes to rest, floor or wall alike. A
    /// short, sharp red flash; `delay_ms` lets it land right as its
    /// [`Particles::blood_streak`] finishes travelling.
    pub fn blood_hit(&mut self, x: u16, y: u16, delay_ms: f32) {
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 90.0,
            age_ms: 0.0,
            frames: vec![('*', Color::Red), ('.', Color::DarkRed)],
        });
    }

    /// A droplet's flight from a wound to wherever it splatters: a trail of red
    /// dots — unlike [`Particles::beam`], blood doesn't need a directional
    /// glyph to read as a spray. A little faster than a thrown item's
    /// [`Particles::hurl`] (70ms/cell) — droplets fly quicker than a hand can
    /// throw — but not by much; it should still read as a spray, not a shot.
    /// `pts` is the traced line, the wound tile excluded. Returns the flight's
    /// total duration in ms, so a caller can time a [`Particles::blood_hit`] to
    /// land right as this finishes.
    pub fn blood_streak(&mut self, pts: &[(u16, u16)]) -> f32 {
        const TRAVEL_MS_PER_CELL: f32 = 55.0;
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                lifetime_ms: TRAVEL_MS_PER_CELL,
                age_ms: 0.0,
                frames: vec![('·', Color::Red), ('·', Color::DarkRed)],
            });
        }
        pts.len() as f32 * TRAVEL_MS_PER_CELL
    }

    /// A wand bolt: a directional streak (`- | \ /`) that races cell by cell
    /// from the caster to the point of impact, each cell flaring white then
    /// settling to `color` then guttering out. `pts` is the traversed line in
    /// map coordinates, caster's own tile excluded. Returns the flight's total
    /// duration in ms, so a caller can time a [`Particles::impact_sparks`] to
    /// land right as the beam arrives.
    pub fn beam(&mut self, pts: &[(u16, u16)], color: Color) -> f32 {
        // Above the ~33ms frame period, so the head visibly advances cell by
        // cell instead of the whole line's long-lived cells all lighting up
        // together on the first frame or two.
        const TRAVEL_MS_PER_CELL: f32 = 40.0;
        for (i, &(x, y)) in pts.iter().enumerate() {
            let glyph = beam_glyph(pts, i);
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                // Cells nearer the caster linger a touch longer, leaving a tail.
                lifetime_ms: 150.0 + (pts.len() - i) as f32 * 10.0,
                age_ms: 0.0,
                // Flickers white/colour twice before settling into a dim dot —
                // flashier than a single white-to-colour fade.
                frames: vec![
                    (glyph, Color::White),
                    (glyph, color),
                    (glyph, Color::White),
                    (glyph, color),
                    ('·', color),
                ],
            });
        }
        pts.len() as f32 * TRAVEL_MS_PER_CELL
    }

    /// A denser, more colourful flourish at a wand bolt's point of impact — a
    /// small ring of offset sparks around the landing cell, on top of the
    /// beam's own last frame there, so a wand hit reads as an actual event and
    /// not just the beam quietly stopping. Each spark flickers white/`color`
    /// like the beam itself, staggered a beat apart so the ring reads as a
    /// quick outward flash rather than everything popping at once. `delay_ms`
    /// times it to land right as the beam (see [`Particles::beam`]'s return)
    /// actually arrives.
    pub fn impact_sparks(&mut self, x: u16, y: u16, color: Color, delay_ms: f32) {
        const RING: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 170.0,
            age_ms: 0.0,
            frames: vec![
                ('‼', Color::White),
                ('*', color),
                ('+', Color::White),
                ('·', color),
            ],
        });
        for (i, &(dx, dy)) in RING.iter().enumerate() {
            let Some((sx, sy)) = on_map(x as i32 + dx, y as i32 + dy) else {
                continue;
            };
            self.push(Particle {
                x: sx,
                y: sy,
                delay_ms: delay_ms + 15.0 + i as f32 * 12.0,
                lifetime_ms: 130.0,
                age_ms: 0.0,
                frames: vec![('*', Color::White), ('+', color), ('.', color)],
            });
        }
    }

    /// A thrown object in flight: the item's own glyph hopping cell by cell from
    /// the thrower's hand to wherever it stops. Slower than a wand bolt — you
    /// can watch a dagger travel. `pts` is the traced line, thrower's own tile
    /// excluded.
    pub fn hurl(&mut self, pts: &[(u16, u16)], glyph: char, color: Color) {
        // Well above the ~33ms frame period, so every cell gets its own visible
        // frame (or two) instead of the flight blurring past between samples.
        const TRAVEL_MS_PER_CELL: f32 = 70.0;
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                lifetime_ms: TRAVEL_MS_PER_CELL * 1.4,
                age_ms: 0.0,
                frames: vec![(glyph, color)],
            });
        }
    }

    /// A DCSS-style area blast. `cells` is `(x, y, distance_from_centre)` for
    /// every tile the blast covers (already LOS-checked by the caller); the ring
    /// expands outward from the core and every cell cycles through `palette`'s
    /// colours before fading. Always opens on the palette's bright first frame —
    /// a primary blast never reads as dark.
    pub fn explosion(&mut self, cells: &[(u16, u16, f32)], palette: BlastPalette) {
        let frames: [(char, Color); 5] = palette.frames();
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

    /// A small, muted echo of an area blast — the cosmetic-only flash on one
    /// creature an effect wand's grenade actually caught, timed to land a beat
    /// after the primary blast's ripple has passed that tile. It marks who the
    /// effect landed on and nothing more: no separate gameplay effect rides on
    /// it. Skips the palette's bright opening frame (that belongs to the
    /// primary blast alone) and starts straight into its darker back half, so
    /// it reads as a fainter secondary pop rather than a second bright flash.
    pub fn secondary_burst(&mut self, x: u16, y: u16, radius: f32, palette: BlastPalette) {
        const FOLLOW_MS: f32 = 120.0;
        let frames = palette.frames();
        self.push(Particle {
            x,
            y,
            delay_ms: radius.ceil() * RIPPLE_MS_PER_TILE + FOLLOW_MS,
            lifetime_ms: 200.0,
            age_ms: 0.0,
            frames: frames[2..].to_vec(),
        });
    }

    /// Smoke billowing up over a fire or cold blast, a beat after its flames
    /// have already rippled through — grey and white, cosmetic only. `cells`
    /// is the same distance-tagged set [`Particles::explosion`] used for the
    /// primary blast, so the smoke follows the same ring pattern outward.
    pub fn smoke_burst(&mut self, cells: &[(u16, u16, f32)]) {
        const FOLLOW_MS: f32 = 150.0;
        let frames = [
            ('≈', Color::White),
            ('≈', Color::Grey),
            ('≈', Color::DarkGrey),
        ];
        for &(x, y, dist) in cells {
            self.push(Particle {
                x,
                y,
                delay_ms: dist * RIPPLE_MS_PER_TILE + FOLLOW_MS,
                lifetime_ms: 260.0,
                age_ms: 0.0,
                frames: frames.to_vec(),
            });
        }
    }

    /// A quick "poof" of smoke — the signature left behind by a teleport or a
    /// polymorph. Grey/white like [`Particles::smoke_burst`], but immediate:
    /// there's no primary blast for it to follow, so no ripple delay beyond
    /// whatever `delay_ms` the caller wants (0 for a single poof, a small
    /// stagger per tile for a ring of them).
    pub fn poof(&mut self, x: u16, y: u16, delay_ms: f32) {
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 220.0,
            age_ms: 0.0,
            frames: vec![
                ('≈', Color::White),
                ('≈', Color::Grey),
                ('≈', Color::DarkGrey),
            ],
        });
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
    if x < 0 || y < 0 || (x as u16) >= MAP_WIDTH || (y as u16) >= MAP_HEIGHT {
        return None;
    }
    Some((x as u16, y as u16))
}
