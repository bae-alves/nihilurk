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
use models::{ChaCha12Rng, SeedableRng};

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
    let mut player_name = "Roog".to_string();
    let mut positional: Option<String> = None;
    let mut iter = args.iter();
    iter.next(); // skip the executable path
    while let Some(arg) = iter.next() {
        if arg == "-s" {
            if let Some(seed_str) = iter.next() {
                seed = seed_str.parse::<u64>().ok();
            }
        } else if arg == "-c" {
            centered_mode = true;
        } else {
            positional = Some(arg.clone());
        }
    }

    // A positional argument is a save file to load if it names an existing file
    // (either verbatim or with a `.save.json` suffix); otherwise it is the
    // player's name for a fresh game.
    let mut load_path: Option<String> = None;
    if let Some(arg) = positional {
        let suffixed = format!("{arg}.save.json");
        if std::path::Path::new(&arg).is_file() {
            load_path = Some(arg);
        } else if std::path::Path::new(&suffixed).is_file() {
            load_path = Some(suffixed);
        } else {
            player_name = arg;
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

    let guard = TerminalGuard::new()?;
    let mut stdout = stdout();
    let mut world = World::new();

    // 2. Initialize Seeded GameRng
    let seed_value = seed.unwrap_or_else(rand::random);
    world.insert_resource(models::GameRng(ChaCha12Rng::seed_from_u64(seed_value)));
    world.insert_resource(models::RngSeed(seed_value));
    world.insert_resource(PackIsOpen {open: false, selected: 0 as usize, action_mode: None, action_selected: 0});
    world.insert_resource(RenderConfig { centered: centered_mode });
    world.insert_resource(TargetingState {active: false, item: None, cursor_x: 0, cursor_y: 0});
    world.insert_resource(LastInventoryRect {rect: None});
    world.insert_resource(PlayerName { what: player_name.to_ascii_uppercase()});
    world.init_resource::<AttackQueue>();
    world.init_resource::<UseQueue>();
    world.init_resource::<GameLog>();

    if let Some(path) = &load_path {
        models::load_game(&mut world, path)?;
        world.resource_mut::<GameLog>().add(format!("Loaded save '{path}'."));
    } else {
        models::initialize_world(&mut world);
    }
    
    // 3. Create the schedule and register systems in execution order
    let mut schedule = Schedule::default();
    schedule.add_systems((
        ai,
        item_system.after(ai),
        combat_system.after(item_system),
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
    // Save the game on exit, then restore the terminal so the message is visible.
    let save_name = format!(
        "{}.save.json",
        world.resource::<PlayerName>().what.to_ascii_lowercase()
    );
    let save_result = models::save_game(&mut world, &save_name);
    drop(guard);
    match save_result {
        Ok(()) => println!("Game saved to '{save_name}'. Resume with: roog {save_name}"),
        Err(e) => eprintln!("Failed to save game: {e}"),
    }

    Ok(())
}