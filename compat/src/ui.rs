//! The dashboard: one row per machine, the verdict beside it, and what Docker
//! saw the selected container doing while it ran.
//!
//! Pure, like `roog-perf`'s. Everything drawn here comes out of an [`App`]
//! that was filled in before `draw` was called -- no widget reads a file or
//! shells out to Docker. That matters more here than it does in the perf rig:
//! the numbers on this screen were measured inside a container with a tenth of
//! a core, and a dashboard that went and ran `docker stats` on every frame
//! would be spending that machine's budget on watching itself.

use std::path::PathBuf;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

use crate::matrix::{self, Class, Exec};
use crate::results::{Footprint, Load, Results, Run, Series, Status};
use crate::verdict::{self, Band};

/// Smallest terminal the dashboard fits in. Below this it says so rather than
/// drawing a table with the verdict column off the right-hand edge, which is
/// the one column nobody can afford to lose.
pub const MIN_WIDTH: u16 = 92;
pub const MIN_HEIGHT: u16 = 24;

/// Everything the dashboard draws, plus the cursor state that says which row
/// the graphs are showing.
pub struct App {
    /// The Linux rows of the matrix, in table order. Bare-metal rows are not
    /// here: nothing was ever executed for them, so they have no CPU or memory
    /// to draw. They get a line in the report instead.
    pub rows: Vec<matrix::Row>,
    pub results: Results,
    pub selected: usize,
    /// Which load's time series the graphs show. The table always grades the
    /// game load; this only moves the graphs.
    pub load: Load,
    /// Re-reading the result files as they are written.
    pub watching: bool,
    pub dir: PathBuf,
}

impl App {
    pub fn new(rows: Vec<matrix::Row>, results: Results, dir: PathBuf, watching: bool) -> Self {
        Self {
            rows,
            results,
            selected: 0,
            load: Load::Game,
            watching,
            dir,
        }
    }

    pub fn reload(&mut self) {
        self.results = Results::load(&self.dir);
    }

    pub fn next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.rows.len();
    }

    pub fn prev(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = (self.selected + self.rows.len() - 1) % self.rows.len();
    }

    /// Flip the graphs between the game load and the reel.
    pub fn toggle_load(&mut self) {
        self.load = match self.load {
            Load::Game => Load::Reel,
            Load::Reel => Load::Game,
        };
    }

    fn current(&self) -> Option<&matrix::Row> {
        self.rows.get(self.selected)
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(f, area);
        return;
    }

    // The table takes the height it needs -- one line per machine plus a
    // header -- and everything left over goes to the graphs, which are the
    // part that benefits from more room.
    let table_height = app.rows.len() as u16 + 4;
    let [top, middle, bottom] = Layout::vertical([
        Constraint::Length(table_height),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .areas(area);

    draw_table(f, top, app);
    draw_graphs(f, middle, app);
    draw_footer(f, bottom, app);
}

fn draw_too_small(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("terminal too small"),
        Line::from(format!(
            "need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        )),
        Line::from(""),
        Line::from("resize, or run: roog-compat report"),
    ];
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" roog-compat "),
            ),
        area,
    );
}

// ---------------------------------------------------------------------------
// The matrix table
// ---------------------------------------------------------------------------

