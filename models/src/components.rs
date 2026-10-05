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
//! ([`MovementType`], [`Faction`], [`SpeedKind`], [`TrapReveal`],
//! the `*Effect` enums) is written to the save by variant position, and every
//! serialised struct by field order. Append new variants and fields; do not
//! reorder existing ones, or old saves change meaning.

use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

use crate::catalog::{ArmorDef, ItemDef, PotionDef, RingDef, ScrollDef, WandDef, WeaponDef};
use crate::constants::hud::LOG_HISTORY_CAP;
use crate::constants::speed::{ACTION_COST, FAST_RATE, NORMAL_RATE, QUICK_RATE, SLOW_RATE};
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
    /// The name. `crate::identify` swaps another in for display where the thing is not yet identified.
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
    /// Column. With `y`, flattened into an array index by [`crate::map::tile_index`].
    pub x: u16,
    /// Row.
    pub y: u16,
}

/// How an entity draws: one glyph in one colour. The colour is packed to a byte
/// against a fixed 16-entry palette on save (see `crate::saveload`).
#[derive(Component)]
pub struct Renderable {
    /// The character it draws as.
    pub glyph: char,
    /// The colour it draws in.
    pub color: Color,
}

/// Whose side an actor is on. Monsters fight the player and (in principle) spare
/// each other; `Ally` fights the monsters. A [`Helper`] is an `Ally`.
///
/// `Spirits` is its own side: peaceful toward everyone until
/// [`SpiritsHostile`] flips, at which point it reads hostile toward the
/// player and their allies exactly like `Monster` does. See
/// [`crate::ai::hostile`].
///
/// Appended, not filed under M: a save encodes a variant by its position
/// (`postcard`), so new variants only ever go at the end.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Faction {
    /// The player, and the one side every [`Faction::Monster`] hunts.
    Player,
    /// Fights [`Faction::Player`] and [`Faction::Ally`], never another monster.
    Monster,
    /// On the player's side: fights [`Faction::Monster`].
    Ally,
    /// Peaceful toward everyone until [`SpiritsHostile`] is set.
    Spirits,
}

/// The hidden pull between the demons (cacodaemons) and the angels/sphynx/
/// elves (eudaemons): minus [`ALIGNMENT_POLE`](crate::constants::spirits::ALIGNMENT_POLE)
/// is fully cacodaemon-aligned, plus that fully eudaemon-aligned, `0` neutral.
/// Interacting with a cacodaemon spirit moves this toward its pole by
/// [`ALIGNMENT_STEP`](crate::constants::spirits::ALIGNMENT_STEP), a eudaemon
/// spirit toward the other. Reaching either pole flips
/// [`SpiritsHostile`] for good. A player-only stat, so it lives on the
/// player entity the same way [`Fighter`]/[`Spellset`] do rather than as a
/// bare resource.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Alignment(pub i8);

/// Whether the spirits have turned on the player for good this run: either
/// [`Alignment`] reached a pole, or the player landed a direct hit (melee or
/// a fired/thrown shot — never an AoE blast or a trap) on a peaceful spirit.
/// Permanent once set; there is no path back down except starting a new run.
#[derive(Resource, Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SpiritsHostile(pub bool);

/// Which way a spirit's row pulls [`Alignment`] when the player interacts
/// with it peacefully: a cacodaemon (the demons) one way, a eudaemon (the
/// angel, sphynx, sylphid, salamander, undyne, gnome) the other, each by
/// [`ALIGNMENT_STEP`](crate::constants::spirits::ALIGNMENT_STEP). Set from
/// `crate::monsters::MonsterDef::spirit_kind`, not saved — cheap to rebuild
/// from the bestiary row on load, the same way `Grants` is.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpiritKind {
    /// A demon: interacting with it moves [`Alignment`] toward the cacodaemon
    /// pole.
    Cacodaemon,
    /// An angel or kin: interacting with it moves [`Alignment`] toward the
    /// eudaemon pole.
    Eudaemon,
}

/// What melee does to a peaceful [`Faction::Spirits`] mob instead of a normal
/// attack: opens a menu, teaches a spell, unmakes the player's gear —
/// whatever that species' row says. `None` (no component at all) falls back
/// to a generic "nothing happens" line — see `crate::spirits::trigger_event`.
/// Set from `crate::monsters::MonsterDef::spirit_event`, not saved, for the
/// same reason as [`SpiritKind`].
#[derive(Component, Clone, Copy)]
pub struct SpiritEvent(pub fn(&mut World, Entity, Entity));

/// The angel's blessing, pending: set the instant its "Tests your faith!"
/// event fires, cleared the next time the player takes any staircase. See
/// [`crate::spirits::apply_test_of_faith`].
#[derive(Component, Default)]
pub struct TestOfFaithLedger;

