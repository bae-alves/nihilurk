//! roog-compat -- the reporting half of the compatibility pipeline.
//!
//!     roog-compat                 the dashboard
//!     roog-compat --watch         the dashboard, re-reading as the matrix runs
//!     roog-compat report          plain text; no UI, for CI and for piping
//!     roog-compat gate            exit non-zero if a machine stopped running roog
//!
//! It measures nothing itself. `compat/cross_build.sh` builds the game for
//! every machine in the matrix and `compat/stress_test_matrix.sh` runs it under
//! Docker with that machine's limits applied; both write their numbers into
//! `target/compat/`, and this reads them back. The split is deliberate --
//! see `results.rs` -- and it is what lets the dashboard be opened over a run
//! that finished last week, or one that is still going.
//!
//! # What the numbers are about
//!
//! Whether **roog** runs on the machine, and how well. Not whether the stress
//! test does. The reel -- Bad Apple, ~800 motes a frame -- is the ceiling, run
//! for the headroom figure and never gated on; the verdict column is always
//! the game's own load on a real dungeon floor. See `verdict.rs` and
//! `docs/explanation/cross-platform-testing.md`.

mod matrix;
mod results;
mod ui;
mod verdict;

use std::fmt::Write;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::matrix::{Class, Exec};
use crate::results::{Load, Results, Status};
use crate::ui::App;
use crate::verdict::Band;

/// Where the shell half writes, relative to the workspace root.
const DEFAULT_DIR: &str = "target/compat";

/// How often `--watch` re-reads the result files. Brisk enough that a row
/// finishing shows up while you are still looking at the screen, and far too
/// slow to cost the containers anything.
const RELOAD: Duration = Duration::from_millis(500);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let opts = match Options::parse(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("roog-compat: {e}");
            return ExitCode::FAILURE;
        }
    };

    let results = Results::load(&opts.dir);
    match opts.mode {
        Mode::Report => report(&results),
        Mode::Gate => gate(&results),
        Mode::Dashboard => dashboard(results, &opts),
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Dashboard,
    Report,
    /// Read the results and say yes or no, in an exit code. Prints one line.
    Gate,
}

struct Options {
    mode: Mode,
    dir: PathBuf,
    watch: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Options {
            mode: Mode::Dashboard,
            dir: PathBuf::from(DEFAULT_DIR),
            watch: false,
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "report" => o.mode = Mode::Report,
                "gate" => o.mode = Mode::Gate,
                "--watch" => o.watch = true,
                "--dir" => {
                    let value = it.next().ok_or("--dir needs a path")?;
                    o.dir = PathBuf::from(value);
                }
                other => return Err(format!("unknown argument `{other}` (try --help)")),
            }
        }
        if o.watch && o.mode != Mode::Dashboard {
            return Err("--watch only applies to the dashboard".into());
        }
        Ok(o)
    }
}

fn print_help() {
    println!(
        "\
roog-compat -- does roog run on the machines it claims to?

USAGE
    roog-compat [--dir DIR]          the dashboard (needs a terminal)
    roog-compat --watch              the dashboard, re-reading while the
                                     matrix runs in another terminal
    roog-compat report [--dir DIR]   plain text; no UI, for CI and pipes
    roog-compat gate                 one line and an exit code: 1 if any
                                     machine stopped running roog

OPTIONS
    --dir <DIR>   where the result files are   [default: target/compat]
    --watch       re-read every 500ms
    -h, --help    this

WHERE THE NUMBERS COME FROM
    ./compat/cross_build.sh          builds the game for every machine,
                                     and reports the size and the blame
    ./compat/stress_test_matrix.sh   runs it under Docker with each
                                     machine's CPU and memory limits
    ./compat/nostd_check.sh          the bare-metal rows: particle-core
                                     compiled for microcontrollers

    or ./compat_test.sh to do all three and open this.

WHAT IS BEING JUDGED
    roog, under the game's own load: a real dungeon floor animated by the
    batches the game actually queues. The Bad Apple reel is the ceiling --
    two to three orders of magnitude past anything the game produces. It
    is reported for headroom and nothing is ever gated on it.

KEYS (dashboard)
    up/down  select machine    l  graph the other load
    r        reload            q / Esc  quit"
    );
}

