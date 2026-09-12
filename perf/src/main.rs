//! roog-perf -- a visual stress test for roog's particle layer, and the tools
//! to read the results without leaving the terminal.
//!
//!     roog-perf                     the dashboard
//!     roog-perf --headless          no UI; numbers on stdout, for CI and perf
//!     roog-perf flame profile.txt   a flamegraph, drawn in the terminal
//!
//! The same binary serves all three so that the thing being profiled and the
//! thing being watched are provably the same code. `perf_test.sh` records the
//! headless run and feeds the result back to `flame`.

mod alloc;
mod flame;
mod frames;
mod game;
mod sampler;
mod scene;
mod screen;
mod ui;
mod viewer;

use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use models::BlastPalette;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::frames::Reel;
use crate::game::Turns;
use crate::scene::{Canvas, Phases, Scene, Workload, Y_OFFSET};
use crate::ui::Metrics;

/// Counting wrapper over the system allocator. Installed process-wide, so the
/// heap numbers on the dashboard cover everything -- the particle layer, the
/// UI, and the rig's own bookkeeping alike. See [`alloc`] for why RSS alone
/// would not do.
#[global_allocator]
static ALLOC: alloc::Tracking<std::alloc::System> = alloc::Tracking::new(std::alloc::System);

/// Where to look for a reel when `--reel` is not given, in order. The reel
/// lives at `perf/bad-apple`, so the rig works both from the workspace root
/// (`cargo run -p roog-perf`) and from inside `perf/`. The underscored
/// spellings are accepted too, since that is how the file is sometimes named
/// upstream.
const DEFAULT_REELS: [&str; 4] = ["perf/bad-apple", "bad-apple", "perf/bad_apple", "bad_apple"];

