//! The special levels: whole floors that are not Rogue's 3x3 of rooms.
//!
//! [`roll_special_level`] decides whether a floor is one, off a seed stream of
//! its own ([`level_rng`]) so the roll never moves an ordinary floor's layout.
//! [`carve`] builds whichever kind it lands on, and hands back the same two
//! things [`super::generate::build_floor`] does for an ordinary floor — the
//! [`Map`], and its rooms as floor tiles with the start room first — so
//! population never has to ask what shape it is stocking.

use fixedbitset::FixedBitSet;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use std::collections::{BTreeMap, HashSet};

use crate::constants::map::{
    BATTLEFIELD_CHANCE, BEE_WORLD_CHANCE, CASTLE_CHANCE, ISLAND_CHANCE, LABYRINTH_CHANCE,
    SPECIAL_LEVEL_MIN_DEPTH, VAULT_CHANCE,
};
use crate::rect::Rect;

use super::generate::{
    Grid, carve_rooms, connect_line, create_corridor, create_room, place_stairs,
    random_point_in_room, roll_dark_rooms, roll_table, room_floor_tiles,
};
use super::streams::level_rng;
use super::{
    FINAL_DEPTH, MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, Map, NEIGHBOUR_DIRS, Rooms, SpecialLevel,
    TileType, tile_index,
};

/// Which special level `depth` is on this seed, if any.
///
/// `NIHILURK_LEVEL` overrides the roll on every floor, depth gate and all —
/// the content author's way to look at a kind without hunting for a seed
/// that has one, the same kind of knob `NIHILURK_SPAWN` is. A value that
/// names no kind is ignored.
pub(super) fn roll_special_level(seed: u64, depth: u8) -> Option<SpecialLevel> {
    if let Some(forced) = std::env::var("NIHILURK_LEVEL")
        .ok()
        .and_then(|v| SpecialLevel::named(&v))
    {
        return Some(forced);
    }
    if !(SPECIAL_LEVEL_MIN_DEPTH..FINAL_DEPTH).contains(&depth) {
        return None;
    }
    const TABLE: [(f64, SpecialLevel); 6] = [
        (BATTLEFIELD_CHANCE, SpecialLevel::Battlefield),
        (LABYRINTH_CHANCE, SpecialLevel::Labyrinth),
        (VAULT_CHANCE, SpecialLevel::Vault),
        (BEE_WORLD_CHANCE, SpecialLevel::BeeWorld),
        (CASTLE_CHANCE, SpecialLevel::Castle),
        (ISLAND_CHANCE, SpecialLevel::Island),
    ];
    roll_table(&mut level_rng(seed, depth), &TABLE)
}

/// Carves `level` from the floor's layout stream: the finished [`Map`] and
/// its rooms, start room first, each one nothing but its own floor tiles.
pub(super) fn carve(level: SpecialLevel, rng: &mut ChaCha12Rng) -> (Map, Rooms) {
    let mut tiles = vec![TileType::Wall; MAP_TILE_COUNT];
    let mut dark = FixedBitSet::with_capacity(MAP_TILE_COUNT);
    let rooms = match level {
        SpecialLevel::Battlefield => battlefield(rng, &mut tiles),
        SpecialLevel::Labyrinth => labyrinth(rng, &mut tiles),
        SpecialLevel::Vault => vault(rng, &mut tiles),
        SpecialLevel::BeeWorld => bee_world(rng, &mut tiles),
        SpecialLevel::Castle => castle(rng, &mut tiles, &mut dark),
        SpecialLevel::Island => island(rng, &mut tiles),
    };
    let map = Map {
        tiles,
        dark,
        special: vec![None; MAP_TILE_COUNT],
        level: Some(level),
    };
    (map, rooms)
}

/// The tile coordinate of flat index `i`.
fn coord(i: usize) -> (u16, u16) {
    (
        (i % MAP_WIDTH as usize) as u16,
        (i / MAP_WIDTH as usize) as u16,
    )
}

/// Every tile of kind `want`, row-major.
fn tiles_of(tiles: &[TileType], want: TileType) -> Vec<(u16, u16)> {
    (0..tiles.len())
        .filter(|&i| tiles[i] == want)
        .map(coord)
        .collect()
}

/// Drops the up- and down-stair on two different random tiles of `floor` and
/// hands back the rest of it: the whole floor as its one room.
fn scatter_stairs(
    rng: &mut ChaCha12Rng,
    mut floor: Vec<(u16, u16)>,
    tiles: &mut [TileType],
) -> Rooms {
    for stair in [TileType::Upstairs, TileType::Downstairs] {
        let (x, y) = floor.swap_remove(rng.gen_range(0..floor.len()));
        tiles[tile_index(x, y)] = stair;
    }
    vec![floor]
}

