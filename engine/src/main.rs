mod update;
mod view;
use models::*;

use crossterm::{
    cursor::{Hide, Show},
    event::{read, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::{stdout, BufWriter};
use bevy_ecs::{prelude::{With, World}, schedule::Schedule, schedule::IntoSystemConfigs};

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

/// Finds a file in the current directory whose name matches `name`
/// case-insensitively, returning its actual on-disk name. This makes save
/// files loadable regardless of the case typed on the command line, since
/// the filesystem itself may be case-sensitive (Linux/macOS).
fn find_case_insensitive(name: &str) -> Option<String> {
    if std::path::Path::new(name).is_file() {
        return Some(name.to_string());
    }
    let dir = std::path::Path::new(name)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let file_name = std::path::Path::new(name).file_name()?.to_str()?;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        if entry.file_name().to_str().is_some_and(|f| f.eq_ignore_ascii_case(file_name))
            && entry.path().is_file()
        {
            return entry.path().to_str().map(|s| s.to_string());
        }
    }
    None
}

/// Blocks until the player presses a key (any key, or a specific one). Ignores
/// key-release events so a single physical press doesn't skip two screens.
fn wait_for_key(accept: impl Fn(KeyCode) -> bool) -> std::io::Result<()> {
    loop {
        if let Event::Key(key) = read()? {
            if key.kind == KeyEventKind::Press && accept(key.code) {
                return Ok(());
            }
        }
    }
}

/// The death sequence: destroy the save, show the "You die..." `--MORE--` panel,
/// then the tombstone. Runs while the terminal guard is still active.
fn run_death_screens<W: std::io::Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut view::Screen,
    save_name: &str,
) -> std::io::Result<()> {
    // The save is destroyed before the player is even prompted.
    let _ = std::fs::remove_file(save_name);

    let offset = view::centering_offset(world);
    let (name, cause, score) = {
        let mut q = world.query_filtered::<(&Score,), With<Player>>();
        let score = q.iter(world).next().map(|(s,)| s.value).unwrap_or(0);
        (
            world.resource::<PlayerName>().what.clone(),
            world.resource::<Ending>().cause.clone(),
            score,
        )
    };

    view::render_you_died(stdout, screen, offset)?;
    wait_for_key(|c| matches!(c, KeyCode::Char(' ') | KeyCode::Enter))?;

    view::render_tombstone(stdout, screen, offset, &name, &cause, score)?;
    wait_for_key(|_| true)?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    // 1. Argument Parsing for Seed
    let args: Vec<String> = std::env::args().collect();
    let mut seed: Option<u64> = None;
    let mut centered_mode = false;
    let mut no_save = false;
    let mut no_blood = false;
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
        } else if arg == "-ns" {
            no_save = true;
        } else if arg == "-nb" {
            no_blood = true;
        } else {
            positional = Some(arg.clone());
        }
    }

    // A positional argument is a save file to load if it names an existing file
    // (either verbatim or with a `.sav` suffix, matched case-insensitively);
    // otherwise it is the player's name for a fresh game.
    let mut load_path: Option<String> = None;
    if let Some(arg) = positional {
        let suffixed = format!("{arg}.sav");
        if let Some(found) = find_case_insensitive(&arg).or_else(|| find_case_insensitive(&suffixed)) {
            load_path = Some(found);
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
    // Buffer all render output so a frame is one write(2), not thousands.
    // render() flushes at the end of each frame.
    let mut stdout = BufWriter::with_capacity(32 * 1024, stdout());
    let mut screen = view::Screen::new();
    let mut world = World::new();

    // 2. Initialize Seeded GameRng
    let seed_value = seed.unwrap_or_else(rand::random);
    world.insert_resource(models::GameRng(ChaCha12Rng::seed_from_u64(seed_value)));
    world.insert_resource(models::RngSeed(seed_value));
    world.insert_resource(PackIsOpen {open: false, selected: 0 as usize, action_mode: None, action_selected: 0});
    world.insert_resource(RenderConfig { centered: centered_mode });
    world.insert_resource(TargetingState {active: false, item: None, cursor_x: 0, cursor_y: 0});
    world.insert_resource(PlayerName { what: player_name.to_ascii_uppercase()});
    world.insert_resource(Depth {what: 1 as u8});
    world.init_resource::<Ending>();
    world.init_resource::<AutoExplore>();
    world.init_resource::<FastMove>();
    world.init_resource::<TravelCursor>();
    world.init_resource::<AttackQueue>();
    world.init_resource::<UseQueue>();
    world.init_resource::<GameLog>();

    if let Some(path) = &load_path {
        models::load_game(&mut world, path)?;
        world.resource_mut::<GameLog>().add(format!("Loaded save '{path}'."));
    } else {
        models::initialize_world(&mut world);
    }

    // `-nb`: disable bloodstains entirely for this run.
    if no_blood {
        world.resource_mut::<BloodStains>().enabled = false;
    }

    // 3. Create the schedule and register systems in execution order
    let mut schedule = Schedule::default();
    schedule.add_systems((
        ai,
        item_system.after(ai),
        combat_system.after(item_system),
        reaper_system.after(combat_system),
        visibility_system.after(reaper_system),
    ));

    // [!] KICKSTART THE ENGINE [!]
    // We must run the systems and render once before the loop, 
    // otherwise the screen will be completely black until the first keypress.
    schedule.run(&mut world);
    view::render(&mut world, &mut stdout, &mut screen)?;

    let save_name = format!(
        "{}.sav",
        world.resource::<PlayerName>().what.to_ascii_lowercase()
    );

    // 4. Main Loop
    while world.resource::<models::GameState>().is_running {

        // Step A: Advance the game. A fast-move run resolves entirely here,
        // taking its own turns without repainting; otherwise we take one
        // auto-explore step, or block at event::read for the player's move.
        if world.resource::<FastMove>().active {
            update::fast_move_run(&mut world, &mut schedule)?;
        } else {
            let turn_taken = if world.resource::<AutoExplore>().active {
                update::auto_explore_step(&mut world)?
            } else if world.resource::<TravelCursor>().active {
                update::travel_cursor_step(&mut world)?;
                false
            } else {
                update::process_input_and_update(&mut world)?
            };

            // Step B: Only let monsters act if the player took a valid action
            if turn_taken {
                schedule.run(&mut world);
            }
        }

        // Step C: Render the world to terminal
        view::render(&mut world, &mut stdout, &mut screen)?;

        // Step C2: Pace the auto-explore walk so it reads as movement rather
        // than a teleport, and stays interruptible.
        if world.resource::<AutoExplore>().active {
            std::thread::sleep(std::time::Duration::from_millis(35));
        }

        // Step D: The player may have just been killed.
        if world.resource::<Ending>().player_dead {
            break;
        }
    }

    if world.resource::<Ending>().player_dead {
        // Death overrides everything: the save is gone and there is nothing to
        // write. Show the epitaph, then restore the terminal.
        run_death_screens(&mut world, &mut stdout, &mut screen, &save_name)?;
        drop(guard);
        return Ok(());
    }

    // Save the game on exit, then restore the terminal so the message is visible.
    if no_save {
        drop(guard);
        println!("Game not saved (-ns).");
    } else {
        let save_result = models::save_game(&mut world, &save_name);
        drop(guard);
        match save_result {
            Ok(()) => println!("Game saved to '{save_name}'. Resume with: roog {save_name}"),
            Err(e) => eprintln!("Failed to save game: {e}"),
        }
    }

    Ok(())
}