// ---------------------------------------------------------------------------
// Plain-text report
// ---------------------------------------------------------------------------

/// The whole matrix as text. This is what `compat_test.sh` prints at the end
/// and what CI keeps, so it has to stand on its own with no colour and no
/// cursor control.
fn report(results: &Results) -> ExitCode {
    let mut out = String::new();
    let w = &mut out;
    let linux = matrix::selected(&[], Some(Class::Linux));
    let bare = matrix::selected(&[], Some(Class::Bare));

    let _ = writeln!(w, "\n=== does roog run here? ===\n");
    if !ui::has_anything(results) {
        let _ = writeln!(
            w,
            "  nothing measured yet in {}.\n\n  ./compat_test.sh    build the matrix, run it, and come back here\n",
            results.dir.display()
        );
        print!("{out}");
        return ExitCode::SUCCESS;
    }

    let _ = writeln!(
        w,
        "  {:<9} {:<7} {:>9} {:>9} {:>9} {:>6} {:>9}  {}",
        "machine", "exec", "binary", "mean", "p99", "drops", "peak rss", "verdict"
    );
    for row in &linux {
        let run = results.run(&row.id, Load::Game);
        let band = run.map(verdict::grade).unwrap_or(Band::Unknown);
        let footprint = results.footprint(&row.id).unwrap_or_default();
        // A skipped or build-only row has a `Run` (so `status_note` below can
        // still explain why) but nothing was ever measured -- its zeroed
        // numeric fields are a placeholder, not a reading of "0 dropped, 0 B
        // peak", and printing them as such next to the verdict would say the
        // opposite of what happened.
        let measured = run.filter(|r| !matches!(r.status, Status::Skipped | Status::BuildOnly));
        let _ = writeln!(
            w,
            "  {:<9} {:<7} {:>9} {:>9} {:>9} {:>6} {:>9}  {}{}",
            row.id,
            exec_word(row.exec),
            cell(footprint.game, ui::bytes),
            measured
                .map(|r| format!("{:.2}ms", r.mean_ms))
                .unwrap_or_else(dash),
            measured
                .map(|r| format!("{:.2}ms", r.p99_ms))
                .unwrap_or_else(dash),
            measured.map(|r| r.dropped.to_string()).unwrap_or_else(dash),
            measured.map(|r| ui::bytes(r.peak_rss)).unwrap_or_else(dash),
            band.label(),
            run.map(ui::status_note).unwrap_or(""),
        );
    }

    let budget = results
        .runs
        .first()
        .map(|r| r.budget_ms)
        .unwrap_or(1000.0 / 30.0);
    let _ = writeln!(
        w,
        "\n  Graded on the game's own load against a {budget:.2} ms frame budget.\n  \
         `mean` is the whole frame: the floor repainted and the motes over it."
    );

    report_machines(w, results, &linux);
    report_what_ran(w, results, &linux);
    report_footprint(w, results, &linux);
    report_ceiling(w, results, &linux);
    report_bare(w, &bare);
    report_failures(w, results, &linux);

    let overall = verdict::overall(&results.runs);
    let _ = writeln!(w, "\n  VERDICT  {}", overall.label());
    let _ = writeln!(w, "           {}", overall.gloss());
    if let Some(worst) = worst_row(results, &linux) {
        let _ = writeln!(w, "           worst machine: {worst}");
    }
    let _ = writeln!(w);
    print!("{out}");
    ExitCode::SUCCESS
}

