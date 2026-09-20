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

/// Every decision this module makes about *time* -- when a mote is visible,
/// which keyframe it is showing, how long a ripple takes to reach a tile --
/// lives in `particle-core`, a `no_std` crate with no dependencies at all. This
/// module owns what a microcontroller cannot have: the `Vec` of keyframes, the
/// crossterm colour, and the ECS resource.
///
/// The split is what makes `compat/`'s bare-metal check worth running. It
/// compiles `particle-core` for an ESP32 and a RISC-V board, and because the
/// game calls the same functions rather than a copy of them, a green check
/// there is a statement about nihilurk. See
/// `docs/explanation/cross-platform-testing.md`.
use particle_core as core_math;

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
    /// Sickly magenta bleeding to dark red. Retired from active duty (the
    /// wand of drain life now burns [`Death`](BlastPalette::Death) instead)
    /// but kept as its own palette rather than deleted.
    Drain,
    /// A blinding gold-white flash. Retired the same way `Drain` was — the
    /// wand of light and the spell Lux both burn
    /// [`Glam`](BlastPalette::Glam) now.
    Dazzle,
    /// Polymorph / haste / slow / teleport — unstable green warp light.
    Warp,
    /// Wand of cancellation — a dead grey wave.
    Void,
    /// The ULTIMATE TRICK SHOT — the Element of Yoord answering a missile.
    /// White through magenta to dark magenta, the only blast in the game that
    /// nothing in the wand table can produce, and the only one that leaves
    /// smoke behind without being on fire.
    Ultimate,
    /// The wand of drain life, and the spell Circle of Death — unnecessary
    /// flames guttering straight to ash: a white flash, a beat of red-on-red
    /// fire, then everything the blast touched goes grey.
    Death,
    /// The wand of light, thrown, and the spell Lux — glam rock, cyan, and
    /// entirely too much of it. Also the spell Frost Nova's own star, layered
    /// on top of an `explosion` of this palette rather than driving it alone.
    Glam,
}

impl BlastPalette {
    /// The one colour that best stands in for this palette outside a blast's
    /// own five-frame cycle — the second frame, past the white flash every
    /// cycle opens on, so it actually reads as the element. Used to tint a
    /// thrown wand's mystic-grenade glyph while it's still in the air, before
    /// [`Particles::explosion`] takes over on impact.
    pub fn accent_color(self) -> Color {
        self.frames()[1].1
    }

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
            BlastPalette::Ultimate => [
                ('#', White),
                ('@', Magenta),
                ('*', DarkMagenta),
                ('+', Magenta),
                ('·', DarkMagenta),
            ],
            BlastPalette::Death => [
                ('#', White),
                ('@', Red),
                ('*', DarkRed),
                ('%', Grey),
                ('·', DarkGrey),
            ],
            BlastPalette::Glam => [
                ('*', White),
                ('✦', Cyan),
                ('✧', Cyan),
                ('+', DarkCyan),
                ('·', DarkCyan),
            ],
        }
    }
}

/// The seven colours a ring of adornment's fireworks come in: every bright
/// terminal colour there is, white excluded. White is what the rest of the
/// effect layer opens *every* burst on, so a white firework would read as one
/// more hit spark instead of the one moment in the run that is pure show. Bright
/// black is in — a firework the colour of the night it goes off against is
/// exactly the sort of joke a ring of adornment would make.
///
/// See [`Particles::firework`].
pub const GLORY_COLORS: [Color; 7] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::DarkGrey,
];

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
    /// Age past which the mote can be culled. Not the inverse of "drawing": a
    /// mote still waiting out its `delay_ms` is neither, and culling it would
    /// delete a ripple before it arrived.
    fn finished(&self) -> bool {
        core_math::finished(self.age_ms, self.delay_ms, self.lifetime_ms)
    }

    /// The `(glyph, colour)` to paint this frame, or `None` if the mote is still
    /// waiting out its delay or has already expired.
    ///
    /// The keyframe *choice* is arithmetic and lives in `particle-core`; what
    /// stays here is the one indexing step, because the keyframes are a `Vec`
    /// of a colour type that crate deliberately knows nothing about.
    pub fn current(&self) -> Option<(char, Color)> {
        let idx = core_math::keyframe(
            self.age_ms,
            self.delay_ms,
            self.lifetime_ms,
            self.frames.len(),
        )?;
        Some(self.frames[idx])
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
    /// How far into the batch the next mote queued is pushed back. A flight
    /// ([`Particles::hurl`], [`Particles::lob`]) advances it by its own span,
    /// so everything the flight goes on to cause — the impact spark, the
    /// blast, the corpse fling, the next flight — opens after the missile has
    /// landed rather than on top of it. Nothing else touches it, and
    /// [`Particles::clear`] drops it with the batch that earned it.
    hold_ms: f32,
}