/// Frames a headless run measures unless told otherwise. Fifteen seconds of
/// reel at 30 fps, and past it the phase shares and the per-frame costs stop
/// moving -- a full 6572-frame pass reports the same numbers and takes 3m39s
/// to do it. `--full` is there when you want the whole video anyway.
const DEFAULT_FRAMES: u64 = 450;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if args.first().is_some_and(|a| a == "flame") {
        return run_flame(&args[1..]);
    }

    let opts = match Options::parse(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("roog-perf: {e}");
            return ExitCode::FAILURE;
        }
    };
    match run_stress(opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("roog-perf: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Where a run's load comes from.
///
/// Two different questions, and a pipeline wants both. The reel drives the
/// particle layer two to three orders of magnitude past anything roog produces,
/// which is how you find the cliff. The game drives it with exactly what roog
/// produces -- one batch of real constructors per turn over a real floor --
/// which is how you find out whether a machine can play it.
///
/// On a desktop the reel is the interesting one and the game load is 0.1 ms of
/// a 33 ms budget. On a Pi Zero it is the other way around: the reel is a
/// foregone conclusion and the game load is the entire question. `compat/`
/// gates on `Game` and keeps `Reel` as the ceiling.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadKind {
    Reel,
    Game,
}

struct Options {
    load: LoadKind,
    /// Seed the `--load game` floor is generated from. Fixed by default: two
    /// machines in the compat matrix must measure the same dungeon, because a
    /// floor with more rooms in it is a floor with more cells to repaint.
    seed: u64,
    reel: Option<PathBuf>,
    fps: f64,
    density: u32,
    duration: Option<Duration>,
    /// Stop after this many frames. Frame-bounded rather than time-bounded is
    /// the reproducible way to measure: a 15-second run renders a different
    /// number of frames on a fast machine than a slow one, and then the totals
    /// underneath it are not comparable. 450 frames is one run either way at
    /// 30 fps, and it is enough -- past that the numbers stop moving.
    frames: Option<u64>,
    /// Which of the three workloads to run. `None` means all of them, one
    /// after another, off a single parse of the reel.
    workload: Option<Workload>,
    /// Run the whole reel rather than [`DEFAULT_FRAMES`] of it. Resolved
    /// against the reel's actual length once it is loaded.
    full: bool,
    headless: bool,
    /// Run frames back to back instead of sleeping out the frame budget.
    ///
    /// This is for `perf record` and nothing else. A paced run is asleep for
    /// ~99% of its wall clock at in-range densities, and a sampling profiler
    /// only sees a running process, so a paced recording is mostly `Reel::load`
    /// with a handful of samples in the layer under test. Unpaced, every
    /// sample lands in the frame loop. The per-frame work is identical either
    /// way -- `dt_ms` is a constant, not a measured delta -- so the simulation
    /// is the same one; only the idle time between frames is gone.
    flat_out: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            load: LoadKind::Reel,
            seed: game::DEFAULT_SEED,
            reel: None,
            fps: 30.0,
            density: 1,
            duration: None,
            frames: None,
            workload: Some(Workload::Particles),
            full: false,
            headless: false,
            flat_out: false,
        }
    }
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Options::default();
        let mut density_given = false;
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--headless" => o.headless = true,
                "--flat-out" => o.flat_out = true,
                "--full" => o.full = true,
                "--frames" => {
                    o.frames = Some(
                        next(&mut it, "--frames")?
                            .parse()
                            .map_err(|_| "bad --frames")?,
                    );
                }
                "--workload" => {
                    let name = next(&mut it, "--workload")?;
                    o.workload = match name {
                        "all" => None,
                        other => Some(Workload::parse(other)?),
                    };
                }
                "--reel" => o.reel = Some(PathBuf::from(next(&mut it, "--reel")?)),
                "--load" => {
                    o.load = match next(&mut it, "--load")? {
                        "reel" => LoadKind::Reel,
                        "game" => LoadKind::Game,
                        other => return Err(format!("unknown load `{other}` (reel | game)")),
                    };
                }
                "--seed" => {
                    o.seed = next(&mut it, "--seed")?.parse().map_err(|_| "bad --seed")?;
                }
                "--fps" => o.fps = next(&mut it, "--fps")?.parse().map_err(|_| "bad --fps")?,
                "--density" => {
                    o.density = next(&mut it, "--density")?
                        .parse()
                        .map_err(|_| "bad --density")?;
                    density_given = true;
                }
                "--duration" => {
                    let secs: f64 = next(&mut it, "--duration")?
                        .parse()
                        .map_err(|_| "bad --duration")?;
                    o.duration = Some(Duration::from_secs_f64(secs));
                }
                other => return Err(format!("unknown argument `{other}` (try --help)")),
            }
        }
        if o.fps <= 0.0 {
            return Err("--fps must be positive".into());
        }
        // `Scene` floors this at 1 anyway; clamping here as well keeps the
        // report honest, since it prints `opts.density` and would otherwise
        // announce a density of 0 while running at 1.
        o.density = o.density.max(1);
        if o.flat_out && !o.headless {
            return Err("--flat-out only applies to --headless".into());
        }
        if o.workload.is_none() && !o.headless {
            return Err("--workload all only applies to --headless".into());
        }
        // Both of these would otherwise be silently ignored, and a dial that
        // does nothing is worse than one that is not there: someone would quote
        // a "x8 game load" figure that was measured at x1.
        if o.load == LoadKind::Game && density_given {
            return Err("--density applies to --load reel; the game queues what it queues".into());
        }
        if o.load == LoadKind::Game && o.full {
            return Err("--full applies to --load reel; --load game has no end".into());
        }
        // A frame count is the default bound for a measured run; a duration,
        // when given, is what `perf record` wants and governs instead. Both
        // given means whichever lands first.
        if o.headless && !o.full && o.frames.is_none() && o.duration.is_none() {
            o.frames = Some(DEFAULT_FRAMES);
        }
        Ok(o)
    }
}

fn next<'a>(it: &mut impl Iterator<Item = &'a String>, flag: &str) -> Result<&'a str, String> {
    it.next()
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn print_help() {
    println!(
        "\
roog-perf -- particle-layer stress test and profile viewer

USAGE
    roog-perf [OPTIONS]              live dashboard (needs a terminal)
    roog-perf --headless [OPTIONS]   measure and print; no UI
    roog-perf flame [FILE]           draw a flamegraph in the terminal

OPTIONS
    --load <WHAT>      reel | game                 [default: reel]
                       `reel` replays Bad Apple: ~800 motes a frame, two to
                       three orders past anything roog produces, which is how
                       you find where the layer stops keeping up.
                       `game` animates a real floor with the batches the game
                       actually queues -- one per turn, played out frame by
                       frame -- which is how you find out whether a machine
                       can play roog. Needs no reel file. See compat/.
    --seed <N>         floor to generate           [--load game only]
    --reel <PATH>      frame reel to replay        [default: perf/bad-apple]
    --fps <N>          target frame rate           [default: 30]
    --density <N>      motes spawned per lit cell  [default: 1]
    --workload <W>     particles | screen | both   [default: particles]
                       `all` runs the three in turn off one parse of the
                       reel and prints a comparison. Headless only.
    --frames <N>       stop after N frames         [default: 450, headless]
    --full             run the whole reel instead  (6572 frames, 3m39s)
    --duration <SECS>  stop after SECS             [default: none; when
                                                   given it bounds the run
                                                   alongside --frames]
    --flat-out         headless only: no frame pacing. Frames run back to
                       back, so a `perf record` over the run samples the
                       particle layer rather than the idle between frames.
    -h, --help         this

FLAME
    Reads folded stacks (`a;b;c 123`) or raw `perf script` output, sniffing
    which. `-` or no argument reads stdin:

        perf script -i perf.data | roog-perf flame -

    --width <N>   columns to draw in   [default: terminal width]
    --depth <N>   deepest stack row    [default: 24]
    --no-color    plain text, for piping to a file

WORKLOADS
    particles   the particle layer alone, into an off-screen canvas
    screen      the base layer painted into the game's 80x25 grid, diffed,
                and turned into escape sequences -- the redraw on its own
    both        the two composed as the game composes them: base layer,
                motes over the top, one diff and flush over the lot

    The base layer is a frame of the reel under `--load reel`, and the
    dungeon floor under `--load game`.

    Watching `screen` or `both` drives a real terminal instead of the
    dashboard, because a redraw is the thing being shown:

        roog-perf --workload screen        the reel, repainted for real

KEYS (dashboard)
    q / Esc  quit      space  pause      + / -  density      b  blast

KEYS (redraw viewer)
    q / Esc / Ctrl-C  quit"
    );
}