/// What each row's container was given, beside what it used. This is the part
/// that makes an emulated row honest: a number measured at 0.4 of a core means
/// nothing without the 0.4 printed next to it.
fn report_machines(w: &mut String, results: &Results, linux: &[matrix::Row]) {
    let _ = writeln!(w, "\n  THE MACHINES (what each container was given)\n");
    let _ = writeln!(
        w,
        "  {:<9} {:>5} {:>8} {:>9} {:>9}  {}",
        "machine", "cpus", "memory", "peak cpu", "peak mem", "standing in for"
    );
    for row in linux {
        let series = results.series_for(&row.id, Load::Game);
        let _ = writeln!(
            w,
            "  {:<9} {:>5} {:>8} {:>9} {:>9}  {}",
            row.id,
            row.cpus,
            row.memory,
            series
                .map(|s| format!("{:.0}%", s.peak_cpu()))
                .unwrap_or_else(dash),
            series.map(|s| ui::bytes(s.peak_mem())).unwrap_or_else(dash),
            row.note,
        );
    }
    let _ = writeln!(
        w,
        "\n  peak cpu and peak mem are the cgroup's, read from outside; 100% is one\n  \
         core. On a qemu row they include the emulator, which the in-process\n  \
         numbers above do not -- the gap between the two is the emulation tax."
    );
}

/// The reel, reported and never graded.
fn report_ceiling(w: &mut String, results: &Results, linux: &[matrix::Row]) {
    let reel: Vec<_> = linux
        .iter()
        .filter_map(|row| results.run(&row.id, Load::Reel).map(|r| (row, r)))
        .collect();
    if reel.is_empty() {
        return;
    }
    let _ = writeln!(
        w,
        "\n  THE CEILING (Bad Apple: ~800 motes a frame, nothing is gated on it)\n"
    );
    let _ = writeln!(
        w,
        "  {:<9} {:>9} {:>9} {:>6}  {}",
        "machine", "mean", "p99", "drops", "headroom over the game's load"
    );
    for (row, run) in reel {
        // A ceiling run that did not finish has no numbers, and printing its
        // zeroes as though they were measurements would read as "instant"
        // rather than "never got there" -- which is the opposite of what
        // happened. Expected on the slowest rows, and not a failure: nothing
        // is gated on the reel.
        if run.status != Status::Ok {
            let _ = writeln!(
                w,
                "  {:<9} {:>9} {:>9} {:>6}  {} -- too slow for the reel, which is allowed",
                row.id,
                "-",
                "-",
                "-",
                run.status.name(),
            );
            continue;
        }
        // How much harder the reel is than the game on this same machine.
        // Measured rather than assumed: the ratio is different on a starved
        // container than on a workstation, because the fixed costs do not
        // shrink.
        let headroom = results
            .run(&row.id, Load::Game)
            .filter(|g| g.mean_ms > 0.0 && g.status == Status::Ok)
            .map(|g| format!("{:.0}x the work", run.mean_ms / g.mean_ms))
            .unwrap_or_else(|| "-".to_string());
        let _ = writeln!(
            w,
            "  {:<9} {:>9} {:>9} {:>6}  {}",
            row.id,
            format!("{:.2}ms", run.mean_ms),
            format!("{:.2}ms", run.p99_ms),
            run.dropped,
            headroom,
        );
    }
}

/// The microcontroller rows. They are not a game and never claimed to be.
fn report_bare(w: &mut String, bare: &[matrix::Row]) {
    if bare.is_empty() {
        return;
    }
    let _ = writeln!(
        w,
        "\n  BARE METAL (particle-core only -- compiled, never executed)\n"
    );
    for row in bare {
        let _ = writeln!(w, "  {:<9} {:<32}  {}", row.id, row.target, row.note);
    }
    let _ = writeln!(
        w,
        "\n  These rows prove the particle arithmetic compiles with no operating\n  \
         system under it. They do not run roog: crossterm needs a terminal, and\n  \
         a microcontroller has none. ./compat/nostd_check.sh is what checks them."
    );
}

