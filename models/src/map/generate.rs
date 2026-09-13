//! Carving a floor: rooms, the corridors between them, the stairs, the dark.
//!
//! Rogue's own generator, and the whole of it is one function's worth of
//! decisions. The playfield is divided into a [`SECTIONS`] x [`SECTIONS`] grid
//! with a gutter between every cell; each cell holds at most one room, placed
//! at a random offset inside it and never allowed to bleed out. That single
//! constraint is what makes everything downstream simple: two rooms can never
//! touch, so "join each pair of neighbouring cells" is enough to guarantee the
//! floor is connected, and no overlap test is needed anywhere.
//!
//! **The order of the dice here is load-bearing.** A floor's layout is a pure
//! function of `(seed, depth)` ([`super::streams::layout_rng`]), which is why a
//! save can throw its map away and rebuild it exactly. Move a `gen_range` and
//! every seed anyone has ever played means something different;
//! `models/tests/determinism.rs` exists to say so.

use fixedbitset::FixedBitSet;
use rand::Rng;
use rand_chacha::ChaCha12Rng;
use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::components::Depth;
use crate::constants::map::DARK_ROOM_CHANCE;
use crate::rect::Rect;

use super::streams::{RngSeed, layout_rng};
use super::{MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, Map, TileType, tile_index};

// ---------------------------------------------------------------------------
// The grid the floor is laid out on
// ---------------------------------------------------------------------------

/// Cells to a side. Three by three is Rogue's own: enough rooms for a floor to
/// have a shape, few enough that every one of them is worth visiting.
const SECTIONS: u16 = 3;

/// Blank tiles between neighbouring cells. Three is what guarantees two rooms
/// can never share a wall however they are placed inside their cells — which
/// is what lets this file skip an overlap test entirely.
const GUTTER: u16 = 3;

/// Blank tiles around the whole playfield, so no room is flush with the edge.
const PADDING: u16 = 1;

/// The smallest room the generator will place. A room narrower than this reads
/// as a wide corridor rather than a place.
const MIN_ROOM_W: u16 = 4;

/// The smallest room height. Three, not four: a 4-high room occupies five
/// tiles once its walls are counted, which overflows a cell.
const MIN_ROOM_H: u16 = 3;

/// How many answers the "how many cells are left empty?" roll has: none, one,
/// two or three. A floor with every cell filled is a floor with no shape, and
/// one with four missing is barely a floor.
const EMPTY_SECTION_CHOICES: usize = 4;

/// Where each cell of the grid starts and how much room it has. Derived once
/// from the constants above, so the arithmetic appears in one place rather
/// than inside the placement loop.
struct Grid {
    section_w: u16,
    section_h: u16,
    /// The grid is centred by giving the leading padding whatever is left over
    /// after the cells divide up the usable space.
    offset_x: u16,
    offset_y: u16,
}

impl Grid {
    fn new() -> Self {
        let gutters = SECTIONS - 1;
        let usable_w = MAP_WIDTH
            .saturating_sub(PADDING * 2)
            .saturating_sub(gutters * GUTTER);
        let usable_h = MAP_HEIGHT
            .saturating_sub(PADDING * 2)
            .saturating_sub(gutters * GUTTER);
        Self {
            section_w: usable_w / SECTIONS,
            section_h: usable_h / SECTIONS,
            offset_x: PADDING + (usable_w % SECTIONS) / 2,
            offset_y: PADDING + (usable_h % SECTIONS) / 2,
        }
    }

    /// The top-left tile of cell `(sx, sy)`.
    fn origin(&self, sx: u16, sy: u16) -> (u16, u16) {
        (
            self.offset_x + sx * self.section_w + sx * GUTTER,
            self.offset_y + sy * self.section_h + sy * GUTTER,
        )
    }