/// One thing a spirit's "choose one of three" menu is offering — the blue
/// demon's spells, the sylphid's weapons, the salamander's armor, undyne's
/// rings, the red demon's gear, the gnome's magic. See [`crate::spirits`].
#[derive(Clone, Copy)]
pub enum OfferOption {
    /// A spell to learn. Free even in a priced menu: see [`OfferOption::price`].
    Spell(SpellEffect),
    /// A weapon to take. Costs Max HP in a priced menu.
    Weapon(&'static WeaponDef),
    /// A suit of armour to take. Costs Max HP in a priced menu.
    Armor(&'static ArmorDef),
    /// A ring to take. Costs Max HP in a priced menu.
    Ring(&'static RingDef),
    /// A scroll to take. Costs Max Ma in a priced menu.
    Scroll(&'static ScrollDef),
    /// A potion to take. Costs Max Ma in a priced menu.
    Potion(&'static PotionDef),
    /// A wand to take. Costs Max Ma in a priced menu.
    Wand(&'static WandDef),
}

/// What a priced offer costs: Max HP for the red demon's gear, Max Ma for the
/// gnome's magic. See [`OfferMenu::priced`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Price {
    /// This much off the player's max HP, for good. A price equal to the whole of
    /// it is still payable, and paying it ends the run.
    MaxHp(i32),
    /// This much off the player's max Ma, for good.
    MaxMa(u8),
}

impl OfferOption {
    /// The name the menu lists this row under.
    pub fn display_name(&self) -> &'static str {
        match self {
            OfferOption::Spell(effect) => crate::catalog::SpellDef::of(*effect).display_name(),
            OfferOption::Weapon(def) => def.display_name(),
            OfferOption::Armor(def) => def.display_name(),
            OfferOption::Ring(def) => def.display_name(),
            OfferOption::Scroll(def) => def.display_name(),
            OfferOption::Potion(def) => def.display_name(),
            OfferOption::Wand(def) => def.display_name(),
        }
    }

    /// What this row costs when the menu is [`OfferMenu::priced`]. The kind
    /// of thing sets the price, so the red demon and the gnome need no table
    /// of their own. `None` for a spell: no spirit sells one.
    pub fn price(&self) -> Option<Price> {
        use crate::constants::spirits::*;
        match self {
            OfferOption::Spell(_) => None,
            OfferOption::Weapon(_) | OfferOption::Armor(_) | OfferOption::Ring(_) => {
                Some(Price::MaxHp(RED_DEMON_GEAR_PRICE))
            }
            OfferOption::Scroll(_) => Some(Price::MaxMa(GNOME_SCROLL_PRICE)),
            OfferOption::Potion(_) => Some(Price::MaxMa(GNOME_POTION_PRICE)),
            OfferOption::Wand(_) => Some(Price::MaxMa(GNOME_WAND_PRICE)),
        }
    }
}

/// The "choose one of three" menu: open, which row the cursor sits on, and
/// exactly what was rolled to offer — rolled once, when the spirit's event
/// opens it, so backing out and reopening the menu is not how this is done
/// (a spirit only ever gets one interaction). See
/// [`crate::spirits::open_offer_menu`], and the `Z` [`SpellsMenu`] this is
/// modeled on.
#[derive(Resource, Default)]
pub struct OfferMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The row the cursor sits on, an index into `options`.
    pub selected: usize,
    /// What was rolled to offer, in the order the rows are drawn.
    pub options: Vec<OfferOption>,
    /// The spirit whose event opened this menu, if a spirit's did — it
    /// poofs once the player confirms a choice (not on cancel, so a
    /// player who backs out can just melee it again). `None` for anything
    /// that reaches this menu some other way.
    pub source: Option<Entity>,
    /// Whether picking a row costs its [`OfferOption::price`] (the red demon,
    /// the gnome) or comes free (every other spirit).
    pub priced: bool,
}

/// One thing that can sit on either side of a [`BarterMenu`]: an entity in a
/// pack (the yellow demon's trade) or a spell either side already knows or
/// offers (the sphynx's). See [`crate::spirits`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tradeable {
    /// An item in a pack, by entity.
    Item(Entity),
    /// A spell, by kind. It moves between spellsets rather than between packs.
    Spell(SpellEffect),
}

/// Which column the cursor is in.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BarterColumn {
    /// The pool of what the player holds.
    #[default]
    Player,
    /// The pool of what the demon or sphynx holds.
    Demon,
}

/// The barter menu: the yellow demon's item trade, or the sphynx's spell
/// trade — same shape either way. `player_side`/`demon_side` are the full
/// pools each column lists (rolled or read once, when the menu opens);
/// `player_selected`/`demon_selected` are what's currently staged to change
/// hands. Confirming moves every selected [`Tradeable`] the way it says to
/// and poofs the demon; cancelling moves nothing. See
/// [`crate::spirits::confirm_barter`].
#[derive(Resource, Default)]
pub struct BarterMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The trader across the table. It poofs once a trade is confirmed.
    pub demon: Option<Entity>,
    /// Which pool the cursor is in.
    pub column: BarterColumn,
    /// The row the cursor sits on, within the pool `column` names.
    pub cursor: usize,
    /// Everything the player could put up.
    pub player_side: Vec<Tradeable>,
    /// Everything the trader could put up. Read it through
    /// [`BarterMenu::demon_visible`], which hides the rows the player cannot
    /// match.
    pub demon_side: Vec<Tradeable>,
    /// What the player has staged to give away. Nothing moves until the trade
    /// is confirmed.
    pub player_selected: Vec<Tradeable>,
    /// What the player has staged to receive.
    pub demon_selected: Vec<Tradeable>,
}

impl BarterMenu {
    /// The demon rows the player can actually reach: one per thing they hold
    /// to put against it, so a short pack or spellset hides the demon's
    /// tail. Rendering, cursor travel and staging all read this, never
    /// `demon_side` directly.
    ///
    /// ```
    /// use models::{BarterMenu, SpellEffect, Tradeable};
    ///
    /// let spell = Tradeable::Spell(SpellEffect::Heal);
    /// let menu = BarterMenu {
    ///     player_side: vec![spell],
    ///     demon_side: vec![spell, spell, spell],
    ///     ..Default::default()
    /// };
    /// // One thing to put against three: only one demon row is in reach.
    /// assert_eq!(menu.demon_visible().len(), 1);
    /// ```
    pub fn demon_visible(&self) -> &[Tradeable] {
        let reach = self.player_side.len().min(self.demon_side.len());
        &self.demon_side[..reach]
    }
}

/// The player's boon companion: the one [`Faction::Ally`] that follows them
/// between floors. There is only ever one. Loyalty is not magic, so this is a
/// plain component and not an effect row a wand of cancellation could strip.
/// See [`crate::companion`].
#[derive(Component)]
pub struct Helper;

/// A thing thrown as an offer of loyalty: a snack for a creature without hands,
/// a fancy of peace for one with them. See [`crate::companion`].
#[derive(Component, Clone, Copy)]
pub struct Treat {
    /// Whether this is meant for an [`crate::effects::ItemUser`].
    pub for_item_users: bool,
}

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
/// rule set again. See [`crate::ai`].
#[derive(Component, Clone, Copy)]
pub struct Aggravated {
    /// Column of the tile the noise came from.
    pub tx: u16,
    /// Row of the tile the noise came from.
    pub ty: u16,
}

