//! Every tuning knob in the particle arithmetic, in one place.
//!
//! These are presentation timings, not balance: they set how an effect *feels*.
//! `models/src/constants.rs` holds the numbers that balance the game; this file
//! holds the two clocks and the one table that the functions in this crate read.
//!
//! [`RIPPLE_MS_PER_TILE`] and [`SHAKE_STEP_MS`] are re-exported from the crate
//! root, so `particle_core::RIPPLE_MS_PER_TILE` keeps resolving. Change the value
//! here; nothing else needs to move unless a doc comment below says so.

/// How long an explosion's ripple takes to cross one tile of radius.
///
/// Above the ~33 ms frame period, so the ring visibly steps outward ring by
/// ring instead of flashing all at once. `models::Particles` re-exports this
/// rather than declaring its own, so a blast and the cosmetic echo timed to
/// follow it cannot drift apart.
pub const RIPPLE_MS_PER_TILE: f32 = 40.0;

/// How long the screen shake holds one displacement before snapping to the
/// next.
///
/// Deliberately a touch *under* the ~33 ms animation frame rather than over it,
/// which is the opposite of [`RIPPLE_MS_PER_TILE`]'s reasoning and for the
/// opposite reason. A ripple wants to be seen stepping outward, so its step is
/// slower than a frame. A shake wants every frame to land somewhere new: hold a
/// displacement across two frames and the eye reads a map that has *moved*
/// rather than one that is shaking. At 35 ms it did exactly that on the opening
/// two frames, which is the worst place to do it.
///
/// Under it, but only just. Drop much below the frame period and the sampling
/// starts *skipping* steps instead of repeating them, and a skipped step lands
/// twice in a row on the same side of the axis — the alternation
/// `SHAKE_PATTERN` exists for, quietly lost. At 32 ms against a 33 ms frame
/// the two stay in lockstep for 32 frames, which is longer than any shake nihilurk
/// has.
///
/// It stays a duration rather than "one step per frame" so `-anim-rate` retunes
/// how long the shake *lasts* without also retuning how fast it vibrates.
pub const SHAKE_STEP_MS: f32 = 32.0;

/// The displacements a shake cycles through, one per [`SHAKE_STEP_MS`].
///
/// Every entry flips the sign of its x against the one before it, so the map
/// is always being thrown back across the axis it just crossed -- that
/// alternation is what makes it read as a shake and not a drift. The y column
/// runs `0 + - 0 + -`, off-phase with x, so the pattern doesn't collapse into
/// a single diagonal line the eye can predict.
pub(crate) const SHAKE_PATTERN: [(i8, i8); 6] =
    [(1, 0), (-1, 1), (1, -1), (-1, 0), (1, 1), (-1, -1)];
