//! Tuning for creatures: `monsters`, `spirits`, `conditions`, `speed`, `lurk`, `abilities`.

/// The lurk: the other playable species. Quadruped, fanged, clawed, furred,
/// and carrying nothing it did not grow itself.
///
/// Its two dice start one under nihil's *bare* ones
/// ([`crate::constants::player::START_POWER`], [`crate::constants::player::START_ARMOR`]), and nihil walks out of
/// the gate holding a mace and wearing ring mail that the lurk never will, so
/// where nihil's numbers are bought, the lurk's are eaten. Less meat and less magic than nihil to pay for the technique it is
/// born with (see `crate::body::wear_lurk`).
pub mod lurk {
    /// Starting (and maximum) hit points. Well under [`crate::constants::player::START_HP`].
    pub const START_HP: i32 = 7;

    /// Starting (and maximum) magic points. Little to spend until it grows.
    pub const START_MAGIC: u8 = 2;

    /// Starting attack die (`1d[power]`) — its claws, and no weapon will ever
    /// add to them.
    pub const START_POWER: i32 = 2;

    /// Starting defence die (`1d[armor]`) — its fur, and no armour will ever
    /// add to it.
    pub const START_ARMOR: i32 = 2;

    /// Odds that one corpse feeds the lurk, rolled per death. Rolled rather
    /// than counted, so a growth is something
    /// that *happens* to a hunt and never something to count down to.
    pub const GROWTH_CHANCE: f64 = 0.30;

    /// What one growth is worth, on whichever of the four numbers it lands.
    pub const GROWTH_STEP: i32 = 1;

    /// How many times the lurk throws each attack: the bite and the second
    /// snap of the jaws.
    pub const NUMBER_OF_ATTACKS: u8 = 2;
}

// ===========================================================================
// Monsters
// ===========================================================================

/// Bestiary-wide defaults. Per-creature stats are rows in `monsters.rs`.
pub mod monsters {
    /// The spawn weight a bestiary row gets when it doesn't `.weight(n)` for
    /// itself. Every default-weight creature is equally likely; a row asking
    /// for less is rarer, more is more common. Relative only — the absolute
    /// value just sets the granularity.
    pub const DEFAULT_SPAWN_WEIGHT: u32 = 20;

    /// How many turns a medusa's gaze leaves the player standing as stone.
    /// Long enough to be the fight's whole shape and short enough to live
    /// through — nothing can kill a petrified player but a war hammer, so this
    /// is a toll in turns rather than in HP. See
    /// `crate::abilities::medusa_gaze`.
    pub const PETRIFY_TURNS: u32 = 5;

    /// The ice monster's odds, on a blow that lands, of paralysing what it hit.
    pub const ICE_MONSTER_PARALYZE_CHANCE: f64 = 3.0 / 6.0;

    /// The lowest of the three chances a bestiary row hands its
    /// [`EquipRoll`](crate::monsters::EquipRoll)s.
    pub const NORMAL_GEAR_CHANCE: f64 = 0.30;
    /// The middle gear chance, for a species that is more often armed.
    pub const HIGH_GEAR_CHANCE: f64 = 0.60;
    /// The highest gear chance, for a species that is armed nearly every time.
    pub const ULTIMATE_GEAR_CHANCE: f64 = 0.90;

    /// How many points of base power a rattlesnake's bite drains — permanently,
    /// and unlike the dart trap's, with no floor: a rattlesnake can drive a
    /// victim's power negative.
    pub const RATTLESNAKE_POWER_DRAIN: i32 = 1;

    /// How many points of max HP a vampire's touch drains per hit.
    pub const VAMPIRE_MAX_HP_DRAIN: i32 = 2;

    /// How far a launcher-wielding monster (a centaur, a medusa) can loose a
    /// shot. Shares the player's own launcher reach.
    pub use crate::constants::items::LAUNCHER_RANGE as MONSTER_SHOT_RANGE;
}

// ===========================================================================
// Spirits
// ===========================================================================

/// The neutral faction. See [`crate::spirits`].
pub mod spirits {
    /// How far [`crate::components::Alignment`] can drift toward either
    /// pole. Reaching it flips [`crate::components::SpiritsHostile`] for
    /// good.
    pub const ALIGNMENT_POLE: i8 = 3;

    /// The odds, per piece of gear the pink demon destroyed, that they join
    /// you as your Helper. Enough pieces make it certain.
    pub const PINK_DEMON_ODDS_PER_PIECE: f64 = 0.25;

    /// What the red demon charges for one piece of gear, in Max HP. Paying
    /// your last Max HP kills you.
    pub const RED_DEMON_GEAR_PRICE: i32 = 3;

