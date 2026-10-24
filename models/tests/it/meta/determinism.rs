//! The seed contract.
//!
//! A run's dungeon must be reproducible, and a floor's *layout* must depend on
//! nothing but the seed and the depth. That is what lets a save rebuild its
//! floor from two numbers, and what stops a new row in a content table from
//! quietly rearranging everybody's dungeon.
//!
//! A floor's *contents* are looser: they depend on the seed, the depth, and the
//! staircase count (`FloorChanges`), so a repeat visit re-stocks the same
//! layout with different things. What they must still *not* depend on is the
//! shared `GameRng` — the blow-by-blow of the run — because that would tie a
//! floor to how a fight went rather than to a clean count.
