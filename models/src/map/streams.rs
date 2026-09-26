//! The three random streams a run draws from, and why they are three.
//!
//! [`GameRng`] is the live stream the *run* spends — every combat roll, every
//! item effect, every trap springing. [`FxRng`] is decoration's own, salted
//! apart so a firework's colour can never reshuffle the dice for everything
//! after it. And the per-floor streams below are derived from the seed rather
//! than drawn from either, which is what lets a save rebuild its map from
//! nothing but `(seed, depth)`.
//!
//! `models/tests/determinism.rs` is the test that holds all three apart.

use bevy_ecs::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

#[derive(Resource)]
pub struct GameRng(pub ChaCha12Rng);

/// The u64 seed the run's RNG was created from. Kept alongside the live RNG
/// state so future features (e.g. regenerating a specific floor) can reseed.
#[derive(Resource)]
pub struct RngSeed(pub u64);

/// A second RNG stream for animation and particle cosmetics only — a death
/// burst's fling direction, a blood splatter's spray — seeded from the same
/// run seed but salted apart from [`GameRng`], the same way
/// [`crate::identify::ItemAppearances`] gets its own stream. Nothing that
/// reads this ever feeds back into gameplay, so cosmetic rolls (or a feature
/// like `-nb` skipping them entirely) can never perturb the shared gameplay
/// stream everything else depends on for determinism.
///
/// Not saved: purely cosmetic, and nothing here is ever replayed, so a
/// reload just reseeds it fresh from the run's seed.
#[derive(Resource)]
pub struct FxRng(pub ChaCha12Rng);

impl FxRng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha12Rng::seed_from_u64(seed ^ 0xF12E_A5A5_C05E_u64))
    }
}

/// One floor's private RNG stream. `salt` picks *which* stream — the walls and
/// the things standing between them draw from two independent ones, so neither
/// can move the other — and `generation` advances a stream to a fresh state
/// without changing which stream it is (used to re-roll a floor's contents on a
/// repeat visit; `0` for the layout, which never changes).
fn floor_stream(seed: u64, depth: u8, salt: u64, generation: u64) -> ChaCha12Rng {
    let base = seed ^ salt.wrapping_mul(depth as u64 + 1);
    ChaCha12Rng::seed_from_u64(base ^ generation.wrapping_mul(GENERATION_SALT))
}

const LAYOUT_SALT: u64 = 0xF100_0BED_5EED;
const LEVEL_SALT: u64 = 0x5_BEC1_A11E_7E1;
const CONTENT_SALT: u64 = 0x0C0F_FEE0_D00D;
/// Odd multiplier that scatters [`FloorChanges`] across the seed space, so
/// consecutive visits to a floor are as unlike each other as two random seeds.
const GENERATION_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// The RNG a floor's **layout** is built from — rooms, corridors, doors, stairs.
///
/// This is the same trick [`initialize_world`] plays for item appearances, and
/// for the same reason. A floor's shape is a pure function of `(seed, depth)`,
/// so nothing else can move it: not the loot rolls, not a new row in a content
/// table, not how long the player spent fighting on the way down. Two
/// consequences worth knowing:
///
/// * A save can rebuild the exact floor it was written on from the seed and the
///   depth alone, which is why the map is not stored in the file.
/// * Climbing back to a floor you have already visited gives you the layout you
///   remember (the contents, though, are re-rolled — see [`content_rng`]).
pub fn layout_rng(seed: u64, depth: u8) -> ChaCha12Rng {
    floor_stream(seed, depth, LAYOUT_SALT, 0)
}

/// The RNG that decides whether a floor is a [`super::SpecialLevel`], and
/// which. A pure function of `(seed, depth)` like [`layout_rng`], but a stream
/// of its own: the roll costs the layout nothing, so every ordinary floor is
/// carved exactly as it was before special levels existed.
pub fn level_rng(seed: u64, depth: u8) -> ChaCha12Rng {
    floor_stream(seed, depth, LEVEL_SALT, 0)
}

/// The RNG a floor's **contents** are drawn from — which monsters, which loot,
/// which traps, where they stand, and what a drop rolls for enchantment and
/// charges.
///
/// A different stream from [`layout_rng`], and a function of `(seed, depth,
/// changes)` where `changes` is [`crate::components::FloorChanges`] — how many
/// times the player has taken a staircase, portal or trapdoor. So the *layout*
/// of a floor is fixed for a seed, but its *contents* change every time it is
/// built: walk back up through floor 7 and it is the same maze re-stocked with
/// different monsters and loot. Two runs on one seed that descend in lockstep
/// still see the same floors; a reload restores `changes`, so it lands on the
/// same re-roll.
///
/// This is not the shared [`GameRng`] and must never be. `GameRng` is the live
/// stream the *run* spends — combat rolls, item effects, traps springing — and
/// anything drawn from it while a floor is being built would tie that floor back
/// to the player's blow-by-blow history rather than to the clean
/// staircase count. `models/tests/determinism.rs` exists to catch that.
pub fn content_rng(seed: u64, depth: u8, changes: u32) -> ChaCha12Rng {
    floor_stream(seed, depth, CONTENT_SALT, changes as u64)
}
