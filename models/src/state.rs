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

/// Set the instant the run ends, one way or the other. The main loop watches
/// this: once the pending `--MORE--` messages are acknowledged it destroys the
/// save file and shows the tombstone (death) or the starfield (victory) before
/// the process exits.
#[derive(Resource, Default)]
pub struct Ending {
    pub player_dead: bool,
    /// Human-readable cause of death, e.g. "Slain by a kobold".
    pub cause: String,
    /// Set when the player carries the Element of Yoord up the final stair (from
    /// Depth 1). A portal can never set this — the last climb must be earned.
    pub player_won: bool,
}
