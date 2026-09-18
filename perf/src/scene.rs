//! The system under test: nihilurk's real [`models::Particles`], driven at video
//! rate off the reel.
//!
//! Nothing is reimplemented here. `Scene::step` calls the same
//! [`models::Particles::blip`] a landed blow calls and the same
//! [`models::Particles::advance`] the engine's post-turn frame loop calls, and
//! `Scene::rasterize` walks `particles.live` exactly the way
//! `engine/src/view.rs` does when it paints the map. If this rig says the
//! particle layer costs X, the game pays X.
//!
//! What it changes is the *load*. A wand bolt queues a few dozen motes; a frame
//! of Bad Apple queues upwards of fifteen hundred, thirty times a second, which
//! is two or three orders of magnitude past anything the game will ever ask
//! for. That is the point -- the interesting number is not whether the layer
//! survives a fireball but where it stops keeping up, and you cannot find that
//! without driving it well past the working range.

use std::time::Instant;

use crossterm::style::Color;
use models::{BlastPalette, MAP_HEIGHT, MAP_WIDTH, Particles};

use crate::frames::Reel;
use crate::game::Turns;
use crate::screen::{Screen, Sink};

/// What a run measures.
///
/// The three are nested on purpose, so their differences are readable: `Screen`
/// is `Both` without the particle layer, and `Both` is `Screen` with it. Put
/// the two reports side by side and the gap between them is exactly what the
/// particles cost on top of a redraw the game was doing anyway.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Workload {
    /// The particle layer alone, painted into an off-screen canvas. No
    /// terminal, no diff -- just `blip`, `advance` and keyframe resolution.
    Particles,
    /// The redraw alone: every lit cell of the reel painted into the
    /// double-buffered grid, then diffed and turned into escape sequences.
    /// Stands in for the game's map-and-actors repaint.
    Screen,
    /// Both, composed the way the game composes them -- the reel painted
    /// first, live motes over the top, then one diff and flush over the lot.
    Both,
}

impl Workload {
    pub fn name(self) -> &'static str {
        match self {
            Workload::Particles => "particles",
            Workload::Screen => "screen",
            Workload::Both => "screen+particles",
        }
    }

    /// Every workload, in the order a report should list them.
    pub const ALL: [Workload; 3] = [Workload::Particles, Workload::Screen, Workload::Both];

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "particles" => Ok(Workload::Particles),
            "screen" => Ok(Workload::Screen),
            "both" => Ok(Workload::Both),
            other => Err(format!(
                "unknown workload `{other}` (particles | screen | both | all)"
            )),
        }
    }

    /// Does this workload touch the terminal grid?
    pub fn redraws(self) -> bool {
        !matches!(self, Workload::Particles)
    }
}

/// Where the 20-row reel sits inside nihilurk's 22-row map.
pub const Y_OFFSET: u16 = 1;

/// Rows above the map on the game's 80x25 grid. `engine/src/view.rs` keeps the
/// status line on row 0 and paints every map cell at `y + 1`; the redraw
/// workloads do the same, so the reel lands in the map rows and the status and
/// log rows stay blank. That matters to the measurement: a frame that repainted
/// all 2000 cells would report a diff cost the game never pays.
pub const STATUS_ROWS: u16 = 1;

/// Per-frame cost, split by phase.
///
/// This is the portable half of the profiling story. `perf` needs a Linux
/// kernel, matching debug symbols and permission to open a performance counter;
/// where any of that is missing, these three numbers still say which phase the
/// budget went to, on any platform, with no tooling at all. They are coarse --
/// three `Instant::now()` pairs, not a sampled call graph -- but they answer
/// the first question you would ask a flamegraph anyway.
#[derive(Clone, Copy, Default)]
pub struct Phases {
    /// [`Particles::advance`]: ageing every live mote and culling the dead.
    pub advance_ns: u64,
    /// [`Particles::blip`] once per lit cell -- the allocation-heavy phase.
    pub spawn_ns: u64,
    /// Walking `live` and resolving each mote's current keyframe into a cell.
    pub raster_ns: u64,
    /// Diffing the grid against what is already on screen and emitting the
    /// escape sequences for the cells that changed. Zero unless the workload
    /// redraws.
    pub flush_ns: u64,
}

