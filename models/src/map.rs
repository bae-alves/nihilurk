use bevy_ecs::prelude::*;
use crossterm::style::Color;
use fixedbitset::FixedBitSet;
use std::collections::HashSet;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

use crate::catalog::{
    spawn_ammo, spawn_armor, spawn_element_of_yoord, spawn_launcher, spawn_potion, spawn_weapon,
};
use crate::components::*;
use crate::equipment::equip_silently;
use crate::identify::{Identified, ItemAppearances};
use crate::monsters::{MonsterDef, spawn_monster};
use crate::rect::Rect;
use crate::spawn::{roll_item, spawn_requested};
use crate::state::*;

// --- Tuning constants -------------------------------------------------------
// All defined and documented in `constants.rs`; re-exported here so the old
// paths (`map::MAP_WIDTH`, `map::FINAL_DEPTH`, ...) keep resolving.
//
//   MAP_WIDTH / MAP_HEIGHT   playfield size in tiles (pins seed layout)
//   FINAL_DEPTH              deepest floor; holds the Element of Yoord
//   DUNGEON_LORD_PATIENCE    turns per level before the forced portal
//   DARK_ROOM_CHANCE         odds a room spawns unlit
//   DESCENT_HEAL_DIVISOR     staircase heal = max_hp / this
//   MONSTER_/TRAP_* , *_LURKER_*, *_ITEM*   floor-crowding budgets
//   DIFFICULTY_TIER_LAST_DEPTH   the depths every budget steps up at
pub use crate::constants::map::{HEIGHT as MAP_HEIGHT, WIDTH as MAP_WIDTH};
pub use crate::constants::progression::{DUNGEON_LORD_PATIENCE, FINAL_DEPTH};

use crate::constants::map::DARK_ROOM_CHANCE;
use crate::constants::player::{SIGHT_RANGE, START_ARMOR, START_HP, START_MAGIC, START_POWER};
use crate::constants::population::{
    CORRIDOR_LURKER_CHANCE, CORRIDOR_LURKER_MIN_DEPTH, HIDDEN_ITEM_CHANCE, ITEM_SLOTS_BASE,
    MONSTER_FILL_CHANCE_BASE, MONSTER_FILL_CHANCE_CAP, MONSTER_FILL_CHANCE_PER_TIER,
    MONSTER_SLOTS_BASE, TRAP_FILL_CHANCE_BASE, TRAP_FILL_CHANCE_CAP, TRAP_FILL_CHANCE_PER_TIER,
    TRAP_SLOTS_BASE,
};
use crate::constants::progression::{DESCENT_HEAL_DIVISOR, DIFFICULTY_TIER_LAST_DEPTH};

#[derive(Resource)]
pub struct GameRng(pub ChaCha12Rng);

/// The u64 seed the run's RNG was created from. Kept alongside the live RNG
/// state so future features (e.g. regenerating a specific floor) can reseed.
#[derive(Resource)]
pub struct RngSeed(pub u64);

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

/// Per-tile record of where a bleeding creature (anything with [`Blood`]) has
/// been hurt. Purely cosmetic: the renderer paints these tiles with a red
/// background while they are in the player's viewshed. Rebuilt per floor and not
/// saved, like the map itself.
#[derive(Resource)]
pub struct BloodStains {
    tiles: FixedBitSet,
    /// When `false` (the `-nb` flag) no tile is ever stained.
    pub enabled: bool,
}

impl BloodStains {
    pub fn new() -> Self {
        Self {
            tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            enabled: true,
        }
    }

    /// Marks the tile at `(x, y)` bloody (unless blood is disabled).
    pub fn stain(&mut self, x: u16, y: u16) {
        if self.enabled && x < MAP_WIDTH && y < MAP_HEIGHT {
            self.tiles.insert(tile_index(x, y));
        }
    }

    pub fn is_bloody(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.tiles.contains(tile_index(x, y))
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

impl Default for BloodStains {
    fn default() -> Self {
        Self::new()
    }
}

/// Lingering smoke from a fire blast — unlike a [`BloodStains`] mark, a puff
/// fades on its own after a few turns instead of staying for the floor's
/// life, DCSS-style. Purely cosmetic: it never blocks movement or sight.
/// Rebuilt per floor and not saved, like the map itself.
#[derive(Resource)]
pub struct Smoke {
    /// Turns left before each tile's puff burns off; 0 means clear.
    turns_left: Vec<u8>,
}

impl Smoke {
    pub fn new() -> Self {
        Self {
            turns_left: vec![0; MAP_TILE_COUNT],
        }
    }

    /// Lays (or refreshes) a puff of smoke on `(x, y)`, good for `turns` more
    /// calls to [`Smoke::tick`].
    pub fn puff(&mut self, x: u16, y: u16, turns: u8) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            let slot = &mut self.turns_left[tile_index(x, y)];
            *slot = (*slot).max(turns);
        }
    }

