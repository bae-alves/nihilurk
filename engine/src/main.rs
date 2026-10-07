//! The binary: parses the command line into a starting world, builds the turn
//! schedule ([`turn_schedule`]), then runs the read-act-render loop until
//! [`Ending`] says the run is over. `screen`, `update` and `view` are its own submodules
//! — everything else the turn touches lives in `models`.

mod screen;
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

/// Puts the terminal into raw, alternate-screen mode for the run and takes it
/// back out on drop, panic included — the [`std::panic::set_hook`] below
/// leaves the terminal sane even if the drop never gets to run because
/// unwinding is disabled.
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
    let outcome = match models::holding_element_of_yoord(world) {
        true => models::leaderboard::Outcome::LoseAscent,
        false => models::leaderboard::Outcome::LoseDescent,
    };
    let _ = models::leaderboard::record(&name, outcome, score);
    let _ = models::bones::deposit(world);

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
    let _ = models::leaderboard::record(&name, models::leaderboard::Outcome::Win, score);

    view::render_win(stdout, screen, offset, &name, score)?;
    wait_for_key(|_| true)?;
    Ok(())
}

/// Prints every name the content tables know, grouped by category. Reads the
/// tables themselves, so a row added today shows up here today.
fn print_content() {
    let names = models::content_names();
    println!("{}", strings::content_header(names.len()));
    println!("{}", strings::content_spawn_hint());

    let mut group = "";
    for (category, name) in &names {
        if *category != group {
            group = category;
            let count = names.iter().filter(|(c, _)| c == category).count();
            println!("{}", strings::content_group_header(group, count));
        }
        println!("  {name}");
    }
}

/// Prints the short command-line guide without entering the alternate screen.
/// The full reference lives in the installed `nihilurk(6)` manual.
fn print_help() {
    println!("{}", strings::help_text());
}

