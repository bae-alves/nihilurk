//! A flamegraph you can read over SSH.
//!
//! The usual `cargo flamegraph` output is an SVG, which assumes a machine with
//! a browser on it. nihilurk's whole premise is that it runs on anything with a
//! terminal (see the portability note in `gdd.md`), and a profiling rig that
//! quietly required X11 to read its own results would not hold to that. So this
//! module renders the graph as text.
//!
//! Two input formats, because which one you have depends on what is installed:
//!
//!   * **folded stacks** -- `main;turn;blip 1234` per line, the format
//!     `inferno-collapse-perf` and `stackcollapse-perf.pl` emit, and the
//!     interchange format every other flamegraph tool speaks.
//!   * **`perf script`** -- raw, so the pipeline only needs `perf` itself and
//!     not the Perl/inferno collapsing step on top of it.
//!
//! The drawing is an *icicle* layout: root at the top, children below. Real
//! flamegraphs put the root at the bottom, but a terminal scrolls downward, and
//! a graph you have to scroll backwards to find the root of is a graph nobody
//! reads. Width is proportional to samples, exactly as in the SVG.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A node in the merged call tree.
#[derive(Default)]
struct Node {
    samples: u64,
    children: BTreeMap<String, Node>,
}

impl Node {
    /// Samples in this node that are not in any child -- time actually spent in
    /// this frame rather than below it. The "self" column of any profiler.
    fn self_samples(&self) -> u64 {
        let in_children: u64 = self.children.values().map(|c| c.samples).sum();
        self.samples.saturating_sub(in_children)
    }
}

/// A parsed profile, ready to render.
pub struct Profile {
    root: Node,
    /// Every distinct frame, by total and self samples, for the hotspot table.
    totals: BTreeMap<String, (u64, u64)>,
}

impl Profile {
    /// Parse either supported format, sniffing which one this is. A folded file
    /// is overwhelmingly lines ending in whitespace-then-digits; `perf script`
    /// is not.
    pub fn parse(input: &str) -> Self {
        if looks_folded(input) {
            return Self::from_folded(input);
        }
        Self::from_folded(&collapse_perf_script(input))
    }

    pub fn from_folded(input: &str) -> Self {
        let mut root = Node::default();
        for line in input.lines() {
            let Some((stack, count)) = line.rsplit_once(' ') else {
                continue;
            };
            let Ok(count) = count.trim().parse::<u64>() else {
                continue;
            };
            if count == 0 {
                continue;
            }
            root.samples += count;
            let mut node = &mut root;
            for frame in stack.split(';').filter(|f| !f.is_empty()) {
                node = node.children.entry(frame.to_string()).or_default();
                node.samples += count;
            }
        }

        let mut totals = BTreeMap::new();
        collect_totals(&root, &mut totals);
        Self { root, totals }
    }

    pub fn total_samples(&self) -> u64 {
        self.root.samples
    }

    pub fn is_empty(&self) -> bool {
        self.root.samples == 0
    }

    /// The icicle graph, `width` columns wide, cut off below `max_depth`.
    /// `color` gates ANSI escapes so the output can be piped into a file.
    pub fn render(&self, width: usize, max_depth: usize, color: bool) -> String {
        let mut out = String::new();
        if self.root.samples == 0 {
            return "  (no samples)\n".to_string();
        }
        let width = width.max(20);
        let mut rows: Vec<Vec<(String, u64, usize)>> = Vec::new();
        // Children of the synthetic root are the real stack roots.
        layout(&self.root, 0, max_depth, 0, &mut rows);

        for row in &rows {
            let mut line = String::new();
            let mut drawn = 0usize;
            for (label, samples, offset) in row {
                let start = scale(*offset as u64, self.root.samples, width);
                let end = scale(*offset as u64 + *samples, self.root.samples, width);
                let cells = end.saturating_sub(start);
                if cells == 0 {
                    continue;
                }
                // Frames are laid out left to right, so any gap is a caller's
                // unattributed self time.
                while drawn < start {
                    line.push(' ');
                    drawn += 1;
                }
                line.push_str(&bar(label, cells, *samples, self.root.samples, color));
                drawn += cells;
            }
            if line.trim().is_empty() {
                continue;
            }
            let _ = writeln!(out, "{line}");
        }
        out
    }

    /// The flat hotspot table: the frames with the most *self* samples, which
    /// is where the time is actually going.
    pub fn hotspots(&self, n: usize) -> Vec<(String, u64, f64, u64)> {
        let total = self.root.samples.max(1);
        let mut rows: Vec<_> = self
            .totals
            .iter()
            .map(|(name, (tot, own))| {
                (name.clone(), *own, *own as f64 * 100.0 / total as f64, *tot)
            })
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.1));
        rows.truncate(n);
        rows
    }
}

fn collect_totals(node: &Node, out: &mut BTreeMap<String, (u64, u64)>) {
    for (name, child) in &node.children {
        let entry = out.entry(name.clone()).or_insert((0, 0));
        entry.0 += child.samples;
        entry.1 += child.self_samples();
        collect_totals(child, out);
    }
}

/// Flatten the tree into one `Vec` of frames per depth, each tagged with its
/// horizontal offset in samples. `base` is where this node's children start on
/// the row, so a child always sits under the parent that called it.
///
/// Children come out of a `BTreeMap`, so sibling order is alphabetical and
/// therefore stable between runs -- a graph that reshuffles itself every time
/// cannot be compared against yesterday's.
fn layout(
    node: &Node,
    depth: usize,
    max_depth: usize,
    base: usize,
    rows: &mut Vec<Vec<(String, u64, usize)>>,
) {
    if depth > max_depth || node.children.is_empty() {
        return;
    }
    let mut offset = base;
    for (name, child) in &node.children {
        while rows.len() <= depth {
            rows.push(Vec::new());
        }
        rows[depth].push((name.clone(), child.samples, offset));
        layout(child, depth + 1, max_depth, offset, rows);
        offset += child.samples as usize;
    }
}