impl Phases {
    pub fn total_ns(&self) -> u64 {
        self.advance_ns + self.spawn_ns + self.raster_ns + self.flush_ns
    }

    /// Running sum, for the end-of-run averages.
    pub fn accumulate(&mut self, other: &Phases) {
        self.advance_ns += other.advance_ns;
        self.spawn_ns += other.spawn_ns;
        self.raster_ns += other.raster_ns;
        self.flush_ns += other.flush_ns;
    }

    /// Each phase as a share of the total, for the breakdown bars. Phases a
    /// workload never runs are left out rather than printed as a row of
    /// zeroes: `screen` has no `spawn`, and a bar chart of nothing is noise.
    pub fn shares(&self) -> Vec<(&'static str, u64, f64)> {
        let total = self.total_ns().max(1) as f64;
        [
            ("advance", self.advance_ns),
            ("spawn", self.spawn_ns),
            ("raster", self.raster_ns),
            ("flush", self.flush_ns),
        ]
        .into_iter()
        .filter(|&(_, ns)| ns > 0)
        .map(|(name, ns)| (name, ns, ns as f64 / total))
        .collect()
    }
}

/// A rendered frame of the particle layer: one glyph and colour per map tile,
/// last write winning, which is how `Screen::put` resolves overlap in the game.
pub struct Canvas {
    pub cells: Vec<Option<(char, Color)>>,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            cells: vec![None; MAP_WIDTH as usize * MAP_HEIGHT as usize],
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Option<(char, Color)> {
        self.cells[y as usize * MAP_WIDTH as usize + x as usize]
    }

    fn put(&mut self, x: u16, y: u16, glyph: char, color: Color) {
        self.cells[y as usize * MAP_WIDTH as usize + x as usize] = Some((glyph, color));
    }

