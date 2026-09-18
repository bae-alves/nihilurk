//! The dashboard: canvas in the middle, memory down the side, runtime metrics
//! along the bottom.
//!
//! Everything here is pure -- it takes a [`Metrics`] snapshot and draws it. No
//! widget reads a clock or a counter for itself, because a dashboard that
//! sampled its own inputs while drawing would be timing the draw and reporting
//! it as the workload.

use std::collections::VecDeque;

use crossterm::style::Color as CtColor;
use models::{MAP_HEIGHT, MAP_WIDTH};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Sparkline};

use crate::alloc::HeapStats;
use crate::scene::{Canvas, Phases};

/// Samples kept for each sparkline. At 30 fps this is about eight seconds of
/// history, which is long enough to see the reel's density swing.
pub const HISTORY: usize = 240;

/// Smallest terminal the full dashboard fits in: 80-column canvas plus its
/// border, plus the sidebar.
pub const MIN_WIDTH: u16 = MAP_WIDTH + 2 + 34;
pub const MIN_HEIGHT: u16 = MAP_HEIGHT + 2 + 11;

/// Everything the dashboard draws, sampled once per frame by the caller.
pub struct Metrics {
    pub frame_ms: VecDeque<u64>,
    pub rss: VecDeque<u64>,
    pub live_particles: VecDeque<u64>,
    pub heap: HeapStats,
    /// Counters as they stood once the reel was parsed and before the first
    /// frame. Subtracted from `heap` so the pane shows what the particle layer
    /// costs, not what loading several megabytes of reel cost.
    pub baseline: HeapStats,
    pub rss_bytes: u64,
    pub cpu_pct: f32,
    pub fps: f64,
    pub target_fps: f64,
    pub last_frame_ms: f64,
    pub worst_frame_ms: f64,
    pub dropped: u64,
    pub frames: u64,
    pub phases: Phases,
    pub phase_totals: Phases,
    pub spawned_total: u64,
    pub peak_live: usize,
    pub reel_frame: usize,
    pub reel_len: usize,
    pub density: u32,
    pub paused: bool,
}

impl Metrics {
    pub fn new(target_fps: f64, density: u32, reel_len: usize) -> Self {
        Self {
            frame_ms: VecDeque::with_capacity(HISTORY),
            rss: VecDeque::with_capacity(HISTORY),
            live_particles: VecDeque::with_capacity(HISTORY),
            heap: HeapStats::default(),
            baseline: HeapStats::default(),
            rss_bytes: 0,
            cpu_pct: 0.0,
            fps: 0.0,
            target_fps,
            last_frame_ms: 0.0,
            worst_frame_ms: 0.0,
            dropped: 0,
            frames: 0,
            phases: Phases::default(),
            phase_totals: Phases::default(),
            spawned_total: 0,
            peak_live: 0,
            reel_frame: 0,
            reel_len,
            density,
            paused: false,
        }
    }

    pub fn push(history: &mut VecDeque<u64>, value: u64) {
        if history.len() == HISTORY {
            history.pop_front();
        }
        history.push_back(value);
    }
}

pub fn draw(f: &mut Frame, canvas: &Canvas, m: &Metrics) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(f, area);
        return;
    }

    // The canvas is a fixed 80x22 map plus its border, so it takes exactly the
    // height it needs and every spare row goes to the runtime pane, where it
    // becomes frame-time history rather than dead space under the animation.
    let [top, bottom] =
        Layout::vertical([Constraint::Length(MAP_HEIGHT + 2), Constraint::Min(9)]).areas(area);
    let [canvas_area, side] =
        Layout::horizontal([Constraint::Length(MAP_WIDTH + 2), Constraint::Min(32)]).areas(top);

    draw_canvas(f, canvas_area, canvas, m);
    draw_sidebar(f, side, m);
    draw_runtime(f, bottom, m);
}

fn draw_too_small(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("terminal too small"),
        Line::from(format!(
            "need {MIN_WIDTH}x{MIN_HEIGHT}, have {}x{}",
            area.width, area.height
        )),
        Line::from(""),
        Line::from("resize, or run with --headless"),
    ];
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(" nihilurk-perf ")),
        area,
    );
}