impl Particles {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, mut p: Particle) {
        p.delay_ms += self.hold_ms;
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

    /// The glancing blow's answer to [`Particles::hit_spark`]: steel skidding
    /// off armour instead of biting into what is under it. Deliberately the
    /// same shape as a landed hit's spark and none of its colour — it opens on
    /// a hard white tick and goes cold immediately, where a real hit burns
    /// down through yellow to a red ember. Shorter, too, and it arms no screen
    /// shake: a glancing blow is the game telling the player the armour ate
    /// the swing, and it should read as less than the blow beside it.
    pub fn clink_spark(&mut self, x: u16, y: u16) {
        self.push(Particle {
            x,
            y,
            delay_ms: 0.0,
            lifetime_ms: 120.0,
            age_ms: 0.0,
            frames: vec![
                ('+', Color::White),
                ('×', Color::Grey),
                ('·', Color::DarkGrey),
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

    /// The flare of a charm taking hold on the tile its reader stands on: a
    /// bright core with a ring of sparks thrown off it, each one a beat behind
    /// the last so the burst reads as flying outward rather than appearing all
    /// at once. `color` says what kind of magic caught — orange for an
    /// enchantment biting into gear, magenta for a charm laid on a pair of
    /// hands.
    ///
    /// Unlike [`Particles::impact_sparks`] there is nothing arriving here: the
    /// ring opens on the colour rather than on a white impact tick, because
    /// nothing hit anything.
    pub fn spark_burst(&mut self, x: u16, y: u16, color: Color) {
        const RING: [(i32, i32); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];
        self.push(Particle {
            x,
            y,
            delay_ms: 0.0,
            lifetime_ms: 240.0,
            age_ms: 0.0,
            frames: vec![
                ('*', color),
                ('‼', Color::White),
                ('+', color),
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
                delay_ms: 40.0 + i as f32 * 18.0,
                lifetime_ms: 160.0,
                age_ms: 0.0,
                frames: vec![('+', color), ('*', Color::White), ('·', color)],
            });
        }
    }

    /// One firework of a ring of adornment's flourish: a whole small blast of
    /// its own, in one bright colour, on `(x, y)` and the ring of tiles around
    /// it. Eight of these go off around whoever put the ring on, `delay_ms`
    /// apart, so the set reads as a chain of detonations running round the
    /// wearer rather than one flat flash.
    ///
    /// The only effect in the game that opens on its colour and *stays* there:
    /// every other burst punctuates itself with a white tick, and white is the
    /// one colour a firework may not be — see [`GLORY_COLORS`].
    pub fn firework(&mut self, x: u16, y: u16, color: Color, delay_ms: f32) {
        const PETALS: [(i32, i32); 8] = [
            (0, -1),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ];
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 420.0,
            age_ms: 0.0,
            frames: vec![
                ('#', color),
                ('@', color),
                ('*', color),
                ('+', color),
                ('·', color),
            ],
        });
        for (i, &(dx, dy)) in PETALS.iter().enumerate() {
            let Some((px, py)) = on_map(x as i32 + dx, y as i32 + dy) else {
                continue;
            };
            self.push(Particle {
                x: px,
                y: py,
                delay_ms: delay_ms + 50.0 + i as f32 * 14.0,
                lifetime_ms: 240.0,
                age_ms: 0.0,
                frames: vec![('*', color), ('+', color), ('·', color)],
            });
        }
    }

    /// The mote that marks one creature an effect just landed on: the
    /// condition's own glyph (`z` asleep, `#` held, `?` confused) flashing over
    /// its tile and fading. Deliberately tiny — the lasting news is the tint the
    /// renderer paints under the creature for as long as the condition holds,
    /// and this only says *when* it took. `delay_ms` staggers a roomful so a
    /// scroll that catches six monsters reads as a wave crossing the room.
    pub fn condition_mark(&mut self, x: u16, y: u16, glyph: char, color: Color, delay_ms: f32) {
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 260.0,
            age_ms: 0.0,
            frames: vec![(glyph, Color::White), (glyph, color), (glyph, color)],
        });
    }

