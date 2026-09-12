//! The ECS vocabulary — every [`Component`], [`Resource`] and [`Event`] the
//! game is made of, in one file.
//!
//! This module is nouns, not verbs. A component here says *what a thing is* — it
//! has a position, it bleeds, it is a scroll of type X — and nothing about what
//! happens as a result. The verbs live in the systems (`crate::combat`,
//! `crate::ai`, `crate::items`, `crate::visibility`, and so on). If you are
//! looking for "what does a potion of healing do", it is not here; it is in
//! `crate::items` (the `potions` submodule).
//!
//! Two recurring shapes are worth knowing before you read:
//!
//! * **Marker components** carry no data — [`Player`], [`Blood`], [`Curse`],
//!   [`Confused`]. Their presence *is* the fact. A system asks
//!   `world.get::<Blood>(e).is_some()` and that is the whole check.
//!
//! * **Type-key components** — [`Potion`], [`Scroll`], [`Wand`], [`Ring`] — hold
//!   one enum value ([`PotionEffect`] &c.) that names *which* one it is. The key
//!   is the item's identity for identification and for the save file; it is
//!   never a description of behaviour. The catalog row ([`crate::catalog`])
//!   turns a key into the components that actually do something, and the
//!   mechanic (`crate::items`) is a `match` on the key.
//!
//! **Transient vs serialised.** Some fields are rebuilt from scratch every frame
//! or every load and are deliberately left out of the save format —
//! [`Viewshed::visible_tiles`], [`Speed::energy`], [`Spotted`], [`DungeonLord`].
//! Each says so in its doc comment. Everything else is expected to round-trip
//! through `crate::saveload`.
//!
//! **Ordering matters for saved types.** Every enum that derives `Serialize`
//! ([`MovementType`], [`Faction`], [`SpeedKind`], [`SnareKind`], [`TrapReveal`],
//! the `*Effect` enums) is written to the save by variant position, and every
//! serialised struct by field order. Append new variants and fields; do not
//! reorder existing ones, or old saves change meaning.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

use crate::effects::{ColdImmune, FireImmune, Grant, Undead};

/// The most a single pack slot will hold before the overflow spills into a
/// second slot. Defined and documented in `constants.rs`; re-exported here so
/// `components::STACK_LIMIT` (and the crate-wide glob) keep resolving.
pub use crate::constants::items::STACK_LIMIT;

// ===========================================================================
// Identity, position, appearance
// ===========================================================================

/// The display name of anything the player can be told about — a monster, an
/// item on the floor, the hero. Item type-keys ([`Potion`] &c.) still carry
/// this: the appearance shown before identification is swapped in on top of it
/// by `crate::identify`.
#[derive(Component)]
pub struct Name {
    pub what: String,
}

impl Name {
    /// The indefinite article that reads correctly before this name:
    /// `"an"` before a vowel sound, `"a"` otherwise. Good enough for the
    /// bestiary and item list (no "an hour" / "a unicorn" edge cases here).
    pub fn article(&self) -> &'static str {
        crate::identify::article_for(&self.what)
    }
}

/// The one entity the keyboard drives and the camera follows. A marker: exactly
/// one entity in a run has it.
#[derive(Component)]
pub struct Player;

/// A tile coordinate. Every entity that exists *somewhere* on the current floor
/// has one; an item tucked into a [`Backpack`] has its `Position` removed until
/// it is dropped again.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

/// How an entity draws: one glyph in one colour. The colour is packed to a byte
/// against a fixed 16-entry palette on save (see `crate::saveload`).
#[derive(Component)]
pub struct Renderable {
    pub glyph: char,
    pub color: Color,
}

/// Whose side an actor is on. Monsters fight the player and (in principle) spare
/// each other; `Ally` is reserved and currently unused.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Faction {
    Player,
    Monster,
    Ally,
}

// ===========================================================================
// Creatures and combat
// ===========================================================================