    pub fn is_smoky(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.turns_left[tile_index(x, y)] > 0
    }

    /// Ages every puff down by one turn — one call per game turn.
    pub fn tick(&mut self) {
        for slot in &mut self.turns_left {
            *slot = slot.saturating_sub(1);
        }
    }

    pub fn clear(&mut self) {
        self.turns_left.fill(0);
    }
}

impl Default for Smoke {
    fn default() -> Self {
        Self::new()
    }
}

/// Schedule step: ages every lingering smoke puff down by one turn.
pub fn smoke_system(world: &mut World) {
    world.resource_mut::<Smoke>().tick();
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

/// Helper function to carve a room into the tiles grid
fn create_room(rect: &Rect, tiles: &mut [TileType], map_width: u16) {
    for y in rect.y1..=rect.y2 {
        for x in rect.x1..=rect.x2 {
            let idx = (y as u16 * map_width + x as u16) as usize;
            tiles[idx] = TileType::Room;
        }
    }
}

/// Helper function to pick a random interior point within a room
fn random_point_in_room(room: &Rect, rng: &mut ChaCha12Rng) -> (u16, u16) {
    let width = (room.x2 - room.x1 + 1).max(1) as u32;
    let height = (room.y2 - room.y1 + 1).max(1) as u32;

    // Use gen_range instead of gen() % width
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

/// Helper function to create a corridor and place doors automatically
fn create_corridor(from: (u16, u16), to: (u16, u16), tiles: &mut [TileType], map_width: u16) {
    let mut x = from.0;
    let mut y = from.1;
    let mut path = Vec::new();

    // 1. Calculate the L-shaped path
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
    path.push((x, y)); // Add the final destination

    // 2. Walk the path and check for room transitions
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

/// The coordinate of the first tile of `want` in `tiles`, row-major.
fn find_tile(tiles: &[TileType], want: TileType) -> Option<(u16, u16)> {
    tiles.iter().position(|&t| t == want).map(|i| {
        (
            (i % MAP_WIDTH as usize) as u16,
            (i / MAP_WIDTH as usize) as u16,
        )
    })
}

/// Whether the player is currently carrying the Element of Yoord.
pub fn holding_element_of_yoord(world: &mut World) -> bool {
    let items: Vec<Entity> = match world
        .query_filtered::<&Backpack, With<Player>>()
        .iter(world)
        .next()
    {
        Some(bp) => bp.items.clone(),
        None => return false,
    };
    items.iter().any(|&e| world.get::<Amulet>(e).is_some())
}

/// Total tile count; the length of a fog-of-war bitset.
pub const MAP_TILE_COUNT: usize = MAP_WIDTH as usize * MAP_HEIGHT as usize;

/// Flattens a tile coordinate into a bitset/array index.
#[inline]
pub const fn tile_index(x: u16, y: u16) -> usize {
    y as usize * MAP_WIDTH as usize + x as usize
}

/// Procedurally computes a map layout from the given RNG. Pure: the same RNG
/// state always yields the same tiles, which is what lets us drop the map from
/// save files and rebuild it from the seed on load.
fn build_tiles(rng: &mut ChaCha12Rng) -> (Vec<TileType>, Vec<Rect>, FixedBitSet) {
    let map_width: u16 = MAP_WIDTH;
    let map_height: u16 = MAP_HEIGHT;

    // Initialize map
    let mut tiles = vec![TileType::Wall; (map_width * map_height) as usize];
    let mut rooms: Vec<Rect> = Vec::new();

    // THE ROGUE GENERATION ALGORITHM WITH STRICT PADDING & 3-TILE GUTTERS
    let gutter_size: u16 = 3;
    let padding: u16 = 1;
    let num_sections: u16 = 3;
    let num_gutters: u16 = num_sections - 1_u16;

    // Usable space = Total - (Padding * 2) - (Gutters * GutterSize)
    let usable_width: u16 = map_width
        .saturating_sub(padding * 2_u16)
        .saturating_sub(num_gutters * gutter_size);
    let usable_height: u16 = map_height
        .saturating_sub(padding * 2_u16)
        .saturating_sub(num_gutters * gutter_size);

    let section_width: u16 = usable_width / num_sections;
    let section_height: u16 = usable_height / num_sections;

    // Center the grid by adding half the remainder space to the initial padding
    let offset_x: u16 = padding + (usable_width % num_sections) / 2_u16;
    let offset_y: u16 = padding + (usable_height % num_sections) / 2_u16;

    let gone_sections_count = rng.gen_range(0..4);

    let mut sections = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    for i in 0..gone_sections_count {
        let swap_idx = i + rng.gen_range(0..(9 - i));
        sections.swap(i, swap_idx);
    }
    let gone_sections = &sections[..gone_sections_count];

    let min_room_w: u16 = 4;
    // Reduced to 3. A 4-high room physically occupies 5 tiles, which overflows the 4-tile tall sections.
    let min_room_h: u16 = 3;

    let mut grid_rooms: [Option<usize>; 9] = [None; 9];

    for section_y in 0_u16..3_u16 {
        for section_x in 0_u16..3_u16 {
            let section_index = (section_y * 3_u16 + section_x) as usize;
            if gone_sections.contains(&section_index) {
                continue;
            }

            // Safely calculate maximum room dimensions so they NEVER bleed out of their section.
            // Subtracting 1_u16 accounts for Rect inclusive bounding (which adds +1 to actual footprint).
            let max_room_w = section_width.saturating_sub(1_u16).max(min_room_w);
            let max_room_h = section_height.saturating_sub(1_u16).max(min_room_h);

            let width_range = (max_room_w - min_room_w) + 1_u16;
            let room_width: u16 = min_room_w + rng.gen_range(0..width_range);

            let height_range = (max_room_h - min_room_h) + 1_u16;
            let room_height: u16 = min_room_h + rng.gen_range(0..height_range);

            // Base position includes calculated offset + section offset + 3-tile gutter per section step
            let base_x = offset_x + (section_x * section_width) + (section_x * gutter_size);
            let base_y = offset_y + (section_y * section_height) + (section_y * gutter_size);

            // Calculate max placement offset from base so the far wall stays completely inside the section
            let max_offset_x = section_width.saturating_sub(room_width + 1_u16);
            let room_x = base_x + rng.gen_range(0..=max_offset_x);

            let max_offset_y = section_height.saturating_sub(room_height + 1_u16);
            let room_y = base_y + rng.gen_range(0..=max_offset_y);

            let room = Rect::new(
                room_x as i32,
                room_y as i32,
                room_width as i32,
                room_height as i32,
            );
            grid_rooms[section_index] = Some(rooms.len());
            create_room(&room, &mut tiles, map_width);
            rooms.push(room);
        }
    }

    let mut connected_pairs: HashSet<(usize, usize)> = HashSet::new();

    // Connect consecutive rooms along each row, then each column of the 3x3
    // grid. A pair joined by the first pass is skipped by the second.
    for y in 0..3 {
        let row = [
            grid_rooms[y * 3],
            grid_rooms[y * 3 + 1],
            grid_rooms[y * 3 + 2],
        ];
        connect_line(
            row,
            &rooms,
            &mut tiles,
            map_width,
            &mut connected_pairs,
            rng,
        );
    }
    for x in 0..3 {
        let col = [grid_rooms[x], grid_rooms[x + 3], grid_rooms[x + 6]];
        connect_line(
            col,
            &rooms,
            &mut tiles,
            map_width,
            &mut connected_pairs,
            rng,
        );
    }

    // Staircases: up in the first room (where the player spawns), down in a
    // random other room. Done here so the map rebuilt from the seed on load —
    // and the map for every new floor — carries the same stairs.
    let up = rooms[0].center();
    tiles[tile_index(up.0 as u16, up.1 as u16)] = TileType::Upstairs;
    let down_room = if rooms.len() > 1 {
        rng.gen_range(1..rooms.len())
    } else {
        0
    };
    let down = random_point_in_room(&rooms[down_room], rng);
    tiles[tile_index(down.0, down.1)] = TileType::Downstairs;

    // Dark rooms: every room past the start room (room 0) has a small chance of
    // being unlit. A dark room's Room floor tiles are flagged so the visibility
    // system treats them like a passage until a wand of light is used there.
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

    (tiles, rooms, dark)
}

/// One floor's private RNG stream. `salt` picks *which* stream — the walls and
/// the things standing between them draw from two independent ones, so neither
/// can move the other — and `generation` advances a stream to a fresh state
/// without changing which stream it is (used to re-roll a floor's contents on a
/// repeat visit; `0` for the layout, which never changes).
fn floor_stream(seed: u64, depth: u8, salt: u64, generation: u64) -> ChaCha12Rng {
    let base = seed ^ salt.wrapping_mul(depth as u64 + 1);
    ChaCha12Rng::seed_from_u64(base ^ generation.wrapping_mul(GENERATION_SALT))
}

const LAYOUT_SALT: u64 = 0xF100_0BED_5EED;
const CONTENT_SALT: u64 = 0x0C0F_FEE0_D00D;
/// Odd multiplier that scatters [`FloorChanges`] across the seed space, so
/// consecutive visits to a floor are as unlike each other as two random seeds.
const GENERATION_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

/// The RNG a floor's **layout** is built from — rooms, corridors, doors, stairs.
///
/// This is the same trick [`initialize_world`] plays for item appearances, and
/// for the same reason. A floor's shape is a pure function of `(seed, depth)`,
/// so nothing else can move it: not the loot rolls, not a new row in a content
/// table, not how long the player spent fighting on the way down. Two
/// consequences worth knowing:
///
/// * A save can rebuild the exact floor it was written on from the seed and the
///   depth alone, which is why the map is not stored in the file.
/// * Climbing back to a floor you have already visited gives you the layout you
///   remember (the contents, though, are re-rolled — see [`content_rng`]).
pub fn layout_rng(seed: u64, depth: u8) -> ChaCha12Rng {
    floor_stream(seed, depth, LAYOUT_SALT, 0)
}

/// The RNG a floor's **contents** are drawn from — which monsters, which loot,
/// which traps, where they stand, and what a drop rolls for enchantment and
/// charges.
///
/// A different stream from [`layout_rng`], and a function of `(seed, depth,
/// changes)` where `changes` is [`crate::components::FloorChanges`] — how many
/// times the player has taken a staircase, portal or trapdoor. So the *layout*
/// of a floor is fixed for a seed, but its *contents* change every time it is
/// built: walk back up through floor 7 and it is the same maze re-stocked with
/// different monsters and loot. Two runs on one seed that descend in lockstep
/// still see the same floors; a reload restores `changes`, so it lands on the
/// same re-roll.
///
/// This is not the shared [`GameRng`] and must never be. `GameRng` is the live
/// stream the *run* spends — combat rolls, item effects, traps springing — and
/// anything drawn from it while a floor is being built would tie that floor back
/// to the player's blow-by-blow history rather than to the clean
/// staircase count. `models/tests/determinism.rs` exists to catch that.
pub fn content_rng(seed: u64, depth: u8, changes: u32) -> ChaCha12Rng {
    floor_stream(seed, depth, CONTENT_SALT, changes as u64)
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
fn corridor_centers(tiles: &[TileType]) -> Vec<(u16, u16)> {
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

/// Tries up to 100 times to reserve a free tile in a random room other than the
/// start room (index 0). Returns the tile it claimed in `occupied`, or `None`.
fn claim_random_spot(
    rooms: &[Rect],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
) -> Option<(u16, u16)> {
    for _ in 0..100 {
        let room_idx = rng.gen_range(1..rooms.len());
        let spot = random_point_in_room(&rooms[room_idx], rng);
        if occupied.insert(spot) {
            return Some(spot);
        }
    }
    None
}

/// On the deepest floor, replaces the down-stair with the Element of Yoord. A
/// no-op on every shallower floor.
fn place_element_of_yoord(world: &mut World, occupied: &mut HashSet<(u16, u16)>, depth: u8) {
    if depth < FINAL_DEPTH {
        return;
    }
    let Some((ex, ey)) = find_tile(&world.resource::<Map>().tiles, TileType::Downstairs) else {
        return;
    };
    world.resource_mut::<Map>().tiles[tile_index(ex, ey)] = TileType::Room;
    spawn_element_of_yoord(world, Position { x: ex, y: ey });
    occupied.insert((ex, ey));
}

/// One trap attempt: up to 100 tries to find a free room tile that isn't the
/// player's landing spot, then spawns a random trap there.
fn place_one_trap(
    world: &mut World,
    rooms: &[Rect],
    occupied: &mut HashSet<(u16, u16)>,
    rng: &mut ChaCha12Rng,
    depth: u8,
    player_start: (u16, u16),
) {
    for _ in 0..100 {
        let room_idx = rng.gen_range(0..rooms.len());
        let (x, y) = random_point_in_room(&rooms[room_idx], rng);
        if (x, y) == player_start {
            continue;
        }
        if world.resource::<Map>().tiles[tile_index(x, y)] != TileType::Room {
            continue;
        }
        if occupied.insert((x, y)) {
            world.spawn(crate::TrapBundle::random(rng, depth, Position { x, y }));
            return;
        }
    }
}

/// Which floor-crowding tier `depth` falls in — `0` on the shallowest floors,
/// rising by one at each boundary in [`DIFFICULTY_TIER_LAST_DEPTH`]. The
/// monster, trap and item budgets all read this. `[3, 6, 9, 12]` gives five tiers:
/// depths 1-3, 4-6, 7-9, 10-12, and 13 on its own. (The damage traps scale on
/// their own coarser bands — `traps::trap_damage_tier`.)
pub fn difficulty_tier(depth: u8) -> u32 {
    DIFFICULTY_TIER_LAST_DEPTH
        .iter()
        .position(|&last| depth <= last)
        .unwrap_or(DIFFICULTY_TIER_LAST_DEPTH.len()) as u32
}

/// Spawns the monsters and items for a freshly built floor. The staircases are
/// carved by [`build_tiles`]. Shared by [`initialize_world`] and [`change_level`].
fn populate_level(world: &mut World, rooms: &[Rect], player_start: (u16, u16)) {
    let (player_x, player_y) = player_start;

    let mut occupied = HashSet::new();

    // The player's tile is already occupied.
    occupied.insert((player_x, player_y));

    // Rough danger tier: deeper floors unlock nastier letters.
    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);

    // Once the Element of Yoord is in the pack, the climb out lifts every depth
    // gate: each floor draws from the whole bestiary, so a dragon can be waiting
    // on floor 1. `pick_species` routes every spawn below through the right draw.
    let anything_goes = holding_element_of_yoord(world);
    let pick_species = |rng: &mut ChaCha12Rng| match anything_goes {
        true => MonsterDef::pick_any(rng),
        false => MonsterDef::pick(depth, rng),
    };

    // Everything below draws from this floor's own stream, never the shared
    // `GameRng` — see [`content_rng`]. The stream is keyed off the staircase
    // count, so a repeat visit re-stocks the same layout; but nothing the
    // player did *on* a floor (fighting, looting) can reach into how the next
    // one is built.
    let seed = world.resource::<RngSeed>().0;
    let changes = world.get_resource::<FloorChanges>().map_or(0, |c| c.count);
    let mut rng = content_rng(seed, depth, changes);

    // Both the monster and trap budgets step up in five depth bands (1-3, 4-6,
    // 7-9, 10-12, and 13 alone — see `difficulty_tier`). Each tier grants one
    // more spawn slot and widens the odds that a given slot actually fills, so
    // the dungeon gets more crowded and more dangerous the deeper you go.
    let tier = difficulty_tier(depth);

    // Monsters: three slots at the surface, +1 per tier. The first slot always
    // fills (no floor is ever completely empty); every later slot fills with a
    // probability that itself climbs one step per tier (capped so a slot is
    // never quite certain).
    let max_monsters = MONSTER_SLOTS_BASE + tier as usize;
    let monster_chance = (MONSTER_FILL_CHANCE_BASE + MONSTER_FILL_CHANCE_PER_TIER * tier as f64)
        .min(MONSTER_FILL_CHANCE_CAP);
    for slot in 0..max_monsters {
        if slot > 0 && !rng.gen_bool(monster_chance) {
            continue;
        }
        let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) else {
            continue;
        };
        let def = pick_species(&mut rng);
        spawn_monster(world, def, Position { x, y });
    }

    // From depth 7 on, every corridor also has a small (5%) chance of hiding a
    // lurker dead centre — right where an unwary traveller runs into it.
    if depth >= CORRIDOR_LURKER_MIN_DEPTH {
        let centers = corridor_centers(&world.resource::<Map>().tiles);
        for (cx, cy) in centers {
            if !rng.gen_bool(CORRIDOR_LURKER_CHANCE) {
                continue;
            }
            if occupied.insert((cx, cy)) {
                let def = pick_species(&mut rng);
                spawn_monster(world, def, Position { x: cx, y: cy });
            }
        }
    }

    // Items: three attempts at the surface, +1 per tier (like the monster and
    // trap budgets). Every attempt that finds a free tile drops an item.
    let max_items = ITEM_SLOTS_BASE + tier as usize;
    for _ in 0..max_items {
        let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) else {
            continue;
        };
        roll_item(world, &mut rng, depth, Position { x, y });
    }

    // 1 floor in 5 hides an extra item in plain sight: it draws nothing and is
    // never announced until a ring of perception turns it up or the player walks
    // straight onto it ("Hey! There's something here!").
    if rng.gen_bool(HIDDEN_ITEM_CHANCE) {
        if let Some((x, y)) = claim_random_spot(rooms, &mut occupied, &mut rng) {
            let item = roll_item(world, &mut rng, depth, Position { x, y });
            world.entity_mut(item).insert((Hidden, Invisible));
        }
    }

    place_element_of_yoord(world, &mut occupied, depth);

    // Traps: placed after the stairs, monsters and loot, before the hero drops
    // in. Like the monster budget, the trap budget steps up a tier at a time —
    // four slots at the surface, +1 per `tier` — and each slot's chance of
    // producing a trap climbs the same way, so the deep floors bristle with them
    // and the first floors rarely hold more than one.
    let max_traps = TRAP_SLOTS_BASE + tier as usize;
    let trap_chance =
        (TRAP_FILL_CHANCE_BASE + TRAP_FILL_CHANCE_PER_TIER * tier as f64).min(TRAP_FILL_CHANCE_CAP);
    for _ in 0..max_traps {
        if !rng.gen_bool(trap_chance) {
            continue;
        }
        place_one_trap(world, rooms, &mut occupied, &mut rng, depth, player_start);
    }

    // Last of all, whatever the content author asked for on the command line.
    spawn_requested(
        world,
        Position {
            x: player_x,
            y: player_y,
        },
        &mut occupied,
    );
}

/// Handles the player using a staircase.
///
/// Without the Element of Yoord the descent rules apply: `>` on a
/// [`TileType::Downstairs`] works, `<` is blocked by the Dungeon Lord's power.
/// Once the Element is in the pack the rules invert — `<` on a
/// [`TileType::Upstairs`] carries the player back up and `>` is dead. On success
/// a fresh floor is built, the player repositioned, [`Depth`] adjusted, 50% of
/// max HP restored and `true` returned (a turn passes); otherwise a log line is
/// added and `false` returned so no turn is consumed.
pub fn change_level(world: &mut World, going_down: bool) -> bool {
    let player_entity = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
        .unwrap();
    let player_pos = *world.get::<Position>(player_entity).unwrap();
    let tile = world.resource::<Map>().tile(player_pos.x, player_pos.y);
    let has_element = holding_element_of_yoord(world);

    if going_down {
        if has_element {
            world
                .resource_mut::<GameLog>()
                .add(if tile == TileType::Downstairs {
                    "The Element of Yoord seeks the sun; it will not let you descend."
                } else {
                    "You cannot go down from here."
                });
            return false;
        }
        if tile != TileType::Downstairs {
            world
                .resource_mut::<GameLog>()
                .add("You cannot go down from here.");
            return false;
        }
        transition_level(world, true, LevelChange::Stairs);
        return true;
    }

    // Going up.
    if !has_element {
        world
            .resource_mut::<GameLog>()
            .add(if tile == TileType::Upstairs {
                "The Dungeon Lord's power prevents you from going upstairs."
            } else {
                "You cannot go up from here."
            });
        return false;
    }
    if tile != TileType::Upstairs {
        world
            .resource_mut::<GameLog>()
            .add("You cannot go up from here.");
        return false;
    }
    if world.resource::<Depth>().what <= 1 {
        // The surface at last — and only ever by the player's own hand on the
        // stair. The run is won.
        if let Some(mut ending) = world.get_resource_mut::<Ending>() {
            ending.player_won = true;
        }
        world.resource_mut::<GameLog>().add(
            "You climb the last stair into open sky, the Element of Yoord blazing in your hands.",
        );
        return true;
    }
    transition_level(world, false, LevelChange::Stairs);
    true
}

/// Why the player is being moved between floors — only affects the log line and
/// whether the arrival heal applies (a trapdoor plunge does not heal).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelChange {
    Stairs,
    Portal,
    Trapdoor,
}

/// Moves the player one floor in the given direction: clears the current floor,
/// builds the adjacent one, repositions the player (on the up-stair when
/// descending, on the down-stair when ascending), re-populates, adjusts
/// [`Depth`], heals 50% of max HP and resets the Dungeon Lord's patience.
/// `cause` only changes the log line and — for [`LevelChange::Trapdoor`] —
/// suppresses the arrival heal.
pub(crate) fn transition_level(world: &mut World, going_down: bool, cause: LevelChange) {
    let player_entity = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
        .unwrap();

    // Gear a monster picked up is carried without a Position, so lay it out on
    // the floor first — otherwise the sweep below walks straight past it and it
    // haunts the save forever.
    let armed_mobs: Vec<(Entity, Position)> = world
        .query_filtered::<(Entity, &Position), (With<Mob>, Without<Player>)>()
        .iter(world)
        .map(|(e, p)| (e, *p))
        .collect();
    for (mob, pos) in armed_mobs {
        crate::equipment::drop_equipment(world, mob, pos);
    }

    // Despawn every monster and every item lying on the floor. Backpack contents
    // (which carry no Position) are left untouched.
    let backpacked: HashSet<Entity> = world
        .query::<&Backpack>()
        .iter(world)
        .flat_map(|bp| bp.items.iter().copied())
        .collect();
    let to_despawn: Vec<Entity> = world
        .iter_entities()
        .filter(|e| {
            !e.contains::<Player>() && e.contains::<Position>() && !backpacked.contains(&e.id())
        })
        .map(|e| e.id())
        .collect();
    for e in to_despawn {
        world.despawn(e);
    }

    // Settle the new depth before building anything: a floor's layout is a pure
    // function of (seed, depth) — see [`layout_rng`] — so the depth has to be
    // known first.
    let depth = {
        let mut d = world.resource_mut::<Depth>();
        d.what = if going_down {
            d.what.saturating_add(1)
        } else {
            d.what.saturating_sub(1).max(1)
        };
        d.what
    };

    // Every floor change re-rolls the destination's contents (not its layout):
    // `content_rng` reads this count, so the same corridors come back stocked
    // differently. Bumped before `populate_level` runs.
    if let Some(mut fc) = world.get_resource_mut::<FloorChanges>() {
        fc.count = fc.count.saturating_add(1);
    }

    let seed = world.resource::<RngSeed>().0;
    let (tiles, rooms, dark) = build_tiles(&mut layout_rng(seed, depth));
    world.insert_resource(Map { tiles, dark });
    world.resource_mut::<BloodStains>().clear();
    world.resource_mut::<Smoke>().clear();

    let fallback = {
        let c = rooms[0].center();
        (c.0 as u16, c.1 as u16)
    };
    // Descending drops you on the new floor's up-stair (its first room);
    // ascending brings you out at the shallower floor's down-stair.
    let start = if going_down {
        fallback
    } else {
        find_tile(&world.resource::<Map>().tiles, TileType::Downstairs).unwrap_or(fallback)
    };

    if let Some(mut pos) = world.get_mut::<Position>(player_entity) {
        pos.x = start.0;
        pos.y = start.1;
    }
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(player_entity) {
        viewshed.visible_tiles.clear();
        viewshed.revealed_tiles.clear();
        viewshed.dirty = true;
    }

    populate_level(world, &rooms, start);

    // A trapdoor plunge is a fall, not a rest: no arrival heal, no magic restore.
    if cause != LevelChange::Trapdoor {
        if let Some(mut fighter) = world.get_mut::<Fighter>(player_entity) {
            let heal = fighter.max_hp / DESCENT_HEAL_DIVISOR;
            fighter.hp = (fighter.hp + heal).min(fighter.max_hp);
        }
        // A staircase also refills the magic pool in full.
        if let Some(mut magic) = world.get_mut::<Magic>(player_entity) {
            magic.points = magic.max_points;
        }
    }

    if let Some(mut dl) = world.get_resource_mut::<DungeonLord>() {
        dl.idle_turns = 0;
    }

    // Transient conditions (haste, slow, dazzle) are treacherous but they do not
    // survive a staircase — using one is one of only two things that clears them.
    crate::helpers::clear_player_conditions(world, player_entity);

    let msg = match cause {
        // Descending, it is the Dungeon Lord who wrenches you down; once you
        // carry the Element it is the Element that tears the way open upward.
        LevelChange::Portal if going_down => {
            format!(
                "The Dungeon Lord opens a portal beneath your feet! You fall downward. (Depth {depth})"
            )
        }
        LevelChange::Portal => {
            format!(
                "The Element of Yoord flares and rips a portal above your head! You rise upward. (Depth {depth})"
            )
        }
        LevelChange::Trapdoor => {
            format!("You crash down onto the floor below in a shower of dust. (Depth {depth})")
        }
        LevelChange::Stairs if going_down => format!("You descend the stairs. (Depth {depth})"),
        LevelChange::Stairs => format!("You climb the stairs. (Depth {depth})"),
    };
    world.resource_mut::<GameLog>().add(msg);
}

/// Exclusive system, run each turn just before visibility is recomputed. Ages
/// the Dungeon Lord's patience; when it runs out, a portal shunts the player to
/// the next level — deeper on the way in, back up once they carry the Element of
/// Yoord. On the deepest floor (without the Element) or the shallowest floor
/// (with it) the portal has nowhere to send them and only flickers.
pub fn dungeon_lord_system(world: &mut World) {
    if world
        .get_resource::<Ending>()
        .map(|e| e.player_dead)
        .unwrap_or(false)
    {
        return;
    }
    match world.get_resource_mut::<DungeonLord>() {
        Some(mut dl) => {
            dl.idle_turns += 1;
            if dl.idle_turns < DUNGEON_LORD_PATIENCE {
                return;
            }
            dl.idle_turns = 0;
        }
        None => return,
    }

    let has_element = holding_element_of_yoord(world);
    let depth = world.resource::<Depth>().what;

    if has_element {
        if depth <= 1 {
            world
                .resource_mut::<GameLog>()
                .add("The Element of Yoord strains toward the sun — but the last stair you must climb yourself.");
            return;
        }
        transition_level(world, false, LevelChange::Portal);
        return;
    }
    if depth >= FINAL_DEPTH {
        world
            .resource_mut::<GameLog>()
            .add("The Dungeon Lord claws at the floor, but there is nowhere deeper to cast you.");
        return;
    }
    transition_level(world, true, LevelChange::Portal);
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    world.insert_resource(Depth { what: 1 });
    world.insert_resource(FloorChanges::default());
    world.insert_resource(BloodStains::new());
    world.insert_resource(Smoke::new());
    world.init_resource::<crate::magicmap::MagicMapReveal>();
    world.insert_resource(Identified::default());
    // This run's cosmetic appearance for every unidentified item type. Drawn
    // from a separate RNG keyed off the same seed (so a given seed always
    // shuffles the same way) rather than the shared `GameRng` stream, so
    // adding new appearance pools here never perturbs dungeon/loot rolls.
    let seed = world.resource::<RngSeed>().0;
    let mut appearance_rng = ChaCha12Rng::seed_from_u64(seed ^ 0x1DEA_5117_FEED_u64);
    world.insert_resource(ItemAppearances::generate(&mut appearance_rng));

    let ((player_x, player_y), rooms) = create_map(world);

    // 1. Roll up the starting gear. Every piece is spawned at the origin like a
    //    drop, then lifted straight into the pack (Position stripped, the way a
    //    picked-up item loses it) so it never shows up as floor loot. The armour,
    //    mace and bow are handed over enchanted to +1 rather than rolled; the
    //    healing potion starts identified.
    let origin = Position { x: 0, y: 0 };
    let pack_up = |world: &mut World, item: Entity| {
        world.entity_mut(item).remove::<Position>();
    };

    let ring_mail = spawn_armor(world, "ring mail", origin);
    world
        .entity_mut(ring_mail)
        .insert(crate::effects::ArmorBonus(1));
    pack_up(world, ring_mail);

    let mace = spawn_weapon(world, "mace", origin);
    world.entity_mut(mace).insert(crate::effects::PowerBonus(1));
    pack_up(world, mace);

    let shortbow = spawn_launcher(world, "short bow", origin);
    world
        .entity_mut(shortbow)
        .insert(crate::effects::ThrowBonus(1));
    pack_up(world, shortbow);

    let arrows = spawn_ammo(world, "arrow", origin);
    if let Some(mut stack) = world.get_mut::<Stack>(arrows) {
        stack.count = 13;
    }
    pack_up(world, arrows);

    let healing = spawn_potion(world, PotionEffect::Healing, origin);
    pack_up(world, healing);
    world
        .resource_mut::<Identified>()
        .potions
        .insert(PotionEffect::Healing);

    let player_name = world.resource::<PlayerName>().what.clone();

    // 2. Spawn the player with the gear in their backpack
    let player = world
        .spawn((
            Player,
            Name { what: player_name },
            Position {
                x: player_x,
                y: player_y,
            },
            Renderable {
                glyph: '@',
                color: Color::Yellow,
            },
            Viewshed {
                visible_tiles: Vec::new(),
                revealed_tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
                range: SIGHT_RANGE,
                dirty: true,
            },
            Fighter {
                hp: START_HP,
                max_hp: START_HP,
                armor: START_ARMOR,
                power: START_POWER,
                max_power: START_POWER,
                armor_bonus: 0,
                power_bonus: 0,
            },
            Magic {
                points: START_MAGIC,
                max_points: START_MAGIC,
            },
            Faction::Player,
            Backpack {
                items: vec![ring_mail, mace, shortbow, arrows, healing],
            },
            Score { value: 0 },
            Blood,
            Speed::new(SpeedKind::Normal),
        ))
        .id();

    // 3. Wear the armour and wield the mace. The bow and arrows wait in the pack;
    //    both weapons want the same hand.
    equip_silently(world, player, ring_mail);
    equip_silently(world, player, mace);

    populate_level(world, &rooms, (player_x, player_y));
}
