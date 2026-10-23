//! What a creature fights with and moves by: movement type, fighter stats, magic, speed
//! and tempo, sight, and what hides from it.

use super::*;

// ===========================================================================
// Creatures and combat
// ===========================================================================

/// Marks an entity as a monster — something the AI drives. Carries the tactic
/// that picks its rule set ([`crate::agents::rule_set_for`]).
#[derive(Component)]
pub struct Mob {
    /// The tactic it thinks with.
    pub movement_type: MovementType,
}

/// A monster's tactic, which picks the rule set it thinks with (see
/// [`crate::agents`]). `Static` never acts at all — the inert placeholder tests
/// reach for. `Chase` hunts the player, `Flee` walks away, `Confused` staggers
/// at random, `Ambush` lies in wait and strikes only what comes alongside (the
/// venus flytrap, the ice monster, a xeroc that has dropped its disguise).
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum MovementType {
    /// Never acts at all.
    Static,
    /// Hunts the player.
    Chase,
    /// Walks away from the player.
    Flee,
    /// Staggers at random.
    Confused,
    /// Retired: aggravation is a state now, the [`Aggravated`] component, laid
    /// over whatever tactic the creature already had. Kept because a save
    /// writes this enum by position; a save that still carries it loads as
    /// `Chase` plus the component. Never set it.
    Aggravated {
        /// Column of the tile the noise came from.
        tx: u16,
        /// Row of the tile the noise came from.
        ty: u16,
    },
    /// Lies in wait and strikes only what comes alongside.
    Ambush,
}

/// Set on every monster on the floor by a scroll of aggravate monsters (or a
/// ring's shriek): out of the player's view, the creature makes a beeline for
/// `(tx, ty)`, the tile the noise came from. In view it thinks with its own
/// rule set again. See [`crate::ai`](mod@crate::ai).
#[derive(Component, Clone, Copy)]
pub struct Aggravated {
    /// Column of the tile the noise came from.
    pub tx: u16,
    /// Row of the tile the noise came from.
    pub ty: u16,
}

/// Everything needed to resolve a fight. Combat is a pair of opposed rolls with
/// no to-hit step: `damage = (1d[power] + power_bonus) - (1d[armor] +
/// armor_bonus)`, each side rolled independently, nothing ever missing. Each
/// `1d` is two dice of that size averaged. See `crate::combat`.
#[derive(Component)]
pub struct Fighter {
    /// Current hit points. Zero or below is dead.
    pub hp: i32,
    /// The most `hp` can hold. A red demon's priced offer lowers it for good
    /// ([`Price::MaxHp`]).
    pub max_hp: i32,
    /// Defence die size: the opposed armour roll is `1d[armor] + armor_bonus`.
    pub armor: i32,
    /// Attack die size: weapon damage rolls `1d[power] + power_bonus`. A
    /// poisoned dart trap permanently drops this; a potion of restore strength
    /// heals it back up to [`Fighter::max_power`], and a green coin gives back
    /// a few points of it.
    pub power: i32,
    /// The unpoisoned value of [`Fighter::power`] — the ceiling that strength
    /// restoration returns it to. Set equal to `power` at creation.
    pub max_power: i32,
    /// Flat modifier added once to the armour roll (may be negative).
    pub armor_bonus: i32,
    /// Flat modifier added once to the damage roll (may be negative).
    pub power_bonus: i32,
}

/// Creatures that bleed. When an entity carrying this takes damage, the tile it
/// is standing on is recorded in [`crate::map::BloodStains`] and rendered with a
/// red background while it stays in the player's view.
#[derive(Component)]
pub struct Blood;

/// The player's pool of magic points. Shown in the HUD as `Ma points/max_points`
/// alongside `HP`. Every run starts with a full pool (see
/// [`crate::initialize_world`]).
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Magic {
    /// Ma to spend now.
    pub points: u8,
    /// The most `points` can hold. A gnome's priced offer lowers it for good
    /// ([`Price::MaxMa`]).
    pub max_points: u8,
}

