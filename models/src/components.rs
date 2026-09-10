use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

/// The most a single pack slot will hold before the overflow spills into a
/// second slot. Defined and documented in `constants.rs`; re-exported here so
/// `components::STACK_LIMIT` (and the crate-wide glob) keep resolving.
pub use crate::constants::items::STACK_LIMIT;

#[derive(Component)]
pub struct Name {
    pub what: String,
}

impl Name {
    /// The indefinite article that reads correctly before this name:
    /// `"an"` before a vowel sound, `"a"` otherwise. Good enough for the
    /// bestiary and item list (no "an hour" / "a unicorn" edge cases here).
    pub fn article(&self) -> &'static str {
        match self.what.chars().next() {
            Some(c) if matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u') => "an",
            _ => "a",
        }
    }
}

#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

#[derive(Component)]
pub struct Renderable {
    pub glyph: char,
    pub color: Color,
}

#[derive(Component)]
pub struct Viewshed {
    /// Transient: recomputed every frame by the visibility system, never saved.
    pub visible_tiles: Vec<(u16, u16)>,
    /// Fog-of-war memory, one bit per map tile (see [`crate::map::tile_index`]).
    pub revealed_tiles: FixedBitSet,
    pub range: u16,
    pub dirty: bool,
}

#[derive(Component)]
pub struct Backpack {
    pub items: Vec<Entity>,
}

#[derive(Component)]
pub struct Score {
    pub value: i32,
}

#[derive(Component)]
pub struct Mob {
    pub movement_type: MovementType,
}

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

/// Not currently drawn or announced: an out-of-view monster, an undiscovered
/// trap, or an invisible thing the player can't perceive. The visibility system
/// owns this for monsters and the invisible item; traps clear it when revealed.
#[derive(Component)]
pub struct Hidden;

/// Intrinsically unseeable without [`crate::effects::SeesInvisible`] — the
/// phantom, and the one-in-ten "invisible" floor item. Pairs with [`Hidden`]:
/// `Invisible` says *why* a thing can't be seen, `Hidden` is the per-turn "can't
/// be seen right now" the renderer reads.
#[derive(Component)]
pub struct Invisible;

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

/// Marker for the Element of Yoord — the relic each run must carry up from the
/// depths. While the player's pack holds an entity with this component the
/// staircases invert: the up-stair works and the down-stair is dead.
#[derive(Component)]
pub struct Amulet;

/// Present while an entity is currently inside the player's viewshed. Added the
/// turn it first enters view (logging "you spotted ..."), removed the turn it
/// leaves, so re-entering view spots it again. Transient, never serialised.
#[derive(Component)]
pub struct Spotted;

/// Creatures that bleed. When an entity carrying this takes damage, the tile it
/// is standing on is recorded in [`crate::map::BloodStains`] and rendered with a
/// red background while it stays in the player's view.
#[derive(Component)]
pub struct Blood;

#[derive(Resource, Default)]
pub struct PlayerName {
    pub what: String,
}

#[derive(Resource, Default)]
pub struct RenderConfig {
    pub centered: bool,
}

#[derive(Event, Clone, Copy)]
pub struct WantsToAttack {
    pub attacker: Entity,
    pub target: Entity,
}

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

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Faction {
    Player,
    Monster,
    Ally,
}

/// What the pack screen can do with the item under the cursor. The list itself
/// lives in [`ActionMenu`]; nothing else in the game enumerates these.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemAction {
    /// Quaff / read / zap / wear it, depending on what it is.
    Use,
    /// Put it down on the tile you're standing on.
    Drop,
    /// Hurl it at a spot you pick with the aiming reticle.
    Throw,
}

impl ItemAction {
    /// The label the pack screen paints, padded to the modal's inner width.
    pub fn label(self) -> &'static str {
        match self {
            ItemAction::Use => " Use    ",
            ItemAction::Drop => " Drop   ",
            ItemAction::Throw => " Throw  ",
        }
    }
}

/// The order the three item actions are offered in. Use always leads; `-dropthrow`
/// swaps the other two, for players who reach for Drop far more often than Throw.
#[derive(Resource, Default)]
pub struct ActionMenu {
    pub drop_first: bool,
}