    /// Rolls one room inside cell `(sx, sy)`: a size, then a position for it.
    ///
    /// Four draws, in this order — width, height, x, y — and every one of them
    /// is bounded so the room's far wall stays inside the cell. Reordering
    /// them reorders every floor in the game.
    fn place_room(&self, rng: &mut ChaCha12Rng, sx: u16, sy: u16) -> Rect {
        // `-1` because a `Rect` is inclusive on both edges, so its footprint is
        // one wider and one taller than the numbers say.
        let max_w = self.section_w.saturating_sub(1).max(MIN_ROOM_W);
        let max_h = self.section_h.saturating_sub(1).max(MIN_ROOM_H);
        let w = MIN_ROOM_W + rng.gen_range(0..(max_w - MIN_ROOM_W) + 1);
        let h = MIN_ROOM_H + rng.gen_range(0..(max_h - MIN_ROOM_H) + 1);

        let (base_x, base_y) = self.origin(sx, sy);
        let x = base_x + rng.gen_range(0..=self.section_w.saturating_sub(w + 1));
        let y = base_y + rng.gen_range(0..=self.section_h.saturating_sub(h + 1));
        Rect::new(x as i32, y as i32, w as i32, h as i32)
    }
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// Procedurally computes a map layout from the given RNG. Pure: the same RNG
/// state always yields the same tiles, which is what lets us drop the map from
/// save files and rebuild it from the seed on load.
///
/// Five steps, and each one spends the RNG before the next begins: which cells
/// stay empty, a room in each of the rest, corridors between neighbours, the
/// two staircases, and finally which rooms are unlit.
pub(super) fn build_tiles(rng: &mut ChaCha12Rng) -> (Vec<TileType>, Vec<Rect>, FixedBitSet) {
    let grid = Grid::new();
    let mut tiles = vec![TileType::Wall; MAP_TILE_COUNT];

    let empty = empty_sections(rng);
    let (cells, mut rooms) = carve_rooms(rng, &grid, &empty, &mut tiles);
    connect_neighbours(rng, &cells, &rooms, &mut tiles);
    place_stairs(rng, &mut rooms, &mut tiles);
    let dark = roll_dark_rooms(rng, &rooms, &tiles);

    (tiles, rooms, dark)
}

/// Which cells of the grid are left without a room, as a set of cell indices.
///
/// A partial Fisher-Yates over the nine cells, stopped after however many the
/// first roll asked for — so the cells that go missing are a uniform sample
/// without replacement, and the whole thing costs one draw per missing cell.
fn empty_sections(rng: &mut ChaCha12Rng) -> Vec<usize> {
    let count = rng.gen_range(0..EMPTY_SECTION_CHOICES);
    let mut cells = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    for i in 0..count {
        let swap = i + rng.gen_range(0..(cells.len() - i));
        cells.swap(i, swap);
    }
    cells[..count].to_vec()
}

/// Rolls a room into every cell that is not `empty` and carves it into `tiles`.
///
/// Returns the cell grid — index into `rooms` for each of the nine cells, or
/// `None` for an empty one — alongside the rooms themselves. The grid is what
/// [`connect_neighbours`] walks; the `Vec` is what everything else wants.
fn carve_rooms(
    rng: &mut ChaCha12Rng,
    grid: &Grid,
    empty: &[usize],
    tiles: &mut [TileType],
) -> ([Option<usize>; 9], Vec<Rect>) {
    let mut cells: [Option<usize>; 9] = [None; 9];
    let mut rooms: Vec<Rect> = Vec::new();

    for sy in 0..SECTIONS {
        for sx in 0..SECTIONS {
            let cell = (sy * SECTIONS + sx) as usize;
            if empty.contains(&cell) {
                continue;
            }
            let room = grid.place_room(rng, sx, sy);
            cells[cell] = Some(rooms.len());
            create_room(&room, tiles, MAP_WIDTH);
            rooms.push(room);
        }
    }
    (cells, rooms)
}

/// Joins every pair of neighbouring rooms: along each row of the grid first,
/// then down each column. A pair the row pass already joined is skipped by the
/// column pass, so no corridor is dug twice.
///
/// Rooms cannot overlap and every cell touches its neighbours, so joining
/// neighbours is enough to make the whole floor reachable — there is no
/// connectivity check anywhere, because there does not need to be one.
fn connect_neighbours(
    rng: &mut ChaCha12Rng,
    cells: &[Option<usize>; 9],
    rooms: &[Rect],
    tiles: &mut [TileType],
) {
    let mut joined: HashSet<(usize, usize)> = HashSet::new();
    for y in 0..3 {
        let row = [cells[y * 3], cells[y * 3 + 1], cells[y * 3 + 2]];
        connect_line(row, rooms, tiles, MAP_WIDTH, &mut joined, rng);
    }
    for x in 0..3 {
        let col = [cells[x], cells[x + 3], cells[x + 6]];
        connect_line(col, rooms, tiles, MAP_WIDTH, &mut joined, rng);
    }
}

/// Up in the first room — where the player spawns — and down in a random other
/// one. Done here rather than at spawn time so the map rebuilt from the seed on
/// load, and the map for every new floor, carries the same stairs.
fn place_stairs(rng: &mut ChaCha12Rng, rooms: &mut [Rect], tiles: &mut [TileType]) {
    let up = rooms[0].center();
    tiles[tile_index(up.0 as u16, up.1 as u16)] = TileType::Upstairs;

    let down_room = if rooms.len() > 1 {
        rng.gen_range(1..rooms.len())
    } else {
        0
    };
    let down = random_point_in_room(&rooms[down_room], rng);
    tiles[tile_index(down.0, down.1)] = TileType::Downstairs;
}

/// Which rooms spawn unlit. Every room past the start room rolls
/// [`DARK_ROOM_CHANCE`]; a dark room's floor tiles are flagged so the
/// visibility system treats them like a passage until a wand of light goes off
/// in there.
fn roll_dark_rooms(rng: &mut ChaCha12Rng, rooms: &[Rect], tiles: &[TileType]) -> FixedBitSet {
    let mut dark = FixedBitSet::with_capacity(MAP_TILE_COUNT);
    for room in rooms.iter().skip(1) {
        if !rng.gen_bool(DARK_ROOM_CHANCE) {
            continue;
        }
        for y in room.y1..=room.y2 {
            for x in room.x1..=room.x2 {
                let (tx, ty) = (x as u16, y as u16);
                if tiles[tile_index(tx, ty)] == TileType::Room {
                    dark.insert(tile_index(tx, ty));
                }
            }
        }
    }
    dark
}
// ---------------------------------------------------------------------------
// Rooms and corridors
// ---------------------------------------------------------------------------

/// Carves `rect` into the tile grid as room floor.
fn create_room(rect: &Rect, tiles: &mut [TileType], map_width: u16) {
    for y in rect.y1..=rect.y2 {
        for x in rect.x1..=rect.x2 {
            let idx = (y as u16 * map_width + x as u16) as usize;
            tiles[idx] = TileType::Room;
        }
    }
}

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

/// Digs a corridor between each consecutive pair of present rooms along one
/// line (a row or a column) of the 3x3 grid, skipping any pair already joined.
fn connect_line(
    cells: [Option<usize>; 3],
    rooms: &[Rect],
    tiles: &mut [TileType],
    map_width: u16,
    connected: &mut HashSet<(usize, usize)>,
    rng: &mut ChaCha12Rng,
) {
    let mut prev: Option<usize> = None;
    for room_idx in cells.into_iter().flatten() {
        let Some(prev_idx) = prev.replace(room_idx) else {
            continue;
        };
        let pair = (prev_idx.min(room_idx), prev_idx.max(room_idx));
        if !connected.insert(pair) {
            continue;
        }
        let a = random_point_in_room(&rooms[prev_idx], rng);
        let b = random_point_in_room(&rooms[room_idx], rng);
        create_corridor(a, b, tiles, map_width);
    }
}

/// Digs an L-shaped corridor from `from` to `to`, turning the tiles where it
/// crosses a room's edge into doors.
fn create_corridor(from: (u16, u16), to: (u16, u16), tiles: &mut [TileType], map_width: u16) {
    let mut x = from.0;
    let mut y = from.1;
    let mut path = Vec::new();

    // Horizontal leg first, then vertical: an L, never a diagonal.
    while x != to.0 {
        path.push((x, y));
        match x < to.0 {
            true => x += 1,
            false => x -= 1,
        }
    }
    while y != to.1 {
        path.push((x, y));
        match y < to.1 {
            true => y += 1,
            false => y -= 1,
        }
    }
    path.push((x, y));

    // Now walk it, watching for the two tiles that matter: where the path
    // enters a room and where it leaves one. Those become doors; everything
    // between them is passage.
    let mut prev_was_room = false;
    for i in 0..path.len() {
        let (px, py) = path[i];
        let idx = (py * map_width + px) as usize;
        let is_room = tiles[idx] == TileType::Room;

        if i == 0 {
            prev_was_room = is_room;
            continue;
        }
        match (is_room, prev_was_room) {
            (true, false) => {
                // Stepped INTO a room. The previous tile becomes a door.
                let prev_idx = (path[i - 1].1 * map_width + path[i - 1].0) as usize;
                if tiles[prev_idx] != TileType::Room {
                    tiles[prev_idx] = TileType::Door;
                }
            }
            (false, true) => {
                // Stepped OUT of a room. The current tile becomes a door.
                tiles[idx] = TileType::Door;
            }
            (false, false) => {
                // Outside of a room, dig a regular passage.
                if tiles[idx] != TileType::Door {
                    tiles[idx] = TileType::Passage;
                }
            }
            (true, true) => {}
        }
        prev_was_room = is_room;
    }
}
/// The coordinate of the first tile of `want` in `tiles`, row-major.
pub(super) fn find_tile(tiles: &[TileType], want: TileType) -> Option<(u16, u16)> {
    tiles.iter().position(|&t| t == want).map(|i| {
        (
            (i % MAP_WIDTH as usize) as u16,
            (i / MAP_WIDTH as usize) as u16,
        )
    })
}
/// Builds the current floor's layout into the [`Map`] resource and returns the
/// player's starting tile. Reads [`Depth`] and [`RngSeed`]; leaves [`GameRng`]
/// untouched.
pub fn create_map(world: &mut World) -> ((u16, u16), Vec<Rect>) {
    let seed = world.resource::<RngSeed>().0;
    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);
    let (tiles, rooms, dark) = build_tiles(&mut layout_rng(seed, depth));

