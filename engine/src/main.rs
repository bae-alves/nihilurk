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
/// then the LOSE panel. Runs while the terminal guard is still active.
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

    view::render_lose(stdout, screen, offset, &name, &cause, score)?;
    wait_for_key(|_| true)?;
    Ok(())
}

/// The victory sequence: show the WIN panel. Unlike death, the save is *not*
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

    view::render_win(stdout, screen, offset, &name, score)?;
    wait_for_key(|_| true)?;
    Ok(())
}

/// Prints every name the content tables know, grouped by category. Reads the
/// tables themselves, so a row added today shows up here today.
fn print_content() {
    let names = models::content_names();
    println!("nihilurk content — {} entries", names.len());
    println!("Spawn any of them with: NIHILURK_SPAWN=\"<name>,<name>\" nihilurk");

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
/// The full reference lives in the installed `nihilurk(6)` manual.
fn print_help() {
    println!(
        "\
nihilurk - terminal roguelike

USAGE
    nihilurk [NAME|SAVE] [OPTIONS]

OPTIONS
    -s SEED          use a reproducible u64 seed
    -c               centre the map on the player
    -ns              do not write a save file
    -nb              disable blood and corpse animation
    -nshake          disable screen shake
    -anim-rate N     set animation pacing multiplier (0.1..=5.0)
    -b BODY          play as nihil (default) or lurk
    -am SPECIES      play as a monster: any bestiary name, e.g. -am dragon
    -content         list names accepted by NIHILURK_SPAWN
    -h, -help, --help show this help and exit

POSITIONAL ARGUMENT (first argument only)
    NAME             start a new run with this player name
    SAVE             load an existing save, with or without .sav

ENVIRONMENT
    NIHILURK_SPAWN       comma-separated names to place near the player on every
                     generated floor; use -content to list valid names

EXAMPLES
    nihilurk
    nihilurk bae
    nihilurk -s 1234 -ns
    nihilurk -b lurk
    nihilurk bae -am dragon
    NIHILURK_SPAWN=\"dragon,ring of protection\" nihilurk

SEE ALSO
    man nihilurk          full command, environment, and spawn API reference
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

/// The turn, in order. Every `.after()`/`.before()` here is load-bearing; the
/// order is documented in `docs/reference/input-and-turn-loop.md`.
fn turn_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        smoke_system.before(tick_effects),
        tick_effects,
        // A xeroc's disguise falls away the instant the player is adjacent to
        // it — before `ai` runs, so the very turn that happens it also gets
        // to lash out as the `Ambush` mob it always was.
        reveal_mimics.after(tick_effects),
        // An active spell resolves before the monsters act, just like the
        // player's ordinary movement already resolved while the key was
        // handled. `ai` skips anything a spell left at 0 HP; `reaper_system`
        // still sweeps the bodies at the end of the turn.
        spell_system.after(reveal_mimics).before(ai),
        // A used item resolves before the monsters act, for the same reason a
        // spell does: the player aimed at the dungeon as it stood when they
        // pressed the key. Zapping is the case that made this load-bearing —
        // a wand that reads one exact tile (teleport, polymorph, haste, slow,
        // cancellation) found the tile empty when `ai` had already walked the
        // target off it, so a correctly aimed zap did nothing at all.
        item_system.after(reveal_mimics).before(ai),
        // And a throw with it: the player let go of it at the floor they were
        // looking at. Only the player ever fills `ThrowQueue`, so nothing of
        // the dungeon's own is being hurried along by this.
        throw_system.after(item_system).before(ai),
        ai.after(spell_system),
        // A coin-greedy orc that just stepped onto a coin it can use claims it
        // here, while `EntityMoved` still marks it — the same tag the trap
        // system reads right after.
        monster_pickup_system.after(ai),
        trap_system.after(monster_pickup_system),
        // Gear changed by anything other than the pack screen — a loaded save, a
        // curse-lifting scroll — has its lent effects reconciled here, before
        // combat and visibility read them.
        equipment_effects_system.after(item_system),
        combat_system.after(equipment_effects_system),
        reaper_system.after(combat_system),
        dungeon_lord_system.after(reaper_system),
        // Passives that act on their own (a ring of regeneration mending you, a
        // ring of teleportation moving you) roll at the *tail* of the turn:
        // late enough that a jump lands at the top of the player's next turn —
        // they see where they are and act before anything else moves — and
        // early enough that visibility still gets a pass over the new tile.
        ability_system.after(dungeon_lord_system),
        visibility_system.after(ability_system),
        // Dead last: everything that can pay the player has paid by now, so a
        // flash armed anywhere in this turn is still lit for this turn's render
        // and dark by the next one.
        score_turn_system.after(visibility_system),
    ));
    schedule
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
    // What the player wakes up as. `-b <body>` picks one of the two written
    // to be played, `-am <species>` wears a bestiary row instead, and the two
    // are one value rather than two flags — which is what makes them
    // mutually exclusive without a cross-check. An unrecognised name either
    // side is a typo worth stopping for, unlike an unrecognised flag: it is
    // the whole run, not a colour.
    let mut body: Option<(models::Body, &str)> = None;
    let mut unknown_body: Option<(String, &str)> = None;
    // How the body was asked for, verbatim ("-b lurk"), and the second
    // spelling if there was one. Named rather than resolved: one of them
    // would have to win, and neither has a claim.
    let mut body_arg: Option<String> = None;
    let mut conflicting_bodies: Option<(String, String)> = None;
    let mut player_name = "nihil".to_string();
    let mut positional: Option<String> = None;
    // Multiplier on every animation frame's on-screen hold time (particles,
    // magic mapping's reveal wipe): the escape hatch for a terminal whose
    // redraw can't keep up with the default pacing, or that renders too
    // slowly for a fast one. `1.0` is the default pacing; clamped so a typo'd
    // value can't freeze the loop or blur every animation into nothing.
    let mut anim_rate: f32 = 1.0;
    // The name is the *first* argument or it is not a name. Anything
    // unrecognised after that is a typo — `nihilurk -b lurk Bae` reads like
    // it names the run and does not, and silently starting a run called
    // "nihil" is the worst of the three things that could happen.
    let mut first = true;
    let mut stray_positional: Option<String> = None;
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
            "-b" | "-am" => {
                let flag = match arg.as_str() {
                    "-b" => "-b",
                    _ => "-am",
                };
                if let Some(name) = iter.next() {
                    let spelling = format!("{flag} {name}");
                    if let Some(first) = body_arg.replace(spelling.clone()) {
                        conflicting_bodies = Some((first, spelling));
                    }
                    match (flag, name.as_str()) {
                        ("-b", "nihil") => body = Some((models::Body::Nihil, "-b")),
                        ("-b", "lurk") => body = Some((models::Body::Lurk, "-b")),
                        ("-b", _) => unknown_body = Some((name.clone(), "-b")),
                        (_, species) => match models::MonsterDef::lookup(species) {
                            Some(def) => body = Some((models::Body::Monster(def), "-am")),
                            None => unknown_body = Some((name.clone(), "-am")),
                        },
                    }
                }
            }
            "-anim-rate" => {
                if let Some(rate_str) = iter.next() {
                    if let Ok(rate) = rate_str.parse::<f32>() {
                        anim_rate = rate.clamp(0.1, 5.0);
                    }
                }
            }
            // A name or a save file, and only ever the first argument: see
            // `too_late_for_a_name` below.
            _ => match positional {
                None if first => positional = Some(arg.clone()),
                _ => stray_positional = Some(arg.clone()),
            },
        }
        first = false;
    }

    // `-content` is the content author's index: every name the tables know, which
    // is exactly the set `NIHILURK_SPAWN` and `models::spawn_named` answer to. Prints
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
            "nihilurk: no flag called '{name}'. Try one of: {}.",
            models::pride::flag_names().join(", ")
        );
    }

    // Both flags at once. You are one creature; the command line has to name
    // one, and picking the rightmost for the player would be guessing at the
    // whole run.
    if let Some((first_flag, second)) = conflicting_bodies {
        eprintln!("nihilurk: {first_flag} and {second} are the same choice. Pick one.");
        return Ok(());
    }

    // A `-b` or `-am` nobody has a row for. Unlike a flag name this is
    // refused outright: the body is the entire run, and starting a run as
    // nihil the player did not ask for is worse than not starting at all.
    if let Some((name, flag)) = unknown_body {
        match flag {
            "-b" => {
                eprintln!("nihilurk: no body called '{name}'. There is nihil, and there is lurk.")
            }
            _ => eprintln!("nihilurk: no monster called '{name}'. Try -content for the bestiary."),
        }
        return Ok(());
    }

    // A name that came too late to be one.
    if let Some(stray) = stray_positional {
        eprintln!(
            "nihilurk: '{stray}' is not a flag, and a name has to come first: nihilurk {stray} ..."
        );
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

    // Two argument shapes that would otherwise resolve to something nobody
    // asked for. Both are refused here, before the terminal is touched, and
    // together they are what keeps "the player's name is a species" meaning
    // "the player is wearing that species" — the equivalence
    // `models::load_game` reads a saved body back out of.
    if let Some((body, flag)) = body {
        if load_path.is_some() {
            eprintln!(
                "nihilurk: a save already knows what body it is in; drop {flag} {} to load it.",
                body.name()
            );
            return Ok(());
        }
    }
    if models::MonsterDef::is_species_name(&player_name) {
        eprintln!("Don't name yourself a monster. It's quite demeaning.");
        return Ok(());
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
        spell_effect: None,
        looking: false,
        reach_attack: false,
        cursor_x: 0,
        cursor_y: 0,
    });
    world.init_resource::<SpellsMenu>();
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
    world.init_resource::<SpellQueue>();
    world.init_resource::<PlayerTempo>();
    world.init_resource::<ExtraMonsterRound>();
    world.init_resource::<GameLog>();
    world.insert_resource(models::Particles::new());
    world.insert_resource(models::Shake::new());
    world.insert_resource(models::AnimRate(anim_rate));

    // Which body a *new* player wakes up in. A loaded save brings its own —
    // the body rides in the save file — so this only ever reaches
    // `initialize_world`.
    world.insert_resource(models::StartingBody(
        body.map(|(b, _)| b).unwrap_or_default(),
    ));

    match &load_path {
        Some(path) => {
            models::load_game(&mut world, path)?;
            // `load_game` already left a fresh (unloaded-game) GameLog behind;
            // swap its welcome line for the loaded-game version.
            world.insert_resource(GameLog {
                history: Vec::new(),
                unread: vec!["Welcome back to nihilurk! Good luck and have fun!".to_string()],
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

    // The turn, in order.
    let mut schedule = turn_schedule();

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
        // a completed run. Show the WIN panel, then keep the save as clear data
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
        Ok(()) => println!("Game saved to '{save_name}'. Resume with: nihilurk {save_name}"),
        Err(e) => eprintln!("Failed to save game: {e}"),
    }

    Ok(())
}