// ---------------------------------------------------------------------------
// The stress test
// ---------------------------------------------------------------------------

/// The load a run was built with, ready to hand to one or three scenes.
enum Load {
    Reel(Reel),
    Game(Turns),
}

fn run_stress(opts: Options) -> Result<(), String> {
    let load = build_load(&opts)?;

    // `--workload all` measures the three in turn off one load. For the reel
    // that saves 59 MiB and several seconds; for both loads it is what makes
    // the three reports comparable, since they then differ in nothing but the
    // work being measured.
    let Some(workload) = opts.workload else {
        return headless_all(load, &opts);
    };

    let mut scene = match load {
        Load::Reel(reel) => Scene::new(reel, opts.density, workload),
        Load::Game(turns) => Scene::game(turns, workload),
    };
    if opts.headless {
        return headless(&mut scene, &opts).map(|_| ());
    }
    if workload.redraws() {
        return viewer::watch(&mut scene, &opts);
    }
    dashboard(&mut scene, &opts)
}

/// Parse the reel, or generate a floor -- whichever the run is driven by.
///
/// Note what `--load game` does *not* need: a reel file. That is the property
/// the compat matrix leans on. The floor is generated from a seed, so the
/// game-load run ships as one static binary with nothing mounted beside it,
/// and a container that cannot fit 11 MiB of Bad Apple can still answer the
/// question that matters -- does roog keep up here.
fn build_load(opts: &Options) -> Result<Load, String> {
    if opts.load == LoadKind::Game {
        let turns = Turns::new(opts.seed);
        eprintln!(
            "roog-perf: floor from seed {}, {} cells painted per frame, {} beats per cycle",
            opts.seed,
            turns.floor_cells(),
            game::Beat::CYCLE.len()
        );
        return Ok(Load::Game(turns));
    }
    let path = resolve_reel(opts.reel.as_deref())?;
    let reel = Reel::load(&path, Y_OFFSET).map_err(|e| e.to_string())?;
    eprintln!(
        "roog-perf: {} frames from {}, {:.0} lit cells/frame mean, {} peak",
        reel.len(),
        path.display(),
        reel.mean_ink(),
        reel.peak_ink()
    );
    Ok(Load::Reel(reel))
}

fn resolve_reel(explicit: Option<&std::path::Path>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(format!("no such reel: {}", p.display()));
    }
    for candidate in DEFAULT_REELS {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(format!(
        "no reel found (looked for {}); pass --reel <PATH>",
        DEFAULT_REELS.join(", ")
    ))
}

/// Shared per-frame work, so the headless run and the dashboard measure exactly
/// the same thing and their numbers are comparable.
///
/// `canvas` is only touched by the particles workload; the two that redraw own
/// a grid of their own inside [`Scene`].
pub fn tick(scene: &mut Scene, canvas: &mut Canvas, dt_ms: f32) -> Phases {
    match scene.workload {
        Workload::Particles => {
            let mut phases = scene.step(dt_ms);
            phases.raster_ns = scene.rasterize(canvas);
            phases
        }
        Workload::Screen => scene.step_screen(),
        Workload::Both => scene.step_both(dt_ms),
    }
}