// ===========================================================================
// Speed and tempo
//
// The player is the clock. Monsters bank energy on each of the player's turns
// and spend it in `crate::ai`; the player's own tempo is run by the engine loop
// through `PlayerTempo`.
// ===========================================================================

/// The tempos an actor can move at. How often each acts is its
/// [`rate`](SpeedKind::rate) against the [`Speed::COST`] of an action. The
/// player is the clock: monsters bank [`Speed::energy`] each of the player's
/// turns and spend it in [`crate::ai`](mod@crate::ai), while the player's own tempo is
/// handled by the engine loop (see [`PlayerTempo`]). Wands of haste/slow
/// monster step a creature one notch along this scale, permanently.
///
/// `Quick` is last in the list rather than between `Normal` and `Fast` where
/// it belongs on the dial, and that is deliberate: a save writes an enum by
/// variant position, so inserting one in the middle would bring every saved
/// `Fast` creature back as something else (see `crate::saveload`). The dial's
/// real order lives in [`faster`](SpeedKind::faster) and
/// [`slower`](SpeedKind::slower).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SpeedKind {
    /// The slowest tempo: banks the least energy per turn ([`SpeedKind::rate`]).
    Slow,
    /// The reference tempo: the other rates are read against it.
    #[default]
    Normal,
    /// The quickest tempo a wand can reach: banks the most energy per turn.
    Fast,
    /// Between `Normal` and `Fast`: the lurk's own tempo, and nothing
    /// else's. A wand can still push a creature onto or off it.
    Quick,
}

impl SpeedKind {
    /// The energy an actor at this tempo banks per player turn. `Normal` is the
    /// reference; acting costs [`Speed::COST`].
    pub fn rate(self) -> i32 {
        match self {
            SpeedKind::Slow => SLOW_RATE,
            SpeedKind::Normal => NORMAL_RATE,
            SpeedKind::Quick => QUICK_RATE,
            SpeedKind::Fast => FAST_RATE,
        }
    }

    /// One step round the haste cycle (wand of haste monster):
    /// `Normal` -> `Fast` -> `Slow` -> `Normal`. Hasting what is already
    /// `Fast` wraps it to `Slow`.
    ///
    /// `Normal` steps straight to `Fast`, skipping `Quick`: a wand of haste
    /// has always been worth a doubling, and quietly halving what it buys to
    /// make room for a class's tempo would be a nerf to every haste in the
    /// game. `Quick` is a place a creature is *born*, not a rung a wand
    /// climbs through.
    ///
    /// ```
    /// use models::SpeedKind;
    ///
    /// assert_eq!(SpeedKind::Slow.faster(), SpeedKind::Normal);
    /// assert_eq!(SpeedKind::Normal.faster(), SpeedKind::Fast);
    /// // `Quick` is a place a creature is born, so a wand takes it to `Fast`.
    /// assert_eq!(SpeedKind::Quick.faster(), SpeedKind::Fast);
    /// assert_eq!(SpeedKind::Fast.faster(), SpeedKind::Slow);
    /// ```
    pub fn faster(self) -> Self {
        match self {
            SpeedKind::Slow => SpeedKind::Normal,
            SpeedKind::Normal | SpeedKind::Quick => SpeedKind::Fast,
            SpeedKind::Fast => SpeedKind::Slow,
        }
    }

    /// One notch slower (wand of slow monster). `Slow` is the floor.
    pub fn slower(self) -> Self {
        match self {
            SpeedKind::Fast | SpeedKind::Quick => SpeedKind::Normal,
            SpeedKind::Normal | SpeedKind::Slow => SpeedKind::Slow,
        }
    }
}

/// An actor's movement tempo plus its running energy pool. Every monster gets one
/// from [`crate::spawn_monster`]; the player gets one in
/// [`crate::initialize_world`]. `energy` is transient game state — it is not
/// serialised and simply resets to zero on load.
#[derive(Component)]
pub struct Speed {
    /// The tempo, which sets how much energy a turn banks ([`SpeedKind::rate`]).
    pub kind: SpeedKind,
    /// Banked energy; each action spends [`Speed::COST`]. Not saved, so a reload
    /// starts every creature at zero.
    pub energy: i32,
}