/// Everything needed to resolve a fight. Combat is a pair of opposed rolls with
/// no to-hit step: `damage = (1d[power] + power_bonus) - (1d[armor] +
/// armor_bonus)`, each side rolled independently, nothing ever missing. See
/// `crate::combat`.
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
/// turns and spend it in [`crate::ai`], while the player's own tempo is
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

    /// One notch quicker (wand of haste monster). `Fast` is the ceiling.
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
    /// ```
    pub fn faster(self) -> Self {
        match self {
            SpeedKind::Slow => SpeedKind::Normal,
            SpeedKind::Normal | SpeedKind::Quick | SpeedKind::Fast => SpeedKind::Fast,
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
/// consults the player's [`SpeedKind`] after every turn: a `Fast` player takes
/// two inputs before the monsters get a move, a `Quick` one takes three for
/// every two, a `Slow` player's single move is followed by two monster rounds,
/// and `Normal` is one-for-one. Transient, never serialised.
#[derive(Resource, Default)]
pub struct PlayerTempo {
    /// Flips on each `Fast`-tempo turn; monsters move only when it flips back to
    /// `false`, so the pattern reads skip / run / skip / run.
    pub fast_parity: bool,
    /// How many turns the player has taken at a tempo that doesn't divide
    /// evenly into monster rounds. `Quick` is the only one: two rounds bought
    /// per three turns, so the third turn of every three is free. Counts on
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
/// [`crate::autofight::visible_enemies`]) so auto-explore and auto-fight are
/// fooled right along with the player. [`crate::monsters::reveal_mimics`]
/// strips this — and the disguise with it — the instant the player is
/// standing next to it. See [`crate::monsters::MonsterDef::mimics`].
#[derive(Component)]
pub struct Mimic;

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

/// What an item is worth in score, paid the moment it is picked up. Coins and
/// the relic carry it; see [`crate::score::award`] and
/// [`crate::items::pickups::pick_up`].
#[derive(Component)]
pub struct Value {
    /// The score paid on pickup.
    pub amount: i32,
}

/// An item that is never carried: it works the instant you step on it and is
/// gone. Every coin is one.
///
/// Three rules follow from "never carried", and together they are what makes a
/// pickup a different kind of thing from an item you stow:
///
/// * **A full pack is no obstacle.** There is nothing to find room for.
/// * **It is left alone when it would do nothing.** Walk over a red coin at
///   full health and it stays on the floor waiting for the day you need it
///   ([`crate::items::pickups::would_help`]), and auto-explore does not detour
///   for one it cannot use either.
/// * **It can be shot.** A missile that comes down on one sets it off like a
///   trap, in a wider burst
///   ([`PICKUP_TRICK_SHOT_RADIUS`](crate::constants::traps::PICKUP_TRICK_SHOT_RADIUS)
///   against [`TRICK_SHOT_RADIUS`](crate::constants::traps::TRICK_SHOT_RADIUS)) — see
///   [`crate::traps::detonate_pickup`].
#[derive(Component)]
pub struct Pickup {
    /// What picking it up does.
    pub effect: PickupEffect,
    /// The number the effect works with — points, hit points, afflictions
    /// lifted — straight off the [`crate::catalog::CoinDef`] row.
    pub amount: i32,
}

/// What stepping on a pickup does. The mechanic is an exhaustive match in
/// [`crate::items::pickups`]; the amount it works with is the `amount` on the
/// [`crate::catalog::CoinDef`] row, so "how much" is data and "what kind" is
/// this.
///
/// Serialised by variant position — append, never reorder.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickupEffect {
    /// Straight into the score.
    Coin,
    /// Hit points, up to the ceiling.
    Health,
    /// Magic points, up to the ceiling.
    Power,
    /// Lifts up to `amount` afflictions.
    Cleanse,
    /// Gives back up to `amount` points of drained melee strength.
    Strength,
    /// The [`Plated`] promise.
    Platinum,
    /// The [`Forged`] promise.
    Forge,
    /// Teaches the taker one random spell, straight into their [`Spellset`] —
    /// the hero coin, uncommon, and unlike every other item in the game never
    /// disguised: it is always just "a hero coin", the one thing in the
    /// dungeon with nothing to identify.
    LearnRandomSpell,
}

/// An actor's carried items, in inventory-letter order. An item in here has had
/// its [`Position`] removed; dropping or throwing puts one back.
#[derive(Component)]
pub struct Backpack {
    /// The items carried, in inventory-letter order.
    pub items: Vec<Entity>,
}

/// Used up on use: a potion or a scroll. [`item_system`](crate::items::item_system)
/// despawns anything carrying this once its effect has been applied.
#[derive(Component)]
pub struct Consume;

/// A wand's remaining charges. Each zap spends one; at zero the wand crumbles.
/// Every wand spawns with [`crate::constants::wands::WAND_CHARGES`], full.
#[derive(Component)]
pub struct Battery {
    /// Zaps left. At zero the wand crumbles.
    pub charges: i8,
}

/// How many identical items share one pack slot. Only ammunition stacks: a
/// quiver of arrows is one entity carrying a number, not thirty entities
/// crowding thirty inventory letters. Throwing spends one; picking more up tops
/// the stack back up to at most [`STACK_LIMIT`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    /// How many are in the stack.
    pub count: u8,
}

/// An item with a firing range of its own: a wand. Feeds the aiming reticle when
/// the item is zapped (a thrown item uses [`crate::items::THROW_RANGE`] instead).
#[derive(Component)]
pub struct Ranged {
    /// How far the reticle reaches when the item is zapped.
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
    /// Which potion this is.
    pub effect: PotionEffect,
}

/// Which potion this is. Identity for identification and saves — see
/// [`crate::catalog::POTIONS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum PotionEffect {
    /// Blinds the drinker for the floor ([`crate::conditions::blind`]): sight
    /// shrinks to the 3x3 around them.
    Blindness,
    /// Confuses the drinker ([`crate::conditions::confuse`]).
    Confusion,
    /// Refills HP and raises max HP for good, by more than
    /// [`Healing`](PotionEffect::Healing) does.
    ExtraHealing,
    /// Nothing happens, and it reports nothing, so a thrown one never gives
    /// itself away.
    FruitJuice,
    /// Raises the attack die for good, floor and ceiling both ([`Fighter::power`]
    /// and [`Fighter::max_power`]).
    GainStrength,
    /// Hastes the drinker ([`crate::conditions::hasten`]).
    Haste,
    /// Refills HP and raises max HP for good, so one drunk at full health is not
    /// wasted.
    Healing,
    /// Marks every magic item on the floor as [`Detected`](crate::effects::Detected)
    /// until the drinker leaves it. The Element of Yoord counts as magic.
    MagicDetection,
    /// Marks every creature on the floor as [`Detected`](crate::effects::Detected)
    /// until the drinker leaves it. They are sensed, not watched: no sighting is
    /// announced.
    MonsterDetection,
    /// Locks the drinker's limbs ([`crate::conditions::paralyse`]). The player
    /// forfeits a share of their turns.
    Paralysis,
    /// Drains the drinker's attack die and does not give it back. Only
    /// [`RestoreStrength`](PotionEffect::RestoreStrength) cures it.
    Poison,
    /// Pulls the drinker up one floor, Element of Yoord or not. On Depth 1 it
    /// wins the run for a player carrying the Element.
    RaiseLevel,
    /// Back up to [`Fighter::max_power`], undoing every poison and poisoned dart
    /// at once.
    RestoreStrength,
    /// Lends the ring of perception's sight for the floor
    /// ([`crate::effects::SeesInvisible`]).
    SeeInvisible,
    /// Plain water. Reports nothing, like [`PotionEffect::FruitJuice`].
    Water,
    // Appended, not filed under M: a save encodes a variant as its position.
    /// Refills Ma and raises max Ma for good.
    Magic,
    /// Resets `Alignment` to neutral. Appended after `Magic` for the same
    /// reason.
    Adjustment,
    /// The wand of polymorph, drunk. Appended for the same reason.
    Polymorph,
}