// ---------------------------------------------------------------------------
// Battlefield
// ---------------------------------------------------------------------------

/// One room, wall to wall. It is lit like any room, so standing anywhere in
/// it shows the whole floor.
fn battlefield(rng: &mut ChaCha12Rng, tiles: &mut [TileType]) -> Rooms {
    let field = Rect::new(1, 1, MAP_WIDTH as i32 - 3, MAP_HEIGHT as i32 - 3);
    create_room(&field, tiles, MAP_WIDTH);
    scatter_stairs(rng, room_floor_tiles(&field, tiles), tiles)
}

// ---------------------------------------------------------------------------
// Labyrinth
// ---------------------------------------------------------------------------

/// Chance each wall still standing between two maze cells is knocked through
/// once the dig is done. A perfect maze has exactly one way anywhere; this is
/// what gives it a few more.
const LABYRINTH_LOOP_CHANCE: f64 = 0.1;

/// A maze of passages. Every odd-numbered tile is a cell and the tile between
/// two cells is the wall that can come down between them: a depth-first dig
/// from a random cell visits every one, then [`LABYRINTH_LOOP_CHANCE`] of the
/// walls left between cells are knocked through for loops. Passages only, so
/// sight never reaches past the 3x3.
fn labyrinth(rng: &mut ChaCha12Rng, tiles: &mut [TileType]) -> Rooms {
    // Cell (cx, cy) is tile (2cx + 1, 2cy + 1), which keeps the border solid.
    let (cols, rows) = ((MAP_WIDTH - 1) / 2, (MAP_HEIGHT - 1) / 2);
    let cell = |cx: u16, cy: u16| (cy * cols + cx) as usize;

    let mut dug = vec![false; cell(0, rows)];
    let start = (rng.gen_range(0..cols), rng.gen_range(0..rows));
    dug[cell(start.0, start.1)] = true;
    tiles[tile_index(2 * start.0 + 1, 2 * start.1 + 1)] = TileType::Passage;

    let mut stack = vec![start];
    while let Some(&(cx, cy)) = stack.last() {
        let next: Vec<(u16, u16)> = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .into_iter()
            .map(|(dx, dy)| (cx as i32 + dx, cy as i32 + dy))
            .filter(|&(nx, ny)| nx >= 0 && ny >= 0 && nx < cols as i32 && ny < rows as i32)
            .map(|(nx, ny)| (nx as u16, ny as u16))
            .filter(|&(nx, ny)| !dug[cell(nx, ny)])
            .collect();
        if next.is_empty() {
            stack.pop();
            continue;
        }
        let (nx, ny) = next[rng.gen_range(0..next.len())];
        dug[cell(nx, ny)] = true;
        tiles[tile_index(cx + nx + 1, cy + ny + 1)] = TileType::Passage;
        tiles[tile_index(2 * nx + 1, 2 * ny + 1)] = TileType::Passage;
        stack.push((nx, ny));
    }

    // The walls between two cells are the tiles with exactly one odd
    // coordinate. A pillar (both even) is never one: knocking it out would
    // open a 2x2 hall in the middle of a maze.
    for y in 1..2 * rows {
        for x in 1..2 * cols {
            let idx = tile_index(x, y);
            if (x + y) % 2 == 1
                && tiles[idx] == TileType::Wall
                && rng.gen_bool(LABYRINTH_LOOP_CHANCE)
            {
                tiles[idx] = TileType::Passage;
            }
        }
    }

    scatter_stairs(rng, tiles_of(tiles, TileType::Passage), tiles)
}

// ---------------------------------------------------------------------------
// Vault
// ---------------------------------------------------------------------------

/// Rows of cells in a vault's honeycomb, and cells across each unshifted row.
/// Every other row sits half a cell over and carries one more, cut in half
/// by the wall at either end, which is what makes the rows interlock like a
/// hive's.
const VAULT_ROWS: i32 = 3;
const VAULT_COLS: i32 = 6;

/// How far a cell's centre may wander off the lattice, in tiles — sideways,
/// then up or down. Enough that no two vaults are the same hive.
const VAULT_JITTER_X: i32 = 2;
const VAULT_JITTER_Y: i32 = 1;

/// What a tile of vertical distance counts for against a tile of horizontal,
/// when deciding which cell a tile belongs to. A terminal cell is about twice
/// as tall as it is wide; two keeps the cells hex-shaped on screen rather
/// than tall slivers.
const VAULT_STRETCH: i32 = 2;

