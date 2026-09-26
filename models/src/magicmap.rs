//! Scroll of magic mapping: the whole floor's layout floods into the player's
//! memory at once.
//!
//! Rather than snap the map on in a single frame, the reveal plays out as an
//! animated wipe — nihilurk's take on the row-by-row `MagicMapReveal` run-state from
//! the "Rust Roguelike" tutorial (chapter 20). There is no run-state machine
//! here, so the wipe is driven the same way the particle layer is:
//! the scroll-of-magic-mapping mechanic (in `crate::items`) arms [`MagicMapReveal`] while resolving
//! the read, and once the turn is over the engine plays it out over a handful of
//! frames, folding one [`magic_map_reveal_step`] into the player's fog-of-war
//! memory per frame and re-rendering between.
//!
//! Each read rolls one of three [`MagicMapStyle`] shapes for the wipe:
//!
//! * [`MagicMapStyle::RowByRow`] — a horizontal curtain, top edge to bottom.
//! * [`MagicMapStyle::Spiral`]   — an arm winding outward from the reader.
//! * [`MagicMapStyle::Explode`]  — concentric shells bursting from the reader.
//!
//! The reveal only touches terrain memory (the `revealed_tiles` bitset on the
//! player's [`Viewshed`]). Monsters and floor items still only draw where the
//! player can actually *see*, so a mapped-but-unvisited room reads as bare
//! architecture — exactly like the tutorial's version.

use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::components::{Player, Viewshed};
use crate::map::{MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, tile_index};

/// Which shape the reveal takes as it floods the map into memory. Rolled at
/// random each time a scroll of magic mapping is read (unless the `NIHILURK_MAGICMAP`
/// dev override is set), so no two reads look quite the same.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MagicMapStyle {
    /// A horizontal curtain falling from the top edge to the bottom.
    #[default]
    RowByRow,
    /// A single arm that winds outward from the reader, turning as it grows.
    Spiral,
    /// Concentric shells racing outward from the reader's own tile.
    Explode,
}

impl MagicMapStyle {
    /// Every style, in a stable order — handy for tests and for the dev override.
    pub const ALL: [MagicMapStyle; 3] = [Self::RowByRow, Self::Spiral, Self::Explode];

    /// Picks a style uniformly at random.
    pub fn roll(rng: &mut ChaCha12Rng) -> Self {
        Self::ALL[rng.gen_range(0..Self::ALL.len())]
    }

    /// Parses the `NIHILURK_MAGICMAP` dev-override value (`rows` / `spiral` /
    /// `explode`, plus a few aliases). `None` for anything unrecognised.
    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "row" | "rows" | "rowbyrow" | "row-by-row" | "curtain" => Some(Self::RowByRow),
            "spiral" | "swirl" => Some(Self::Spiral),
            "explode" | "explosion" | "blast" | "burst" => Some(Self::Explode),
            _ => None,
        }
    }

    /// The log line that lands when the reader takes the map in this way.
    pub fn flavour(self) -> &'static str {
        match self {
            Self::RowByRow => strings::magicmap_row_by_row(),
            Self::Spiral => strings::magicmap_spiral(),
            Self::Explode => strings::magicmap_explode(),
        }
    }

    /// The pause the engine holds between frames for this style, tuned so every
    /// shape finishes in roughly the same half-second regardless of how many
    /// waves it breaks into.
    pub fn frame_ms(self) -> u64 {
        match self {
            Self::Spiral => 8,
            Self::RowByRow | Self::Explode => 20,
        }
    }
}

/// Transient: armed the turn a scroll of magic mapping is read, cleared once the
/// wipe has committed its last wave. Never serialised — a save written mid-wipe
/// reloads with every wave already in memory, so there is nothing left to play.
#[derive(Resource, Default)]
pub struct MagicMapReveal {
    pub active: bool,
    pub style: MagicMapStyle,
    /// Reveal waves, in play order; the engine commits one wave of tile indices
    /// to the player's fog-of-war memory per frame.
    waves: Vec<Vec<usize>>,
    cursor: usize,
}

impl MagicMapReveal {
    /// Arm a reveal centred on `hero` (map coords), laid out in `style`.
    pub fn start(&mut self, hero: (u16, u16), style: MagicMapStyle) {
        self.active = true;
        self.style = style;
        self.cursor = 0;
        self.waves = build_waves(hero, style);
    }

    /// Disarm the wipe.
    pub fn stop(&mut self) {
        self.active = false;
        self.waves.clear();
        self.cursor = 0;
    }

    /// Per-frame pause for the armed style. See [`MagicMapStyle::frame_ms`].
    pub fn frame_ms(&self) -> u64 {
        self.style.frame_ms()
    }
}

