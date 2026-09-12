mod update;
mod view;
use models::*;

use bevy_ecs::{
    prelude::{With, World},
    schedule::IntoSystemConfigs,
    schedule::Schedule,
};
use crossterm::{
    cursor::{Hide, Show},
    event::{Event, KeyCode, KeyEventKind, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{BufWriter, stdout};

// Import our rng seed types
use models::{ChaCha12Rng, SeedableRng};

use crate::ai::ai;
use crate::visibility::visibility_system;
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
        if entry
            .file_name()
            .to_str()
            .is_some_and(|f| f.eq_ignore_ascii_case(file_name))
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

/// The victory sequence: show the starfield. Unlike death, the save is *not*
/// destroyed — the caller keeps it as "clear data" (see [`models::clear_data`]).
fn run_victory_screens<W: std::io::Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut view::Screen,
) -> std::io::Result<()> {
    let offset = view::centering_offset(world);
    let (name, score) = {
        let mut q = world.query_filtered::<&Score, With<Player>>();
        let score = q.iter(world).next().map(|s| s.value).unwrap_or(0);
        (world.resource::<PlayerName>().what.clone(), score)
    };

    view::render_victory(stdout, screen, offset, &name, score)?;
    wait_for_key(|_| true)?;
    Ok(())
}

/// Prints every name the content tables know, grouped by category. Reads the
/// tables themselves, so a row added today shows up here today.
fn print_content() {
    let names = models::content_names();
    println!("roog content — {} entries", names.len());
    println!("Spawn any of them with: ROOG_SPAWN=\"<name>,<name>\" roog");

    let mut group = "";
    for (category, name) in &names {
        if *category != group {
            group = category;
            let count = names.iter().filter(|(c, _)| c == category).count();
            println!("\n{group} ({count})");
        }
        println!("  {name}");
    }
}

/// One player-side step of the main loop: an auto-explore tick, a travel-cursor
/// tick, or a blocking read of the player's own move. Returns whether a turn was
/// spent (and monsters should therefore act).
fn player_step(world: &mut World) -> std::io::Result<bool> {
    if world.resource::<AutoExplore>().active {
        return update::auto_explore_step(world);
    }
    if world.resource::<TravelCursor>().active {
        update::travel_cursor_step(world)?;
        return Ok(false);
    }
    update::process_input_and_update(world)
}

fn main() -> std::io::Result<()> {
    // 1. Argument Parsing for Seed
    let args: Vec<String> = std::env::args().collect();
    let mut seed: Option<u64> = None;
    let mut centered_mode = false;
    let mut no_save = false;
    let mut no_blood = false;
    let mut no_shake = false;
    let mut list_content = false;
    let mut player_name = "Roog".to_string();
    let mut positional: Option<String> = None;
    // Multiplier on every animation frame's on-screen hold time (particles,
    // magic mapping's reveal wipe): the escape hatch for a terminal whose
    // redraw can't keep up with the default pacing, or that renders too
    // slowly for a fast one. `1.0` is the default pacing; clamped so a typo'd
    // value can't freeze the loop or blur every animation into nothing.
    let mut anim_rate: f32 = 1.0;
    let mut iter = args.iter();
    iter.next(); // skip the executable path
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-s" => {
                if let Some(seed_str) = iter.next() {
                    seed = seed_str.parse::<u64>().ok();
                }
            }
            "-c" => centered_mode = true,
            "-ns" => no_save = true,
            "-nb" => no_blood = true,
            "-nshake" => no_shake = true,
            "-content" => list_content = true,
            "-anim-rate" => {
                if let Some(rate_str) = iter.next() {
                    if let Ok(rate) = rate_str.parse::<f32>() {
                        anim_rate = rate.clamp(0.1, 5.0);
                    }
                }
            }
            _ => positional = Some(arg.clone()),
        }
    }

    // `-content` is the content author's index: every name the tables know, which
    // is exactly the set `ROOG_SPAWN` and `models::spawn_named` answer to. Prints
    // and exits without ever touching the terminal's alternate screen, so it
    // pipes into `grep` and `less` like any other listing.
    if list_content {
        print_content();
        return Ok(());
    }

    // A positional argument is a save file to load if it names an existing file
    // (either verbatim or with a `.sav` suffix, matched case-insensitively);
    // otherwise it is the player's name for a fresh game.
    let mut load_path: Option<String> = None;
    if let Some(arg) = positional {
        let suffixed = format!("{arg}.sav");
        match find_case_insensitive(&arg).or_else(|| find_case_insensitive(&suffixed)) {
            Some(found) => load_path = Some(found),
            None => player_name = arg,
        }
    }

    // Yoko Taro-style clear data: a won save is kept, not deleted. Recognise it
    // here — before the alternate screen — and make the player consent to
    // spending it before a new journey overwrites it. Default is No.
    if let Some(path) = &load_path {
        if let Some(clear) = models::clear_data(path)? {
            println!(
                "{} has ascended with the Element of Yoord and brought happiness back to the world. \
If you start another journey, the Element will also return to the Dungeon Lord. Do it? ([Y]es/[N]o)",
                clear.player_name
            );
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                // Default No — the clear data is left untouched.
                println!("The world keeps its light. Farewell.");
                return Ok(());
            }
            // Yes: begin anew under the winner's name. Drop the load so a fresh
            // world is built; its exit-save overwrites the clear data, and the
            // Element goes back to the Dungeon Lord.
            player_name = clear.player_name;
            load_path = None;
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = execute!(std::io::stderr(), LeaveAlternateScreen, Show);
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
    world.init_resource::<PackIsOpen>();
    world.insert_resource(RenderConfig {
        centered: centered_mode,
    });
    world.insert_resource(TargetingState {
        active: false,
        item: None,
        throwing: false,
        cursor_x: 0,
        cursor_y: 0,
    });
    world.init_resource::<QuitPrompt>();
    world.insert_resource(PlayerName {
        what: player_name.to_ascii_uppercase(),
    });
    world.insert_resource(Depth { what: 1 });
    world.init_resource::<DungeonLord>();
    world.init_resource::<Ending>();
    world.init_resource::<AutoExplore>();
    world.init_resource::<AutoPickup>();
    world.init_resource::<FastMove>();
    world.init_resource::<TravelCursor>();
    world.init_resource::<MagicMapReveal>();
    world.init_resource::<AttackQueue>();
    world.init_resource::<UseQueue>();
    world.init_resource::<ThrowQueue>();
    world.init_resource::<PlayerTempo>();
    world.init_resource::<GameLog>();
    world.insert_resource(models::Particles::new());
    world.insert_resource(models::Shake::new());
    world.insert_resource(models::AnimRate(anim_rate));

    match &load_path {
        Some(path) => {
            models::load_game(&mut world, path)?;
            // `load_game` already left a fresh (unloaded-game) GameLog behind;
            // swap its welcome line for the loaded-game version.
            world.insert_resource(GameLog {
                history: Vec::new(),
                unread: vec!["Welcome back to roog! Good luck and have fun!".to_string()],
            });
        }
        None => models::initialize_world(&mut world),
    }

    // `-nb`: disable bloodstains entirely for this run.
    if no_blood {
        world.resource_mut::<BloodStains>().enabled = false;
    }

    // `-nshake`: nail the map down. Nothing arms a shake for the rest of the run.
    if no_shake {
        world.resource_mut::<Shake>().enabled = false;
    }

    // 3. Create the schedule and register systems in execution order
    let mut schedule = Schedule::default();
    schedule.add_systems((
        smoke_system.before(snare_system),
        snare_system,
        passive_ability_system.after(snare_system),
        ai.after(passive_ability_system),
        trap_system.after(ai),
        throw_system.after(trap_system),
        item_system.after(throw_system),
        // Gear changed by anything other than the pack screen — a loaded save, a
        // curse-lifting scroll — has its lent effects reconciled here, before
        // combat and visibility read them.
        equipment_effects_system.after(item_system),
        combat_system.after(equipment_effects_system),
        reaper_system.after(combat_system),
        dungeon_lord_system.after(reaper_system),
        visibility_system.after(dungeon_lord_system),
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
        // A fast-move run resolves entirely inside `fast_move_run`; capture the
        // flag first so clearing it there doesn't also trigger a player step.
        let fast_moving = world.resource::<FastMove>().active;
        if fast_moving {
            update::fast_move_run(&mut world, &mut schedule)?;
        }
        if !fast_moving {
            let turn_taken = player_step(&mut world)?;

            // Step B: Only let monsters act if the player took a valid action
            if turn_taken {
                schedule.run(&mut world);
            }
        }

        // Step B2: Play any hit / beam / blast animation this turn queued. A
        // no-op unless a system asked for particles, so auto-explore and
        // fast-move (which never fight) pass straight through.
        view::play_particles(&mut world, &mut stdout, &mut screen)?;

        // Step B3: Sweep in a scroll of magic mapping, one row per frame. A
        // no-op unless a scroll was just read.
        view::play_magic_map(&mut world, &mut stdout, &mut screen)?;

        // Step C: Render the world to terminal
        view::render(&mut world, &mut stdout, &mut screen)?;

        // Step C1: Let any screen shake this turn armed finish rocking. Unlike
        // B2 and B3 this never blocks the player: it runs only while nothing is
        // waiting to be read, and a keypress settles the map and is handed
        // straight back to Step A unread. It also guarantees the map is home
        // before the loop blocks again, so a shake can never be left frozen
        // mid-lurch on screen.
        view::play_shake(&mut world, &mut stdout, &mut screen)?;

        // Step C2: Pace the auto-explore walk so it reads as movement rather
        // than a teleport, and stays interruptible.
        if world.resource::<AutoExplore>().active {
            std::thread::sleep(std::time::Duration::from_millis(35));
        }

        // Pace the turns a sleeping player auto-forfeits, so a lungful of gas
        // reads as time passing rather than a freeze. A bear trap is not paced
        // here — the player is still pressing keys.
        if models::player_incapacitated(&mut world) {
            std::thread::sleep(std::time::Duration::from_millis(90));
        }

        // Step D: The run may have just ended, in triumph or otherwise.
        if world.resource::<Ending>().player_dead || world.resource::<Ending>().player_won {
            break;
        }
    }

    if world.resource::<Ending>().player_won {
        // A win is sticky — even a monster's parting blow the same turn can't rob
        // a completed run. Show the starfield, then keep the save as clear data
        // (it serialises with `cleared: true`) rather than deleting it.
        run_victory_screens(&mut world, &mut stdout, &mut screen)?;
        if no_save {
            drop(guard);
            println!("Clear data not saved (-ns).");
            return Ok(());
        }
        let save_result = models::save_game(&mut world, &save_name);
        drop(guard);
        match save_result {
            Ok(()) => println!("Clear data saved to '{save_name}'."),
            Err(e) => eprintln!("Failed to save clear data: {e}"),
        }
        return Ok(());
    }

    if world.resource::<Ending>().player_dead {
        // Death: the save is gone and there is nothing to write. Show the
        // epitaph, then restore the terminal.
        run_death_screens(&mut world, &mut stdout, &mut screen, &save_name)?;
        drop(guard);
        return Ok(());
    }

    // Save the game on exit, then restore the terminal so the message is visible.
    if no_save {
        drop(guard);
        println!("Game not saved (-ns).");
        return Ok(());
    }
    let save_result = models::save_game(&mut world, &save_name);
    drop(guard);
    match save_result {
        Ok(()) => println!("Game saved to '{save_name}'. Resume with: roog {save_name}"),
        Err(e) => eprintln!("Failed to save game: {e}"),
    }

    Ok(())
}
