//! Tuning for the player: starting numbers (`player`) and `progression`.

// ===========================================================================
// The player
// ===========================================================================

/// The hero's opening line. These seed a *new* game only — a loaded save
/// carries its own numbers (see `saveload.rs`), so changing them never touches
/// a run in progress.
pub mod player {
    /// Starting (and maximum) hit points. The whole bestiary is tuned against a
    /// player on this much HP — see `combat-and-balance.md` — so raising this quietly
    /// makes the early floors safer across the board. There is no level-up:
    /// this is the number for the whole run, bar potions of raise level.
    pub const START_HP: i32 = 12;

    /// Starting armour die (`1d[armor]` on the defence roll), before the +1 ring
    /// mail the hero spawns wearing. Matches an unarmoured townsperson.
    pub const START_ARMOR: i32 = 3;

    /// Starting power die (`1d[power]` on the attack roll), before the +1 mace.
    pub const START_POWER: i32 = 3;

    /// Starting (and maximum) magic points — the pool abilities draw on, shown
    /// as `Ma X/Y` on the HUD. Refilled in full by every staircase, alongside
    /// the arrival heal ([`crate::constants::progression::DESCENT_HEAL_DIVISOR`]).
    pub const START_MAGIC: u8 = 4;

    /// Sight radius, in tiles, for the player's Viewshed. Rooms flood-fill on
    /// top of this; corridors and dark rooms are cut back to the always-on 3x3.
    ///
    /// A loaded save keeps the range it was saved with (`saveload.rs` restores
    /// it), so this too only affects new games.
    pub const SIGHT_RANGE: u16 = 12;

    /// Fraction of max HP at or below which the player gets a one-time "badly
    /// wounded" warning as they cross down into it. See
    /// `crate::helpers::apply_damage`.
    pub const LOW_HP_WARNING_FRACTION: f32 = 0.3;
}

// ===========================================================================
// Progression / the descent
// ===========================================================================

/// Depth, the Dungeon Lord's patience, and what a staircase does for you.
pub mod progression {
    /// The deepest floor of a run. It has no down-stair — the Element of Yoord
    /// sits where the stair would be, and carrying it back out is the game.
    ///
    /// **If you change this:** the Element-of-Yoord placement and the
    /// staircase-inversion logic key off it automatically, but
    /// `gdd.md` names the final depth in
    /// prose and would need a pass. Monster/trap `min_depth` values in the catalog
    /// are relative to 1, not to this, so deep content still appears — just
    /// with fewer floors to spread over.
    pub const FINAL_DEPTH: u8 = 13;

    /// Turns the player may dawdle on one level before the Dungeon Lord loses
    /// patience and portals them onward — down on the way in, up once they
    /// carry the Element. Reset by every level change. Lower is crueller; this
    /// is the clock the whole "one room past the Lord's patience" death runs
    /// on.
    pub const DUNGEON_LORD_PATIENCE: u32 = 260;

    /// A staircase is a rest: arriving on a new floor heals
    /// `max_hp / DESCENT_HEAL_DIVISOR` and refills the magic pool in full. A
    /// trapdoor plunge is *not* a rest and skips both. Set the divisor to 1 for
    /// a full heal on every floor (a much easier run); a larger divisor makes
    /// attrition bite sooner.
    pub const DESCENT_HEAL_DIVISOR: i32 = 3;

    /// The last floor of each floor-crowding tier below the deepest one. The
    /// monster and trap *budgets* (`constants::population`, spent in `map.rs`)
    /// gain a slot and widen their fill odds at each of these depths. Each entry
    /// closes a tier, and the floors after the last entry, up to
    /// [`FINAL_DEPTH`], are the hardest band.
    ///
    /// The damage traps scale on their own, coarser bands
    /// ([`crate::constants::traps::TRAP_DAMAGE_TIER_LAST_DEPTH`]).
    ///
    /// **If you change this:** `map::difficulty_tier` reads it, and `gdd.md`
    /// and `docs/how-to/tune-rarity-and-depth.md` name the boundaries.
    pub const DIFFICULTY_TIER_LAST_DEPTH: [u8; 4] = [3, 6, 9, 12];
}