/// Marks an entity as a monster — something the AI drives. Carries the tactic it
/// uses to pick a move each turn.
#[derive(Component)]
pub struct Mob {
    pub movement_type: MovementType,
}

/// A monster's movement tactic. `Static` holds still, `Chase` walks toward the
/// player when it can see them, `Flee` walks away, `Confused` staggers at
/// random.
#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum MovementType {
    Static,
    Chase,
    Flee,
    Confused,
    /// Set on every creature by a scroll of aggravate monsters: the mob homes in
    /// on `(tx, ty)` — the tile the reader stood on — from anywhere on the floor,
    /// in or out of the player's view, and lunges the moment it draws alongside
    /// them. See [`crate::ai`].
    Aggravated {
        tx: u16,
        ty: u16,
    },
}

/// Everything needed to resolve a fight. Combat is a pair of opposed rolls with
/// no to-hit step: `damage = (1d[power] + power_bonus) - (1d[armor] +
/// armor_bonus)`, each side rolled independently, nothing ever missing. See
/// `crate::combat`.
#[derive(Component)]
pub struct Fighter {
    pub hp: i32,
    pub max_hp: i32,
    /// Defence die size: the opposed armour roll is `1d[armor] + armor_bonus`.
    pub armor: i32,
    /// Attack die size: weapon damage rolls `1d[power] + power_bonus`. A
    /// poisoned dart trap permanently drops this; a potion of restore strength
    /// (not yet wired) will heal it back up to [`Fighter::max_power`].
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
    pub points: u8,
    pub max_points: u8,
}

// ===========================================================================
// Speed and tempo
//
// The player is the clock. Monsters bank energy on each of the player's turns
// and spend it in `crate::ai`; the player's own tempo is run by the engine loop
// through `PlayerTempo`.
// ===========================================================================

/// The three tempos an actor can move at. `Fast` acts twice for every `Normal`
/// action; `Slow` acts once for every two. The player is the clock: monsters
/// bank [`Speed::energy`] each of the player's turns and spend it in
/// [`crate::ai`], while the player's own tempo is handled by the engine loop
/// (see [`PlayerTempo`]). Wands of haste/slow monster step a creature one notch
/// along this scale, permanently.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SpeedKind {
    Slow,
    #[default]
    Normal,
    Fast,
}

impl SpeedKind {
    /// The energy an actor at this tempo banks per player turn. `Normal` is the
    /// reference; acting costs [`Speed::COST`].
    pub fn rate(self) -> i32 {
        match self {
            SpeedKind::Slow => 1,
            SpeedKind::Normal => 2,
            SpeedKind::Fast => 4,
        }
    }

    /// One notch quicker (wand of haste monster). `Fast` is the ceiling.
    pub fn faster(self) -> Self {
        match self {
            SpeedKind::Slow => SpeedKind::Normal,
            SpeedKind::Normal | SpeedKind::Fast => SpeedKind::Fast,
        }
    }

