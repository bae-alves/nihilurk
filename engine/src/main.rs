//! The binary: parses the command line into a starting world, builds the turn
//! schedule ([`models::turn_schedule`]), then runs the read-act-render loop until
//! [`Ending`] says the run is over. `screen`, `update` and `view` are its own submodules
//! — everything else the turn touches lives in `models`.

mod constants;
mod screen;
mod update;
mod view;
use models::*;

use bevy_ecs::prelude::{With, World};
use crossterm::{
    cursor::{Hide, Show},
    event::{Event, KeyCode, KeyEventKind, read},
    execute,
    style::{Color as CrosstermColor, Print, ResetColor, SetForegroundColor},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{BufWriter, Write, stdout};
use std::path::{Path, PathBuf};

use models::{ChaCha12Rng, SeedableRng};

/// Puts the terminal into raw, alternate-screen mode for the run and takes it
/// back out on drop, panic included — the [`std::panic::set_hook`] below
/// leaves the terminal sane even if the drop never gets to run because
/// unwinding is disabled.
pub struct TerminalGuard;

impl TerminalGuard {
    /// Enters raw mode and the alternate screen with the cursor hidden. Fails
    /// if the terminal refuses either; nothing is left half-set.
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

/// Finds a file in `name`'s directory whose name matches `name`'s
/// case-insensitively, returning its actual on-disk path. This makes save
/// files loadable regardless of the case typed on the command line. It always
/// reads the directory, because on a case-insensitive filesystem (macOS,
/// Windows) the typed name "exists" and would come back as typed. An exact
/// match wins over a different-case one.
fn find_case_insensitive(name: &Path) -> Option<PathBuf> {
    let dir = name
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = name.file_name()?.to_str()?;
    let mut matches: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|f| f.eq_ignore_ascii_case(file_name))
                && entry.path().is_file()
        })
        .map(|entry| entry.path())
        .collect();
    matches.sort();
    let exact = matches
        .iter()
        .position(|p| p.file_name() == name.file_name());
    matches.into_iter().nth(exact.unwrap_or(0))
}