/// Type-key for a scroll. Mechanic: the `scrolls` submodule of `crate::items`.
#[derive(Component)]
pub struct Scroll {
    /// Which scroll this is.
    pub effect: ScrollEffect,
}

/// Which scroll this is. Identity for identification and saves — see
/// [`crate::catalog::SCROLLS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollEffect {
    /// Charges the reader's hands instead of acting now: the next blow they land
    /// confuses what it hits ([`crate::effects::ConfusingTouch`]).
    MonsterConfusion,
    /// Maps the whole floor into the reader's memory, wiped out from where they
    /// stand ([`crate::magicmap`]).
    MagicMapping,
    /// Roots everything in sight where it stands for a while
    /// ([`crate::effects::Rooted`]). A held monster still bites what comes
    /// within reach.
    HoldMonster,
    /// Puts everything in sight to sleep. Now and then the words turn on the
    /// reader and put them out instead.
    Sleep,
    /// A permanent plus on the worn armour, and its curse lifted. Fizzles with
    /// nothing worn.
    EnchantArmor,
    /// Reveals the hidden enchantment and curse of every piece of gear in the
    /// pack.
    Identify,
    /// Every monster in view turns tail for good.
    ScareMonster,
    /// Marks the mundane items on the floor as [`Detected`](crate::effects::Detected):
    /// exactly what [`PotionEffect::MagicDetection`] skips.
    FoodDetection,
    /// Moves the reader to a random open tile on the floor.
    Teleportation,
    /// A permanent plus on the wielded weapon, and its curse lifted. Fizzles
    /// with an empty hand.
    EnchantWeapon,
    /// Conjures a creature from the bestiary beside the reader.
    CreateMonster,
    /// Destroys every cursed item the reader has equipped. Cursed items left in
    /// the pack are untouched.
    RemoveCurse,
    /// Every creature on the floor homes in on the reader's tile, out of sight
    /// ([`Aggravated`]).
    AggravateMonsters,
    /// Does nothing, and means it.
    BlankPaper,
    /// Brands the wielded weapon [`Vorpal`] against one random species. A weapon
    /// that is already vorpal crumbles instead.
    VorpalizeWeapon,
    /// 1... 2... Poof! Forgets one random spell off the reader's [`Spellset`]
    /// and every tile they have ever seen on this floor.
    Amnesia,
    /// Every monster the reader can see is charmed — a plain
    /// [`Faction::Ally`], the same as [`WandEffect::Charming`] lands on one.
    Charming,
    /// Opens a trapdoor under the reader: the same plunge as [`TrapEffect::Trapdoor`].
    /// Appended for the save-order reason above.
    Pitfall,
    /// Makes the spirits neutral again: [`crate::spirits::atone`].
    Atonement,
}

/// Type-key for a wand. Mechanic: the `wands` submodule of `crate::items`
/// (zapped), and `throwing` (hurled).
#[derive(Component)]
pub struct Wand {
    /// Which wand this is.
    pub effect: WandEffect,
}