    fn clear(&mut self) {
        self.cells.fill(None);
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

/// The reel, the particle layer, and the dials that decide how hard to push.
pub struct Scene {
    pub fx: Particles,
    reel: Reel,
    frame_idx: usize,
    /// Motes spawned per lit cell. The load dial: 1 replays the video, 8 asks
    /// what happens at eight times the particle count.
    pub density: u32,
    /// Every mote spawned since the run began.
    pub spawned_total: u64,
    /// Peak simultaneous live motes.
    pub peak_live: usize,
    /// Which of the three workloads this scene runs.
    pub workload: Workload,
    /// Set when the run is driven by the game's own load rather than the reel:
    /// one batch of real `Particles` constructors per turn, played out frame by
    /// frame until the last mote dies. `None` is the reel, which spawns a mote
    /// per lit cell instead.
    ///
    /// The two are different questions. The reel asks where the layer stops
    /// keeping up; this asks whether the machine can play nihilurk. On a desktop
    /// only the first is interesting. On a Pi Zero only the second is.
    turns: Option<Turns>,
    /// The terminal grid, for the two workloads that redraw.
    screen: Screen,
    sink: Sink,
    /// Cells the flush actually emitted, summed over the run, and the byte
    /// count that went with them.
    pub cells_drawn: u64,
    pub last_cells: usize,
}

impl Scene {
    pub fn new(reel: Reel, density: u32, workload: Workload) -> Self {
        Self {
            fx: Particles::new(),
            reel,
            frame_idx: 0,
            density: density.max(1),
            spawned_total: 0,
            peak_live: 0,
            workload,
            turns: None,
            screen: Screen::new(),
            sink: Sink::new(),
            cells_drawn: 0,
            last_cells: 0,
        }
    }

    /// A scene driven by the game's own load: a real floor as the base layer,
    /// real particle batches one turn at a time.
    ///
    /// `density` has no meaning here and is not taken. The reel's load is a
    /// dial because the reel is synthetic; the game's load is whatever the game
    /// queues, and a "x8 game" would be a measurement of nothing.
    pub fn game(turns: Turns, workload: Workload) -> Self {
        let reel = turns.reel();
        let mut scene = Self::new(reel, 1, workload);
        scene.turns = Some(turns);
        scene
    }

    /// The turn loop, when this scene is running the game's load.
    pub fn turns(&self) -> Option<&Turns> {
        self.turns.as_ref()
    }

    /// Total bytes of escape sequences the run has generated.
    pub fn bytes_written(&self) -> u64 {
        self.sink.total_bytes
    }

    pub fn last_frame_bytes(&self) -> usize {
        self.sink.last_bytes
    }

    pub fn reel(&self) -> &Reel {
        &self.reel
    }

    /// Hand the reel back so the next workload can run off the same parse.
    pub fn into_reel(self) -> Reel {
        self.reel
    }

    pub fn frame_index(&self) -> usize {
        self.frame_idx
    }

    /// Advance the layer by `dt_ms` and queue the next frame of the reel.
    /// Returns the cost of each phase.
    pub fn step(&mut self, dt_ms: f32) -> Phases {
        let mut phases = Phases::default();

        let t0 = Instant::now();
        self.fx.advance(dt_ms);
        phases.advance_ns = t0.elapsed().as_nanos() as u64;

        let t1 = Instant::now();
        let spawned = self.spawn_phase();
        phases.spawn_ns = t1.elapsed().as_nanos() as u64;

        self.spawned_total += spawned;
        self.peak_live = self.peak_live.max(self.fx.live.len());
        self.frame_idx = self.frame_idx.wrapping_add(1);
        phases
    }

    /// Queue this frame's motes, whichever load is driving.
    ///
    /// Both branches are timed as `spawn` and both are the game's own
    /// constructors; what differs is how many and how often. The reel blips
    /// every lit cell every frame. The game queues one batch per turn and then
    /// spends several frames animating it -- so most frames queue nothing at
    /// all, which is itself the finding: on the game's real load the spawn
    /// phase is mostly idle and the frame budget goes on the redraw.
    fn spawn_phase(&mut self) -> u64 {
        if let Some(turns) = &mut self.turns {
            return turns.step(&mut self.fx);
        }
        let frame = &self.reel.frames[self.frame_idx % self.reel.frames.len().max(1)];
        for ink in &frame.ink {
            for _ in 0..self.density {
                self.fx.blip(ink.x, ink.y, ink.glyph, ink.color);
            }
        }
        frame.ink.len() as u64 * self.density as u64
    }

    /// Paint the live motes into `canvas`, mirroring the particle pass in
    /// `engine/src/view.rs` -- including its bounds check, so a mote queued off
    /// the map costs what it costs in the game and then gets dropped.
    pub fn rasterize(&self, canvas: &mut Canvas) -> u64 {
        let t = Instant::now();
        canvas.clear();
        for p in &self.fx.live {
            if p.x >= MAP_WIDTH || p.y >= MAP_HEIGHT {
                continue;
            }
            let Some((glyph, color)) = p.current() else {
                continue;
            };
            canvas.put(p.x, p.y, glyph, color);
        }
        t.elapsed().as_nanos() as u64
    }

    /// Paint the reel's lit cells into the terminal grid, then diff and flush.
    ///
    /// No particle layer at all -- this is the redraw on its own, standing in
    /// for the game's map-and-actors repaint. `Y_OFFSET + 1` puts the reel in
    /// the map rows: the grid's row 0 is the status line, as in the game, and
    /// leaving it blank is what keeps the changed-cell count honest instead of
    /// repainting all 2000 every frame.
    pub fn step_screen(&mut self) -> Phases {
        let mut phases = Phases::default();

        let t0 = Instant::now();
        self.screen.clear();
        let frame = self.reel.frame(self.frame_idx);
        for ink in &frame.ink {
            self.screen
                .put(ink.x, ink.y + STATUS_ROWS, ink.glyph, ink.color);
        }
        phases.raster_ns = t0.elapsed().as_nanos() as u64;

        phases.flush_ns = self.flush_frame();
        self.frame_idx = self.frame_idx.wrapping_add(1);
        phases
    }

    /// The full stack, composed the way `engine/src/view.rs` composes it: the
    /// base layer first, live motes painted over the top, then one diff and
    /// one flush over the lot.
    ///
    /// Read against [`Scene::step_screen`], the difference between the two
    /// reports is what the particle layer costs on top of a repaint the game
    /// performs every frame regardless.
    pub fn step_both(&mut self, dt_ms: f32) -> Phases {
        let t0 = Instant::now();
        self.fx.advance(dt_ms);
        let advance_ns = t0.elapsed().as_nanos() as u64;

        let t1 = Instant::now();
        let spawned = self.spawn_phase();
        let spawn_ns = t1.elapsed().as_nanos() as u64;
        self.spawned_total += spawned;
        self.peak_live = self.peak_live.max(self.fx.live.len());

        let t2 = Instant::now();
        self.screen.clear();
        let frame = self.reel.frame(self.frame_idx);
        for ink in &frame.ink {
            self.screen
                .put(ink.x, ink.y + STATUS_ROWS, ink.glyph, ink.color);
        }
        for p in &self.fx.live {
            if p.x >= MAP_WIDTH || p.y >= MAP_HEIGHT {
                continue;
            }
            let Some((glyph, color)) = p.current() else {
                continue;
            };
            self.screen.put(p.x, p.y + STATUS_ROWS, glyph, color);
        }
        let raster_ns = t2.elapsed().as_nanos() as u64;

        let flush_ns = self.flush_frame();
        self.frame_idx = self.frame_idx.wrapping_add(1);
        Phases {
            advance_ns,
            spawn_ns,
            raster_ns,
            flush_ns,
        }
    }

    /// Diff, emit, and bank the frame's byte count. Returns the nanoseconds it
    /// took. The sink cannot fail -- it is a `Vec` -- so the `Result` is an
    /// artefact of sharing `Write` with the real thing.
    fn flush_frame(&mut self) -> u64 {
        let t = Instant::now();
        let drawn = self.screen.flush(&mut self.sink, (0, 0)).unwrap_or(0);
        let ns = t.elapsed().as_nanos() as u64;
        self.sink.take_frame();
        self.cells_drawn += drawn as u64;
        self.last_cells = drawn;
        ns
    }

    /// Mix in the heavier constructors the reel never reaches on its own: a
    /// full-map blast plus a beam across the diagonal. `blip` allocates a
    /// one-element keyframe `Vec`; [`Particles::explosion`] allocates a
    /// five-element one per cell, so this is the same code path with a
    /// noticeably fatter allocation, and worth being able to trigger on demand
    /// while watching the heap counters.
    pub fn burst(&mut self, palette: BlastPalette) {
        let cx = MAP_WIDTH as f32 / 2.0;
        let cy = MAP_HEIGHT as f32 / 2.0;
        let mut cells = Vec::with_capacity(MAP_WIDTH as usize * MAP_HEIGHT as usize);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let dx = x as f32 - cx;
                let dy = (y as f32 - cy) * 2.0;
                cells.push((x, y, (dx * dx + dy * dy).sqrt() / 4.0));
            }
        }
        self.fx.explosion(&cells, palette);

        let line: Vec<(u16, u16)> = (0..MAP_WIDTH)
            .map(|x| (x, (x * MAP_HEIGHT / MAP_WIDTH).min(MAP_HEIGHT - 1)))
            .collect();
        self.fx.beam(&line, Color::Cyan);
        self.spawned_total += cells.len() as u64 + line.len() as u64;
    }
}