/// The run itself, as the process inside the container saw it.
///
/// The triple here is read out of the results file rather than off the matrix,
/// so a `results.tsv` copied from a build machine still says what was actually
/// built and run, even if this checkout's `matrix.tsv` has since moved on.
fn report_what_ran(w: &mut String, results: &Results, linux: &[matrix::Row]) {
    let _ = writeln!(w, "\n  WHAT RAN ({})\n", Load::Game.what());
    let _ = writeln!(
        w,
        "  {:<9} {:<32} {:>7} {:>8} {:>7} {:>9}",
        "machine", "triple", "frames", "wall", "fps", "in-proc"
    );
    for row in linux {
        let Some(run) = results.run(&row.id, Load::Game) else {
            continue;
        };
        // A skipped or build-only row never started a container -- there is
        // no "what ran" to report, and a zeroed line here would read as a
        // row that started and instantly died, which did not happen.
        if matches!(run.status, Status::Skipped | Status::BuildOnly) {
            continue;
        }
        let _ = writeln!(
            w,
            "  {:<9} {:<32} {:>7} {:>8} {:>7} {:>9}",
            row.id,
            run.target,
            run.frames,
            format!("{:.1}s", run.wall_s),
            format!("{:.1}", run.fps),
            // One decimal, like the dashboard's CPU pane: under the game's own
            // load a healthy row sits well below 1% of a core, and rounding to
            // whole percent prints "0%" for every machine that is coping.
            format!("{:.1}%", run.cpu_pct),
        );
    }
    let _ = writeln!(
        w,
        "\n  `fps` is paced, so a machine with headroom and one with none both\n  \
         report the target rate -- it only falls once a row is already losing.\n  \
         `in-proc` is CPU as the process saw itself; the cgroup figure above\n  \
         includes qemu where there is qemu."
    );
}

/// How big it came out. The full attribution -- which crate is responsible for
/// which bytes -- is `cross_build.sh`'s Blame stage, and lands in
/// `blame-<machine>.txt` beside these results.
fn report_footprint(w: &mut String, results: &Results, linux: &[matrix::Row]) {
    let footprints: Vec<_> = linux
        .iter()
        .filter_map(|row| results.footprint(&row.id).map(|f| (row, f)))
        .filter(|(_, f)| f.game > 0)
        .collect();
    if footprints.is_empty() {
        return;
    }
    let _ = writeln!(
        w,
        "\n  BINARY FOOTPRINT (what ships, --release, stripped)\n"
    );
    // Exact bytes as well as the rounded figure: this is the table someone
    // diffs against the last run, and "1.8 MiB" is the same string for an
    // 84 KiB spread.
    let _ = writeln!(
        w,
        "  {:<9} {:<32} {:>10} {:>10} {:>10}",
        "machine", "triple", "game", "bytes", "rig"
    );
    for (row, f) in &footprints {
        let _ = writeln!(
            w,
            "  {:<9} {:<32} {:>10} {:>10} {:>10}",
            row.id,
            row.target,
            ui::bytes(f.game),
            f.game,
            ui::bytes(f.rig),
        );
    }
    let _ = writeln!(
        w,
        "\n  `game` is the whole of roog as one static musl binary -- no libc to\n  \
         install, no INTERP segment, nothing to go wrong on a machine with a\n  \
         different distribution on it. `rig` is roog-perf, which is what the\n  \
         container actually executes and is not shipped to anyone.\n  \
         Who is to blame for those bytes: {}/blame-<machine>.txt",
        results.dir.display()
    );
}

/// Where to go when a row did not run cleanly. A failed or timed-out
/// container is the one result that cannot be read off a number, so the
/// report hands over the log -- a skipped or build-only row never started a
/// container in the first place, so there is neither a log worth reading nor
/// one to clean up, and listing it here would send someone chasing nothing.
fn report_failures(w: &mut String, results: &Results, linux: &[matrix::Row]) {
    let broken: Vec<_> = linux
        .iter()
        .filter_map(|row| {
            let run = results.run(&row.id, Load::Game)?;
            match run.status {
                Status::Failed | Status::Timeout => Some((row, run)),
                Status::Ok | Status::Skipped | Status::BuildOnly => None,
            }
        })
        .collect();
    if broken.is_empty() {
        return;
    }
    let _ = writeln!(w, "\n  WHAT TO LOOK AT\n");
    for (row, run) in broken {
        let _ = writeln!(
            w,
            "  {:<9} {:<8}  {}/run-{}-game.log",
            row.id,
            run.status.name(),
            results.dir.display(),
            row.id,
        );
        // The container name is fixed rather than random precisely so that a
        // run killed halfway can still be found by hand.
        let _ = writeln!(
            w,
            "  {:<9} {:<8}  docker rm -f {}   (if it is somehow still up)",
            "",
            "",
            row.container(),
        );
    }
}