    world.insert_resource(Map { tiles, dark });

    // Return the center of the very first room so we can spawn the player safely away from doors
    let start_pos = rooms[0].center();
    ((start_pos.0 as u16, start_pos.1 as u16), rooms)
}

/// Rebuilds the [`Map`] resource for one floor, without touching the live
/// [`GameRng`] resource or spawning any actors. Used on load, where the map is
/// reconstructed from `(seed, depth)` rather than read out of the save file.
pub fn regenerate_map(world: &mut World, seed: u64, depth: u8) {
    let (tiles, _rooms, dark) = build_tiles(&mut layout_rng(seed, depth));
    world.insert_resource(Map { tiles, dark });
}

// What a floor is populated *with* is no longer decided here. Which creature,
// which item and which trap are weighted draws over the content tables
// themselves -- MonsterDef::pick, spawn::roll_item and TrapDef::pick. This file
// decides only how many and where.
/// The centre tile of every distinct corridor on the floor. A "corridor" is one
/// 4-connected blob of [`TileType::Passage`] tiles (doors and rooms break the
/// connection); its centre is the passage tile nearest the blob's centroid, so
/// an L-bend still resolves to a tile that is actually on the path.
pub(super) fn corridor_centers(tiles: &[TileType]) -> Vec<(u16, u16)> {
    let width = MAP_WIDTH as usize;
    let mut seen = vec![false; tiles.len()];
    let mut centers = Vec::new();

    for start in 0..tiles.len() {
        if seen[start] || tiles[start] != TileType::Passage {
            continue;
        }
        let blob = flood_corridor(start, width, tiles, &mut seen);
        centers.push(corridor_center(&blob, width));
    }

    centers
}