/// Which wand this is. Identity for identification and saves — see
/// [`crate::catalog::WANDS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WandEffect {
    /// Lights the room (or passage) the zapper stands in for good, and turns up
    /// any hidden trap there. Takes no target ([`WandEffect::needs_target`]).
    Light,
    /// A bolt of armour-ignoring force along the aimed line.
    Striking,
    /// A bolt of lightning along the aimed line.
    Lightning,
    /// A fire blast over [`BLAST_RADIUS`](crate::constants::wands::BLAST_RADIUS)
    /// where aimed, resisted by [`FireImmune`] ([`Element::Fire`]).
    Fire,
    /// A cold blast over [`BLAST_RADIUS`](crate::constants::wands::BLAST_RADIUS)
    /// where aimed, resisted by [`ColdImmune`] ([`Element::Cold`]).
    Cold,
    /// Replaces the monster on the aimed tile with a random other species.
    /// Aimed at the zapper's own tile, it polymorphs them.
    Polymorph,
    /// A bolt of magic missile along the aimed line.
    MagicMissile,
    /// Steps the target one notch faster, for good.
    HasteMonster,
    /// Steps the target one notch slower, for good.
    SlowMonster,
    /// A bolt that hands the HP it takes back to the zapper, never past their
    /// maximum ([`Element::Drain`]).
    DrainLife,
    /// Does nothing, and says so.
    Nothing,
    /// Flings the target monster to a random open tile.
    TeleportAway,
    /// Drags the target monster to a tile beside the zapper.
    TeleportTo,
    /// Strips every marker effect from the target and resets its tempo. The
    /// player zapped by it loses far more.
    Cancellation,
    /// Tames the monster on the target tile: a plain [`Faction::Ally`], not
    /// the [`Helper`] — it fights at your side but doesn't follow downstairs,
    /// and taking a second one doesn't retire a first. See
    /// [`crate::companion::charm`].
    Charming,
    /// Bores a tunnel through rock along the aim, [`DIG_RANGE`] tiles deep
    /// (never through the map's outer wall). See
    /// [`crate::items::wands`]'s `dig_tunnel`.
    ///
    /// [`DIG_RANGE`]: crate::constants::wands::DIG_RANGE
    Digging,
    /// The zapper and what stands on the aimed tile trade places; thrown, the
    /// blast's creatures trade among themselves. See
    /// [`crate::items::wands`]'s `swap_with_target`.
    Swapping,
}

impl WandEffect {
    /// Whether zapping this wand opens the aiming reticle. Every wand needs a
    /// target except the wand of light, which floods the room the zapper stands
    /// in and so is "used" immediately like a potion or scroll.
    pub fn needs_target(self) -> bool {
        !matches!(self, WandEffect::Light)
    }
}

/// Which active spell this is — a spell's identity, the same way [`WandEffect`]
/// is a wand's. See [`crate::catalog::SpellDef`].
///
/// Serialised by variant position — append, never reorder.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellEffect {
    /// A fire blast like [`WandEffect::Fire`], but the damage is whatever the
    /// caster's own attack deals. A dragon's breath is this spell; the player
    /// knows it as Fireball.
    DragonBreath,
    /// A poisoned dart cast at range: the dart trap's damage and its depth-scaled
    /// strength drain.
    Sting,
    /// Armour-ignoring damage, and a chance to paralyse the target.
    Thunderbolt,
    /// Lifts the caster's single worst affliction.
    Cure,
    /// Does nothing now, and strengthens the caster's next swing
    /// ([`crate::effects::Bided`]).
    Bide,
    /// A line of armour-ignoring damage: [`WandEffect::Striking`], cast.
    ForceLance,
    /// [`ScrollEffect::Identify`] on demand.
    Identify,
    /// Plants a revealed arrow trap on each of the caster's four diagonals.
    Setup,
    /// [`WandEffect::Light`] hurled as a grenade: a wide, hot burst that blinds.
    Lux,
    /// Armour-ignoring drain on every hostile in view, handed back to the caster
    /// as HP.
    CircleOfDeath,
    /// For the rest of the floor, wand-shaped harm bounces off the caster and
    /// nothing a monster's blow carries takes hold.
    MagicWard,
    /// Refills the caster's HP to their ceiling without raising it.
    Heal,
    /// A fire grenade thrown at the aimed tile. Each impact may call another
    /// down nearby.
    MeteorStrike,
    /// Cold damage to everything in view, and paralysis for what survives.
    FrostNova,
    /// [`ScrollEffect::MagicMapping`] on demand.
    MagicMapping,
    /// Hastes the caster ([`crate::conditions::hasten`]).
    HasteSelf,
    /// Appended, not filed under P: a save encodes a variant as its
    /// position. See [`crate::items::wands::polymorph_entity`].
    PolymorphSelf,
    /// Polymorphs the creature on the aimed tile, as [`WandEffect::Polymorph`]
    /// does.
    PolymorphOther,
    /// Opens a trapdoor under the caster: [`ScrollEffect::Pitfall`] on demand.
    GateDown,
}

impl SpellEffect {
    /// Whether triggering this spell opens the aiming reticle at all. Most
    /// attacks do; every skill that works on the caster alone or on
    /// everything in view has nothing to aim at, and fires the instant its
    /// slot is pressed — the same courtesy [`WandEffect::needs_target`] gives
    /// the wand of light.
    pub fn needs_target(self) -> bool {
        !matches!(
            self,
            SpellEffect::Cure
                | SpellEffect::GateDown
                | SpellEffect::Bide
                | SpellEffect::Identify
                | SpellEffect::Setup
                | SpellEffect::CircleOfDeath
                | SpellEffect::MagicWard
                | SpellEffect::Heal
                | SpellEffect::FrostNova
                | SpellEffect::MagicMapping
                | SpellEffect::HasteSelf
                | SpellEffect::PolymorphSelf
        )
    }
}

/// The two shapes an active spell comes in — an attack wand's own split
/// ([`crate::items::wands::is_attack_wand`]), drawn again here because a spell
/// answers to it too: a staff's [`crate::effects::TurboMagic`] multiplies the
/// cost ([`TURBO_MAGIC_COST_MULT`](crate::constants::spells::TURBO_MAGIC_COST_MULT))
/// and the damage ([`TURBO_MAGIC_POWER_MULT`](crate::constants::spells::TURBO_MAGIC_POWER_MULT))
/// of an [`Attack`](SpellKind::Attack), and leaves a
/// [`Skill`](SpellKind::Skill) — the utility half, potions and scrolls play the
/// same way — alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellKind {
    /// Deals damage — [`crate::effects::TurboMagic`]'s business.
    Attack,
    /// Everything else a spell can do.
    Skill,
}

/// The player's active-ability bar: up to four spells, each picked by its row
/// letter from the `Z` menu.
///
/// A spell is coded the way a potion, scroll or wand is — one identity enum,
/// one catalog row ([`crate::catalog::SpellDef`]), one mechanic keyed off it
/// ([`crate::items::spells`]) — because it is exactly as *active* as any of
/// those. It differs from every item in the game in what it is not: it
/// carries no [`Item`] marker, is never spawned with a [`Position`], holds no
/// pack slot, and cannot be dropped or thrown. It lives here, permanently, and
/// costs [`Magic`] per use instead of a battery running dry.
#[derive(Component, Default, Clone, Serialize, Deserialize)]
pub struct Spellset {
    /// The spells known, in slot order.
    pub slots: Vec<SpellEffect>,
}