/// One line per machine: how big the binary is, how it ran, and the verdict.
///
/// The verdict column is always the game load, whatever the graphs below are
/// showing. Grading on the reel would fail rows that play roog perfectly well;
/// see [`crate::verdict`].
fn draw_table(f: &mut Frame, area: Rect, app: &App) {
    let inner = titled(f, area, " does roog run here? ", Color::Cyan);
    let mut lines = vec![
        header_line(),
        Line::from(Span::styled(
            "-".repeat(inner.width as usize),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    for (i, row) in app.rows.iter().enumerate() {
        lines.push(row_line(app, row, i == app.selected));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn header_line() -> Line<'static> {
    let head = format!(
        "{:<9} {:<9} {:>8} {:>8} {:>8} {:>6} {:>8}  {}",
        "machine", "exec", "binary", "mean", "p99", "drops", "peak rss", "verdict"
    );
    Line::from(Span::styled(
        head,
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    ))
}

fn row_line(app: &App, row: &matrix::Row, selected: bool) -> Line<'static> {
    let run = app.results.run(&row.id, Load::Game);
    let band = run.map(verdict::grade).unwrap_or(Band::Unknown);
    let footprint = app.results.footprint(&row.id).unwrap_or_default();
    let marker = match selected {
        true => ">",
        false => " ",
    };
    let cells = format!(
        "{marker}{:<8} {:<9} {:>8} {:>8} {:>8} {:>6} {:>8}  ",
        row.id,
        exec_label(row.exec),
        size_cell(footprint),
        ms_cell(run.map(|r| r.mean_ms)),
        ms_cell(run.map(|r| r.p99_ms)),
        drops_cell(run),
        rss_cell(run),
    );
    let style = match selected {
        true => Style::default().add_modifier(Modifier::BOLD),
        false => Style::default(),
    };
    Line::from(vec![
        Span::styled(cells, style.fg(Color::White)),
        Span::styled(
            band.label().to_string(),
            Style::default()
                .fg(band_color(band))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            memory_flag(app, row, run),
            Style::default().fg(Color::Yellow),
        ),
    ])
}

/// A `!` beside the verdict when the run finished but came close to the cap.
/// It passed, so it is not a failure; it is also one floor away from not
/// passing, which is worth a character.
fn memory_flag(app: &App, row: &matrix::Row, run: Option<&Run>) -> String {
    let Some(run) = run else {
        return String::new();
    };
    let limit = app
        .results
        .series_for(&row.id, Load::Game)
        .map(Series::limit)
        .unwrap_or(0);
    if verdict::memory_is_tight(run.peak_rss, limit) {
        return "  ! near the memory cap".to_string();
    }
    String::new()
}

fn exec_label(exec: Exec) -> &'static str {
    match exec {
        Exec::Native => "native",
        Exec::Qemu => "qemu",
        Exec::None => "-",
    }
}

fn size_cell(f: Footprint) -> String {
    match f.game {
        0 => "-".to_string(),
        n => bytes(n),
    }
}

fn ms_cell(ms: Option<f64>) -> String {
    match ms {
        Some(v) => format!("{v:.2}ms"),
        None => "-".to_string(),
    }
}

fn drops_cell(run: Option<&Run>) -> String {
    match run {
        Some(r) => r.dropped.to_string(),
        None => "-".to_string(),
    }
}

fn rss_cell(run: Option<&Run>) -> String {
    match run {
        Some(r) => bytes(r.peak_rss),
        None => "-".to_string(),
    }
}

fn band_color(band: Band) -> Color {
    match band {
        Band::Plays => Color::LightGreen,
        Band::Playable => Color::Green,
        Band::Janky => Color::Yellow,
        Band::Unplayable => Color::LightRed,
        Band::DoesNotRun => Color::Red,
        Band::Unknown => Color::DarkGray,
    }
}

// ---------------------------------------------------------------------------
// The graphs
// ---------------------------------------------------------------------------

/// CPU and memory for the selected container, as Docker saw them from outside.
///
/// Outside is the important word. `roog-perf` reports RSS and CPU as the
/// *process* sees itself; these are the cgroup's numbers, which include the
/// emulator on a qemu row. Where the two disagree, the gap is the tax.
fn draw_graphs(f: &mut Frame, area: Rect, app: &App) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);
    let Some(row) = app.current() else {
        f.render_widget(
            Paragraph::new("no Linux rows in the matrix")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    };
    let series = app.results.series_for(&row.id, app.load);
    draw_cpu(f, left, app, row, series);
    draw_memory(f, right, app, row, series);
}

fn draw_cpu(f: &mut Frame, area: Rect, app: &App, row: &matrix::Row, series: Option<&Series>) {
    let title = format!(" cpu -- {} under {} ", row.id, app.load.name());
    let inner = titled(f, area, &title, Color::Magenta);
    let [text, graph] = Layout::vertical([Constraint::Length(4), Constraint::Min(1)]).areas(inner);

    let Some(series) = series.filter(|s| !s.samples.is_empty()) else {
        f.render_widget(no_samples(app), inner);
        return;
    };
    // The cap is what the row says the machine has; the peak is what the
    // container actually took. A peak that sits on the cap is a row that is
    // CPU-bound, which is the honest reading of "this hardware is too slow".
    f.render_widget(
        Paragraph::new(vec![
            kv("budget", &format!("{} cpu", row.cpus), Color::DarkGray),
            // One decimal place: a paced game run on a machine with headroom
            // sits near 1% of a core, and rounding that to a whole number
            // reports the same "1%" for a row using a tenth of what another
            // one is using.
            kv(
                "peak",
                &format!("{:.1}%", series.peak_cpu()),
                Color::Magenta,
            ),
            kv(
                "last",
                &format!("{:.1}%", series.last().map(|s| s.cpu_pct).unwrap_or(0.0)),
                Color::White,
            ),
            Line::from(Span::styled(
                format!("  100% is one core, over {:.0}s", sampled_for(series)),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ]),
        text,
    );
    f.render_widget(
        Sparkline::default()
            .data(series.cpu_line())
            .style(Style::default().fg(Color::Magenta)),
        graph,
    );
}

fn draw_memory(f: &mut Frame, area: Rect, app: &App, row: &matrix::Row, series: Option<&Series>) {
    let title = format!(" memory -- {} of {} ", row.id, row.memory);
    let inner = titled(f, area, &title, Color::Cyan);
    let [text, gauge_area, graph] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);

    let Some(series) = series.filter(|s| !s.samples.is_empty()) else {
        f.render_widget(no_samples(app), inner);
        return;
    };
    let peak = series.peak_mem();
    let limit = series.limit();
    f.render_widget(
        Paragraph::new(vec![
            kv("cap", &bytes(limit), Color::DarkGray),
            kv("peak", &bytes(peak), Color::Cyan),
            kv(
                "last",
                &bytes(series.last().map(|s| s.mem_bytes).unwrap_or(0)),
                Color::White,
            ),
        ]),
        text,
    );
    // Against the cap rather than against the peak: the question is how much
    // of this machine's RAM roog needs, and a gauge normalised to its own peak
    // would read 100% on every row.
    let ratio = match limit {
        0 => 0.0,
        n => (peak as f64 / n as f64).clamp(0.0, 1.0),
    };
    f.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(format!("{:.1}% of cap", ratio * 100.0))
            .gauge_style(Style::default().fg(gauge_color(ratio))),
        gauge_area,
    );
    f.render_widget(
        Sparkline::default()
            .data(series.mem_line())
            .style(Style::default().fg(Color::Cyan)),
        graph,
    );
}

/// How long the container was sampled for, in seconds. Read off the last
/// sample rather than counted, so a dropped sample does not shorten it.
fn sampled_for(series: &Series) -> f64 {
    series.samples.last().map(|s| s.elapsed_s).unwrap_or(0.0)
}

fn gauge_color(ratio: f64) -> Color {
    if ratio > 0.90 {
        return Color::Red;
    }
    if ratio > 0.60 {
        return Color::Yellow;
    }
    Color::Green
}

/// What to say where a graph would be when nothing has been recorded yet.
/// Different advice depending on whether the run is still going.
fn no_samples(app: &App) -> Paragraph<'static> {
    let hint = match app.watching {
        true => "waiting for the container to start...",
        false => "no samples: run ./compat/stress_test_matrix.sh",
    };
    Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
    ])
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let inner = titled(f, area, keys(), Color::DarkGray);
    let run = app
        .current()
        .and_then(|row| app.results.run(&row.id, Load::Game));
    let band = run.map(verdict::grade).unwrap_or(Band::Unknown);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{}  ", band.label()),
                Style::default()
                    .fg(band_color(band))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(band.gloss(), Style::default().fg(Color::Gray)),
            Span::styled(where_it_ran(app), Style::default().fg(Color::DarkGray)),
        ])),
        inner,
    );
}