fn scale(value: u64, total: u64, width: usize) -> usize {
    ((value as u128 * width as u128) / total.max(1) as u128) as usize
}

/// One frame's bar: the label, truncated to fit, on a coloured ground.
fn bar(label: &str, cells: usize, samples: u64, total: u64, color: bool) -> String {
    let pct = samples as f64 * 100.0 / total.max(1) as f64;
    let short = shorten(label);
    // Without colour there is nothing to separate one frame from the next, so
    // the leading column is given over to a divider and the label gets the
    // rest. Colour mode needs no divider and uses the full width.
    let avail = match color {
        true => cells,
        false => cells.saturating_sub(1),
    };
    // Only worth spending columns on the percentage if the bar is wide enough
    // that the name still survives beside it.
    let text = match avail >= short.chars().count() + 8 {
        true => format!("{short} {pct:.1}%"),
        false => short,
    };
    let mut body: String = text.chars().take(avail).collect();
    while body.chars().count() < avail {
        body.push(' ');
    }
    if !color {
        return format!("|{body}");
    }
    let (r, g, b) = heat(label);
    format!("\x1b[48;2;{r};{g};{b}m\x1b[38;2;20;20;20m{body}\x1b[0m")
}

/// Rust symbols are long. Drop the leading path segments and the generics, keep
/// the last two `::` components -- `models::particles::Particles::blip` reads
/// fine as `Particles::blip` and fits in a terminal.
///
/// Public because the self-time table needs it too: a frame like
/// `from_iter<alloc::boxed::Box<dyn FnOnce<(), Output=()> + Send, Global>, ..>`
/// is three hundred columns of noise in a table whose whole job is to be
/// skimmed.
pub fn shorten(label: &str) -> String {
    let no_generics = strip_generics(label);
    let depth = no_generics.split("::").count();
    if depth <= 2 {
        return no_generics;
    }
    no_generics
        .split("::")
        .skip(depth - 2)
        .collect::<Vec<_>>()
        .join("::")
}

fn strip_generics(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut depth = 0i32;
    for c in label.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The classic flamegraph warm palette, keyed off the frame name so a function
/// keeps its colour between runs.
fn heat(label: &str) -> (u8, u8, u8) {
    let mut h: u64 = 1469598103934665603;
    for b in label.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    let t = (h % 1000) as f64 / 1000.0;
    (
        (205.0 + 50.0 * t) as u8,
        (40.0 + 160.0 * t) as u8,
        (30.0 + 25.0 * t) as u8,
    )
}

fn looks_folded(input: &str) -> bool {
    let mut checked = 0;
    let mut folded = 0;
    for line in input.lines().filter(|l| !l.trim().is_empty()).take(40) {
        checked += 1;
        let ends_in_count = line
            .rsplit_once(' ')
            .map(|(_, n)| n.trim().parse::<u64>().is_ok())
            .unwrap_or(false);
        if ends_in_count && line.contains(';') {
            folded += 1;
        }
    }
    checked > 0 && folded * 2 > checked
}

/// Fold `perf script` output into the collapsed format, so the pipeline needs
/// only `perf` itself and not `inferno`/`stackcollapse-perf.pl` as well.
///
/// `perf script` prints one sample as a header line followed by its stack,
/// leaf first, one frame per indented line, terminated by a blank line.
pub fn collapse_perf_script(input: &str) -> String {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut stack: Vec<String> = Vec::new();

    let flush = |stack: &mut Vec<String>, counts: &mut BTreeMap<String, u64>| {
        if stack.is_empty() {
            return;
        }
        stack.reverse();
        *counts.entry(stack.join(";")).or_insert(0) += 1;
        stack.clear();
    };

    for line in input.lines() {
        if line.trim().is_empty() {
            flush(&mut stack, &mut counts);
            continue;
        }
        // Header lines start at column 0; stack frames are indented.
        if !line.starts_with([' ', '\t']) {
            flush(&mut stack, &mut counts);
            continue;
        }
        let Some(sym) = perf_symbol(line) else {
            continue;
        };
        stack.push(sym);
    }
    flush(&mut stack, &mut counts);

    let mut out = String::new();
    for (stack, n) in counts {
        let _ = writeln!(out, "{stack} {n}");
    }
    out
}

/// Pull the symbol out of one `perf script` stack line:
/// `\t    55f8c0a1b2c3 models::particles::Particles::blip+0x1a (/path/to/bin)`
fn perf_symbol(line: &str) -> Option<String> {
    let rest = line.trim_start();
    // Drop the leading address.
    let rest = rest.split_once(' ').map(|(_, r)| r).unwrap_or(rest);
    // Drop the trailing `(dso)`.
    let rest = match rest.rfind(" (") {
        Some(i) => &rest[..i],
        None => rest,
    };
    // Drop the `+0x..` offset within the symbol.
    let rest = match rest.rfind("+0x") {
        Some(i) => &rest[..i],
        None => rest,
    };
    let sym = strip_hash(rest.trim());
    if sym.is_empty() || sym == "[unknown]" {
        return None;
    }
    Some(sym)
}

/// Rust mangles a trailing `::h<16 hex>` onto symbols; it is noise in a graph.
fn strip_hash(sym: &str) -> String {
    let Some(i) = sym.rfind("::h") else {
        return sym.to_string();
    };
    let tail = &sym[i + 3..];
    if tail.len() == 16 && tail.chars().all(|c| c.is_ascii_hexdigit()) {
        return sym[..i].to_string();
    }
    sym.to_string()
}
