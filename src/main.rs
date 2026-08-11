mod model;
mod update;
mod view;
mod rect;
mod visibility;

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::stdout;
use bevy_ecs::{prelude::World, schedule::Schedule};

use crate::visibility::visibility_system;

/// RAII Guard that manages Crossterm terminal setup and cleanup.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn main() -> std::io::Result<()> {
    let _guard = TerminalGuard::new()?;
    let mut stdout = stdout();
    let mut world = World::new();
    model::initialize_world(&mut world);
    // 1. Create the schedule and register systems in execution order
    let mut schedule = Schedule::default();
    schedule.add_systems((
        // monster_ai_system,    // AI runs
        visibility_system,       // FOV recalculates AFTER movement, BEFORE render
    ));

    // 2. Main Loop
    while world.resource::<model::GameState>().is_running {
        // Step A: Capture keypresses / update intent
        update::process_input_and_update(&mut world)?;
        // Step B: Run all ECS systems (visibility, movement, combat)
        schedule.run(&mut world);
        // Step C: Render the world to terminal
        view::render(&mut world, &mut stdout)?;
    }

    Ok(())
}