    /// One notch slower (wand of slow monster). `Slow` is the floor.
    pub fn slower(self) -> Self {
        match self {
            SpeedKind::Fast => SpeedKind::Normal,
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
    pub kind: SpeedKind,
    pub energy: i32,
}

impl Speed {
    /// The energy one action costs, in `Normal`-tempo units.
    pub const COST: i32 = 2;

    pub fn new(kind: SpeedKind) -> Self {
        Self { kind, energy: 0 }
    }
}

/// Drives the player's half of the speed system (see [`Speed`]). The engine loop
/// consults the player's [`SpeedKind`] after every turn: a `Fast` player takes
/// two inputs before the monsters get a move (tracked by `fast_parity`), a
/// `Slow` player's single move is followed by two monster rounds, and `Normal`
/// is one-for-one. Transient, never serialised.
#[derive(Resource, Default)]
pub struct PlayerTempo {
    /// Flips on each `Fast`-tempo turn; monsters move only when it flips back to
    /// `false`, so the pattern reads skip / run / skip / run.
    pub fast_parity: bool,
}

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
    pub range: u16,
    pub dirty: bool,
}

/// Not currently drawn or announced: an out-of-view monster, an undiscovered
/// trap, or an invisible thing the player can't perceive. The visibility system
/// owns this for monsters and the invisible item; traps clear it when revealed.
#[derive(Component)]
pub struct Hidden;

/// Intrinsically unseeable without [`crate::effects::SeesInvisible`] — the
/// phantom, and the one-floor-in-five "invisible" hidden floor item. Pairs with [`Hidden`]:
/// `Invisible` says *why* a thing can't be seen, `Hidden` is the per-turn "can't
/// be seen right now" the renderer reads.
#[derive(Component)]
pub struct Invisible;

/// Present while an entity is currently inside the player's viewshed. Added the
/// turn it first enters view (logging "you spotted ..."), removed the turn it
/// leaves, so re-entering view spots it again. Transient, never serialised.
#[derive(Component)]
pub struct Spotted;

// ===========================================================================
// Items: on the floor and in the pack
// ===========================================================================

/// Marker for anything that can sit on the floor and be picked up. The display
/// name lives on the [`Name`] component, same as monsters.
#[derive(Component)]
pub struct Item;

/// What an item cashes in for at the end of a run. Coins and the relic carry it;
/// nothing spends it during play.
#[derive(Component)]
pub struct Value {
    pub amount: i32,
}

/// An actor's carried items, in inventory-letter order. An item in here has had
/// its [`Position`] removed; dropping or throwing puts one back.
#[derive(Component)]
pub struct Backpack {
    pub items: Vec<Entity>,
}

/// Used up on use: a potion or a scroll. [`item_system`](crate::items::item_system)
/// despawns anything carrying this once its effect has been applied.
#[derive(Component)]
pub struct Consume;

/// A wand's remaining charges. Each zap spends one; at zero the wand crumbles.
/// A floor drop rolls `2d6 + 1` (see [`crate::catalog::roll_wand_charges`]).
#[derive(Component)]
pub struct Battery {
    pub charges: i8,
}

/// How many identical items share one pack slot. Only ammunition stacks: a
/// quiver of arrows is one entity carrying a number, not thirty entities
/// crowding thirty inventory letters. Throwing spends one; picking more up tops
/// the stack back up to at most [`STACK_LIMIT`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub count: u8,
}

/// An item with a firing range of its own: a wand. Feeds the aiming reticle when
/// the item is zapped (a thrown item uses [`crate::items::THROW_RANGE`] instead).
#[derive(Component)]
pub struct Ranged {
    pub range: i32,
}

/// Marker for the Element of Yoord — the relic each run must carry up from the
/// depths. While the player's pack holds an entity with this component the
/// staircases invert: the up-stair works and the down-stair is dead.
#[derive(Component)]
pub struct Amulet;

// ===========================================================================
// Items: type keys
//
// One component + one enum per identifiable kind. The enum value is the item's
// identity for `crate::identify` and the save file, and the thing the mechanic
// in `crate::items` matches on. It is never a description of behaviour — that is
// assembled from other components by the catalog row.
// ===========================================================================

/// Type-key for a potion. Mechanic: the `potions` submodule of `crate::items`.
#[derive(Component)]
pub struct Potion {
    pub effect: PotionEffect,
}

/// Which potion this is. Identity for identification and saves — see
/// [`crate::catalog::POTIONS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum PotionEffect {
    Blindness,
    Confusion,
    ExtraHealing,
    FruitJuice,
    GainStrength,
    Haste,
    Healing,
    MagicDetection,
    MonsterDetection,
    Paralysis,
    Poison,
    RaiseLevel,
    RestoreStrength,
    SeeInvisible,
    Water,
}

/// Type-key for a scroll. Mechanic: the `scrolls` submodule of `crate::items`.
#[derive(Component)]
pub struct Scroll {
    pub effect: ScrollEffect,
}

