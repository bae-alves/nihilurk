use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

#[derive(Component)]
pub struct Name {
    pub what: String
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

#[derive(Component, Clone, Copy, PartialEq, Eq)]
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
    Aggravated { tx: u16, ty: u16 },
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

#[derive(Component)]
pub struct Hidden;

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

/// The bundle of innate magical properties a monster is born with, gathered into
/// one component so a wand of cancellation can strip the lot in a single stroke.
/// Every [`crate::MonsterBundle`] carries one (all-`false` by default);
/// [`crate::spawn_monster`] fills in the flags a species needs.
#[derive(Component, Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Traits {
    /// A wand of fire cannot burn this creature (the dragon).
    pub fire_immune: bool,
    /// A wand of cold cannot freeze this creature (the yeti).
    pub cold_immune: bool,
    /// Undead: a wand of draining passes straight through, healing its wielder
    /// nothing (zombie, phantom, vampire, wraith).
    pub undead: bool,
    /// Every vorpal weapon slays this creature in one blow, whatever the weapon's
    /// rolled bane (the Jabberwock). See [`crate::combat::resolve_attack`].
    pub vorpal_target: bool,
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
    pub slot_idx: Option<usize>
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Faction {
    Player,
    Monster,
    Ally,
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

#[derive(Component)]
pub struct Wield {
    pub wielder: Option<Entity>,
    pub pow_increase: i8,
    pub pow_bonus: i8,
}

#[derive(Component)]
pub struct Wear {
    pub wearer: Option<Entity>,
    pub arm_increase: i8,
    pub arm_bonus: i8,
}

#[derive(Component)]
pub struct PutOn {
    pub bearer: Option<Entity>,
    pub effect: RingEffect,
}

/// Tag for a cursed piece of equipment. Rolled on at spawn for the majority of
/// weapon/armour/ring drops (see [`crate::items::enchant_equipment`]). Once a
/// cursed item is equipped it can't be taken off again until the curse is lifted
/// by a scroll of remove curse.
#[derive(Component)]
pub struct Curse;

/// A weapon that has been vorpalized (scroll of vorpalize weapon). Any hit from
/// it that draws blood slays a creature named `bane` outright — as it does any
/// creature whose [`Traits::vorpal_target`] is set, regardless of `bane`. See
/// [`crate::combat::resolve_attack`].
#[derive(Component)]
pub struct Vorpal {
    pub bane: String,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum RingEffect {
    Protection,
    AddStrength,
    SustainStrength,
    Searching,
    SeeInvisible,
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
    pub cursor_x: i16,
    pub cursor_y: i16,
}

#[derive(Resource)]
pub struct GameLog {
    pub history: Vec<String>,
    pub unread: Vec<String>, // The queue of messages waiting for a --MORE-- acknowledgment
}

#[derive(Resource)]
pub struct Depth{
    pub what: u8
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

#[derive(Component)]
pub struct Battery {
    pub charges: i8,
}

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