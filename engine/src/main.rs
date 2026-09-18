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
    style::{Color as CrosstermColor, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{BufWriter, stdout};

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

/// Prints the short command-line guide without entering the alternate screen.
/// The full reference lives in the installed `roog(6)` manual.
fn print_help() {
    println!(
        "\
roog - terminal roguelike

USAGE
    roog [OPTIONS] [NAME|SAVE]

OPTIONS
    -s SEED          use a reproducible u64 seed
    -c               centre the map on the player
    -ns              do not write a save file
    -nb              disable blood and corpse animation
    -nshake          disable screen shake
    -anim-rate N     set animation pacing multiplier (0.1..=5.0)
    -content         list names accepted by ROOG_SPAWN
    -h, -help, --help show this help and exit

POSITIONAL ARGUMENT
    NAME             start a new run with this player name
    SAVE             load an existing save, with or without .sav

ENVIRONMENT
    ROOG_SPAWN       comma-separated names to place near the player on every
                     generated floor; use -content to list valid names

EXAMPLES
    roog
    roog bae
    roog -s 1234 -ns
    ROOG_SPAWN=\"dragon,ring of protection\" roog

SEE ALSO
    man roog          full command, environment, and spawn API reference
    docs/reference/cli-and-env.md
    docs/reference/spawn-api.md
"
    );
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
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "-h" | "-help" | "--help"))
    {
        print_help();
        return Ok(());
    }
    let mut seed: Option<u64> = None;
    let mut centered_mode = false;
    let mut no_save = false;
    let mut no_blood = false;
    let mut no_shake = false;
    let mut list_content = false;
    let mut pride_off = false;
    // The flag this run flies: the stripes the scorekeeper's DOUBLE and COMBO!
    // and the log's proudest line are painted in. `-pride <name>`; an
    // unrecognised name says so and flies the rainbow anyway.
    let mut pride = models::pride::PrideFlag::default_flag();
    let mut unknown_flag: Option<String> = None;
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
            "-pride" => {
                if let Some(name) = iter.next() {
                    match models::pride::PrideFlag::named(name) {
                        Some(flag) => pride = flag,
                        None => unknown_flag = Some(name.clone()),
                    }
                }
            }
            "-prideoff" => pride_off = true,
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

    // `-prideoff` is documented as swapping the stripes for plain red. It does
    // not do that. See `models::pride::PRIDE_OFF_REFUSAL` — this is the whole
    // implementation, and the terminal is never even set up for it.
    if pride_off {
        execute!(
            stdout(),
            SetForegroundColor(CrosstermColor::Red),
            Print(models::pride::PRIDE_OFF_REFUSAL),
            Print("\n"),
            ResetColor
        )?;
        return Ok(());
    }

    // A `-pride` nobody has a row for: say so, name the ones that exist, and
    // fly the rainbow rather than refusing to start over a cosmetic.
    if let Some(name) = unknown_flag {
        eprintln!(
            "roog: no flag called '{name}'. Try one of: {}.",
            models::pride::flag_names().join(", ")
        );
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

    // Every resource the schedule and the renderer read has to exist before
    // either runs. A loaded save replaces the run-state ones below; the
    // presentation ones (`Particles`, `Shake`, `AnimRate`, `Pride`) are never
    // saved and are set from the command line either way.
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
        move_effect: None,
        looking: false,
        reach_attack: false,
        cursor_x: 0,
        cursor_y: 0,
    });
    world.init_resource::<MovesMenu>();
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
    // The scorekeeper's own two: what it is shouting, and what has died this
    // turn. Initialised here as well as in `initialize_world`, because a loaded
    // save skips that and still has a score to shout about.
    world.init_resource::<models::ScoreFlash>();
    world.init_resource::<models::Combo>();
    world.init_resource::<AttackQueue>();
    world.init_resource::<UseQueue>();
    world.init_resource::<ThrowQueue>();
    world.init_resource::<MoveQueue>();
    world.init_resource::<PlayerTempo>();
    world.init_resource::<ExtraMonsterRound>();
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

    // `-pride`: the flag this run flies. Not saved — it is a preference, so a
    // reloaded save flies whatever flag the command line asks for this time.
    world.insert_resource(models::pride::Pride(pride));

    // The turn, in order. Every `.after()` here is load-bearing; the order is
    // documented in `docs/reference/input-and-turn-loop.md`.
    let mut schedule = Schedule::default();
    schedule.add_systems((
        smoke_system.before(snare_system),
        snare_system,
        // A xeroc's disguise falls away the instant the player is adjacent to
        // it — before `ai` runs, so the very turn that happens it also gets
        // to lash out as the `Ambush` mob it always was.
        reveal_mimics.after(snare_system),
        ai.after(reveal_mimics),
        // A coin-greedy orc that just stepped onto a coin it can use claims it
        // here, while `EntityMoved` still marks it — the same tag the trap
        // system reads right after.
        monster_pickup_system.after(ai),
        trap_system.after(monster_pickup_system),
        throw_system.after(trap_system),
        item_system.after(throw_system),
        // An active move resolves the same moment a zapped wand would; there's
        // no ordering reason it has to follow items rather than sit beside
        // them, only that it needs somewhere fixed to be.
        move_system.after(item_system),
        // Gear changed by anything other than the pack screen — a loaded save, a
        // curse-lifting scroll — has its lent effects reconciled here, before
        // combat and visibility read them.
        equipment_effects_system.after(move_system),
        combat_system.after(equipment_effects_system),
        reaper_system.after(combat_system),
        dungeon_lord_system.after(reaper_system),
        // Passives that act on their own (a ring of regeneration mending you, a
        // ring of teleportation moving you) roll at the *tail* of the turn:
        // late enough that a jump lands at the top of the player's next turn —
        // they see where they are and act before anything else moves — and
        // early enough that visibility still gets a pass over the new tile.
        passive_ability_system.after(dungeon_lord_system),
        visibility_system.after(passive_ability_system),
        // Dead last: everything that can pay the player has paid by now, so a
        // flash armed anywhere in this turn is still lit for this turn's render
        // and dark by the next one.
        score_turn_system.after(visibility_system),
    ));

    // One turn and one frame before the loop, so the player is looking at a
    // dungeon rather than a black screen when the first `read()` blocks.
    schedule.run(&mut world);
    view::render(&mut world, &mut stdout, &mut screen)?;

    let save_name = format!(
        "{}.sav",
        world.resource::<PlayerName>().what.to_ascii_lowercase()
    );

    // The main loop. Its steps are named A..D because
    // `docs/reference/input-and-turn-loop.md` walks them in that order.
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