    /// A thrown object in flight: the item's own glyph hopping cell by cell from
    /// the thrower's hand to wherever it stops. Slower than a wand bolt — you
    /// can watch a dagger travel. `pts` is the traced line, thrower's own tile
    /// excluded.
    pub fn hurl(&mut self, pts: &[(u16, u16)], glyph: char, color: Color) {
        // Well above the ~33ms frame period, so every cell gets its own visible
        // frame (or two) instead of the flight blurring past between samples.
        const TRAVEL_MS_PER_CELL: f32 = 70.0;
        const LIFETIME_MS: f32 = TRAVEL_MS_PER_CELL * 1.4;
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                lifetime_ms: LIFETIME_MS,
                age_ms: 0.0,
                frames: vec![(glyph, color)],
            });
        }
        self.hold_ms += flight_span(pts.len(), TRAVEL_MS_PER_CELL, LIFETIME_MS);
    }

    /// A thrown wand in flight: a tumbling mystic grenade rather than the
    /// wand's own catalog glyph, tinted with the [`BlastPalette`] its charges
    /// will burst in ([`BlastPalette::accent_color`]). Noticeably slower than
    /// [`Particles::hurl`] — this is a lob, not a snap throw, and the whole
    /// point is watching it arc in before it goes off — while
    /// [`Particles::explosion`], the burst that follows on impact, stays as
    /// fast as any other blast. `pts` is the traced line, thrower's own tile
    /// excluded.
    pub fn lob(&mut self, pts: &[(u16, u16)], color: Color) {
        const TRAVEL_MS_PER_CELL: f32 = 130.0;
        const LIFETIME_MS: f32 = TRAVEL_MS_PER_CELL * 1.4;
        const TUMBLE: [char; 4] = ['o', 'O', '0', 'O'];
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * TRAVEL_MS_PER_CELL,
                lifetime_ms: LIFETIME_MS,
                age_ms: 0.0,
                frames: TUMBLE.iter().map(|&g| (g, color)).collect(),
            });
        }
        self.hold_ms += flight_span(pts.len(), TRAVEL_MS_PER_CELL, LIFETIME_MS);
    }

    /// A dying creature's corpse (`%`), flung away from the blow that killed
    /// it — see [`crate::helpers::death_burst`]. Heavier and a touch slower
    /// than a thrown item's [`Particles::hurl`] (this is a body, not a
    /// dagger), and unlike a thrown item it doesn't just wink out on landing:
    /// its final cell lingers much longer, flashing white on impact before
    /// settling into a dim, dead rest frame. `pts` is the traced flight path,
    /// the death tile excluded. Returns the flight's total duration in ms, so
    /// a caller can time a wall splatter to land right as the corpse arrives.
    ///
    /// `stretch` multiplies every duration in the flight: `1.0` for a monster,
    /// and the player's own death drags it out (see
    /// [`crate::helpers::death_burst`]) — the last thing a run does is worth
    /// watching, and it is the one death nobody has to be kept waiting *from*.
    pub fn death_fling(&mut self, pts: &[(u16, u16)], color: Color, stretch: f32) -> f32 {
        const TRAVEL_MS_PER_CELL: f32 = 65.0;
        let per_cell = TRAVEL_MS_PER_CELL * stretch;
        for (i, &(x, y)) in pts.iter().enumerate() {
            let last = i + 1 == pts.len();
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * per_cell,
                lifetime_ms: if last {
                    380.0 * stretch
                } else {
                    per_cell * 1.3
                },
                age_ms: 0.0,
                frames: if last {
                    vec![('%', Color::White), ('%', color), ('%', Color::DarkGrey)]
                } else {
                    vec![('%', color)]
                },
            });
        }
        pts.len() as f32 * per_cell
    }

    /// One shard of a death burst's bone shrapnel — a Mortal-Kombat-style
    /// flourish flying outward from the death tile alongside
    /// [`Particles::death_fling`]. Quicker and shorter-lived than the corpse
    /// itself, so the bones visibly outrace it before clattering out of
    /// sight. `pts` is the traced flight path, the death tile excluded.
    /// `stretch` is [`Particles::death_fling`]'s, applied the same way, so the
    /// shards of a slowed death stay ahead of its corpse rather than beating
    /// it off screen by a second.
    pub fn bone_shard(&mut self, pts: &[(u16, u16)], glyph: char, stretch: f32) {
        const TRAVEL_MS_PER_CELL: f32 = 45.0;
        let per_cell = TRAVEL_MS_PER_CELL * stretch;
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.push(Particle {
                x,
                y,
                delay_ms: i as f32 * per_cell,
                lifetime_ms: per_cell * 1.6,
                age_ms: 0.0,
                frames: vec![(glyph, Color::White), (glyph, Color::Grey)],
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
                delay_ms: core_math::ripple_delay(dist),
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
            delay_ms: core_math::follow_delay(radius, FOLLOW_MS),
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
                delay_ms: core_math::ripple_delay(dist) + FOLLOW_MS,
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
        self.tinted_poof(x, y, delay_ms, Color::Grey);
    }

    /// A [`Particles::poof`] that carries a colour through its middle frame, so
    /// the puff says *what kind* of vanishing it was — magenta for a creature
    /// yanked away by a teleport trap, against the plain grey of one that simply
    /// fell through a trapdoor. It opens white and settles to the same dark grey
    /// wisp either way; only the beat between them is tinted.
    pub fn tinted_poof(&mut self, x: u16, y: u16, delay_ms: f32, color: Color) {
        self.push(Particle {
            x,
            y,
            delay_ms,
            lifetime_ms: 220.0,
            age_ms: 0.0,
            frames: vec![('≈', Color::White), ('≈', color), ('≈', Color::DarkGrey)],
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
        self.hold_ms = 0.0;
    }
}

/// How long a flight of `cells` cells, one leaving every `per_cell` ms and each
/// holding for `lifetime`, stays on screen: the last cell's own delay plus its
/// hold. This is what a flight adds to [`Particles::hold_ms`], so the hold
/// covers the flight down to its final flicker rather than to the moment the
/// last cell lights up. A flight of nothing takes no time and holds nothing.
fn flight_span(cells: usize, per_cell: f32, lifetime: f32) -> f32 {
    match cells {
        0 => 0.0,
        n => (n - 1) as f32 * per_cell + lifetime,
    }
}

/// The NetHack-style beam glyph for segment `i` of a traced line: `-`
/// horizontal, `|` vertical, `\` and `/` for the two diagonals (screen space, so
/// y grows downward). Direction is taken from the neighbouring points.
fn beam_glyph(pts: &[(u16, u16)], i: usize) -> char {
    core_math::beam_glyph(pts, i)
}

/// Clamp helper: keep an animation tile on the map before it is queued.
pub fn on_map(x: i32, y: i32) -> Option<(u16, u16)> {
    core_math::on_map(x, y, MAP_WIDTH, MAP_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A five-cell traced line, thrower's own tile already excluded.
    fn path(len: u16) -> Vec<(u16, u16)> {
        (1..=len).map(|i| (i, 1)).collect()
    }

    /// When the last mote of the current batch goes dark.
    fn batch_end(fx: &Particles) -> f32 {
        fx.live
            .iter()
            .map(|p| p.delay_ms + p.lifetime_ms)
            .fold(0.0, f32::max)
    }

    /// When the mote queued last starts drawing.
    fn last_start(fx: &Particles) -> f32 {
        fx.live.last().expect("a mote was just queued").delay_ms
    }

    #[test]
    fn a_flight_lands_before_anything_it_caused_starts() {
        for (name, mut fx) in [
            ("hurl", {
                let mut fx = Particles::new();
                fx.hurl(&path(5), '↑', Color::Grey);
                fx
            }),
            ("lob", {
                let mut fx = Particles::new();
                fx.lob(&path(5), Color::Green);
                fx
            }),
        ] {
            let landed = batch_end(&fx);
            fx.hit_spark(5, 1);
            assert!(
                last_start(&fx) >= landed,
                "{name}: the impact opens at {}ms, {landed}ms before the flight is over",
                last_start(&fx)
            );
        }
    }

    #[test]
    fn a_lob_holds_the_screen_longer_than_a_hurl_does() {
        let mut hurled = Particles::new();
        hurled.hurl(&path(5), '↑', Color::Grey);
        hurled.hit_spark(5, 1);

        let mut lobbed = Particles::new();
        lobbed.lob(&path(5), Color::Green);
        lobbed.hit_spark(5, 1);

        assert!(
            last_start(&lobbed) > last_start(&hurled),
            "a lob is the slow half of the beat: {}ms vs a hurl's {}ms",
            last_start(&lobbed),
            last_start(&hurled)
        );
    }

    #[test]
    fn two_flights_in_one_turn_go_one_after_the_other() {
        let mut fx = Particles::new();
        fx.hurl(&path(4), '↑', Color::Grey);
        let first_landed = batch_end(&fx);
        let queued = fx.live.len();

        fx.hurl(&path(4), '↑', Color::Grey);
        let second_opens = fx.live[queued].delay_ms;

        assert!(
            second_opens >= first_landed,
            "the second arrow leaves at {second_opens}ms, while the first is still in the air until {first_landed}ms"
        );
    }

    #[test]
    fn a_flight_that_never_happened_holds_nothing_back() {
        let mut fx = Particles::new();
        fx.hurl(&[], '↑', Color::Grey);
        fx.hit_spark(1, 1);
        assert_eq!(last_start(&fx), 0.0);
    }

    #[test]
    fn the_hold_dies_with_the_batch_that_earned_it() {
        let mut fx = Particles::new();
        fx.hurl(&path(5), '↑', Color::Grey);
        fx.clear();
        fx.hit_spark(1, 1);
        assert_eq!(last_start(&fx), 0.0);
    }
}