/// Flood-fills one 4-connected run of [`TileType::Passage`] tiles from `start`,
/// marking every tile it reaches in `seen` and returning them.
fn flood_corridor(start: usize, width: usize, tiles: &[TileType], seen: &mut [bool]) -> Vec<usize> {
    let mut stack = vec![start];
    let mut blob: Vec<usize> = Vec::new();
    seen[start] = true;
    while let Some(idx) = stack.pop() {
        blob.push(idx);
        let (x, y) = ((idx % width) as i32, (idx / width) as i32);
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let nidx = tile_index(nx as u16, ny as u16);
            if !seen[nidx] && tiles[nidx] == TileType::Passage {
                seen[nidx] = true;
                stack.push(nidx);
            }
        }
    }
    blob
}

/// The passage tile nearest a corridor blob's centroid — an L-bend still
/// resolves to a tile that is actually on the path.
fn corridor_center(blob: &[usize], width: usize) -> (u16, u16) {
    let n = blob.len() as i64;
    let (sx, sy) = blob.iter().fold((0i64, 0i64), |(sx, sy), &i| {
        (sx + (i % width) as i64, sy + (i / width) as i64)
    });
    let (cx, cy) = (sx / n, sy / n);
    let best = *blob
        .iter()
        .min_by_key(|&&i| {
            let dx = (i % width) as i64 - cx;
            let dy = (i / width) as i64 - cy;
            dx * dx + dy * dy
        })
        .unwrap();
    ((best % width) as u16, (best / width) as u16)
}
