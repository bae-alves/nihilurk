//! Conditions on the player, traps and snares, and the score.

use super::*;

/// A promise the dungeon made you, and the terms are the same for both of the
/// coins that make one: **reach the next staircase without being hurt again**
/// and it pays out. Take a single point of damage and it is gone, with a line
/// saying so.
///
/// [`Plated`] pays a permanent point of attack or defence die — the coin flip
/// is the dungeon's, not yours. The count is how many coins are owed; they
/// stack to three, all pay at the staircase, and one blow takes them all. [`Forged`] pays a point of plus on the weapon
/// in your hand or the armour on your back, exactly as the matching scroll
/// would, curse and all.
///
/// They are conditions and not items because that is how they behave: carried
/// on the player, shown on the HUD (`PLAT`, `FORG`), and lost to something that
/// happens *to* you. Unlike every other condition they are not lifted by the
/// staircase — the staircase is what cashes them
/// (`crate::items::pickups::settle_promises`).
#[derive(Component)]
pub struct Plated(pub u8);

/// The forge's half of the same bargain. See [`Plated`].
#[derive(Component)]
pub struct Forged(pub u8);

// ===========================================================================
// Traps and snares
// ===========================================================================

/// How a trap becomes known to the player before it is triggered. Rolled once,
/// with equal probability, when the trap spawns; read by `crate::visibility`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrapReveal {
    /// Revealed as soon as its tile is in the player's viewshed.
    Sight,
    /// Revealed once the player is on an orthogonally/diagonally adjacent tile.
    Adjacent,
    /// Never revealed until it goes off.
    Triggered,
}

/// The trap marker component. `revealed` latches: once a trap is known it stays
/// drawn (in fog-of-war grey when out of sight), like a discovered staircase.
/// Owned by `crate::traps` (`trap_system`, `spring_trap`) and
/// `crate::visibility`.
#[derive(Component)]
pub struct Trap {
    /// Which trap this is: its row, which says what it does to whatever
    /// steps on it.
    pub row: &'static crate::traps::TrapDef,
    /// How the player gets to know it is there before it goes off.
    pub reveal: TrapReveal,
    /// Whether the player knows about it yet. Once true, it stays true.
    pub revealed: bool,
}

/// Marker inserted on any actor that changed [`Position`] this turn, so
/// `crate::traps::trap_system` knows whose feet to check. Transient: cleared at
/// the end of every `trap_system` run and never serialised.
#[derive(Component)]
pub struct EntityMoved;

// ===========================================================================
// Score
// ===========================================================================

/// The running score shown on the HUD. Coins and the relic add to it on pickup.
///
/// `i64`, not `i32`: nothing caps how many rings of adornment a dungeon hands
/// out and every one of them doubles this. Every write to it goes through
/// [`crate::score`], which saturates rather than wraps — a score is always a
/// multiple of a hundred, and a multiple of a hundred that wraps lands on
/// exactly zero.
#[derive(Component)]
pub struct Score {
    /// The run's score so far.
    pub value: i64,
}