/// Commit the next wave of the armed reveal to the player's revealed-tiles
/// memory and advance. Returns `true` while the wipe is still running and
/// `false` once the last wave has been played (at which point [`MagicMapReveal`]
/// has switched itself back off).
///
/// The engine calls this once per animation frame, exactly like it ages the
/// particle layer one frame at a time. A no-op returning `false` when nothing is
/// armed (or when there is no reveal resource at all, as in a headless test).
pub fn magic_map_reveal_step(world: &mut World) -> bool {
    let wave = {
        let Some(mut mm) = world.get_resource_mut::<MagicMapReveal>() else {
            return false;
        };
        if !mm.active {
            return false;
        }
        if mm.cursor >= mm.waves.len() {
            mm.stop();
            return false;
        }
        let c = mm.cursor;
        mm.cursor += 1;
        std::mem::take(&mut mm.waves[c])
    };

    let mut query = world.query_filtered::<&mut Viewshed, With<Player>>();
    if let Some(mut viewshed) = query.iter_mut(world).next() {
        if viewshed.revealed_tiles.len() < MAP_TILE_COUNT {
            viewshed.revealed_tiles.grow(MAP_TILE_COUNT);
        }
        for i in wave {
            viewshed.revealed_tiles.insert(i);
        }
    }

    true
}

/// Play the armed reveal to completion instantly, skipping the animation. Used
/// when the player cuts the wipe short with a keypress.
pub fn finish_magic_map_reveal(world: &mut World) {
    while magic_map_reveal_step(world) {}
}

// ---------------------------------------------------------------------------
// Wave layouts
// ---------------------------------------------------------------------------

fn build_waves(hero: (u16, u16), style: MagicMapStyle) -> Vec<Vec<usize>> {
    match style {
        MagicMapStyle::RowByRow => rows_waves(),
        MagicMapStyle::Spiral => spiral_waves(hero),
        MagicMapStyle::Explode => explode_waves(hero),
    }
}

/// One wave per map row, top to bottom.
fn rows_waves() -> Vec<Vec<usize>> {
    (0..MAP_HEIGHT)
        .map(|y| (0..MAP_WIDTH).map(|x| tile_index(x, y)).collect())
        .collect()
}

/// Concentric shells expanding from `hero`, bucketed by Euclidean distance into
/// ~two dozen frames so the blast reads as a growing circle.
fn explode_waves(hero: (u16, u16)) -> Vec<Vec<usize>> {
    const WAVES: usize = 26;
    let (hx, hy) = (hero.0 as f32, hero.1 as f32);

    let max_d = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| ((x as f32 - hx).powi(2) + (y as f32 - hy).powi(2)).sqrt())
        .fold(1.0_f32, f32::max);

    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); WAVES];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let d = ((x as f32 - hx).powi(2) + (y as f32 - hy).powi(2)).sqrt();
            let b = ((d / max_d) * (WAVES - 1) as f32).round() as usize;
            buckets[b.min(WAVES - 1)].push(tile_index(x, y));
        }
    }
    buckets.retain(|b| !b.is_empty());
    buckets
}

/// A square spiral of tiles starting at `hero`, sliced into ~60 frames so the
/// arm visibly winds outward.
fn spiral_waves(hero: (u16, u16)) -> Vec<Vec<usize>> {
    const WAVES: usize = 60;

    let mut order: Vec<usize> = Vec::with_capacity(MAP_TILE_COUNT);
    let mut seen = vec![false; MAP_TILE_COUNT];
    let visit = |x: i32, y: i32, order: &mut Vec<usize>, seen: &mut [bool]| {
        if (0..MAP_WIDTH as i32).contains(&x) && (0..MAP_HEIGHT as i32).contains(&y) {
            let i = tile_index(x as u16, y as u16);
            if !seen[i] {
                seen[i] = true;
                order.push(i);
            }
        }
    };

    let (mut x, mut y) = (hero.0 as i32, hero.1 as i32);
    visit(x, y, &mut order, &mut seen);

    // right N, down N, left N+1, up N+1, right N+2, ... — the classic outward
    // square spiral. The arm runs off-grid on the long sides once it grows past
    // the map; `visit` just drops those. Capped so an off-centre hero can't loop
    // forever.
    let dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let mut dir = 0usize;
    let mut seg = 1i32;
    let seg_cap = (MAP_WIDTH + MAP_HEIGHT) as i32 * 2;
    while order.len() < MAP_TILE_COUNT && seg < seg_cap {
        for _ in 0..2 {
            let (dx, dy) = dirs[dir % 4];
            for _ in 0..seg {
                x += dx;
                y += dy;
                visit(x, y, &mut order, &mut seen);
            }
            dir += 1;
        }
        seg += 1;
    }

    chunk_into(order, WAVES)
}

/// Splits `order` into at most `target` contiguous chunks of near-equal size,
/// preserving order. Empty input yields no waves.
fn chunk_into(order: Vec<usize>, target: usize) -> Vec<Vec<usize>> {
    if order.is_empty() {
        return Vec::new();
    }
    let target = target.max(1);
    let chunk = order.len().div_ceil(target);
    order.chunks(chunk).map(<[usize]>::to_vec).collect()
}
