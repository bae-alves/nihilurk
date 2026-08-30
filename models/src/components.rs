use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

#[derive(Component)]
pub struct Name {
    pub what: String
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
}

#[derive(Component)]
pub struct Fighter {
    pub hp: i32,
    pub max_hp: i32,
    /// Defence die size: the opposed armour roll is `1d[armor] + armor_bonus`.
    pub armor: i32,
    /// Attack die size: weapon damage rolls `1d[power] + power_bonus`.
    pub power: i32,
    /// Flat modifier added once to the armour roll (may be negative).
    pub armor_bonus: i32,
    /// Flat modifier added once to the damage roll (may be negative).
    pub power_bonus: i32,
}

#[derive(Component)]
pub struct Hidden;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Component)]
pub struct Item {
    pub name: String,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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