/// A honeycomb of rooms: a voronoi partition of the floor around a jittered
/// hex lattice of cell centres, every pair of neighbouring cells joined by
/// exactly one door, and the two staircases in two different corner cells.
///
/// A tile is wall when any of its eight neighbours belongs to a lower-numbered
/// cell. Eight, not four: it keeps the floors of two cells from ever touching
/// corner to corner, which a diagonal step — or the flood that lights a
/// room — would otherwise slip straight through.
fn vault(rng: &mut ChaCha12Rng, tiles: &mut [TileType]) -> Rooms {
    let (w, h) = (MAP_WIDTH as i32 - 2, MAP_HEIGHT as i32 - 2);
    let mut centres = Vec::new();
    for row in 0..VAULT_ROWS {
        let shifted = row % 2 == 1;
        let y = 1 + (2 * row + 1) * h / (2 * VAULT_ROWS);
        for col in 0..VAULT_COLS + shifted as i32 {
            let x = 1 + (2 * col + !shifted as i32) * w / (2 * VAULT_COLS);
            let jx = rng.gen_range(-VAULT_JITTER_X..=VAULT_JITTER_X);
            let jy = rng.gen_range(-VAULT_JITTER_Y..=VAULT_JITTER_Y);
            centres.push(((x + jx).clamp(1, w), (y + jy).clamp(1, h)));
        }
    }

    // Which cell every interior tile belongs to; the border belongs to none.
    let mut cell = vec![usize::MAX; MAP_TILE_COUNT];
    for y in 1..=h {
        for x in 1..=w {
            cell[tile_index(x as u16, y as u16)] = (0..centres.len())
                .min_by_key(|&i| {
                    let (dx, dy) = (x - centres[i].0, (y - centres[i].1) * VAULT_STRETCH);
                    dx * dx + dy * dy
                })
                .expect("a vault has cells");
        }
    }
    for y in 1..=h {
        for x in 1..=w {
            let idx = tile_index(x as u16, y as u16);
            let bordered = NEIGHBOUR_DIRS
                .iter()
                .any(|&(dx, dy)| cell[tile_index((x + dx) as u16, (y + dy) as u16)] < cell[idx]);
            if !bordered {
                tiles[idx] = TileType::Room;
            }
        }
    }

    // A wall tile with two different cells' floor on opposite sides is a
    // place a door could go between them. Keyed in a `BTreeMap` so the pairs
    // are visited — and the dice spent — in the same order every time.
    let floor_cell = |tiles: &[TileType], x: u16, y: u16| {
        let idx = tile_index(x, y);
        (tiles[idx] == TileType::Room).then_some(cell[idx])
    };
    let mut doorways: BTreeMap<(usize, usize), Vec<(u16, u16)>> = BTreeMap::new();
    for y in 1..=h as u16 {
        for x in 1..=w as u16 {
            if tiles[tile_index(x, y)] != TileType::Wall {
                continue;
            }
            for ((ax, ay), (bx, by)) in [((x - 1, y), (x + 1, y)), ((x, y - 1), (x, y + 1))] {
                if let (Some(a), Some(b)) = (floor_cell(tiles, ax, ay), floor_cell(tiles, bx, by))
                    && a != b
                {
                    doorways
                        .entry((a.min(b), a.max(b)))
                        .or_default()
                        .push((x, y));
                }
            }
        }
    }
    for spots in doorways.values() {
        let (x, y) = spots[rng.gen_range(0..spots.len())];
        tiles[tile_index(x, y)] = TileType::Door;
    }

    // Two different corners, and in each the floor tile of the corner's own
    // cell that lies nearest it.
    let corners = [(1, 1), (w, 1), (1, h), (w, h)];
    let first = rng.gen_range(0..corners.len());
    let second = (first + rng.gen_range(1..corners.len())) % corners.len();
    let mut up_cell = 0;
    for (stair, (cx, cy)) in [
        (TileType::Upstairs, corners[first]),
        (TileType::Downstairs, corners[second]),
    ] {
        let home = cell[tile_index(cx as u16, cy as u16)];
        let spot = (0..MAP_TILE_COUNT)
            .filter(|&i| cell[i] == home && tiles[i] == TileType::Room)
            .min_by_key(|&i| {
                let (x, y) = coord(i);
                let (dx, dy) = (x as i32 - cx, y as i32 - cy);
                dx * dx + dy * dy
            })
            .expect("a corner cell has floor");
        tiles[spot] = stair;
        if stair == TileType::Upstairs {
            up_cell = home;
        }
    }

    let mut rooms = vec![Vec::new(); centres.len()];
    for i in 0..MAP_TILE_COUNT {
        if tiles[i] == TileType::Room {
            rooms[cell[i]].push(coord(i));
        }
    }
    rooms.swap(0, up_cell);
    rooms.retain(|room| !room.is_empty());
    rooms
}

