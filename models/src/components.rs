use bevy_ecs::prelude::*;
use crossterm::style::Color;
use std::collections::HashSet;

#[derive(Component)]
pub struct Name {
    pub what: String
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Passage;

#[derive(Component)]
pub struct Room;

#[derive(Component)]
pub struct Door;

#[derive(Component, Clone, Copy)]
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
    pub visible_tiles: Vec<(u16, u16)>,
    pub revealed_tiles: HashSet<(u16, u16)>,
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
    pub armor: i32,
    pub power: i32,
}

#[derive(Component)]
pub struct Hidden;

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
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
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

#[derive(Resource)]
pub struct GameLog {
    pub history: Vec<String>,
    pub unread: Vec<String>, // The queue of messages waiting for a --MORE-- acknowledgment
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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