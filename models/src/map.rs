//! The dungeon floor: the terrain itself, and everything that happens to one.
//!
//! This file is the **terrain resource and nothing else** — [`Map`], the
//! [`TileType`] it is made of, and the handful of questions every other system
//! asks it ("is that a wall?", "can I cut that corner?"). Terrain is one
//! `Vec<TileType>` rather than an entity per tile: a 1,760-entity floor with a
//! `Renderable` each would cost more to iterate every frame than the whole rest
//! of the world put together.
//!
//! Everything a floor *does* lives in a submodule, the same way `crate::items`
//! is split by what the player is doing to an item:
//!
//! * [`streams`] — the three RNGs, and the per-floor ones derived from a seed.
//! * [`generate`] — carving a floor: rooms, corridors, stairs, the dark.
//! * [`population`] — what stands on one once it is carved.
//! * [`levels`] — moving between floors, and starting a run.
//! * [`overlays`] — the cosmetic mess a floor accumulates.
//!
//! All five are re-exported here, so `models::change_level` and
//! `map::BloodStains` keep resolving and a caller never has to know which file
//! a thing came from.

use crossterm::style::Color;
use fixedbitset::FixedBitSet;

use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::rect::Rect;

mod generate;
mod levels;
mod overlays;
mod population;
mod streams;

pub use generate::{create_map, regenerate_map};
pub use levels::*;
pub use overlays::*;
pub use population::difficulty_tier;
pub use streams::*;

pub(crate) use levels::{LevelChange, transition_level, win_with_style};

// --- Tuning constants -------------------------------------------------------
// All defined and documented in `constants.rs`; re-exported here so the old
// paths (`map::MAP_WIDTH`, `map::FINAL_DEPTH`, ...) keep resolving.
//
//   MAP_WIDTH / MAP_HEIGHT   playfield size in tiles (pins seed layout)
//   FINAL_DEPTH              deepest floor; holds the Element of Yoord
//   DUNGEON_LORD_PATIENCE    turns per level before the forced portal
pub use crate::constants::map::{HEIGHT as MAP_HEIGHT, WIDTH as MAP_WIDTH};
pub use crate::constants::progression::{DUNGEON_LORD_PATIENCE, FINAL_DEPTH};

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum TileType {
    Wall,
    Room,
    Passage,
    Door,
    Upstairs,
    Downstairs,
}

/// The dungeon terrain for the current floor: one [`TileType`] per coordinate,
/// row-major (see [`tile_index`]). This is the single source of truth for the
/// map — terrain is no longer stored as one ECS entity per tile.
#[derive(Resource, Clone)]
pub struct Map {
    pub tiles: Vec<TileType>,
    /// One bit per tile: set on the floor of a "dark" room. Visibility inside a
    /// dark room is cut to the always-on 3x3 (as if it were a passage) until a
    /// wand of light is zapped there, which clears the bits for the whole room.
    /// Rolled deterministically from the seed in [`build_tiles`]; the cleared
    /// state is persisted in the save file.
    pub dark: FixedBitSet,
}

impl Map {
    /// The tile at `(x, y)`. Out-of-bounds coordinates read as solid [`TileType::Wall`]
    /// so callers can probe freely without bounds checks.
    #[inline]
    pub fn tile(&self, x: u16, y: u16) -> TileType {
        if x >= MAP_WIDTH || y >= MAP_HEIGHT {
            return TileType::Wall;
        }
        self.tiles[tile_index(x, y)]
    }

    /// Whether `(x, y)` blocks movement.
    #[inline]
    pub fn blocks(&self, x: u16, y: u16) -> bool {
        self.tile(x, y) == TileType::Wall
    }

    /// Whether `(x, y)` is the floor of a still-unlit dark room.
    #[inline]
    pub fn is_dark(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.dark.contains(tile_index(x, y))
    }

    /// Clears the dark flag for a single tile (a wand of light sweeping a room).
    #[inline]
    pub fn light_tile(&mut self, x: u16, y: u16) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            self.dark.set(tile_index(x, y), false);
        }
    }

    /// Whether a single step from `(fx, fy)` to `(tx, ty)` is allowed by the
    /// diagonal-movement rule. Orthogonal steps always pass; a diagonal step is
    /// only allowed between two tiles of the same kind — you can round a bend in
    /// a passage, but you can't cut the corner of a doorway or slip diagonally
    /// between a room and a corridor. Says nothing about walls or occupants;
    /// combine with [`Map::blocks`].
    #[inline]
    pub fn diagonal_step_ok(&self, fx: u16, fy: u16, tx: u16, ty: u16) -> bool {
        if fx == tx || fy == ty {
            return true; // orthogonal (or no move)
        }
        tile_kind(self.tile(fx, fy)) == tile_kind(self.tile(tx, ty))
    }

    /// Whether the wall at `(x, y)` bounds a room (or a doorway into one). Rogue
    /// only draws these; the loose walls hugging a corridor are left as blank
    /// space so passages read as tunnels through the dark rather than trenches.
    pub fn is_room_wall(&self, x: u16, y: u16) -> bool {
        if self.tile(x, y) != TileType::Wall {
            return false;
        }
        // Only room floor (and the stairs that sit on it) makes a wall worth
        // drawing. A wall that merely touches a Door — but no room tile — is
        // outside the room, hugging the corridor, and stays dark.
        NEIGHBOUR_DIRS.iter().any(|&(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            nx >= 0
                && ny >= 0
                && matches!(
                    self.tile(nx as u16, ny as u16),
                    TileType::Room | TileType::Upstairs | TileType::Downstairs
                )
        })
    }
}

/// Coarse grouping of tiles for the diagonal-movement rule: room floor and the
/// staircases standing on it count as one kind, so a diagonal step onto stairs
/// still works. Walls get their own bucket but never matter — [`Map::blocks`]
/// rejects them first.
fn tile_kind(t: TileType) -> u8 {
    match t {
        TileType::Room | TileType::Upstairs | TileType::Downstairs => 0,
        TileType::Passage => 1,
        TileType::Door => 2,
        TileType::Wall => 3,
    }
}

/// The glyph and lit colour used to draw a terrain tile.
pub fn tile_appearance(t: TileType) -> (char, Color) {
    match t {
        TileType::Room => ('.', Color::Green),
        TileType::Passage => ('▒', Color::White),
        TileType::Wall => ('#', Color::DarkYellow),
        TileType::Door => ('+', Color::Yellow),
        TileType::Downstairs => ('>', Color::Cyan),
        TileType::Upstairs => ('<', Color::Cyan),
    }
}

/// Total tile count; the length of a fog-of-war bitset.
pub const MAP_TILE_COUNT: usize = MAP_WIDTH as usize * MAP_HEIGHT as usize;

/// Flattens a tile coordinate into a bitset/array index.
#[inline]
pub const fn tile_index(x: u16, y: u16) -> usize {
    y as usize * MAP_WIDTH as usize + x as usize
}

/// The eight neighbouring offsets, in row-major read order.
const NEIGHBOUR_DIRS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// A uniformly random tile inside `room`, walls excluded.
fn random_point_in_room(room: &Rect, rng: &mut ChaCha12Rng) -> (u16, u16) {
    let width = (room.x2 - room.x1 + 1).max(1) as u32;
    let height = (room.y2 - room.y1 + 1).max(1) as u32;

    // `gen_range`, not `gen() % width`: modulo would bias the low coordinates,
    // and a biased draw here would bias every stair and every spawn on the floor.
    let rx = room.x1 as u32 + rng.gen_range(0..width);
    let ry = room.y1 as u32 + rng.gen_range(0..height);

    (rx as u16, ry as u16)
}