fn dashboard(scene: &mut Scene, opts: &Options) -> Result<(), String> {
    if !std::io::stdout().is_terminal() {
        return Err("stdout is not a terminal; use --headless".into());
    }

    let mut terminal = ratatui::init();
    let result = dashboard_loop(scene, opts, &mut terminal);
    ratatui::restore();
    result
}

fn dashboard_loop(
    scene: &mut Scene,
    opts: &Options,
    terminal: &mut ratatui::DefaultTerminal,
) -> Result<(), String> {
    let mut sampler = sampler::Sampler::spawn();
    let mut canvas = Canvas::new();
    let mut m = Metrics::new(opts.fps, opts.density, scene.reel().len());
    // See `headless`: the reel is already on the heap and is not what this
    // measures.
    m.baseline = alloc::stats();

    let frame_dur = Duration::from_secs_f64(1.0 / opts.fps);
    let dt_ms = (1000.0 / opts.fps) as f32;
    let start = Instant::now();
    let mut next_frame = Instant::now();

    loop {
        let work_start = Instant::now();

        if !m.paused {
            let phases = tick(scene, &mut canvas, dt_ms);
            m.phases = phases;
            m.phase_totals.accumulate(&phases);
            m.frames += 1;
        }

        // Heap counters are read here, in the frame loop, because they are
        // three relaxed atomic loads; RSS and CPU come off the sampler thread.
        m.heap = alloc::stats();
        let sample = sampler.poll();
        m.rss_bytes = sample.rss_bytes;
        m.cpu_pct = sample.cpu_pct;
        m.spawned_total = scene.spawned_total;
        m.peak_live = scene.peak_live;
        m.reel_frame = scene.frame_index();
        m.density = scene.density;

        Metrics::push(&mut m.rss, sample.rss_bytes);
        Metrics::push(&mut m.live_particles, scene.fx.live.len() as u64);

        terminal
            .draw(|f| ui::draw(f, &canvas, &m))
            .map_err(|e| e.to_string())?;

        // Frame time is the work, not the wait: the sleep below is headroom,
        // and counting it would report a perfectly idle rig as fully busy.
        let elapsed = work_start.elapsed();
        m.last_frame_ms = elapsed.as_secs_f64() * 1000.0;
        m.worst_frame_ms = m.worst_frame_ms.max(m.last_frame_ms);
        Metrics::push(&mut m.frame_ms, (m.last_frame_ms * 100.0) as u64);
        m.fps = fps_over(&m.frame_ms, opts.fps);

        if let Some(limit) = opts.duration {
            if start.elapsed() >= limit {
                return Ok(());
            }
        }

        next_frame += frame_dur;
        let now = Instant::now();
        match now < next_frame {
            true => {
                if wait_for_key(next_frame - now, scene, &mut m)? {
                    return Ok(());
                }
            }
            false => {
                // Overran the budget. Count it and resynchronise rather than
                // trying to catch up, which would only dig deeper.
                m.dropped += 1;
                next_frame = now;
                if wait_for_key(Duration::ZERO, scene, &mut m)? {
                    return Ok(());
                }
            }
        }
    }
}

/// Poll for input for up to `budget`. Returns `true` if the user asked to quit.
fn wait_for_key(budget: Duration, scene: &mut Scene, m: &mut Metrics) -> Result<bool, String> {
    let deadline = Instant::now() + budget;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if !event::poll(left).map_err(|e| e.to_string())? {
            return Ok(false);
        }
        let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        // Raw mode swallows SIGINT, so Ctrl-C has to be handled by hand or the
        // dashboard cannot be interrupted the way every other terminal program
        // can.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        // Modified keys are not shortcuts. Without this the terminal's NUL --
        // which arrives as Ctrl-Space, and which a pty emits on EOF -- reads as
        // the pause key and freezes an unattended run on its first frame.
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Char(' ') => m.paused = !m.paused,
            KeyCode::Char('+') | KeyCode::Char('=') => scene.density = scene.density.min(63) + 1,
            KeyCode::Char('-') => scene.density = scene.density.saturating_sub(1).max(1),
            KeyCode::Char('b') => scene.burst(BlastPalette::Fire),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
    }
}

/// Frames per second over the sampled window, from recorded work times. Frames
/// that fit in the budget are charged the budget, since the rig sleeps out the
/// remainder; only frames that overrun drag the number down.
fn fps_over(history: &VecDeque<u64>, target: f64) -> f64 {
    if history.is_empty() {
        return 0.0;
    }
    let budget_hundredths = (100_000.0 / target) as u64;
    let total: u64 = history.iter().map(|&h| h.max(budget_hundredths)).sum();
    let mean_ms = total as f64 / history.len() as f64 / 100.0;
    1000.0 / mean_ms.max(0.0001)
}