impl Speed {
    /// The energy one action costs, in `Normal`-tempo units.
    pub const COST: i32 = ACTION_COST;

    /// A creature at tempo `kind` with no energy banked.
    pub fn new(kind: SpeedKind) -> Self {
        Self { kind, energy: 0 }
    }
}

/// Drives the player's half of the speed system (see [`Speed`]). The engine loop
/// consults the player's [`SpeedKind`] after every turn: each kind banks its
/// rate (see [`crate::constants::speed`]) per player turn against
/// [`ACTION_COST`] per action, and monsters act on the normal rate. A faster
/// player gets extra inputs per monster round, a slower one gives monsters
/// extra rounds. Transient, never serialised.
#[derive(Resource, Default)]
pub struct PlayerTempo {
    /// Flips on each `Fast`-tempo turn; monsters move only when it flips back to
    /// `false`, so the pattern reads skip / run / skip / run.
    pub fast_parity: bool,
    /// How many turns the player has taken at a tempo that doesn't divide
    /// evenly into monster rounds. `Quick` is the only one: its rate buys
    /// fewer monster rounds than turns, so some turns are free. Counts on
    /// its own rather than reusing `fast_parity` because the two patterns are
    /// different lengths and a creature can be moved from one to the other
    /// mid-floor by a wand.
    pub quick_beat: u8,
}

/// Set by a greatclub's heavy swing ([`crate::effects::HeavySwing`]): the
/// weight of a landed blow costs the wielder a beat of their own, played out
/// as one extra monster round immediately after this player action. The
/// engine's turn loop checks this after every player action and clears it once
/// spent — transient, never serialised.
#[derive(Resource, Default)]
pub struct ExtraMonsterRound(pub bool);

// ===========================================================================
// Perception and memory
// ===========================================================================

/// One actor's field of view and its remembered map. Owned by
/// `crate::visibility`.
#[derive(Component)]
pub struct Viewshed {
    /// Transient: recomputed every frame by the visibility system, never saved.
    pub visible_tiles: Vec<(u16, u16)>,
    /// Fog-of-war memory, one bit per map tile (see [`crate::map::tile_index`]).
    pub revealed_tiles: FixedBitSet,
    /// Seeded from [`crate::constants::player::SIGHT_RANGE`] and saved, but no
    /// system reads it: sight is the 3x3 around the viewer plus the lit room
    /// they stand in (`visible_from` in `crate::visibility`), with no radius.
    pub range: u16,
    /// Set when something changed what the viewer can see: a step, a door, a
    /// wand of light. The visibility system recomputes only dirty viewsheds and
    /// clears the flag.
    pub dirty: bool,
}

/// Not currently drawn or announced: an out-of-view monster, an undiscovered
/// trap, or an invisible thing the player can't perceive. The visibility system
/// owns this for monsters and the invisible item; traps clear it when revealed.
#[derive(Component)]
pub struct Hidden;

/// Intrinsically unseeable without [`crate::effects::SeesInvisible`] — the
/// phantom, and the invisibly-stashed floor item a floor hides at
/// [`crate::constants::population::HIDDEN_ITEM_CHANCE`]. Pairs with
/// [`Hidden`]: `Invisible` says *why* a thing can't be seen, `Hidden` is the
/// per-turn "can't be seen right now" the renderer reads.
#[derive(Component)]
pub struct Invisible;

/// A xeroc still wearing its disguise: its [`Name`] and [`Renderable`] read as
/// an ordinary item, and it is excluded from every "a monster is nearby"
/// check ([`crate::autoexplore::monster_in_sight`],
/// `crate::autofight::visible_enemies`) so auto-explore and auto-fight are
/// fooled right along with the player. [`crate::monsters::reveal_mimics`]
/// strips this — and the disguise with it — the instant the player is
/// standing next to it. See `crate::monsters::MonsterDef::mimics`.
#[derive(Component)]
pub struct Mimic;

/// Present while an entity is currently inside the player's viewshed. Added the
/// turn it first enters view (logging "you spotted ..."), removed the turn it
/// leaves, so re-entering view spots it again. Transient, never serialised.
#[derive(Component)]
pub struct Spotted;