// ---------------------------------------------------------------------------
// Bee World
// ---------------------------------------------------------------------------

/// Share of a bee world's interior that starts out as rock, before smoothing
/// turns the noise into caves.
const BEE_ROCK_CHANCE: f64 = 0.43;

/// Smoothing passes: each one makes a tile rock when five or more of the nine
/// tiles around and including it are rock, and floor otherwise.
const BEE_SMOOTHING_PASSES: usize = 5;

/// One cave: random rock smoothed into caverns by cellular automata, every
/// cavern but the biggest filled back in, and the stairs anywhere in what is
/// left.
fn bee_world(rng: &mut ChaCha12Rng, tiles: &mut [TileType]) -> Rooms {
    let interior =
        || (1..MAP_HEIGHT - 1).flat_map(|y| (1..MAP_WIDTH - 1).map(move |x| tile_index(x, y)));
    for idx in interior() {
        if !rng.gen_bool(BEE_ROCK_CHANCE) {
            tiles[idx] = TileType::Room;
        }
    }
    for _ in 0..BEE_SMOOTHING_PASSES {
        let before = tiles.to_vec();
        for idx in interior() {
            let rock = [(0, 0)]
                .iter()
                .chain(NEIGHBOUR_DIRS.iter())
                .filter(|&&(dx, dy)| {
                    let n = (idx as i32 + dy * MAP_WIDTH as i32 + dx) as usize;
                    before[n] == TileType::Wall
                })
                .count();
            tiles[idx] = match rock >= 5 {
                true => TileType::Wall,
                false => TileType::Room,
            };
        }
    }

    let cave = biggest_cave(tiles);
    for idx in interior() {
        tiles[idx] = TileType::Wall;
    }
    for &(x, y) in &cave {
        tiles[tile_index(x, y)] = TileType::Room;
    }
    scatter_stairs(rng, cave, tiles)
}