/// Type-key for a ring, the twin of [`Potion`] / [`Scroll`] / [`Wand`]. What the
/// ring *does* is not read from here — it rides along as modifier components and
/// [`crate::effects::Grants`], attached by its [`crate::catalog::RingDef`] row.
/// This tag exists so the ring can be identified and saved.
#[derive(Component)]
pub struct Ring {
    /// Which ring this is.
    pub effect: RingEffect,
}

/// The name of a ring type — its identity for identification and saving, not a
/// description of its behaviour. See [`crate::catalog::RINGS`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum RingEffect {
    /// A bonus to the armour roll.
    Protection,
    /// A bonus to the damage roll, and strength that a poisoned dart cannot drain
    /// ([`crate::effects::SustainsStrength`]).
    Strength,
    /// Sees what is invisible: hidden traps, stashes and monsters
    /// ([`crate::effects::SeesInvisible`]).
    Perception,
    /// Worn once, for one action, it doubles the run's score and is gone.
    Adornment,
    /// Now and then everything on the floor learns where the wearer is
    /// ([`crate::effects::AggravatesMonsters`]).
    AggravateMonster,
    /// A bonus to the throw roll, on a hurled dagger as much as a loosed arrow.
    Sharpshooting,
    /// A bonus to the damage roll, without [`RingEffect::Strength`]'s protection
    /// from drain.
    IncreaseDamage,
    /// Knits the wearer back together as they go
    /// ([`crate::effects::Regenerates`]).
    Regeneration,
    /// Slows the wearer one notch ([`crate::effects::Sluggish`]).
    SlowDigestion,
    /// Now and then the wearer is somewhere else ([`crate::effects::Teleportitis`]).
    Teleportation,
    /// Nothing notices the wearer until it is within
    /// [`STEALTH_RANGE`](crate::constants::rings::STEALTH_RANGE) tiles
    /// ([`crate::effects::Stealthy`]).
    Stealth,
    /// What the wearer has on cannot be corroded ([`crate::effects::SustainsArmor`]).
    MaintainArmor,
    /// Appended: a save encodes a variant as its position.
    Polymorph,
}

/// Tag for a cursed piece of equipment. Rolled on at spawn for the majority of
/// weapon/armour/ring drops (see [`crate::catalog::enchant_equipment`]). Once a
/// cursed item is equipped it can't be taken off again until the curse is lifted
/// by a scroll of remove curse.
#[derive(Component)]
pub struct Curse;

/// A bones ghost whose name matches the current character's own — set only
/// by `crate::map::levels::spawn_bones_ghost`. `crate::combat` reads this to
/// decide when a fight with the ghost is narrated as "you" against yourself
/// instead of by name.
#[derive(Component)]
pub struct GhostOfPlayer;

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
    /// The name of the species it slays outright.
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
    /// Resisted by [`FireImmune`].
    Fire,
    /// Resisted by [`ColdImmune`].
    Cold,
    /// Life drain. Resisted by [`Undead`](crate::effects::Undead), which has no
    /// life to take.
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
            Element::Fire => strings::element_fire_noun(),
            Element::Cold => strings::element_cold_noun(),
            Element::Drain => strings::element_drain_noun(),
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

/// The effect that turns a lobbed missile into a loosed one. An arrow answers
/// to [`crate::effects::FireArrow`], a quarrel to
/// [`crate::effects::FireQuarrel`]; the bow and crossbow are simply things that
/// grant those. Neither missile knows a launcher exists, and no launcher knows
/// what ammunition is — they meet at the effect, like everything else here.
/// What the missile rolls once loosed is [`LaunchedDamage`], not this.
#[derive(Component, Clone, Copy)]
pub struct LaunchedBy(pub crate::effects::Grant);

/// What this item rolls once [`LaunchedBy`] says it has been loosed rather
/// than lobbed, in place of doubling [`ThrownDamage`]. A quarrel still gets
/// the full double (a crossbow's whole point); an arrow gets less than that —
/// a deliberate nerf on the bow, the most efficient weapon in the game by a
/// wide margin — so the two dials live apart instead of one shared multiplier.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchedDamage(pub i32);

/// A bow or a crossbow: gear that is worth nothing swung and everything drawn.
/// It contributes no attack die, so its enchantment has no melee roll to land
/// on and lands on [`crate::effects::ThrowBonus`] instead — a +2 bow puts +2 on
/// every arrow it looses. See [`crate::catalog::enchant_equipment`].
#[derive(Component, Clone, Copy)]
pub struct Launcher;

/// A melee weapon that reaches past adjacency: a bardiche strikes two tiles
/// out, a whip five. Aimed with its own reticle (`v`) rather than a walk into
/// the target's tile — see `crate::combat::resolve_reach_attack` and
/// `engine`'s `begin_reach_attack`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reach(pub i32);

/// A reach weapon's strike does not stop at the first body in its line — a
/// bardiche runs clean through, the way a piercing thrown spear does (see
/// [`Piercing`]). A whip, with no polearm's length behind it, lacks this and
/// stops at the first thing it finds.
#[derive(Component, Clone, Copy)]
pub struct ReachPiercing;

// ===========================================================================
// Player conditions
// ===========================================================================

// `ConfusingTouch` and `Bided` used to be declared here. They are buffs the
// bearer holds rather than conditions that impair them, so they are effects
// now and live with the rest in `crate::effects` — which is also what puts
// them in the save file and within reach of a wand of cancellation.

