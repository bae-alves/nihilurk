//! Watching a redraw workload, as a redraw.
//!
//! The particle dashboard draws through ratatui, which owns the terminal and
//! paints its own diff. That is the right way to watch the particle layer --
//! panes, sparklines, a memory graph -- and the wrong way to watch a *redraw*,
//! because the thing being measured is exactly the part ratatui would be doing
//! instead.
//!
//! So this viewer takes the terminal directly and drives the rig's own
//! [`crate::screen::Screen`] against it: clear, put, diff, emit. What lands on
//! the glass is the same escape-sequence stream the headless run counts bytes
//! of, at the same frame rate, which makes it the honest picture of the
//! workload. The reel loops, so it runs until you stop it.

use std::io::{IsTerminal, Write, stdout};
use std::time::{Duration, Instant};

use ratatui::crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};

use crate::Options;
use crate::scene::{Canvas, Scene};
use crate::screen::{SCREEN_H, SCREEN_W};

/// Enter the alternate screen, run the reel, and put the terminal back however
/// the run ends.
///
/// The guard is not a nicety: this switches the terminal into raw mode, and a
/// run that returned early -- a broken pipe, a too-small window, a `?` on a
/// write -- without undoing that would hand the shell back with no echo and no
/// line editing.
pub fn watch(scene: &mut Scene, opts: &Options) -> Result<(), String> {
    if !stdout().is_terminal() {
        return Err("stdout is not a terminal; use --headless".into());
    }
    // One row past the grid, because the status line is written below it
    // rather than into it -- a line that changed every frame *inside* the grid
    // would be counted as part of the redraw being measured.
    let needed_h = SCREEN_H + 1;
    let (w, h) = size().map_err(|e| e.to_string())?;
    if w < SCREEN_W || h < needed_h {
        return Err(format!(
            "terminal too small: need {SCREEN_W}x{needed_h}, have {w}x{h}"
        ));
    }

    let _guard = TerminalGuard::enter().map_err(|e| e.to_string())?;
    run(scene, opts).map_err(|e| e.to_string())
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide, Clear(ClearType::All))?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Nothing here can be allowed to fail: this runs on the way out of a
        // panic as well as a clean exit, and a terminal left in raw mode is a
        // worse outcome than a swallowed error.
        let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn run(scene: &mut Scene, opts: &Options) -> std::io::Result<()> {
    let frame_dur = Duration::from_secs_f64(1.0 / opts.fps);
    let dt_ms = (1000.0 / opts.fps) as f32;
    // The viewer never paints into it -- `Scene` owns the grid for these two
    // workloads -- but `tick` takes one for the particles path.
    let mut canvas = Canvas::new();
    let mut next_frame = Instant::now();
    let mut worst_ms = 0.0f64;
    let mut frames = 0u64;

    loop {
        let work_start = Instant::now();
        let phases = crate::tick(scene, &mut canvas, dt_ms);
        let work_ms = work_start.elapsed().as_secs_f64() * 1000.0;
        worst_ms = worst_ms.max(work_ms);
        frames += 1;

        status(scene, opts, phases.flush_ns, work_ms, worst_ms)?;

        // Same bound `headless` and the dashboard stop at. Left out, this loop
        // only ever ends on a quit key -- "the reel loops, so it runs until you
        // stop it" above is right for a human at a keyboard, and wrong for
        // compat/'s screen check, which drives this over a detached container
        // with nothing able to press `q`.
        if let Some(limit) = opts.frames {
            if frames >= limit {
                return Ok(());
            }
        }

        next_frame += frame_dur;
        let now = Instant::now();
        // Overran: resynchronise rather than trying to catch up, which only
        // digs deeper.
        let wait = match now < next_frame {
            true => next_frame - now,
            false => {
                next_frame = now;
                Duration::ZERO
            }
        };
        if quit_requested(wait)? {
            return Ok(());
        }
    }
}

/// One line under the reel, written straight to the terminal rather than
/// through the grid -- the grid is the thing being measured, and a status line
/// changing every frame inside it would be counted as part of the redraw.
fn status(
    scene: &Scene,
    opts: &Options,
    flush_ns: u64,
    work_ms: f64,
    worst_ms: f64,
) -> std::io::Result<()> {
    let budget = 1000.0 / opts.fps;
    let mut out = stdout();
    execute!(out, ratatui::crossterm::cursor::MoveTo(0, SCREEN_H))?;
    write!(
        out,
        " {} · frame {}/{} · {:.2} ms work ({:.0}% of {:.1} ms) · worst {:.2} · \
         flush {:.2} ms · {} cells, {} B · {} live · q to quit  ",
        scene.workload.name(),
        scene.frame_index() % scene.reel().len().max(1),
        scene.reel().len(),
        work_ms,
        work_ms / budget * 100.0,
        budget,
        worst_ms,
        flush_ns as f64 / 1e6,
        scene.last_cells,
        scene.last_frame_bytes(),
        crate::ui::count(scene.fx.live.len() as u64),
    )?;
    out.flush()
}

/// Poll for up to `budget`. `true` means the user asked to stop.
fn quit_requested(budget: Duration) -> std::io::Result<bool> {
    let deadline = Instant::now() + budget;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if !event::poll(left)? {
            return Ok(false);
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        // Raw mode swallows SIGINT, so Ctrl-C has to be caught by hand or the
        // viewer cannot be interrupted the way every other terminal program can.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
    }
}
