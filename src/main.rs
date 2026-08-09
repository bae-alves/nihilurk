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
use bevy_ecs::prelude::World;

fn main() -> std::io::Result<()> {
    // --- TERMINAL SETUP ---
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;

    // --- GAME INITIALIZATION ---
    let mut world = World::default();
    model::initialize_world(&mut world);

    // --- MAIN LOOP ---
    while world.resource::<model::GameState>().is_running {
        view::render(&mut world, &mut stdout)?;
        update::process_input_and_update(&mut world)?;
    }

    // --- TERMINAL CLEANUP ---
    execute!(stdout, Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}