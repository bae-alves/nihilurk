mod update;
mod view;
use models::*;

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::stdout;
use bevy_ecs::{prelude::World, schedule::Schedule, schedule::IntoSystemConfigs};

// Import our rng seed types
use rand::{SeedableRng, rngs::StdRng};

use crate::visibility::visibility_system;
use crate::ai::ai;
use models::combat_system;

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
    // 1. Argument Parsing for Seed
    let args: Vec<String> = std::env::args().collect();
    let mut seed: Option<u64> = None;
    let mut centered_mode = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-s" {
            if let Some(seed_str) = iter.next() {
                seed = seed_str.parse::<u64>().ok();
            }
        } else if arg == "-c" {
            centered_mode = true;
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            Show
        );
        let _ = disable_raw_mode();
        original_hook(panic_info);
    }));

    let _guard = TerminalGuard::new()?;
    let mut stdout = stdout();
    let mut world = World::new();

    // 2. Initialize Seeded GameRng
    let rng = if let Some(s) = seed {
        StdRng::seed_from_u64(s)
    } else {
        StdRng::from_entropy()
    };
    world.insert_resource(models::GameRng(rng));
    world.insert_resource(PackIsOpen {open: false});
    world.insert_resource(RenderConfig { centered: centered_mode });
    world.init_resource::<AttackQueue>();
    world.init_resource::<GameLog>();
    models::initialize_world(&mut world);
    
    // 3. Create the schedule and register systems in execution order
    let mut schedule = Schedule::default();
    schedule.add_systems((
        ai,
        combat_system.after(ai),
        visibility_system.after(combat_system),
    ));

    // [!] KICKSTART THE ENGINE [!]
    // We must run the systems and render once before the loop, 
    // otherwise the screen will be completely black until the first keypress.
    schedule.run(&mut world);
    view::render(&mut world, &mut stdout)?;

    // 4. Main Loop
    while world.resource::<models::GameState>().is_running {
        
        // Step A: Wait for move (Thread pauses here at event::read)
        let turn_taken = update::process_input_and_update(&mut world)?;
        
        // Step B: Only let monsters act if the player took a valid action
        if turn_taken {
            schedule.run(&mut world);
        }

        // Step C: Render the world to terminal
        view::render(&mut world, &mut stdout)?;
    }

    Ok(())
}