/// Which scroll this is. Identity for identification and saves — see
/// [`crate::catalog::SCROLLS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollEffect {
    MonsterConfusion,
    MagicMapping,
    HoldMonster,
    Sleep,
    EnchantArmor,
    Identify,
    ScareMonster,
    FoodDetection,
    Teleportation,
    EnchantWeapon,
    CreateMonster,
    RemoveCurse,
    AggravateMonsters,
    BlankPaper,
    VorpalizeWeapon,
}

/// Type-key for a wand. Mechanic: the `wands` submodule of `crate::items`
/// (zapped), and `throwing` (hurled).
#[derive(Component)]
pub struct Wand {
    pub effect: WandEffect,
}

/// Which wand this is. Identity for identification and saves — see
/// [`crate::catalog::WANDS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WandEffect {
    Light,
    Striking,
    Lightning,
    Fire,
    Cold,
    Polymorph,
    MagicMissile,
    HasteMonster,
    SlowMonster,
    DrainLife,
    Nothing,
    TeleportAway,
    TeleportTo,
    Cancellation,
}

impl WandEffect {
    /// Whether zapping this wand opens the aiming reticle. Every wand needs a
    /// target except the wand of light, which floods the room the zapper stands
    /// in and so is "used" immediately like a potion or scroll.
    pub fn needs_target(self) -> bool {
        !matches!(self, WandEffect::Light)
    }
}

/// Type-key for a ring, the twin of [`Potion`] / [`Scroll`] / [`Wand`]. What the
/// ring *does* is not read from here — it rides along as modifier components and
/// [`crate::effects::Grants`], attached by its [`crate::catalog::RingDef`] row.
/// This tag exists so the ring can be identified and saved.
#[derive(Component)]
pub struct Ring {
    pub effect: RingEffect,
}

/// The name of a ring type — its identity for identification and saving, not a
/// description of its behaviour. See [`crate::catalog::RINGS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum RingEffect {
    Protection,
    Strength,
    Perception,
    Adornment,
    AggravateMonster,
    Dexterity,
    IncreaseDamage,
    Regeneration,
    SlowDigestion,
    Teleportation,
    Stealth,
    MaintainArmor,
}

/// Tag for a cursed piece of equipment. Rolled on at spawn for the majority of
/// weapon/armour/ring drops (see [`crate::catalog::enchant_equipment`]). Once a
/// cursed item is equipped it can't be taken off again until the curse is lifted
/// by a scroll of remove curse.
#[derive(Component)]
pub struct Curse;

/// A weapon, suit of armour or launcher whose enchantment plus and curse status
/// the player has actually learned — by wearing it or by a scroll of identify
/// singling it out (see [`crate::equipment::toggle_equipped`] and
/// [`crate::items::scrolls`]). Until then [`crate::identify::display_name`]
/// keeps both hidden, the same way a potion hides its effect. A potion, scroll,
/// wand or ring never needs this — their own [`crate::identify::Identified`]
/// registry already answers the question.
#[derive(Component)]
pub struct KnownQuality;

/// A weapon that has been vorpalized (scroll of vorpalize weapon). Any hit from
/// it that draws blood slays a creature named `bane` outright — as it does any
/// creature carrying [`crate::effects::VorpalTarget`], regardless of `bane`. See
/// [`crate::combat::resolve_attack`].
#[derive(Component)]
pub struct Vorpal {
    pub bane: String,
}

/// The three flavours of elemental damage a wand or a blast can carry. A
/// creature can be immune to one — and the immunity is a plain component, so a
/// dragon's innate [`FireImmune`] and a future ring of fire resistance's are the
/// same thing to the code that checks. Lives here rather than in
/// `crate::items` because both the wand mechanic and the throwing mechanic
/// reach for it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Fire,
    Cold,
    Drain,
}

