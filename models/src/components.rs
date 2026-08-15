use bevy_ecs::prelude::*;
use crossterm::style::Color;
use std::collections::HashSet;

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

#[derive(Event, Clone, Copy)]
pub struct WantsToAttack {
    pub attacker: Entity,
    pub target: Entity,
}

#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Faction {
    Player,
    Monster,
    Ally,
}

#[derive(Resource, Default)]
pub struct AttackQueue {
    pub attacks: Vec<WantsToAttack>,
}