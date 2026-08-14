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

#[derive(Component)]
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