impl Element {
    /// Which element a wand's damage carries, if any. Non-elemental damage
    /// (magic missile, lightning, striking) returns `None` and is never
    /// resisted.
    pub fn of(effect: WandEffect) -> Option<Element> {
        match effect {
            WandEffect::Fire => Some(Element::Fire),
            WandEffect::Cold => Some(Element::Cold),
            WandEffect::DrainLife => Some(Element::Drain),
            _ => None,
        }
    }

    /// The marker effect that shrugs this element off.
    pub fn immunity(self) -> Grant {
        match self {
            Element::Fire => Grant::of::<FireImmune>(),
            Element::Cold => Grant::of::<ColdImmune>(),
            Element::Drain => Grant::of::<Undead>(),
        }
    }

    /// The word for this element in an "unharmed by the ___" log line.
    pub fn noun(self) -> &'static str {
        match self {
            Element::Fire => "flames",
            Element::Cold => "cold",
            Element::Drain => "evil magic",
        }
    }
}

// ===========================================================================
// Items: throwing and launchers
//
// A missile and a launcher never name each other — they meet at an effect (see
// `crate::effects::FireArrow`). Resolution is `crate::items::throwing`.
// ===========================================================================

/// What this item does to a creature it is thrown into: the die rolled on
/// impact (see [`crate::items::throw_system`]). Only things meant to hurt when
/// they land carry it — a dagger does, a wand does not, and an item without one
/// simply bounces off and falls at the target's feet.
///
/// An improvised missile — a mace, a suit of plate mail — is still measured
/// against the target's armour plus. A purpose-built one ([`Projectile`]) is not.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThrownDamage(pub i32);

/// Made to be thrown: an arrow, a quarrel, a dagger, a spear. Three things
/// follow from it, and they are the same three for all four — the roll goes
/// straight around the target's armour (a point already in flight does not care
/// what you are wearing), the missile is spent on the creature it strikes rather
/// than clattering to the floor, and nothing ever catches one out of the air.
///
/// A projectile that finds no target is not spent: it lands where it fell and
/// can be picked up again.
#[derive(Component, Clone, Copy)]
pub struct Projectile;

/// This missile does not stop at the first thing it hits. A hurled dagger or
/// spear runs the whole line you aimed down, spending itself on every creature
/// standing in it — the NecroDancer trick, and the reason a corridor full of
/// kobolds is worth one spear.
///
/// Everything without it resolves on the first creature in the way, which is why
/// aiming past a monster does not work.
#[derive(Component, Clone, Copy)]
pub struct Piercing;

/// The effect that turns a lobbed missile into a loosed one, doubling its die.
/// An arrow answers to [`crate::effects::FireArrow`], a quarrel to
/// [`crate::effects::FireQuarrel`]; the bow and crossbow are simply things that
/// grant those. Neither missile knows a launcher exists, and no launcher knows
/// what ammunition is — they meet at the effect, like everything else here.
#[derive(Component, Clone, Copy)]
pub struct LaunchedBy(pub crate::effects::Grant);

/// A bow or a crossbow: gear that is worth nothing swung and everything drawn.
/// It contributes no attack die, so its enchantment has no melee roll to land
/// on and lands on [`crate::effects::ThrowBonus`] instead — a +2 bow puts +2 on
/// every arrow it looses. See [`crate::catalog::enchant_equipment`].
#[derive(Component, Clone, Copy)]
pub struct Launcher;

// ===========================================================================
// Player conditions
// ===========================================================================

/// A transient affliction on the **player** (a monster is confused through
/// [`MovementType::Confused`] instead). Half of every walk or swing while it
/// lasts goes off in a random direction ("You stumble foolishly"), and fast
/// movement, auto-explore and auto-fight all refuse to run. It is treacherous:
/// it does not wear off with time — only using a staircase or being caught by a
/// wand of cancellation clears it (both through
/// `crate::helpers::clear_player_conditions`). Shown in the HUD as `CONF`.
#[derive(Component)]
pub struct Confused;