/// A promise the dungeon made you, and the terms are the same for both of the
/// coins that make one: **reach the next staircase without being hurt again**
/// and it pays out. Take a single point of damage and it is gone, with a line
/// saying so.
///
/// [`Plated`] pays a permanent point of attack or defence die — the coin flip
/// is the dungeon's, not yours. [`Forged`] pays a point of plus on the weapon
/// in your hand or the armour on your back, exactly as the matching scroll
/// would, curse and all.
///
/// They are conditions and not items because that is how they behave: carried
/// on the player, shown on the HUD (`PLAT`, `FORG`), and lost to something that
/// happens *to* you. Unlike every other condition they are not lifted by the
/// staircase — the staircase is what cashes them
/// ([`crate::items::pickups::settle_promises`]).
#[derive(Component)]
pub struct Plated;

/// The forge's half of the same bargain. See [`Plated`].
#[derive(Component)]
pub struct Forged;

// ===========================================================================
// Traps and snares
// ===========================================================================

/// Which of the six trap kinds a [`Trap`] entity is. The effect always fires
/// when the trap is stepped on — there is no saving throw. The mechanic keyed
/// off each variant lives in `crate::traps` (`apply_trap_effect`); the catalog row
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
    /// What it does to whatever steps on it.
    pub effect: TrapEffect,
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

// `Confused`, `Blind`, `Paralyzed`, `MagicWard` and `Detected` used to live
// here too. They are effects now — things a creature *has*, that something
// else asks about — so they live in `crate::effects` with the rest, which is
// also what gives them a lifetime and puts them in the save file by name.
//
// `Snare` and `SnareKind` used to live here. The three holds they described
// are ordinary effects now — `Asleep`, `Pinned` and `Rooted` in
// `crate::effects` — each with its own clock, so a creature can be asleep and
// pinned at once and come out of each when its own turns run out.

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
    /// Whoever swings.
    pub attacker: Entity,
    /// Whoever is swung at.
    pub target: Entity,
}

/// Intent: `user` uses `item` — quaff, read, zap or (un)equip, depending on what
/// it is. `target` is the aimed tile for a wand; `slot_idx` is the pack row it
/// came from, so it can go back exactly there. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Event, Clone, Copy)]
pub struct WantsToUse {
    /// Whoever quaffs, reads, zaps or equips.
    pub user: Entity,
    /// The item used.
    pub item: Entity,
    /// The aimed tile. `None` for anything that is not aimed.
    pub target: Option<Position>,
    /// The pack row the item came from. `None` when it did not come from one.
    pub slot_idx: Option<usize>,
}

/// A hurled item, in flight from `thrower` towards `target`. Resolved by
/// [`crate::items::throw_system`], which is where it finds out what it hits.
#[derive(Event, Clone, Copy)]
pub struct WantsToThrow {
    /// Whoever throws.
    pub thrower: Entity,
    /// What is thrown.
    pub item: Entity,
    /// The tile it is thrown at.
    pub target: Position,
}

/// The turn's pending attacks. Filled by input / AI, drained by
/// `crate::combat`.
#[derive(Resource, Default)]
pub struct AttackQueue {
    /// Taken whole when combat runs. An attack pushed while those resolve waits
    /// for the next run.
    pub attacks: Vec<WantsToAttack>,
}

/// The turn's pending item uses. Drained by
/// [`item_system`](crate::items::item_system).
#[derive(Resource, Default)]
pub struct UseQueue {
    /// Taken whole when the item system runs; a use pushed meanwhile waits for
    /// the next run.
    pub uses: Vec<WantsToUse>,
}

/// The turn's pending throws. Drained by [`crate::items::throw_system`].
#[derive(Resource, Default)]
pub struct ThrowQueue {
    /// Taken whole when the throw system runs; a throw pushed meanwhile waits
    /// for the next run.
    pub throws: Vec<WantsToThrow>,
}

/// Intent: `user` triggers active spell `effect` at `target` — a spell's twin of
/// [`WantsToUse`], minus everything about an item because a spell isn't one.
/// Drained by [`spell_system`](crate::items::spell_system).
#[derive(Event, Clone, Copy)]
pub struct WantsToCast {
    /// Whoever casts.
    pub user: Entity,
    /// The spell cast.
    pub effect: SpellEffect,
    /// The aimed tile.
    pub target: Position,
}

/// The turn's pending spells. Drained by
/// [`spell_system`](crate::items::spell_system).
#[derive(Resource, Default)]
pub struct SpellQueue {
    /// Taken whole when the spell system runs; a cast pushed meanwhile waits
    /// for the next run.
    pub spells: Vec<WantsToCast>,
}

// ===========================================================================
// UI and input state (resources)
// ===========================================================================

/// The name the player typed at the start of the run, shown on the death screen.
#[derive(Resource, Default)]
pub struct PlayerName {
    /// The name the player typed.
    pub what: String,
}

/// Whether the viewport scrolls to keep the player centred (`-centered`).
#[derive(Resource, Default)]
pub struct RenderConfig {
    /// `true` when the run started with `-centered`.
    pub centered: bool,
}

// The pack screen's own state — `ItemAction`, `PackMode` and `PackIsOpen` —
// lives in [`crate::pack`], next to the row filtering that decides what each of
// its ten modes shows.

/// Whether the `Z` spells menu is open, and which slot the cursor sits on (one
/// per [`SPELLSET_CAP`](crate::constants::spells::SPELLSET_CAP)). Picking a row — by its letter, or by navigating and confirming
/// — opens the aiming reticle on that spell exactly the way the pack's `Use`
/// row does on an item. This menu is the only way to an active spell; no key
/// fires a slot directly.
#[derive(Resource, Default)]
pub struct SpellsMenu {
    /// Whether the menu is up and taking the keyboard.
    pub open: bool,
    /// The slot the cursor sits on.
    pub selected: usize,
}

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
    /// Whether the prompt is up.
    pub open: bool,
}