/// The selected row's container, spelled out: what roog was built for, what
/// Docker was told to pretend to be, and the image it ran in. A row that is
/// mysteriously fast or mysteriously dead is usually explained by one of these
/// three, and nothing else on the screen says them.
fn where_it_ran(app: &App) -> String {
    let Some(row) = app.current() else {
        return String::new();
    };
    format!("   {} on {} in {}", row.note, row.platform, row.image)
}

/// The key hints, drawn into the footer's border so they cost no rows.
pub fn keys() -> &'static str {
    " up/down machine   l load   r reload   q quit "
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render a titled block and hand back the space inside it.
fn titled(f: &mut Frame, area: Rect, title: &str, color: Color) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(title.to_string(), Style::default().fg(color)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

fn kv<'a>(key: &'a str, value: &str, color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<9}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{n} B"),
        _ => format!("{value:.1} {}", UNITS[unit]),
    }
}

/// Rows the dashboard can draw: the ones that were actually executed.
pub fn linux_rows() -> Vec<matrix::Row> {
    matrix::selected(&[], Some(Class::Linux))
}

/// Whether a set of results has anything worth drawing, so the caller can
/// print advice instead of an empty grid.
pub fn has_anything(results: &Results) -> bool {
    !results.is_empty()
}

/// Status text for one run, for the plain-text report.
pub fn status_note(run: &Run) -> &'static str {
    match run.status {
        Status::Ok => "",
        Status::Failed => {
            "  (container failed -- check the log; on a capped row this is usually OOM)"
        }
        Status::Timeout => "  (killed by the runner's clock -- too slow to finish)",
        Status::Skipped => "  (not attempted)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Run, Sample, Status};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Draw the whole dashboard into an off-screen buffer.
    ///
    /// A TUI's failure mode is a panic from a layout that does not fit, and it
    /// happens on someone else's terminal rather than on the one it was
    /// written on. Rendering against `TestBackend` is what makes the draw path
    /// exercised by `cargo test` instead of by a person resizing a window.
    fn render(app: &App, width: u16, height: u16) -> Terminal<TestBackend> {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("test backend starts");
        terminal.draw(|f| draw(f, app)).expect("a frame is drawn");
        terminal
    }

    fn run(id: &str, mean_ms: f64) -> Run {
        Run {
            id: id.to_string(),
            target: "armv7-unknown-linux-musleabihf".into(),
            load: Load::Game,
            status: Status::Ok,
            frames: 450,
            mean_ms,
            p99_ms: mean_ms * 2.0,
            dropped: 0,
            fps: 29.5,
            peak_rss: 10 * 1024 * 1024,
            cpu_pct: 61.0,
            wall_s: 20.0,
            budget_ms: 1000.0 / 30.0,
        }
    }

    fn app_with_results() -> App {
        let rows = linux_rows();
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let series = Series {
            samples: (0..20)
                .map(|i| Sample {
                    elapsed_s: i as f64,
                    cpu_pct: 40.0 + i as f64,
                    mem_bytes: 8_000_000 + i * 100_000,
                    mem_limit: 512 * 1024 * 1024,
                })
                .collect(),
        };
        let results = Results {
            runs: ids.iter().map(|id| run(id, 12.0)).collect(),
            series: ids
                .iter()
                .map(|id| (id.clone(), Load::Game, series.clone()))
                .collect(),
            footprints: ids
                .iter()
                .map(|id| {
                    (
                        id.clone(),
                        Footprint {
                            game: 1_800_000,
                            rig: 1_600_000,
                        },
                    )
                })
                .collect(),
            dir: PathBuf::from("target/compat"),
        };
        App::new(rows, results, PathBuf::from("target/compat"), false)
    }

    #[test]
    fn the_dashboard_draws_at_its_minimum_size() {
        render(&app_with_results(), MIN_WIDTH, MIN_HEIGHT);
    }

    #[test]
    fn the_dashboard_draws_on_a_large_terminal() {
        render(&app_with_results(), 200, 60);
    }

    #[test]
    fn a_terminal_below_the_minimum_says_so_instead_of_panicking() {
        // The row that matters is the verdict, and it is the first thing off
        // the right-hand edge, so a cramped terminal gets a message rather
        // than a table with its conclusion cut off.
        let terminal = render(&app_with_results(), 40, 10);
        let rendered = terminal.backend().to_string();
        assert!(rendered.contains("too small"), "got:\n{rendered}");
    }

    #[test]
    fn it_draws_before_anything_has_been_measured() {
        // This is the state the dashboard is in when it is opened alongside a
        // matrix run that has not produced a row yet, so it must not be the
        // one case that was never drawn.
        let app = App::new(
            linux_rows(),
            Results::default(),
            PathBuf::from("target/compat"),
            true,
        );
        let terminal = render(&app, 120, 40);
        assert!(terminal.backend().to_string().contains("waiting"));
    }

    #[test]
    fn every_verdict_band_has_a_colour_and_a_label() {
        // A band added without a colour would draw in the terminal's default
        // and read as a different kind of result than it is.
        for band in [
            Band::Plays,
            Band::Playable,
            Band::Janky,
            Band::Unplayable,
            Band::DoesNotRun,
            Band::Unknown,
        ] {
            assert_ne!(band_color(band), Color::Reset, "{:?}", band);
            assert!(!band.label().is_empty());
        }
    }

    #[test]
    fn the_cursor_wraps_in_both_directions() {
        let mut app = app_with_results();
        let last = app.rows.len() - 1;
        app.prev();
        assert_eq!(
            app.selected, last,
            "up from the first row wraps to the last"
        );
        app.next();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn the_cursor_survives_an_empty_matrix() {
        let mut app = App::new(Vec::new(), Results::default(), PathBuf::new(), false);
        app.next();
        app.prev();
        assert_eq!(app.selected, 0);
        render(&app, 120, 40);
    }
}