/// The largest 8-connected run of [`TileType::Room`] — 8 because a diagonal
/// step between two floor tiles is a legal step, so two caverns touching only
/// at a corner are one cave to anything walking it.
fn biggest_cave(tiles: &[TileType]) -> Vec<(u16, u16)> {
    let mut seen = vec![false; tiles.len()];
    let mut biggest = Vec::new();
    for start in 0..tiles.len() {
        if seen[start] || tiles[start] != TileType::Room {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![coord(start)];
        let mut cave = Vec::new();
        while let Some((x, y)) = stack.pop() {
            cave.push((x, y));
            for &(dx, dy) in &NEIGHBOUR_DIRS {
                let n = tile_index((x as i32 + dx) as u16, (y as i32 + dy) as u16);
                if !seen[n] && tiles[n] == TileType::Room {
                    seen[n] = true;
                    stack.push(coord(n));
                }
            }
        }
        if cave.len() > biggest.len() {
            biggest = cave;
        }
    }
    biggest
}

// ---------------------------------------------------------------------------
// Castle
// ---------------------------------------------------------------------------

/// The keep's top-left floor tile. The castle takes the middle column of
/// Rogue's grid, and this stands the keep in the middle of that column with
/// room above and below it for the towers.
const KEEP_X: i32 = 37;
const KEEP_Y: i32 = 7;

/// The keep's floor, and each tower's, to a side. A tower that small still
/// has room for a whole floor's stock.
const KEEP_SIZE: i32 = 7;
const TOWER_SIZE: i32 = 5;

/// The castle's five rooms, keep first. Each tower shares the keep's side
/// wall and overlaps two of its rows, which is where its one door goes; the
/// row midway down the keep is left clear between the towers for the gates.
/// [`castle`] carves these and population stocks them, so this is the one
/// place the castle's shape is written down.
pub(super) fn castle_rooms() -> [Rect; 5] {
    let side = TOWER_SIZE - 1; // `Rect::new` takes the far edge's offset
    let (west, east) = (KEEP_X - 1 - TOWER_SIZE, KEEP_X + KEEP_SIZE + 1);
    let (north, south) = (KEEP_Y + 2 - TOWER_SIZE, KEEP_Y + KEEP_SIZE - 2);
    [
        Rect::new(KEEP_X, KEEP_Y, KEEP_SIZE - 1, KEEP_SIZE - 1),
        Rect::new(west, north, side, side),
        Rect::new(east, north, side, side),
        Rect::new(west, south, side, side),
        Rect::new(east, south, side, side),
    ]
}

/// Rogue's rooms down the left and right columns of the grid, and the castle
/// in the middle one: the keep, a walled tower on each corner with one door
/// into it, and a gate on either side of the keep with a corridor out to the
/// middle room on that side. Only the side rooms can roll dark.
fn castle(rng: &mut ChaCha12Rng, tiles: &mut [TileType], dark: &mut FixedBitSet) -> Rooms {
    // The middle column is the castle's, and no side cell is ever left empty:
    // the gate corridors below count on the middle rooms being there.
    let (cells, mut rooms) = carve_rooms(rng, &Grid::new(), &[1, 4, 7], tiles);
    let mut joined = HashSet::new();
    for col in [0, 2] {
        let line = [cells[col], cells[col + 3], cells[col + 6]];
        connect_line(line, &rooms, tiles, MAP_WIDTH, &mut joined, rng);
    }

    for room in castle_rooms() {
        create_room(&room, tiles, MAP_WIDTH);
    }
    let (west_wall, east_wall) = ((KEEP_X - 1) as u16, (KEEP_X + KEEP_SIZE) as u16);
    for x in [west_wall, east_wall] {
        for y in [KEEP_Y, KEEP_Y + KEEP_SIZE - 1] {
            tiles[tile_index(x, y as u16)] = TileType::Door;
        }
    }
    let gate_y = (KEEP_Y + KEEP_SIZE / 2) as u16;
    for (x, cell) in [(west_wall, 3), (east_wall, 5)] {
        tiles[tile_index(x, gate_y)] = TileType::Door;
        let side = &rooms[cells[cell].expect("every side cell holds a room")];
        let to = random_point_in_room(side, rng);
        create_corridor((x, gate_y), to, tiles, MAP_WIDTH);
    }

    let side_rooms = rooms.len();
    rooms.extend(castle_rooms());
    place_stairs(rng, &mut rooms, tiles);
    *dark = roll_dark_rooms(rng, &rooms[..side_rooms], tiles, &vec![None; side_rooms]);
    rooms.iter().map(|r| room_floor_tiles(r, tiles)).collect()
}

// ---------------------------------------------------------------------------
// Island
// ---------------------------------------------------------------------------

/// The island's size: the half-width and half-height of its ellipse, each
/// rolled from its range, in tiles.
const ISLAND_HALF_WIDTH: std::ops::RangeInclusive<i32> = 14..=22;
const ISLAND_HALF_HEIGHT: std::ops::RangeInclusive<i32> = 5..=7;

/// How far each row of shore may reach past the true ellipse, or fall short
/// of it, so the coast never comes out as a clean curve.
const ISLAND_SHORE_JITTER: i32 = 2;

/// Land in the middle of deep water. Every row of the island is one unbroken
/// run of floor around the map's centre column, so however ragged the shore
/// comes out the land is one piece. It is room floor, and water carries
/// sight the way floor does, so the whole floor is in view from anywhere on
/// it. The map's own rim stays rock.
fn island(rng: &mut ChaCha12Rng, tiles: &mut [TileType]) -> Rooms {
    for y in 1..MAP_HEIGHT - 1 {
        for x in 1..MAP_WIDTH - 1 {
            tiles[tile_index(x, y)] = TileType::Water;
        }
    }
    let (cx, cy) = (MAP_WIDTH as i32 / 2, MAP_HEIGHT as i32 / 2);
    let rx = rng.gen_range(ISLAND_HALF_WIDTH) as f64;
    let ry = rng.gen_range(ISLAND_HALF_HEIGHT);
    // Strictly inside the ellipse: its top and bottom rows are a single tile
    // wide, which reads as a spike rather than a shore.
    for y in cy - ry + 1..cy + ry {
        let dy = (y - cy) as f64 / ry as f64;
        let curve = (rx * (1.0 - dy * dy).sqrt()).round() as i32;
        let half = (curve + rng.gen_range(-ISLAND_SHORE_JITTER..=ISLAND_SHORE_JITTER)).max(0);
        for x in cx - half..=cx + half {
            tiles[tile_index(x as u16, y as u16)] = TileType::Room;
        }
    }
    scatter_stairs(rng, tiles_of(tiles, TileType::Room), tiles)
}