/// Prints the internal leaderboard, highest score first, without entering the
/// alternate screen — the same way `-content` never touches the terminal.
fn print_leaderboard() {
    let entries = models::leaderboard::top(models::leaderboard::LEADERBOARD_STORE_LIMIT);
    if entries.is_empty() {
        println!("{}", strings::leaderboard_empty());
        return;
    }
    println!("{}", strings::leaderboard_header(entries.len()));
    for (rank, (name, outcome, score, when)) in entries.into_iter().enumerate() {
        println!(
            "{}",
            strings::leaderboard_entry(rank + 1, &name, &outcome.to_string(), score, &when)
        );
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

/// The turn, in order. Every `.after()`/`.before()` here is load-bearing; the
/// order is documented in `docs/reference/input-and-turn-loop.md`.
fn turn_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems((
        smoke_system.before(tick_effects),
        tick_effects,
        reveal_mimics.after(tick_effects),
        spell_system.after(reveal_mimics).before(ai),
        item_system.after(reveal_mimics).before(ai),
        throw_system.after(item_system).before(ai),
        ai.after(spell_system),
        trap_system.after(ai),
        equipment_effects_system
            .after(item_system)
            .after(trap_system),
        combat_system.after(equipment_effects_system),
        reaper_system.after(combat_system),
        dungeon_lord_system.after(reaper_system),
        ability_system.after(dungeon_lord_system),
        sink_system.after(ability_system),
        visibility_system.after(sink_system),
        score_turn_system.after(visibility_system),
    ));
    schedule
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args()
        .map(|a| models::strip_control_chars(&a))
        .collect();
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
    let mut no_bones = false;
    let mut endless = false;
    let mut list_content = false;
    let mut show_leaderboard = false;
    let mut pride_off = false;
    let mut pride = models::pride::PrideFlag::default_flag();
    let mut unknown_flag: Option<String> = None;
    let mut body: Option<(models::Body, &str)> = None;
    let mut unknown_body: Option<(String, &str)> = None;
    let mut body_arg: Option<String> = None;
    let mut conflicting_bodies: Option<(String, String)> = None;
    let mut player_name = "null".to_string();
    let mut positional: Option<String> = None;
    let mut anim_rate: f32 = 1.0;
    let mut first = true;
    let mut stray_positional: Option<String> = None;
    let mut iter = args.iter();
    iter.next();
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
            "-nobones" => no_bones = true,
            "-endless" => endless = true,
            "-content" => list_content = true,
            "-scores" => show_leaderboard = true,
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
                    match (flag, name.as_str(), models::MonsterDef::lookup(name)) {
                        ("-b", "nihil", _) => body = Some((models::Body::Nihil, "-b")),
                        ("-b", "lurk", _) => body = Some((models::Body::Lurk, "-b")),
                        ("-b", _, _) => unknown_body = Some((name.clone(), "-b")),
                        (_, _, Some(def)) => body = Some((models::Body::Monster(def), "-am")),
                        (_, _, None) => unknown_body = Some((name.clone(), "-am")),
                    }
                }
            }
            "-anim-rate" => {
                if let Some(rate) = iter.next().and_then(|s| s.parse::<f32>().ok()) {
                    anim_rate = rate.clamp(0.1, 5.0);
                }
            }
            _ => match positional {
                None if first => positional = Some(arg.clone()),
                _ => stray_positional = Some(arg.clone()),
            },
        }
        first = false;
    }

    if list_content {
        print_content();
        return Ok(());
    }

    if show_leaderboard {
        print_leaderboard();
        return Ok(());
    }

    if pride_off {
        execute!(
            stdout(),
            SetForegroundColor(CrosstermColor::Red),
            Print(models::pride::pride_off_refusal()),
            Print("\n"),
            ResetColor
        )?;
        return Ok(());
    }

    if let Some(name) = unknown_flag {
        eprintln!(
            "{}",
            strings::no_such_pride_flag(&name, &models::pride::flag_names().join(", "))
        );
    }

    if let Some((first_flag, second)) = conflicting_bodies {
        eprintln!("{}", strings::conflicting_bodies(&first_flag, &second));
        return Ok(());
    }

    if let Some((name, flag)) = unknown_body {
        match flag {
            "-b" => eprintln!("{}", strings::no_such_body(&name)),
            _ => eprintln!("{}", strings::no_such_monster(&name)),
        }
        return Ok(());
    }

    if let Some(stray) = stray_positional {
        eprintln!("{}", strings::stray_positional(&stray));
        return Ok(());
    }

    let mut load_path: Option<String> = None;
    if let Some(arg) = positional {
        let suffixed = format!("{arg}.sav");
        match find_case_insensitive(&arg).or_else(|| find_case_insensitive(&suffixed)) {
            Some(found) => load_path = Some(found),
            None => player_name = arg,
        }
    }

    if let Some(path) = &load_path {
        if let Some(clear) = models::clear_data(path)? {
            println!("{}", strings::clear_data_prompt(&clear.player_name));
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                println!("{}", strings::world_keeps_its_light());
                return Ok(());
            }
            player_name = clear.player_name;
            load_path = None;
        }
    }

    if let Some((body, flag)) = body {
        if load_path.is_some() {
            eprintln!("{}", strings::body_conflicts_with_load(flag, body.name()));
            return Ok(());
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = execute!(std::io::stderr(), LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
        original_hook(panic_info);
    }));

    let guard = TerminalGuard::new()?;
    let mut stdout = BufWriter::with_capacity(32 * 1024, stdout());
    let mut screen = view::Screen::new();
    let mut world = World::new();

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
    world.init_resource::<OfferMenu>();
    world.init_resource::<BarterMenu>();
    world.init_resource::<QuitPrompt>();
    world.init_resource::<HelpMenu>();
    world.init_resource::<CommandBar>();
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

    world.insert_resource(models::StartingBody(
        body.map(|(b, _)| b).unwrap_or_default(),
    ));

    world.insert_resource(models::Endless { enabled: endless });

    match &load_path {
        Some(path) => {
            models::load_game(&mut world, path)?;
            let mut unread = vec![LogEntry::plain(strings::welcome_back())];
            if let Some(notice) = strings::beta_notice() {
                unread.push(LogEntry::plain(notice));
            }
            world.insert_resource(GameLog {
                history: Vec::new(),
                unread,
            });
        }
        None => models::initialize_world(&mut world),
    }

    if no_blood {
        world.resource_mut::<BloodStains>().enabled = false;
    }

    if no_shake {
        world.resource_mut::<Shake>().enabled = false;
    }

    if no_bones {
        world.resource_mut::<models::bones::Bones>().enabled = false;
    }

    world.insert_resource(models::pride::Pride(pride));

    let mut schedule = turn_schedule();

    schedule.run(&mut world);
    view::render(&mut world, &mut stdout, &mut screen)?;

    let save_name = format!(
        "{}.sav",
        world.resource::<PlayerName>().what.to_ascii_lowercase()
    );

    while world.resource::<models::GameState>().is_running {
        let fast_moving = world.resource::<FastMove>().active;
        if fast_moving {
            update::fast_move_run(&mut world, &mut schedule)?;
        }
        if !fast_moving {
            let turn_taken = player_step(&mut world)?;

            if turn_taken {
                schedule.run(&mut world);
            }
        }

        view::play_particles(&mut world, &mut stdout, &mut screen)?;

        view::play_magic_map(&mut world, &mut stdout, &mut screen)?;

        view::render(&mut world, &mut stdout, &mut screen)?;

        view::play_shake(&mut world, &mut stdout, &mut screen)?;

        if world.resource::<AutoExplore>().active {
            std::thread::sleep(std::time::Duration::from_millis(35));
        }

        if models::player_incapacitated(&mut world) {
            std::thread::sleep(std::time::Duration::from_millis(90));
        }

        if world.resource::<Ending>().player_dead || world.resource::<Ending>().player_won {
            break;
        }
    }

    if world.resource::<Ending>().player_won {
        run_victory_screens(&mut world, &mut stdout, &mut screen)?;
        if no_save {
            drop(guard);
            println!("{}", strings::clear_data_not_saved());
            return Ok(());
        }
        let save_result = models::save_game(&mut world, &save_name);
        drop(guard);
        match save_result {
            Ok(()) => println!("{}", strings::clear_data_saved(&save_name)),
            Err(e) => eprintln!("{}", strings::failed_to_save_clear_data(&e.to_string())),
        }
        return Ok(());
    }

    if world.resource::<Ending>().player_dead {
        run_death_screens(&mut world, &mut stdout, &mut screen, &save_name)?;
        drop(guard);
        return Ok(());
    }

    if no_save {
        drop(guard);
        println!("{}", strings::game_not_saved());
        return Ok(());
    }
    let save_result = models::save_game(&mut world, &save_name);
    drop(guard);
    match save_result {
        Ok(()) => println!("{}", strings::game_saved(&save_name)),
        Err(e) => eprintln!("{}", strings::failed_to_save_game(&e.to_string())),
    }

    Ok(())
}