// ---------------------------------------------------------------------------
// Headless
// ---------------------------------------------------------------------------

/// The same workload with the UI taken out: what `perf record` profiles and
/// what CI compares run to run. Prints a plain-text report -- no cursor
/// control, no colour, nothing that needs a terminal.
fn headless(scene: &mut Scene, opts: &Options) -> Result<Summary, String> {
    let mut sampler = sampler::Sampler::spawn();
    let mut canvas = Canvas::new();
    // Snapshot the counters *after* the reel is parsed. The reel is several
    // megabytes of `Ink` on the heap and it is not the thing being measured;
    // without subtracting it, it lands in the churn total and makes every mote
    // look an order of magnitude fatter than it is.
    let baseline = alloc::stats();
    let dt_ms = (1000.0 / opts.fps) as f32;
    let frame_dur = Duration::from_secs_f64(1.0 / opts.fps);
    // Two bounds, whichever lands first. The frame count is the default and the
    // reproducible one -- the same 450 frames of reel render on any machine, so
    // the totals underneath are comparable run to run in a way a wall-clock
    // window is not. A duration is what `perf record` wants, and `--full` is
    // the whole video for when 450 frames is not the argument you are making.
    let limit = opts.duration.unwrap_or(Duration::MAX);
    let frame_limit = match (opts.full, opts.frames) {
        (true, _) => scene.reel().len() as u64,
        (false, Some(n)) => n,
        (false, None) => u64::MAX,
    };

    let mut totals = Phases::default();
    let mut samples: Vec<f64> = Vec::new();
    let mut peak_rss = 0u64;
    let mut cpu = 0.0f32;
    let start = Instant::now();
    let mut next_frame = Instant::now();
    let mut dropped = 0u64;

    while start.elapsed() < limit && (samples.len() as u64) < frame_limit {
        let work_start = Instant::now();
        let phases = tick(scene, &mut canvas, dt_ms);
        totals.accumulate(&phases);
        samples.push(work_start.elapsed().as_secs_f64() * 1000.0);

        let s = sampler.poll();
        peak_rss = peak_rss.max(s.rss_bytes);
        cpu = s.cpu_pct;

        // Unpaced: no budget, so nothing to sleep out and nothing to drop.
        if opts.flat_out {
            continue;
        }

        next_frame += frame_dur;
        let now = Instant::now();
        if now >= next_frame {
            dropped += 1;
            next_frame = now;
            continue;
        }
        std::thread::sleep(next_frame - now);
    }

    Ok(report(Report {
        scene,
        opts,
        samples: &samples,
        totals: &totals,
        baseline,
        peak_rss,
        cpu,
        dropped,
        elapsed: start.elapsed(),
    }))
}

/// One row of the `--workload all` comparison.
struct Summary {
    workload: Workload,
    frames: u64,
    mean_ms: f64,
    p99_ms: f64,
    dropped: u64,
    /// Mean nanoseconds per frame in the diff-and-emit phase; zero for a
    /// workload that never touches the grid.
    flush_ns: u64,
    cells_per_frame: f64,
    bytes_per_frame: f64,
    allocs_per_frame: u64,
}

/// Run all three workloads off one parse of the reel and compare them.
///
/// Sharing the reel is not just a saving of 59 MiB and a few seconds. It is
/// what makes the three rows comparable: same frames, same machine, same
/// moment, differing in nothing but the work being measured. Read down the
/// column and the gap between `screen` and `screen+particles` is what the
/// particle layer costs on top of a repaint the game does anyway.
fn headless_all(load: Load, opts: &Options) -> Result<(), String> {
    let mut rows = Vec::new();
    match load {
        Load::Reel(reel) => {
            let mut reel = Some(reel);
            for workload in Workload::ALL {
                let taken = reel.take().expect("reel is put back every iteration");
                let mut scene = Scene::new(taken, opts.density, workload);
                rows.push(headless(&mut scene, opts)?);
                reel = Some(scene.into_reel());
            }
        }
        // The floor is cloned rather than handed on, so all three workloads
        // animate the same dungeon from the same first turn. Regenerating from
        // the seed would give the same floor; cloning removes the "would".
        Load::Game(turns) => {
            for workload in Workload::ALL {
                let mut scene = Scene::game(turns.clone(), workload);
                rows.push(headless(&mut scene, opts)?);
            }
        }
    }
    compare(&rows, opts);
    Ok(())
}