/// The save a positional argument names: a file at `arg` as typed, or else
/// `<arg>` or `<arg>.sav` in `data`, any case.
fn find_save(arg: &str, data: &Path) -> Option<PathBuf> {
    let typed = Path::new(arg);
    if typed.is_file() {
        return Some(typed.to_path_buf());
    }
    find_case_insensitive(&data.join(arg))
        .or_else(|| find_case_insensitive(&data.join(format!("{arg}.sav"))))
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
    save_path: &Path,
) -> std::io::Result<()> {
    let _ = std::fs::remove_file(save_path);

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
fn print_content(out: &mut impl Write) -> std::io::Result<()> {
    let names = models::content_names();
    writeln!(out, "{}", strings::content_header(names.len()))?;
    writeln!(out, "{}", strings::content_spawn_hint())?;

    let mut group = "";
    for (category, name) in &names {
        if *category != group {
            group = category;
            let count = names.iter().filter(|(c, _)| c == category).count();
            writeln!(out, "{}", strings::content_group_header(group, count))?;
        }
        writeln!(out, "  {name}")?;
    }
    Ok(())
}

/// A reader that hangs up early (`nihilurk -content | head`) is done, not
/// broken. Rust ignores SIGPIPE, so the write fails instead of ending the
/// process; this turns that one error into success.
fn finish_listing(result: std::io::Result<()>) -> std::io::Result<()> {
    match result {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

/// Turns an unreadable save into the player-facing refusal and exits; any
/// other I/O error passes through untouched.
fn refuse_save(e: std::io::Error) -> std::io::Error {
    if e.kind() == std::io::ErrorKind::InvalidData {
        eprintln!("{}", strings::invalid_save());
        std::process::exit(1);
    }
    e
}

/// Prints the short command-line guide without entering the alternate screen.
/// The full reference lives in the installed `nihilurk(6)` manual.
fn print_help(out: &mut impl Write) -> std::io::Result<()> {
    writeln!(out, "{}", strings::help_text())
}

/// Prints the internal leaderboard, highest score first, without entering the
/// alternate screen — the same way `-content` never touches the terminal. A
/// leaderboard file it can't read is named on stderr and left as it is.
fn print_leaderboard(out: &mut impl Write) -> std::io::Result<()> {
    let Ok(entries) = models::leaderboard::top(models::leaderboard::LEADERBOARD_STORE_LIMIT) else {
        let path = models::leaderboard::path();
        eprintln!(
            "{}",
            strings::leaderboard_unreadable(&path.display().to_string())
        );
        return Ok(());
    };
    if entries.is_empty() {
        return writeln!(out, "{}", strings::leaderboard_empty());
    }
    writeln!(out, "{}", strings::leaderboard_header(entries.len()))?;
    for (rank, (name, outcome, score, when)) in entries.into_iter().enumerate() {
        writeln!(
            out,
            "{}",
            strings::leaderboard_entry(rank + 1, &name, &outcome.to_string(), score, &when)
        )?;
    }
    Ok(())
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

/// Maps a double-dash longhand to the short flag the parser matches on.
/// Anything else comes back unchanged.
fn canonical_flag(arg: &str) -> &str {
    match arg {
        "--seed" => "-s",
        "--centered" => "-c",
        "--no-save" => "-ns",
        "--no-blood" => "-nb",
        "--no-shake" => "-nshake",
        "--no-bones" => "-nobones",
        "--body" => "-b",
        "--as-monster" => "-am",
        "--leaderboard" => "-scores",
        "--endless" => "-endless",
        "--anim-rate" => "-anim-rate",
        "--content" => "-content",
        "--pride" => "-pride",
        "--prideoff" => "-prideoff",
        _ => arg,
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_flag, find_save};

    fn data_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nihilurk-main-unit-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_bare_name_loads_from_the_data_dir_in_any_case() {
        let data = data_dir("name");
        std::fs::write(data.join("lurk.sav"), b"").unwrap();

        assert_eq!(find_save("LURK", &data), Some(data.join("lurk.sav")));
        assert_eq!(find_save("Lurk.sav", &data), Some(data.join("lurk.sav")));
        assert_eq!(find_save("nobody", &data), None);
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_typed_path_loads_as_typed() {
        let data = data_dir("typed");
        let elsewhere = data_dir("elsewhere").join("old.sav");
        std::fs::write(&elsewhere, b"").unwrap();

        let typed = elsewhere.to_str().unwrap();
        assert_eq!(find_save(typed, &data), Some(elsewhere.clone()));
        let _ = std::fs::remove_dir_all(&data);
        let _ = std::fs::remove_dir_all(elsewhere.parent().unwrap());
    }

    #[test]
    fn longhands_map_to_their_short_flags() {
        assert_eq!(canonical_flag("--leaderboard"), "-scores");
        assert_eq!(canonical_flag("--no-save"), "-ns");
        assert_eq!(canonical_flag("--as-monster"), "-am");
        assert_eq!(canonical_flag("--anim-rate"), "-anim-rate");
        assert_eq!(canonical_flag("bae"), "bae");
    }
}

/// The text `invalid argument` quotes for a flag whose value is missing or bad.
fn flag_with_value(flag: &str, value: Option<&String>) -> String {
    match value {
        Some(v) => format!("{flag} {v}"),
        None => flag.to_string(),
    }
}

/// Parses the command line, builds the world, then runs the read-act-render
/// loop until the run ends. Prints the ending screens and writes the save or
/// leaderboard entry on the way out.
fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args()
        .map(|a| models::strip_control_chars(&a))
        .collect();
    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "-h" | "-help" | "--help"))
    {
        return finish_listing(print_help(&mut stdout().lock()));
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
    let mut invalid_argument: Option<String> = None;
    let mut iter = args.iter();
    iter.next();
    while let Some(arg) = iter.next() {
        match canonical_flag(arg) {
            "-s" => {
                let value = iter.next();
                match value.and_then(|v| v.parse::<u64>().ok()) {
                    Some(n) => seed = Some(n),
                    None => {
                        invalid_argument.get_or_insert(flag_with_value("-s", value));
                    }
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
            "-pride" => match iter.next().filter(|v| !v.starts_with('-')) {
                Some(name) => match models::pride::PrideFlag::named(name) {
                    Some(flag) => pride = flag,
                    None => unknown_flag = Some(name.clone()),
                },
                None => {
                    invalid_argument.get_or_insert("-pride".to_string());
                }
            },
            "-prideoff" => pride_off = true,
            "-b" | "-am" => {
                let flag = match canonical_flag(arg) {
                    "-b" => "-b",
                    _ => "-am",
                };
                let value = iter.next();
                if let Some(name) = value.filter(|v| !v.starts_with('-')) {
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
                } else {
                    invalid_argument.get_or_insert(flag_with_value(flag, value));
                }
            }
            "-anim-rate" => {
                let value = iter.next();
                match value
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|r| r.is_finite())
                {
                    Some(rate) => anim_rate = rate.clamp(0.1, 5.0),
                    None => {
                        invalid_argument.get_or_insert(flag_with_value("-anim-rate", value));
                    }
                }
            }
            _ if arg.starts_with('-') => {
                invalid_argument.get_or_insert_with(|| arg.clone());
            }
            _ => match positional {
                None if first => positional = Some(arg.clone()),
                _ => stray_positional = Some(arg.clone()),
            },
        }
        first = false;
    }

    if let Some(arg) = invalid_argument {
        eprintln!("{}", strings::invalid_argument(&arg));
        return Ok(());
    }

    if list_content {
        return finish_listing(print_content(&mut stdout().lock()));
    }

    if show_leaderboard {
        return finish_listing(print_leaderboard(&mut stdout().lock()));
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

    let data_dir = models::data_dir();
    let mut load_path: Option<(PathBuf, String)> = None;
    if let Some(arg) = positional {
        match find_save(&arg, &data_dir) {
            Some(found) => load_path = Some((found, arg)),
            None => player_name = arg,
        }
    }

    if let Some((path, _)) = &load_path {
        if let Some(clear) = models::clear_data(path).map_err(refuse_save)? {
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
    world.init_resource::<PlayerActionQueue>();
    world.init_resource::<PlayerActionQueue>();
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
        Some((path, _)) => {
            if let Err(e) = models::load_game(&mut world, path) {
                drop(guard);
                return Err(refuse_save(e));
            }
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

    let name = world.resource::<PlayerName>().what.to_ascii_lowercase();
    let (save_path, resume_as) =
        load_path.unwrap_or_else(|| (data_dir.join(format!("{name}.sav")), name));
    let shown_path = save_path.display().to_string();

    let mut time_stopped = models::time_stopped(&mut world);
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

        if !no_save {
            update::save_on_time_edge(&mut world, &mut time_stopped, &save_path);
        }

        view::play_particles(&mut world, &mut stdout, &mut screen)?;

        view::play_magic_map(&mut world, &mut stdout, &mut screen)?;

        view::render(&mut world, &mut stdout, &mut screen)?;

        view::play_shake(&mut world, &mut stdout, &mut screen)?;

        if world.resource::<AutoExplore>().active {
            std::thread::sleep(std::time::Duration::from_millis(
                constants::timing::AUTOEXPLORE_STEP_MS,
            ));
        }

        if models::player_incapacitated(&mut world) {
            std::thread::sleep(std::time::Duration::from_millis(
                constants::timing::INCAPACITATED_PAUSE_MS,
            ));
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
        let save_result = models::save_game(&mut world, &save_path);
        drop(guard);
        match save_result {
            Ok(()) => println!("{}", strings::clear_data_saved(&shown_path)),
            Err(e) => eprintln!("{}", strings::failed_to_save_clear_data(&e.to_string())),
        }
        return Ok(());
    }

    if world.resource::<Ending>().player_dead {
        run_death_screens(&mut world, &mut stdout, &mut screen, &save_path)?;
        drop(guard);
        return Ok(());
    }

    if no_save {
        drop(guard);
        println!("{}", strings::game_not_saved());
        return Ok(());
    }
    if models::time_stopped(&mut world) {
        drop(guard);
        println!("{}", strings::time_stopped_not_saved().join(" "));
        return Ok(());
    }
    let save_result = models::save_game(&mut world, &save_path);
    drop(guard);
    match save_result {
        Ok(()) => println!("{}", strings::game_saved(&shown_path, &resume_as)),
        Err(e) => eprintln!("{}", strings::failed_to_save_game(&e.to_string())),
    }

    Ok(())
}