/// The particle layer itself, one `Line` per map row.
///
/// Built row by row rather than cell by cell on purpose: at eight thousand live
/// motes a span-per-cell would allocate more in the UI than the system under
/// test does, and the dashboard would end up dominating its own memory graph.
/// Runs of identical style are merged into one span for the same reason.
fn draw_canvas(f: &mut Frame, area: Rect, canvas: &Canvas, m: &Metrics) {
    let mut lines = Vec::with_capacity(MAP_HEIGHT as usize);
    for y in 0..MAP_HEIGHT {
        let mut spans: Vec<Span> = Vec::new();
        let mut run = String::new();
        let mut run_color = Color::Reset;
        for x in 0..MAP_WIDTH {
            let (glyph, color) = match canvas.get(x, y) {
                Some((g, c)) => (g, to_ratatui(c)),
                None => (' ', Color::Reset),
            };
            if color != run_color && !run.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut run),
                    Style::default().fg(run_color),
                ));
            }
            run_color = color;
            run.push(glyph);
        }
        if !run.is_empty() {
            spans.push(Span::styled(run, Style::default().fg(run_color)));
        }
        lines.push(Line::from(spans));
    }

    let title = format!(
        " canvas · frame {}/{} · {} live · x{} ",
        m.reel_frame % m.reel_len.max(1),
        m.reel_len,
        m.live_particles.back().copied().unwrap_or(0),
        m.density
    );
    let pause = match m.paused {
        true => " [PAUSED] ",
        false => "",
    };
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_bottom(Span::styled(
                    pause,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_sidebar(f: &mut Frame, area: Rect, m: &Metrics) {
    let [rss_area, heap_area, pop_area] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Min(6),
    ])
    .areas(area);

    // ---- RSS ----
    let [rss_text, rss_graph] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)])
        .areas(inner(f, rss_area, " resident set size ", Color::Cyan));
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("now  ", Style::default().fg(Color::DarkGray)),
                Span::styled(bytes(m.rss_bytes), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("peak ", Style::default().fg(Color::DarkGray)),
                Span::raw(bytes(m.rss.iter().copied().max().unwrap_or(0))),
            ]),
        ]),
        rss_text,
    );
    f.render_widget(
        Sparkline::default()
            .data(m.rss.iter().copied().collect::<Vec<u64>>())
            .style(Style::default().fg(Color::Cyan)),
        rss_graph,
    );

    // ---- Heap ----
    let heap_inner = inner(f, heap_area, " heap (tracking allocator) ", Color::Magenta);
    let run_allocs = m.heap.total_allocs.saturating_sub(m.baseline.total_allocs);
    let run_bytes = m.heap.total_bytes.saturating_sub(m.baseline.total_bytes);
    let per_frame = match m.frames {
        0 => 0,
        n => run_allocs / n,
    };
    // Live figures are net of the baseline too, so this reads as "what the
    // particle layer is holding right now" rather than "that plus the reel".
    let run_live = m.heap.live_bytes.saturating_sub(m.baseline.live_bytes);
    let run_blocks = m.heap.live_blocks.saturating_sub(m.baseline.live_blocks);
    f.render_widget(
        Paragraph::new(vec![
            kv("live", &bytes(run_live as u64), Color::Magenta),
            kv("blocks", &count(run_blocks as u64), Color::White),
            kv("allocs", &count(run_allocs), Color::White),
            kv("alloc/frame", &count(per_frame), Color::Yellow),
            kv("churned", &bytes(run_bytes), Color::DarkGray),
            kv(
                "reel",
                &bytes(m.baseline.live_bytes as u64),
                Color::DarkGray,
            ),
            Line::from(vec![Span::styled(
                "  RSS lags; this does not",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )]),
        ]),
        heap_inner,
    );

    // ---- Particle population ----
    let pop_inner = inner(f, pop_area, " live particles ", Color::Green);
    let [pop_text, pop_graph] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(pop_inner);
    f.render_widget(
        Paragraph::new(vec![
            kv(
                "now",
                &count(m.live_particles.back().copied().unwrap_or(0)),
                Color::Green,
            ),
            kv("peak", &count(m.peak_live as u64), Color::White),
            kv("spawned", &count(m.spawned_total), Color::DarkGray),
        ]),
        pop_text,
    );
    f.render_widget(
        Sparkline::default()
            .data(m.live_particles.iter().copied().collect::<Vec<u64>>())
            .style(Style::default().fg(Color::Green)),
        pop_graph,
    );
}