fn compare(rows: &[Summary], opts: &Options) {
    let budget = 1000.0 / opts.fps;
    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(w, "\n=== comparison (budget {budget:.2} ms/frame) ===\n");
    let _ = writeln!(
        w,
        "  {:<18}{:>8}{:>8}{:>8}{:>6}{:>10}{:>10}{:>10}",
        "workload", "mean", "p99", "flush", "drop", "cells/f", "bytes/f", "allocs/f"
    );
    for r in rows {
        let flush = match r.flush_ns {
            0 => "--".to_string(),
            ns => format!("{:.3}", ns as f64 / 1e6 / r.frames.max(1) as f64),
        };
        let cells = match r.cells_per_frame {
            c if c <= 0.0 => "--".to_string(),
            c => format!("{c:.0}"),
        };
        let bytes = match r.bytes_per_frame {
            b if b <= 0.0 => "--".to_string(),
            b => ui::count(b as u64),
        };
        let _ = writeln!(
            w,
            "  {:<18}{:>8.3}{:>8.3}{:>8}{:>6}{:>10}{:>10}{:>10}",
            r.workload.name(),
            r.mean_ms,
            r.p99_ms,
            flush,
            r.dropped,
            cells,
            bytes,
            ui::count(r.allocs_per_frame)
        );
    }
    // The one subtraction worth doing for the reader, since the whole point of
    // running all three is the difference between the last two.
    if let (Some(screen), Some(both)) = (
        rows.iter().find(|r| r.workload == Workload::Screen),
        rows.iter().find(|r| r.workload == Workload::Both),
    ) {
        let delta = both.mean_ms - screen.mean_ms;
        let _ = writeln!(
            w,
            "\n  Particles add {:.3} ms to a redraw that costs {:.3} ms without them\n  \
             -- {:.0}% more work, and {:.1}% of the {budget:.1} ms budget in total.",
            delta,
            screen.mean_ms,
            delta / screen.mean_ms.max(1e-9) * 100.0,
            both.mean_ms / budget * 100.0
        );
    }
    print!("{out}");
    let _ = std::io::stdout().flush();
}

/// Everything the end-of-run report prints, gathered so the printer takes one
/// argument instead of nine.
struct Report<'a> {
    scene: &'a Scene,
    opts: &'a Options,
    samples: &'a [f64],
    totals: &'a Phases,
    baseline: alloc::HeapStats,
    peak_rss: u64,
    cpu: f32,
    dropped: u64,
    elapsed: Duration,
}

