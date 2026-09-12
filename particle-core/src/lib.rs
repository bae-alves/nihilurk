//! The arithmetic half of roog's particle layer: when a mote is visible, which
//! of its keyframes is showing, which glyph a beam segment draws, and where a
//! ripple's delay comes from.
//!
//! It is `no_std`, allocates nothing, and depends on no crate at all -- not
//! `crossterm`, not `bevy_ecs`, not `core::fmt`. That is the whole reason it is
//! a separate crate rather than a module of `models`: this is the only part of
//! the game that can be *proved* to run somewhere without an operating system,
//! and `compat/` proves it on every build by compiling this crate for
//! `riscv32imc-unknown-none-elf` and `xtensa-esp32-none-elf`.
//!
//! The proof is only worth something because the game runs this code and not a
//! copy of it. [`models::particles`] owns the `Vec` of keyframes, the colour
//! type and the ECS resource; every decision it makes about *time* is one of
//! the functions below. If this crate is wrong, the game is wrong, and
//! `models/tests/particles.rs` fails -- which is the property `perf/src/screen.rs`
//! deliberately gave up and has regretted in writing ever since (see
//! `docs/explanation/performance-testing.md`, "The grid is a copy").
//!
//! # What is deliberately absent
//!
//! No colour type appears anywhere in this crate. A keyframe lookup returns an
//! *index*, not a `(char, Color)`, so the caller's palette type -- crossterm's
//! on a terminal, a packed RGB565 on a display panel -- never has to cross the
//! boundary. That is what keeps the crate portable, and it costs the caller one
//! index into a slice it already owns.
//!
//! No `f32` method from `std` is reached for unconditionally either. `sqrt`,
//! `ceil`, `floor` and friends live in `std` rather than `core` because they
//! lower to libm calls a bare-metal target has no reason to link. [`ceil`] is
//! the one roog needs, and it is the crate's single `#[cfg(feature = "std")]`:
//! hosted builds take the intrinsic, bare-metal builds take a hand-rolled
//! branch, and `tests/parity.rs` proves the two agree over the domain roog
//! uses. See [`ceil`].

#![no_std]

// `f32::ceil` is an inherent method defined in `std`, so reaching it takes more
// than turning the feature on: the crate has to link `std` for the method to be
// in scope at all. This is the only line in the crate that knows what an
// operating system is, and it is behind the flag.
#[cfg(feature = "std")]
extern crate std;

/// How long an explosion's ripple takes to cross one tile of radius.
///
/// Above the ~33 ms frame period, so the ring visibly steps outward ring by
/// ring instead of flashing all at once. `models::Particles` re-exports this
/// rather than declaring its own, so a blast and the cosmetic echo timed to
/// follow it cannot drift apart.
pub const RIPPLE_MS_PER_TILE: f32 = 40.0;

/// Whether a mote has been born and is not yet dead.
///
/// `age_ms` is measured from when the *batch* was queued, not from when this
/// mote starts drawing -- which is what lets one batch ripple outward, every
/// mote sharing a clock and differing only in `delay_ms`.
#[inline]
pub fn visible(age_ms: f32, delay_ms: f32, lifetime_ms: f32) -> bool {
    age_ms >= delay_ms && age_ms < delay_ms + lifetime_ms
}

/// Whether a mote is past its lifetime and can be culled.
///
/// Note this is not `!visible`: a mote still waiting out its delay is neither
/// visible nor finished, and culling it would delete a ripple before it
/// arrived.
#[inline]
pub fn finished(age_ms: f32, delay_ms: f32, lifetime_ms: f32) -> bool {
    age_ms >= delay_ms + lifetime_ms
}

/// Which keyframe a mote is showing, or `None` if it is still waiting out its
/// delay or has already expired.
///
/// The mote steps through its `frames` evenly across its lifetime, so a spark
/// can decay `‼ → * → +` on a clock rather than on a frame counter -- the same
/// animation plays at 30 fps and at 8.
///
/// `frames` is the number of keyframes, and 0 yields `None` rather than
/// panicking: a mote with no keyframes cannot draw, and the caller indexing a
/// slice it owns should not have to prove that twice.
///
/// The `0.999` clamp is what keeps the last frame reachable without the index
/// running off the end: at `t == 1.0` exactly, `t * frames` is `frames`.
#[inline]
pub fn keyframe(age_ms: f32, delay_ms: f32, lifetime_ms: f32, frames: usize) -> Option<usize> {
    if frames == 0 || !visible(age_ms, delay_ms, lifetime_ms) {
        return None;
    }
    let t = ((age_ms - delay_ms) / lifetime_ms).clamp(0.0, 0.999);
    let idx = (t * frames as f32) as usize;
    Some(idx.min(frames - 1))
}

