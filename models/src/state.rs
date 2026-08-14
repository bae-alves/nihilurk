use bevy_ecs::prelude::*;

#[derive(Resource)]
pub struct GameState {
    pub is_running: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self { is_running: true }
    }
}