fn draw_runtime(f: &mut Frame, area: Rect, m: &Metrics) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);

    // ---- Frame time ----
    let frame_inner = inner(f, left, " runtime ", Color::Yellow);
    let [text, graph] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(frame_inner);
    let budget = 1000.0 / m.target_fps;
    let headroom = 100.0 - (m.last_frame_ms / budget * 100.0);
    let fps_color = match m.fps >= m.target_fps * 0.95 {
        true => Color::Green,
        false => Color::Red,
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("fps ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:>6.1}", m.fps),
                    Style::default().fg(fps_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" / {:.0} target", m.target_fps),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("   dropped ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    count(m.dropped),
                    Style::default().fg(match m.dropped {
                        0 => Color::Green,
                        _ => Color::Red,
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("frame ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:>6.2} ms", m.last_frame_ms),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!(
                        "   worst {:.2} ms   budget {:.1} ms",
                        m.worst_frame_ms, budget
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled("headroom ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{headroom:>5.1}%"),
                    Style::default().fg(match headroom > 25.0 {
                        true => Color::Green,
                        false => Color::Red,
                    }),
                ),
                Span::styled(
                    "   [q]uit [space]pause [+/-]density [b]urst",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ]),
        text,
    );
    // Autoscaled to the window's own worst frame rather than pinned to the
    // budget. Against a 33 ms budget a 0.2 ms frame is a flat line, and jitter
    // -- the thing this graph is for -- would be invisible. The absolute
    // numbers and the budget are on the line above, so nothing is lost.
    f.render_widget(
        Sparkline::default()
            .data(m.frame_ms.iter().copied().collect::<Vec<u64>>())
            .style(Style::default().fg(Color::Yellow)),
        graph,
    );

    // ---- CPU + phase breakdown ----
    let cpu_inner = inner(f, right, " cpu · phase breakdown ", Color::Blue);
    let [gauge_area, phase_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(cpu_inner);
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(match m.cpu_pct > 90.0 {
                true => Color::Red,
                false => Color::Blue,
            }))
            .ratio((m.cpu_pct as f64 / 100.0).clamp(0.0, 1.0))
            .label(format!("cpu {:.1}%", m.cpu_pct)),
        gauge_area,
    );

    let width = phase_area.width.saturating_sub(22) as usize;
    let mut lines = Vec::new();
    for (name, ns, share) in m.phase_totals.shares() {
        let filled = (share * width as f64) as usize;
        lines.push(Line::from(vec![
            Span::styled(format!("{name:<8}"), Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>7.3} ms ", ns as f64 / 1e6 / m.frames.max(1) as f64),
                Style::default().fg(Color::White),
            ),
            Span::styled("█".repeat(filled), Style::default().fg(phase_color(name))),
        ]));
    }
    f.render_widget(Paragraph::new(lines), phase_area);
}

fn phase_color(name: &str) -> Color {
    match name {
        "advance" => Color::Cyan,
        "spawn" => Color::Magenta,
        _ => Color::Green,
    }
}

/// Render a titled block into `area` and hand back the space inside it.
fn inner(f: &mut Frame, area: Rect, title: &str, color: Color) -> Rect {
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
        Span::styled(format!("{key:<12}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

/// nihilurk draws in the sixteen ANSI colours; ratatui names them differently.
fn to_ratatui(c: CtColor) -> Color {
    match c {
        CtColor::Black => Color::Black,
        CtColor::DarkGrey => Color::DarkGray,
        CtColor::Grey => Color::Gray,
        CtColor::White => Color::White,
        CtColor::Red => Color::LightRed,
        CtColor::DarkRed => Color::Red,
        CtColor::Green => Color::LightGreen,
        CtColor::DarkGreen => Color::Green,
        CtColor::Yellow => Color::LightYellow,
        CtColor::DarkYellow => Color::Yellow,
        CtColor::Blue => Color::LightBlue,
        CtColor::DarkBlue => Color::Blue,
        CtColor::Magenta => Color::LightMagenta,
        CtColor::DarkMagenta => Color::Magenta,
        CtColor::Cyan => Color::LightCyan,
        CtColor::DarkCyan => Color::Cyan,
        _ => Color::Reset,
    }
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

pub fn count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