impl ActionMenu {
    pub fn actions(&self) -> [ItemAction; 3] {
        if self.drop_first {
            [ItemAction::Use, ItemAction::Drop, ItemAction::Throw]
        } else {
            [ItemAction::Use, ItemAction::Throw, ItemAction::Drop]
        }
    }

    /// The action sitting at menu row `idx`.
    pub fn at(&self, idx: usize) -> ItemAction {
        self.actions()[idx.min(2)]
    }
}

#[derive(Resource, Default)]
pub struct PackIsOpen {
    pub open: bool,
    pub selected: usize,
    pub action_mode: Option<usize>,
    pub action_selected: usize,
}

#[derive(Resource, Default)]
pub struct AttackQueue {
    pub attacks: Vec<WantsToAttack>,
}

#[derive(Resource, Default)]
pub struct UseQueue {
    pub uses: Vec<WantsToUse>,
}

#[derive(Resource, Default)]
pub struct ThrowQueue {
    pub throws: Vec<WantsToThrow>,
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

#[derive(Component)]
pub struct Ranged {
    pub range: i32,
}

/// Tag for a cursed piece of equipment. Rolled on at spawn for the majority of
/// weapon/armour/ring drops (see [`crate::items::enchant_equipment`]). Once a
/// cursed item is equipped it can't be taken off again until the curse is lifted
/// by a scroll of remove curse.
#[derive(Component)]
pub struct Curse;

/// A weapon that has been vorpalized (scroll of vorpalize weapon). Any hit from
/// it that draws blood slays a creature named `bane` outright — as it does any
/// creature carrying [`crate::effects::VorpalTarget`], regardless of `bane`. See
/// [`crate::combat::resolve_attack`].
#[derive(Component)]
pub struct Vorpal {
    pub bane: String,
}

/// A ring's type tag, the twin of [`Potion`] / [`Scroll`] / [`Wand`]. What the
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

#[derive(Component)]
pub struct Scroll {
    pub effect: ScrollEffect,
}

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

#[derive(Resource)]
pub struct GameLog {
    pub history: Vec<String>,
    pub unread: Vec<String>, // The queue of messages waiting for a --MORE-- acknowledgment
}

#[derive(Resource)]
pub struct Depth {
    pub what: u8,
}

/// Tracks how long the player has lingered on one dungeon level. Every turn adds
/// one; every level change resets it to zero. When it reaches
/// [`crate::map::DUNGEON_LORD_PATIENCE`] the Dungeon Lord opens a portal under
/// the player's feet and shunts them to the next level. Transient, never saved.
#[derive(Resource, Default)]
pub struct DungeonLord {
    pub idle_turns: u32,
}

impl Default for GameLog {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            unread: vec!["Welcome to ROOG! Use arrow keys to move.".to_string()],
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

#[derive(Component)]
pub struct Value {
    pub amount: i32,
}

/// Marker for anything that can sit on the floor and be picked up. The display
/// name lives on the [`Name`] component, same as monsters.
#[derive(Component)]
pub struct Item;

#[derive(Component)]
pub struct Consume;

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

/// How many identical items share one pack slot. Only ammunition stacks: a
/// quiver of arrows is one entity carrying a number, not thirty entities
/// crowding thirty inventory letters. Throwing spends one; picking more up tops
/// the stack back up to at most [`STACK_LIMIT`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stack {
    pub count: u8,
}

#[derive(Component)]
pub struct Battery {
    pub charges: i8,
}

/// A transient affliction on the **player** (a monster is confused through
/// [`MovementType::Confused`] instead). Half of every walk or swing while it
/// lasts goes off in a random direction ("You stumble foolishly"), and fast
/// movement, auto-explore and auto-fight all refuse to run. It is treacherous:
/// it does not wear off with time — only using a staircase or being caught by a
/// wand of cancellation clears it. Shown in the HUD as `CONF`.
#[derive(Component)]
pub struct Confused;

#[derive(Component)]
pub struct Potion {
    pub effect: PotionEffect,
}

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

#[derive(Component)]
pub struct Wand {
    pub effect: WandEffect,
}

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

/// The player's pool of magic points. Shown in the HUD as `Ma points/max_points`
/// alongside `HP`. Every run starts with a full pool (see
/// [`crate::initialize_world`]).
#[derive(Component, Clone, Copy, Serialize, Deserialize)]
pub struct Magic {
    pub points: u8,
    pub max_points: u8,
}