/// The NetHack-style beam glyph for segment `i` of a traced line: `-`
/// horizontal, `|` vertical, `\` and `/` for the two diagonals.
///
/// Screen space, so y grows downward and the diagonals read the way they look
/// rather than the way a graph would draw them. Direction is taken from the
/// neighbouring points, which is why a one-cell "line" has no direction to
/// report and falls back to `*`.
pub fn beam_glyph(pts: &[(u16, u16)], i: usize) -> char {
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

/// Keep an animation tile on the map before it is queued.
///
/// The dimensions are parameters rather than the `MAP_WIDTH`/`MAP_HEIGHT`
/// constants because this crate knows nothing about roog's map; `models`
/// supplies them at the one call site.
#[inline]
pub fn on_map(x: i32, y: i32, width: u16, height: u16) -> Option<(u16, u16)> {
    if x < 0 || y < 0 || (x as u16) >= width || (y as u16) >= height {
        return None;
    }
    Some((x as u16, y as u16))
}

/// The delay before a ripple reaches a cell `dist` tiles from the blast's core.
#[inline]
pub fn ripple_delay(dist: f32) -> f32 {
    dist * RIPPLE_MS_PER_TILE
}

/// The delay before a secondary flash lands on a creature `radius` tiles out:
/// after the primary ripple has swept past that tile, plus `follow_ms`.
///
/// Rounded *up* a whole tile deliberately. Landing the echo half a tile early
/// puts it inside the primary blast's own bright frames, where it is invisible;
/// a beat late is what makes it read as a second event.
#[inline]
pub fn follow_delay(radius: f32, follow_ms: f32) -> f32 {
    ceil(radius) * RIPPLE_MS_PER_TILE + follow_ms
}

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
/// [`SHAKE_PATTERN`] exists for, quietly lost. At 32 ms against a 33 ms frame
/// the two stay in lockstep for 32 frames, which is longer than any shake roog
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
const SHAKE_PATTERN: [(i8, i8); 6] = [(1, 0), (-1, 1), (1, -1), (-1, 0), (1, 1), (-1, -1)];

/// Where the screen sits `age_ms` into a shake of `duration_ms` that started at
/// `amplitude` cells, as a whole-cell displacement.
///
/// A terminal cannot displace by half a cell, so the decay is in the
/// *amplitude*, not in a smooth position: the shake steps through
/// [`SHAKE_PATTERN`] at a fixed rate while the radius it throws the map to
/// shrinks from `amplitude` to 1, then stops dead. Rounded *up* (like
/// [`follow_delay`], and for a kindred reason): the last few steps of a decay
/// that rounded down would be displacements of zero -- a shake that is still
/// nominally running while nothing on screen moves, which reads as a stutter
/// at the end rather than a finish.
///
/// Returns `(0, 0)` once the shake is spent, and for a zero or negative
/// `duration_ms`, so a caller need not special-case the resting state.
#[inline]
pub fn shake_offset(age_ms: f32, duration_ms: f32, amplitude: i8) -> (i8, i8) {
    if duration_ms <= 0.0 || age_ms >= duration_ms || amplitude <= 0 {
        return (0, 0);
    }
    let step = (age_ms.max(0.0) / SHAKE_STEP_MS) as usize;
    let (dx, dy) = SHAKE_PATTERN[step % SHAKE_PATTERN.len()];
    let remaining = 1.0 - age_ms.max(0.0) / duration_ms;
    let magnitude = ceil(amplitude as f32 * remaining) as i8;
    (dx * magnitude, dy * magnitude)
}

/// `f32::ceil`, on a target that may not have one.
///
/// This is the crate's one real fork in the road, and the reason it has a
/// feature flag at all. `f32::ceil` lives in `std`, not `core`: it is an
/// intrinsic that lowers to a libm call (or, on x86-64, a single `roundss`),
/// and a bare-metal target has no libm to lower to. So a hosted build takes the
/// intrinsic and a `no_std` build takes the hand-rolled branch below.
///
/// The hand-rolled version is exact over the domain roog uses it on -- finite
/// values within `i32`'s range, in practice a blast radius of 0..=20 -- and
/// that is all it claims. Rust's float-to-int casts saturate rather than wrap,
/// so a NaN comes back as 0.0 instead of misbehaving, but no caller should be
/// there. `tests/parity.rs` runs both branches against `f32::ceil` across that
/// domain, which is what stops the feature flag from quietly changing what the
/// game draws.
#[inline]
pub fn ceil(x: f32) -> f32 {
    #[cfg(feature = "std")]
    {
        x.ceil()
    }
    #[cfg(not(feature = "std"))]
    {
        let truncated = x as i32 as f32;
        if truncated < x {
            return truncated + 1.0;
        }
        truncated
    }
}

/// The `no_std` branch of [`ceil`], always compiled, so the parity test can
/// reach it on a hosted build where `ceil` itself resolves to the intrinsic.
/// Not public API: nothing but the test should call this instead of [`ceil`].
#[doc(hidden)]
#[inline]
pub fn ceil_portable(x: f32) -> f32 {
    let truncated = x as i32 as f32;
    if truncated < x {
        return truncated + 1.0;
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mote_waits_out_its_delay_then_draws_then_dies() {
        // 100 ms of delay, 200 ms of life, two keyframes.
        assert_eq!(keyframe(50.0, 100.0, 200.0, 2), None, "still waiting");
        assert_eq!(keyframe(100.0, 100.0, 200.0, 2), Some(0), "first frame");
        assert_eq!(keyframe(250.0, 100.0, 200.0, 2), Some(1), "second half");
        assert_eq!(keyframe(300.0, 100.0, 200.0, 2), None, "expired");
    }

    #[test]
    fn waiting_is_neither_visible_nor_finished() {
        assert!(!visible(10.0, 100.0, 200.0));
        assert!(
            !finished(10.0, 100.0, 200.0),
            "a mote still waiting out its delay must not be culled"
        );
    }

    #[test]
    fn the_last_keyframe_is_reachable_and_the_index_never_runs_off() {
        // The one off-by-one this function exists to prevent.
        for frames in 1..8 {
            let last = keyframe(199.9, 0.0, 200.0, frames);
            assert_eq!(last, Some(frames - 1), "{frames} keyframes");
        }
    }

    #[test]
    fn no_keyframes_draws_nothing() {
        assert_eq!(keyframe(0.0, 0.0, 100.0, 0), None);
    }

    #[test]
    fn beam_segments_take_their_glyph_from_the_run_of_the_line() {
        let horizontal: [(u16, u16); 3] = [(1, 5), (2, 5), (3, 5)];
        assert_eq!(beam_glyph(&horizontal, 1), '-');
        let vertical: [(u16, u16); 3] = [(5, 1), (5, 2), (5, 3)];
        assert_eq!(beam_glyph(&vertical, 1), '|');
        let down_right: [(u16, u16); 3] = [(1, 1), (2, 2), (3, 3)];
        assert_eq!(beam_glyph(&down_right, 1), '\\');
        let up_right: [(u16, u16); 3] = [(1, 3), (2, 2), (3, 1)];
        assert_eq!(beam_glyph(&up_right, 1), '/');
        assert_eq!(beam_glyph(&[(1, 1)], 0), '*', "a point has no direction");
    }

    #[test]
    fn off_map_tiles_are_dropped() {
        assert_eq!(on_map(-1, 4, 80, 22), None);
        assert_eq!(on_map(4, -1, 80, 22), None);
        assert_eq!(on_map(80, 4, 80, 22), None);
        assert_eq!(on_map(4, 22, 80, 22), None);
        assert_eq!(on_map(0, 0, 80, 22), Some((0, 0)));
        assert_eq!(on_map(79, 21, 80, 22), Some((79, 21)));
    }

    #[test]
    fn a_shake_starts_at_its_amplitude_and_ends_at_rest() {
        assert_eq!(shake_offset(0.0, 200.0, 2), (2, 0), "opens at full throw");
        assert_eq!(shake_offset(200.0, 200.0, 2), (0, 0), "spent");
        assert_eq!(shake_offset(999.0, 200.0, 2), (0, 0), "long spent");
        assert_eq!(shake_offset(0.0, 0.0, 2), (0, 0), "no duration, no shake");
        assert_eq!(shake_offset(10.0, 200.0, 0), (0, 0), "no amplitude");
    }

    #[test]
    fn every_step_of_a_shake_actually_displaces_something() {
        // The stutter this function exists to prevent: a decay that rounds down
        // spends its last steps sitting at (0, 0) while still claiming to run.
        let duration = 460.0;
        let mut age = 0.0;
        while age < duration {
            let (dx, dy) = shake_offset(age, duration, 2);
            assert!(
                dx != 0 || dy != 0,
                "{age}ms into a {duration}ms shake the map stood still"
            );
            age += SHAKE_STEP_MS;
        }
    }

    #[test]
    fn consecutive_steps_throw_the_map_back_the_other_way() {
        // What separates a shake from a drift: x never repeats its sign.
        let duration = 1000.0; // long enough that amplitude never decays to 0
        let signs: [i8; 6] = core::array::from_fn(|i| {
            shake_offset(i as f32 * SHAKE_STEP_MS, duration, 1)
                .0
                .signum()
        });
        for pair in signs.windows(2) {
            assert_eq!(pair[0], -pair[1], "two steps ran the same way: {signs:?}");
        }
    }

    #[test]
    fn the_throw_shrinks_as_the_shake_runs_out() {
        // Amplitude 2 spends its first half at 2 cells and its second at 1.
        assert_eq!(shake_offset(0.0, 400.0, 2).0.abs(), 2);
        assert_eq!(shake_offset(210.0, 400.0, 2).0.abs(), 1);
        assert_eq!(shake_offset(399.0, 400.0, 2).0.abs(), 1);
    }

    #[test]
    fn the_echo_lands_after_the_ripple_has_passed() {
        // A creature 2.5 tiles out is swept at 100 ms; the echo must not be
        // inside that.
        assert!(follow_delay(2.5, 120.0) > ripple_delay(2.5));
    }
}