// ===========================================================================
// Traps and snares
// ===========================================================================

/// Which of the six trap kinds a [`Trap`] entity is. The effect always fires
/// when the trap is stepped on — there is no saving throw. The mechanic keyed
/// off each variant lives in `crate::traps` (`spring_trap`); the catalog row
/// (name, glyph, rarity, debut depth) is [`crate::traps::TrapDef`].
///
/// Serialised by variant position — append, never reorder (a saved trapdoor
/// would become something else).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrapEffect {
    /// Drops the victim straight to the next floor down. No escape.
    Trapdoor,
    /// Clamps shut: the victim is [`Snare`]d and cannot move — though it may
    /// still strike an adjacent foe — until it works free.
    Bear,
    /// A hiss of gas: the victim sleeps through its next few turns.
    Sleep,
    /// Flings the victim to a random open tile on the current floor.
    Teleport,
    /// Fires a bolt; on a clean miss the arrow lands on the floor as loot.
    /// Damage scales with depth (`crate::constants::traps`).
    Arrow,
    /// A poisoned dart: light damage, and on a hit it saps melee power for good
    /// — more of it the deeper you are — unless a ring of strength is worn.
    Dart,
}

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
    pub effect: TrapEffect,
    pub reveal: TrapReveal,
    pub revealed: bool,
}

/// Marker inserted on any actor that changed [`Position`] this turn, so
/// `crate::traps::trap_system` knows whose feet to check. Transient: cleared at
/// the end of every `trap_system` run and never serialised.
#[derive(Component)]
pub struct EntityMoved;

/// Why an actor is losing turns to a [`Snare`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnareKind {
    /// Bear trap: physically pinned. Movement is impossible — a thrash against
    /// the jaws wastes the turn and draws blood
    /// ([`crate::constants::traps::BEAR_TRAP_THRASH_DAMAGE`]) — but the victim
    /// can still attack an adjacent foe.
    Bear,
    /// Sleeping gas: out cold. No action of any kind until it wears off.
    Sleep,
}

/// An actor that cannot act freely for `turns` more turns. Aged by
/// `crate::traps::snare_system`; removed (with a wake-up log line for the
/// player) when it hits zero. A [`SnareKind::Sleep`] snare forfeits the turn
/// outright; a [`SnareKind::Bear`] snare only blocks movement. `crate::ai`
/// applies the same rule to snared monsters.
#[derive(Component)]
pub struct Snare {
    pub turns: u32,
    pub kind: SnareKind,
}

// ===========================================================================
// Score
// ===========================================================================

/// The running score shown on the HUD. Coins and the relic add to it on pickup.
#[derive(Component)]
pub struct Score {
    pub value: i32,
}

// ===========================================================================
// Events and their queues
//
// An input handler pushes an intent onto a queue resource; the matching system
// drains the queue once per turn and resolves each one. The `Event` derive is
// kept for the types even though they travel by queue.
// ===========================================================================

/// Intent: `attacker` swings at `target`. Drained by `crate::combat`.
#[derive(Event, Clone, Copy)]
pub struct WantsToAttack {
    pub attacker: Entity,
    pub target: Entity,
}

/// Intent: `user` uses `item` — quaff, read, zap or (un)equip, depending on what
/// it is. `target` is the aimed tile for a wand; `slot_idx` is the pack row it
/// came from, so it can go back exactly there. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Event, Clone, Copy)]
pub struct WantsToUse {
    pub user: Entity,
    pub item: Entity,
    pub target: Option<Position>,
    pub slot_idx: Option<usize>,
}

/// A hurled item, in flight from `thrower` towards `target`. Resolved by
/// [`crate::items::throw_system`], which is where it finds out what it hits.
#[derive(Event, Clone, Copy)]
pub struct WantsToThrow {
    pub thrower: Entity,
    pub item: Entity,
    pub target: Position,
}