fn worst_row(results: &Results, linux: &[matrix::Row]) -> Option<String> {
    linux
        .iter()
        .filter_map(|row| {
            let run = results.run(&row.id, Load::Game)?;
            Some((verdict::grade(run), row.id.clone()))
        })
        .max_by_key(|(band, _)| *band)
        .filter(|(band, _)| *band > Band::Plays)
        .map(|(band, id)| format!("{id} ({})", band.label()))
}

fn exec_word(exec: Exec) -> &'static str {
    match exec {
        Exec::Native => "native",
        Exec::Qemu => "qemu",
        Exec::None => "-",
    }
}

fn cell(value: u64, f: fn(u64) -> String) -> String {
    match value {
        0 => "-".to_string(),
        n => f(n),
    }
}

fn dash() -> String {
    "-".to_string()
}

// ---------------------------------------------------------------------------
// Gate
// ---------------------------------------------------------------------------

/// One line and an exit code, for CI and for `compat_test.sh`'s last stage.
///
/// A machine that got slower does not fail. A machine that stopped running
/// roog does. See [`Band::is_failure`] for why the line is drawn there.
fn gate(results: &Results) -> ExitCode {
    let band = verdict::overall(&results.runs);
    let broken: Vec<&str> = results
        .runs
        .iter()
        .filter(|r| r.load == Load::Game && verdict::grade(r).is_failure())
        .map(|r| r.id.as_str())
        .collect();

    if results.runs.is_empty() {
        println!("roog-compat: nothing measured -- run ./compat_test.sh");
        return ExitCode::FAILURE;
    }
    if broken.is_empty() {
        println!(
            "roog-compat: {} -- roog runs on every machine in the matrix",
            band.label()
        );
        return ExitCode::SUCCESS;
    }
    println!(
        "roog-compat: {} -- roog no longer runs on: {}",
        band.label(),
        broken.join(", ")
    );
    ExitCode::FAILURE
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

fn dashboard(results: Results, opts: &Options) -> ExitCode {
    if !std::io::stdout().is_terminal() {
        eprintln!("roog-compat: stdout is not a terminal; use `roog-compat report`");
        return ExitCode::FAILURE;
    }
    let app = App::new(ui::linux_rows(), results, opts.dir.clone(), opts.watch);
    let mut terminal = ratatui::init();
    let result = dashboard_loop(app, &mut terminal);
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("roog-compat: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dashboard_loop(mut app: App, terminal: &mut ratatui::DefaultTerminal) -> Result<(), String> {
    let mut last_reload = Instant::now();
    loop {
        terminal
            .draw(|f| ui::draw(f, &app))
            .map_err(|e| e.to_string())?;

        // A short poll rather than a blocking read, so `--watch` picks up new
        // rows without needing a keypress to wake it.
        if event::poll(Duration::from_millis(120)).map_err(|e| e.to_string())? {
            let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.prev(),
                KeyCode::Char('l') => app.toggle_load(),
                KeyCode::Char('r') => app.reload(),
                _ => {}
            }
        }

        if app.watching && last_reload.elapsed() >= RELOAD {
            app.reload();
            last_reload = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_is_rejected_where_it_would_do_nothing() {
        let args = vec!["report".to_string(), "--watch".to_string()];
        assert!(Options::parse(&args).is_err());
    }

    #[test]
    fn the_subcommands_parse() {
        let dash = Options::parse(&[]).expect("no args is the dashboard");
        assert_eq!(dash.mode, Mode::Dashboard);
        let rep = Options::parse(&["report".to_string()]).expect("report");
        assert_eq!(rep.mode, Mode::Report);
        let gate = Options::parse(&["gate".to_string()]).expect("gate");
        assert_eq!(gate.mode, Mode::Gate);
    }

    #[test]
    fn an_unknown_flag_is_an_error_rather_than_being_ignored() {
        assert!(Options::parse(&["--densiy".to_string()]).is_err());
    }
}
