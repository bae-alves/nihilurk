pub struct GameState {
    pub player_x: u16,
    pub player_y: u16,
    pub is_running: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player_x: 10,
            player_y: 10,
            is_running: true,
        }
    }
}