/// The turn's pending attacks. Filled by input / AI, drained by
/// `crate::combat`.
#[derive(Resource, Default)]
pub struct AttackQueue {
    pub attacks: Vec<WantsToAttack>,
}

/// The turn's pending item uses. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Resource, Default)]
pub struct UseQueue {
    pub uses: Vec<WantsToUse>,
}

/// The turn's pending throws. Drained by [`crate::items::throw_system`].
#[derive(Resource, Default)]
pub struct ThrowQueue {
    pub throws: Vec<WantsToThrow>,
}

// ===========================================================================
// UI and input state (resources)
// ===========================================================================

/// The name the player typed at the start of the run, shown on the death screen.
#[derive(Resource, Default)]
pub struct PlayerName {
    pub what: String,
}

/// Whether the viewport scrolls to keep the player centred (`-centered`).
#[derive(Resource, Default)]
pub struct RenderConfig {
    pub centered: bool,
}

// The pack screen's own state — `ItemAction`, `PackMode` and `PackIsOpen` —
// lives in [`crate::pack`], next to the row filtering that decides what each of
// its ten modes shows.

/// Whether the "Really quit?" prompt is up.
///
/// Quitting is the one irreversible thing a keystroke can do — the run is
/// written out and the terminal goes away — so `Q` and `X` ask first, and a
/// misfire costs a `n` instead of a session. Ctrl+C is deliberately *not*
/// routed through here: it is the shell's own kill, a player reaching for it
/// means it, and a program that argues with Ctrl+C is a program you have to
/// kill twice.
#[derive(Resource, Default)]
pub struct QuitPrompt {
    pub open: bool,
}

/// The aiming reticle: which item is being aimed, whether this is a throw or a
/// zap, and where the cursor is.
#[derive(Resource, Default)]
pub struct TargetingState {
    pub active: bool,
    pub item: Option<Entity>,
    /// The reticle is aiming a throw rather than a zap: the range is
    /// [`crate::items::THROW_RANGE`] instead of the item's own, and confirming
    /// hurls the item instead of using it.
    pub throwing: bool,
    pub cursor_x: i16,
    pub cursor_y: i16,
}

// ===========================================================================
// Run state (resources)
// ===========================================================================

/// The message log: everything that has happened (`history`, capped at 50) and
/// everything the player has not yet acknowledged with `--MORE--` (`unread`).
#[derive(Resource)]
pub struct GameLog {
    pub history: Vec<String>,
    pub unread: Vec<String>, // The queue of messages waiting for a --MORE-- acknowledgment
}

impl Default for GameLog {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            unread: vec!["Welcome to roog! Good luck and have fun!".to_string()],
        }
    }
}

impl GameLog {
    pub fn add<S: Into<String>>(&mut self, message: S) {
        let msg = message.into();
        self.history.push(msg.clone());
        self.unread.push(msg); // Push to the unread queue!

        if self.history.len() > 50 {
            self.history.remove(0);
        }
    }
}

/// The current dungeon floor, 1-based.
#[derive(Resource)]
pub struct Depth {
    pub what: u8,
}

/// How many times the player has moved between floors this run — every
/// staircase, portal and trapdoor bumps it by one. It salts
/// [`crate::map::content_rng`], so a floor's *layout* is still a pure function
/// of `(seed, depth)` but its *contents* are re-rolled every time it is built:
/// climb back up and the corridors you remember are stocked with different
/// monsters and loot. Saved, so a reload lands on the same re-roll.
#[derive(Resource, Default)]
pub struct FloorChanges {
    pub count: u32,
}

/// Tracks how long the player has lingered on one dungeon level. Every turn adds
/// one; every level change resets it to zero. When it reaches
/// [`crate::map::DUNGEON_LORD_PATIENCE`] the Dungeon Lord opens a portal under
/// the player's feet and shunts them to the next level. Transient, never saved.
#[derive(Resource, Default)]
pub struct DungeonLord {
    pub idle_turns: u32,
}