    /// What the gnome charges, in Max Ma: a scroll, a potion, a wand.
    pub const GNOME_SCROLL_PRICE: u8 = 1;
    /// What the gnome charges for a potion, in Max Ma.
    pub const GNOME_POTION_PRICE: u8 = 1;
    /// What the gnome charges for a wand, in Max Ma.
    pub const GNOME_WAND_PRICE: u8 = 2;

    /// The spawn weight every spirit row gets: a twentieth of an ordinary
    /// monster's default, and the smallest weight there is. Rarer means
    /// raising [`crate::constants::monsters::DEFAULT_SPAWN_WEIGHT`] and every
    /// explicit `.weight(n)` with it.
    pub const SPAWN_WEIGHT: u32 = 1;

    /// How many things a barterer lays on the table: the yellow demon's
    /// pack, the sphynx's spells. Inclusive on both ends.
    pub const BARTER_STOCK_MIN: usize = 2;
    /// The most things a barterer lays out; see [`BARTER_STOCK_MIN`].
    pub const BARTER_STOCK_MAX: usize = 4;

    /// How far one spirit's poof moves [`crate::components::Alignment`]: toward
    /// the cacodaemon pole for a demon, toward the eudaemon pole for an angel
    /// or kin.
    pub const ALIGNMENT_STEP: i8 = 1;

    /// What gaining a Helper does to [`crate::components::Alignment`].
    pub const HELPER_GAINED_ALIGNMENT: i8 = 1;

    /// What losing a Helper to an explosion or a system shock does to
    /// [`crate::components::Alignment`].
    pub const HELPER_BLOWN_UP_ALIGNMENT: i8 = -2;

    /// The angel's test of faith takes the player's current HP down by this
    /// divisor (never below one point), on top of cancelling every effect.
    pub const TEST_OF_FAITH_HP_DIVISOR: i32 = 2;

    /// What the test of faith pays off with on the weapon and the armour: a
    /// flat enchantment, and the curse lifted.
    pub const TEST_OF_FAITH_GEAR_BONUS: i32 = 3;

    /// What a dud equipped item (no enchantment at all) rerolls to when the test
    /// of faith pays off.
    pub const TEST_OF_FAITH_DUD_BONUS: i32 = 1;
}

// ===========================================================================
// Passive abilities
// ===========================================================================

/// The odds and the step behind the abilities that act by themselves (the
/// `ABILITIES` table in `crate::abilities`). The effect markers carry no
/// numbers; the table row names one of these.
pub mod abilities {
    /// A ring of aggravate monster: the odds, on each turn its bearer acts, that
    /// everything on the floor learns where they are.
    pub const AGGRAVATES_MONSTERS_CHANCE: f64 = 0.10;

    /// A ring of regeneration: the odds, on each turn its bearer acts, that it
    /// tries to mend something.
    pub const REGENERATES_CHANCE: f64 = 0.50;

    /// Rogue's teleportitis, at NetHack's odds, per turn the bearer acts. The
    /// jump lands at the top of their next turn.
    pub const TELEPORTITIS_CHANCE: f64 = 1.0 / 85.0;

    /// A ring of polymorph: the odds, per turn the bearer acts, that it turns
    /// them into something else.
    pub const POLYMORPHITIS_CHANCE: f64 = 1.0 / 83.0;

    /// What each landed hit of a rapier (or a lurk's own technique) adds to
    /// [`crate::effects::Momentum`].
    pub const MOMENTUM_PER_HIT: i32 = 2;
}

// ===========================================================================
// Conditions
// ===========================================================================

/// What the afflictions cost. Paralysis has its own dial in
/// `potions`; confusion's lives here.
pub mod conditions {
    /// While the player is confused, the odds that any one step goes off in a
    /// random direction instead of the intended one.
    pub const CONFUSION_STUMBLE_CHANCE: f64 = 0.5;
}

// ===========================================================================
// Speed
// ===========================================================================

/// The tempo scale: energy banked per player turn by each
/// [`SpeedKind`](crate::components::SpeedKind), and what one action costs. How
/// often a creature acts is its rate against the cost.
pub mod speed {
    /// Energy a `Slow` creature banks per player turn.
    pub const SLOW_RATE: i32 = 1;

    /// Energy a `Normal` creature banks per player turn. The reference tempo.
    pub const NORMAL_RATE: i32 = 2;

    /// Energy a `Quick` creature banks per player turn: the lurk's tempo.
    pub const QUICK_RATE: i32 = 3;

    /// Energy a `Fast` creature banks per player turn.
    pub const FAST_RATE: i32 = 4;

    /// What one action costs, in the same units.
    pub const ACTION_COST: i32 = 2;
}
