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

/// Set the instant the player is killed. The main loop watches this: once the
/// pending `--MORE--` messages are acknowledged it destroys the save file and
/// shows the tombstone screen before the process exits.
#[derive(Resource, Default)]
pub struct Ending {
    pub player_dead: bool,
    /// Human-readable cause of death, e.g. "Slain by a kobold".
    pub cause: String,
}