fn report(r: Report<'_>) -> Summary {
    let Report {
        scene,
        opts,
        samples,
        totals,
        baseline,
        peak_rss,
        cpu,
        dropped,
        elapsed,
    } = r;
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| {
        sorted
            .get(((sorted.len() as f64 * q) as usize).min(sorted.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0)
    };
    let n = samples.len().max(1);
    let mean: f64 = samples.iter().sum::<f64>() / n as f64;
    let budget = 1000.0 / opts.fps;
    let heap = alloc::stats();
    // Run-only figures: everything the reel cost at startup is subtracted, so
    // what is left is the particle layer and the frame loop around it.
    let run_allocs = heap.total_allocs - baseline.total_allocs;
    let run_bytes = heap.total_bytes - baseline.total_bytes;

    let mut out = String::new();
    let w = &mut out;
    let _ = writeln!(
        w,
        "\n=== roog-perf: {} under {} ===\n",
        scene.workload.name(),
        match opts.load {
            LoadKind::Reel => "Bad Apple",
            LoadKind::Game => "the game's own load",
        }
    );
    // Two loads, two things worth saying about the source. The reel's size is
    // its frame count and ink density; the floor's is how much of the screen it
    // paints and how many turns went by.
    match scene.turns() {
        Some(turns) => {
            let _ = writeln!(
                w,
                "  floor         seed {}, {} cells painted per frame",
                opts.seed,
                scene.reel().mean_ink() as u64
            );
            let _ = writeln!(
                w,
                "  load          {:.0} fps target, {} turns animated, {} motes queued",
                opts.fps,
                ui::count(turns.turns),
                ui::count(turns.spawned)
            );
            let cycle: Vec<&str> = game::Beat::CYCLE.iter().map(|b| b.name()).collect();
            let _ = writeln!(w, "  turn cycle    {}", cycle.join(" "));
        }
        None => {
            let _ = writeln!(
                w,
                "  reel          {} frames, {:.0} lit cells/frame mean, {} peak",
                scene.reel().len(),
                scene.reel().mean_ink(),
                scene.reel().peak_ink()
            );
            let _ = writeln!(
                w,
                "  load          {:.0} fps target x{} density -> {} motes/frame peak",
                opts.fps,
                opts.density,
                ui::count(scene.reel().peak_ink() as u64 * opts.density as u64)
            );
        }
    }
    // An unpaced run's fps is "how fast could it go", not "did it keep up",
    // and the two must never be read off the same line without a label.
    let paced = match opts.flat_out {
        true => ", unpaced",
        false => "",
    };
    let _ = writeln!(
        w,
        "  frames        {n} in {:.1}s wall ({:.1} fps achieved{paced})",
        elapsed.as_secs_f64(),
        n as f64 / elapsed.as_secs_f64().max(1e-9)
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "  FRAME TIME (budget {budget:.2} ms)");
    let _ = writeln!(
        w,
        "    mean        {mean:>8.3} ms   ({:.0}% of budget)",
        mean / budget * 100.0
    );
    let _ = writeln!(w, "    p50         {:>8.3} ms", pick(0.50));
    let _ = writeln!(w, "    p95         {:>8.3} ms", pick(0.95));
    let _ = writeln!(w, "    p99         {:>8.3} ms", pick(0.99));
    let _ = writeln!(
        w,
        "    max         {:>8.3} ms",
        sorted.last().copied().unwrap_or(0.0)
    );
    let _ = writeln!(w, "    dropped     {dropped:>8}");
    let _ = writeln!(w);
    let _ = writeln!(w, "  PHASES (mean per frame)");
    for (name, ns, share) in totals.shares() {
        let ms = ns as f64 / 1e6 / n as f64;
        let bar = "#".repeat((share * 40.0) as usize);
        let _ = writeln!(
            w,
            "    {name:<10}{ms:>8.3} ms  {:>5.1}%  {bar}",
            share * 100.0
        );
    }
    let _ = writeln!(w);
    if scene.workload.redraws() {
        let _ = writeln!(w, "  REDRAW (per frame)");
        let _ = writeln!(
            w,
            "    cells       {:>10}  of {} on the grid",
            ui::count(scene.cells_drawn / n as u64),
            ui::count(screen::SCREEN_W as u64 * screen::SCREEN_H as u64)
        );
        let _ = writeln!(
            w,
            "    bytes       {:>10}  of escape sequences handed to the terminal",
            ui::count(scene.bytes_written() / n as u64)
        );
        let _ = writeln!(
            w,
            "    per cell    {:>10.1} bytes",
            scene.bytes_written() as f64 / scene.cells_drawn.max(1) as f64
        );
        let _ = writeln!(w);
    }
    // The baseline is whatever the load cost to set up before the clock
    // started -- 59 MiB of parsed reel, or a generated floor and the world it
    // came out of. Neither is particle churn, and counting either would make
    // every mote look several times fatter than it is.
    let source = match opts.load {
        LoadKind::Reel => "reel",
        LoadKind::Game => "floor",
    };
    let _ = writeln!(w, "  MEMORY ({source} baseline subtracted)");
    let _ = writeln!(
        w,
        "    at rest     {:>10}  in {} blocks, the {source} itself, built once",
        ui::bytes(baseline.live_bytes as u64),
        ui::count(baseline.live_blocks as u64)
    );
    // Net of the reel, exactly as the dashboard's heap pane reports it. The
    // gross figure is the reel plus a rounding error and says nothing about
    // the particle layer; the net one tracks the live mote population.
    let _ = writeln!(
        w,
        "    live heap   {:>10}  in {} blocks, net of the {source}, end of run",
        ui::bytes(heap.live_bytes.saturating_sub(baseline.live_bytes) as u64),
        ui::count(heap.live_blocks.saturating_sub(baseline.live_blocks) as u64)
    );
    let _ = writeln!(
        w,
        "    allocations {:>10}  during the run",
        ui::count(run_allocs)
    );
    let _ = writeln!(
        w,
        "    per frame   {:>10}",
        ui::count(run_allocs / n as u64)
    );
    // Meaningless under the game's load: a few dozen motes against a run's
    // worth of frame-loop bookkeeping is a division that reports kilobytes per
    // mote for a mote that costs eight bytes.
    if scene.spawned_total > 0 && opts.load == LoadKind::Reel {
        let _ = writeln!(
            w,
            "    per mote    {:>10.2} bytes",
            run_bytes as f64 / scene.spawned_total as f64
        );
    }
    let _ = writeln!(w, "    churned     {:>10}", ui::bytes(run_bytes));
    let _ = writeln!(
        w,
        "    peak RSS    {:>10}  whole process, {source} included",
        ui::bytes(peak_rss)
    );
    if scene.spawned_total > 0 {
        let _ = writeln!(
            w,
            "    peak live   {:>10} motes",
            ui::count(scene.peak_live as u64)
        );
    }
    let _ = writeln!(w, "    cpu         {cpu:>10.1} %");
    let _ = writeln!(w);
    // The closing note only makes sense where the particle layer is doing the
    // allocating. Under the game's load it is not: most frames queue nothing at
    // all, so `per mote` divides a run's worth of one-off allocations by a
    // handful of motes and reports a number that means nothing. What is worth
    // saying there is the opposite finding -- how little of the frame the
    // layer accounts for.
    if scene.workload != Workload::Screen && opts.load == LoadKind::Reel {
        let _ = writeln!(
            w,
            "  Every mote heap-allocates a keyframe Vec of its own -- that is the\n  \
             `per mote` figure, and it is why `spawn` leads the phase breakdown.\n  \
             At {} motes/frame the layer allocates roughly {} times a second.",
            ui::count(scene.reel().peak_ink() as u64 * opts.density as u64),
            ui::count((run_allocs as f64 / elapsed.as_secs_f64().max(1e-9)) as u64)
        );
    }
    if let Some(turns) = scene.turns() {
        let _ = writeln!(
            w,
            "  {} turns of real animation in {} frames -- {:.1} frames per turn,\n  \
             and {:.0}% of them queued nothing at all. The game's load is mostly\n  \
             the redraw; the motes are what ride on top of it.",
            ui::count(turns.turns),
            ui::count(n as u64),
            n as f64 / turns.turns.max(1) as f64,
            100.0 - (turns.turns as f64 / n as f64 * 100.0)
        );
    }
    print!("{out}");
    let _ = std::io::stdout().flush();

    Summary {
        workload: scene.workload,
        frames: n as u64,
        mean_ms: mean,
        p99_ms: pick(0.99),
        dropped,
        flush_ns: totals.flush_ns,
        cells_per_frame: scene.cells_drawn as f64 / n as f64,
        bytes_per_frame: scene.bytes_written() as f64 / n as f64,
        allocs_per_frame: run_allocs / n as u64,
    }
}

// ---------------------------------------------------------------------------
// flame
// ---------------------------------------------------------------------------

fn run_flame(args: &[String]) -> ExitCode {
    let mut path: Option<String> = None;
    let mut width: Option<usize> = None;
    let mut depth = 24usize;
    let mut color = std::io::stdout().is_terminal();

    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--no-color" => color = false,
            "--width" => width = it.next().and_then(|v| v.parse().ok()),
            "--depth" => depth = it.next().and_then(|v| v.parse().ok()).unwrap_or(depth),
            other => path = Some(other.to_string()),
        }
    }

    let input = match read_input(path.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("roog-perf flame: {e}");
            return ExitCode::FAILURE;
        }
    };

    let profile = flame::Profile::parse(&input);
    if profile.is_empty() {
        eprintln!(
            "roog-perf flame: no samples found.\n\
             Expected folded stacks (`a;b;c 123`) or `perf script` output."
        );
        return ExitCode::FAILURE;
    }

    let width = width.unwrap_or_else(terminal_width);
    println!(
        "\n  {} samples, {} frames deep at most, {} columns\n",
        ui::count(profile.total_samples()),
        depth,
        width
    );
    print!("{}", profile.render(width, depth, color));

    println!("\n  SELF TIME (where the cycles actually went)\n");
    println!("  {:>7}  {:>6}  {:>9}  {}", "self", "%", "total", "frame");
    // The table is the half of this output people actually read, so its labels
    // get the same treatment the bars get -- generics dropped, path trimmed --
    // and are then cut to the columns the three number fields leave behind.
    let room = width.saturating_sub(30).max(20);
    for (name, own, pct, total) in profile.hotspots(15) {
        let label: String = flame::shorten(&name).chars().take(room).collect();
        println!(
            "  {:>7}  {:>5.1}%  {:>9}  {}",
            ui::count(own),
            pct,
            ui::count(total),
            label
        );
    }
    println!();
    ExitCode::SUCCESS
}

fn read_input(path: Option<&str>) -> Result<String, String> {
    let from_stdin = matches!(path, None | Some("-"));
    if from_stdin {
        if std::io::stdin().is_terminal() {
            return Err("nothing on stdin; pass a file or pipe `perf script` in".into());
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| e.to_string())?;
        return Ok(buf);
    }
    let path = path.unwrap_or_default();
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn terminal_width() -> usize {
    ratatui::crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(100)
        .clamp(40, 200)
}
