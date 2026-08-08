mod model;
mod update;
mod view;

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::stdout;

use model::GameState;

fn main() -> std::io::Result<()> {
    // --- TERMINAL SETUP ---
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;

    // --- GAME INITIALIZATION ---
    let mut state = GameState::new();

    // --- MAIN LOOP ---
    while state.is_running {
        // 1. View
        view::render(&state, &mut stdout)?;

        // 2. Input & Update (Merged)
        update::process_input_and_update(&mut state)?;
    }

    // --- TERMINAL CLEANUP ---
    execute!(stdout, Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}