/// The aiming reticle: which item is being aimed, whether this is a throw or a
/// zap, and where the cursor is.
#[derive(Resource, Default)]
pub struct TargetingState {
    /// Whether the reticle is up.
    pub active: bool,
    /// The item being thrown or zapped. `None` for a spell, a look, or a reach
    /// strike.
    pub item: Option<Entity>,
    /// The reticle is aiming a throw rather than a zap: the range is
    /// [`crate::items::THROW_RANGE`] instead of the item's own, and confirming
    /// hurls the item instead of using it.
    pub throwing: bool,
    /// The reticle is triggering an active spell rather than an item —
    /// `item` is `None` whenever this is `Some`. Confirming queues a
    /// [`WantsToCast`] instead of a [`WantsToUse`].
    pub spell_effect: Option<SpellEffect>,
    /// The reticle is a plain look: nothing is queued and no turn is spent —
    /// confirming only logs what's on the aimed tile. `item` and `spell_effect`
    /// are both `None` whenever this is set, and unlike every other reticle
    /// purpose it may be confirmed on the player's own tile.
    pub looking: bool,
    /// The reticle is a reach weapon's strike (a bardiche, a whip) rather than
    /// a thrown or zapped item: its range is the wielded weapon's own
    /// [`Reach`], and confirming resolves the strike in place — no item ever
    /// leaves the wielder's hand. See `crate::combat::resolve_reach_attack`.
    pub reach_attack: bool,
    /// The cursor's column, as a map tile coordinate.
    pub cursor_x: i16,
    /// The cursor's row, as a map tile coordinate.
    pub cursor_y: i16,
}

// ===========================================================================
// Run state (resources)
// ===========================================================================

/// Why a log line is painted the way it is — decided once, by whoever writes
/// the message, and carried on [`LogEntry`] from then on. The alternative
/// (guessing a line's category back out of its rendered English text — see
/// `hud::log_paint`'s doc comment for why that used to be the design) breaks
/// the instant a translation stops sharing English's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogCategory {
    /// No special paint: the default colour.
    #[default]
    Plain,
    /// A trick shot's shout — the ordinary one and the ULTIMATE one alike.
    TrickShot,
    /// A combo kill's "With style.", and the ring of adornment's own line of
    /// the same six words — the same feat, so the same colour.
    Combo,
    /// A combo kill lucky enough to earn the flag instead — striped, not
    /// solid; see `hud::log_paint`.
    Pride,
    /// A curse taking hold or letting go.
    Curse,
    /// A dazzle landing on the player.
    Dazzle,
    /// The player crossing the low-HP threshold.
    Wounded,
    /// The player's own tempo shifting faster.
    Haste,
    /// The player's own tempo shifting slower.
    Slowed,
    /// The player throwing or firing something.
    Thrown,
    /// A bones ghost's own line — its arrival bark, or a combat line where it
    /// shares the current character's name and is treated as "you" in a
    /// different colour from the real you (see `crate::bones`).
    Ghost,
    /// A faerie shapeshifter's reveal, the dog's death (see
    /// `crate::combat`'s `reveal_faerie`). Pink.
    Faerie,
}

/// One line for the message log: its text, and the [`LogCategory`] it was
/// written with. Derefs to `str` and compares equal to one, so most existing
/// callers that only care about the text (a test's `.contains(...)`) never
/// have to know this wraps anything.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// The words.
    pub text: String,
    /// How it is painted.
    pub category: LogCategory,
}

impl LogEntry {
    /// A line with no special paint ([`LogCategory::Plain`]).
    pub fn plain<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            category: LogCategory::Plain,
        }
    }

    /// A line painted as `category`.
    ///
    /// ```
    /// use models::{LogCategory, LogEntry};
    ///
    /// let line = LogEntry::tagged("Bang!", LogCategory::TrickShot);
    /// assert!(line == *"Bang!");
    /// assert_eq!(line.category, LogCategory::TrickShot);
    /// ```
    pub fn tagged<S: Into<String>>(text: S, category: LogCategory) -> Self {
        Self {
            text: text.into(),
            category,
        }
    }
}

impl std::ops::Deref for LogEntry {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for LogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl PartialEq<str> for LogEntry {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

/// The message log: everything that has happened (`history`, capped at [`LOG_HISTORY_CAP`]) and
/// everything the player has not yet acknowledged with `--MORE--` (`unread`).
/// `history` is plain text — nothing ever colours the scrollback — while
/// `unread` is what's actually painted, so it keeps each line's [`LogCategory`].
#[derive(Resource)]
pub struct GameLog {
    /// Everything that has happened, as plain text.
    pub history: Vec<String>,
    /// What the player has not yet acknowledged, with each line's category.
    pub unread: Vec<LogEntry>, // The queue of messages waiting for a --MORE-- acknowledgment
}

impl Default for GameLog {
    fn default() -> Self {
        let mut unread = vec![LogEntry::plain(strings::welcome_new_run())];
        // Only a binary whose translation isn't finished has one of these —
        // see `strings::beta_notice`'s own doc comment.
        if let Some(notice) = strings::beta_notice() {
            unread.push(LogEntry::plain(notice));
        }
        Self {
            history: Vec::new(),
            unread,
        }
    }
}

impl GameLog {
    /// A plain (white) log line — everything that isn't one of the handful of
    /// categories [`GameLog::add_colored`] exists for.
    pub fn add<S: Into<String>>(&mut self, message: S) {
        self.add_colored(message, LogCategory::Plain);
    }

    /// A log line tagged with the category that decides how it's painted —
    /// set here, at the message's origin, never guessed at later from its text.
    pub fn add_colored<S: Into<String>>(&mut self, message: S, category: LogCategory) {
        let msg = message.into();
        self.history.push(msg.clone());
        self.unread.push(LogEntry::tagged(msg, category));

        if self.history.len() > LOG_HISTORY_CAP {
            self.history.remove(0);
        }
    }
}

/// The current dungeon floor, 1-based.
#[derive(Resource)]
pub struct Depth {
    /// The floor number, counted down from the top.
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
    /// Staircases, portals and trapdoors taken so far this run.
    pub count: u32,
}

/// Tracks how long the player has lingered on one dungeon level. Every turn adds
/// one; every level change resets it to zero. When it reaches
/// [`crate::map::DUNGEON_LORD_PATIENCE`] the Dungeon Lord opens a portal under
/// the player's feet and shunts them to the next level. Transient, never saved.
#[derive(Resource, Default)]
pub struct DungeonLord {
    /// Turns spent on this floor since arriving.
    pub idle_turns: u32,
}
