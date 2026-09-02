use std::collections::HashSet;
use fixedbitset::FixedBitSet;
use bevy_ecs::prelude::*;
use crossterm::style::Color;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;

use crate::catalog::{
    spawn_element_of_yoord, spawn_wand, ItemDef, ARMORS, COINS, POTIONS, RINGS, SCROLLS, WANDS,
    WEAPONS,
};
use crate::rect::Rect;
use crate::components::*;
use crate::state::*;
use crate::monsters::{spawn_monster, MonsterDef, BESTIARY};
use crate::identify::{Identified, ItemAppearances};

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
    Downstairs
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
            TileType::Wall
        } else {
            self.tiles[tile_index(x, y)]
        }
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
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                // Only room floor (and the stairs that sit on it) makes a wall
                // worth drawing. A wall that merely touches a Door — but no room
                // tile — is outside the room, hugging the corridor, and stays dark.
                match self.tile(nx as u16, ny as u16) {
                    TileType::Room | TileType::Upstairs | TileType::Downstairs => {
                        return true;
                    }
                    _ => {}
                }
            }
        }
        false
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
        TileType::Upstairs => ('<', Color::Cyan)
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

/// Helper function to create a corridor and place doors automatically
fn create_corridor(from: (u16, u16), to: (u16, u16), tiles: &mut [TileType], map_width: u16) {
    let mut x = from.0;
    let mut y = from.1;
    let mut path = Vec::new();

    // 1. Calculate the L-shaped path
    while x != to.0 {
        path.push((x, y));
        if x < to.0 { x += 1; } else { x -= 1; }
    }
    while y != to.1 {
        path.push((x, y));
        if y < to.1 { y += 1; } else { y -= 1; }
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
        if is_room && !prev_was_room {
            // Stepped INTO a room. The previous tile becomes a door.
            let prev_idx = (path[i - 1].1 * map_width + path[i - 1].0) as usize;
            if tiles[prev_idx] != TileType::Room {
                tiles[prev_idx] = TileType::Door;
            }
        } else if !is_room && prev_was_room {
            // Stepped OUT of a room. The current tile becomes a door.
            tiles[idx] = TileType::Door;
        } else if !is_room {
            // Outside of a room, dig a regular passage.
            if tiles[idx] != TileType::Door {
                tiles[idx] = TileType::Passage;
            }
        }
        prev_was_room = is_room;
    }
}

pub const MAP_WIDTH: u16 = 80;
pub const MAP_HEIGHT: u16 = 22;

/// The deepest floor of a run. It has no down-stair: the Element of Yoord sits
/// where the stair would be, and retrieving it is the whole point of the descent.
pub const FINAL_DEPTH: u8 = 13;

/// Turns the player may dawdle on one level before the Dungeon Lord loses
/// patience and portals them onward (down on the way in, up once they carry the
/// Element). Reset by every level change.
pub const DUNGEON_LORD_PATIENCE: u32 = 260;

/// The eight neighbouring offsets, ordered for the guardian ring around the
/// Element of Yoord.
const RING_DIRS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1), (-1, 0),
    (1, 0), (-1, 1), (0, 1), (1, 1),
];

/// The coordinate of the first tile of `want` in `tiles`, row-major.
fn find_tile(tiles: &[TileType], want: TileType) -> Option<(u16, u16)> {
    tiles.iter().position(|&t| t == want).map(|i| {
        ((i % MAP_WIDTH as usize) as u16, (i / MAP_WIDTH as usize) as u16)
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
    let usable_width: u16 = map_width.saturating_sub(padding * 2_u16).saturating_sub(num_gutters * gutter_size);
    let usable_height: u16 = map_height.saturating_sub(padding * 2_u16).saturating_sub(num_gutters * gutter_size);

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
    
    // 1. Horizontal connections (Left to Right)
    for y in 0..3 {
        let mut prev_room: Option<usize> = None;
        for x in 0..3 {
            if let Some(room_idx) = grid_rooms[y * 3 + x] {
                if let Some(prev_idx) = prev_room {
                    let pair = if prev_idx < room_idx { 
                        (prev_idx, room_idx) 
                    } else { 
                        (room_idx, prev_idx) 
                    };
                    
                    if connected_pairs.insert(pair) {
                        let pt1 = random_point_in_room(&rooms[prev_idx], rng);
                        let pt2 = random_point_in_room(&rooms[room_idx], rng);
                        
                        create_corridor(pt1, pt2, &mut tiles, map_width);
                    }
                }
                prev_room = Some(room_idx);
            }
        }
    }
    
    // 2. Vertical connections pass (Top to Bottom)
    for x in 0..3 {
        let mut prev_room: Option<usize> = None;
        for y in 0..3 {
            if let Some(room_idx) = grid_rooms[y * 3 + x] {
                if let Some(prev_idx) = prev_room {
                    let pair = if prev_idx < room_idx { (prev_idx, room_idx) } else { (room_idx, prev_idx) };

                    if connected_pairs.insert(pair) {
                        let pt1 = random_point_in_room(&rooms[prev_idx], rng);
                        let pt2 = random_point_in_room(&rooms[room_idx], rng);
                        
                        create_corridor(pt1, pt2, &mut tiles, map_width);
                    }
                }
                prev_room = Some(room_idx);
            }
        }
    }

    // Staircases: up in the first room (where the player spawns), down in a
    // random other room. Done here so the map rebuilt from the seed on load —
    // and the map for every new floor — carries the same stairs.
    let up = rooms[0].center();
    tiles[tile_index(up.0 as u16, up.1 as u16)] = TileType::Upstairs;
    let down_room = if rooms.len() > 1 { rng.gen_range(1..rooms.len()) } else { 0 };
    let down = random_point_in_room(&rooms[down_room], rng);
    tiles[tile_index(down.0, down.1)] = TileType::Downstairs;

    // Dark rooms: every room past the start room (room 0) has a small chance of
    // being unlit. A dark room's Room floor tiles are flagged so the visibility
    // system treats them like a passage until a wand of light is used there.
    let mut dark = FixedBitSet::with_capacity(MAP_TILE_COUNT);
    for room in rooms.iter().skip(1) {
        if !rng.gen_bool(0.15) {
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

/// Generates a fresh map, inserts the [`Map`] resource, and returns the player start.
pub fn create_map(world: &mut World) -> ((u16, u16), Vec<Rect>) {
    let mut game_rng = world.remove_resource::<GameRng>().unwrap();
    let (tiles, rooms, dark) = build_tiles(&mut game_rng.0);
    world.insert_resource(game_rng);

    world.insert_resource(Map { tiles, dark });

    // Return the center of the very first room so we can spawn the player safely away from doors
    let start_pos = rooms[0].center();
    ((start_pos.0 as u16, start_pos.1 as u16), rooms)
}

/// Rebuilds the [`Map`] resource deterministically from `seed`, without touching
/// the live `GameRng` resource or spawning any actors. Used on load, where the
/// map is reconstructed from the seed rather than the save file.
pub fn regenerate_map(world: &mut World, seed: u64) {
    let mut rng = ChaCha12Rng::seed_from_u64(seed);
    let (tiles, _rooms, dark) = build_tiles(&mut rng);
    world.insert_resource(Map { tiles, dark });
}

/// Picks a species appropriate for `depth`. The bestiary is split into danger
/// tiers ([`MonsterDef::tier`]); each floor rolls from every tier it has already
/// unlocked, so early letters keep showing up as fodder while deeper letters get
/// mixed in.
fn pick_monster(depth: u8, rng: &mut ChaCha12Rng) -> &'static MonsterDef {
    // depth 1-2 -> tier 0, 3-4 -> up to tier 1, 5-6 -> tier 2, 7+ -> all tiers.
    let max_tier = ((depth.max(1) - 1) / 2).min(3);
    let pool: Vec<&MonsterDef> = BESTIARY.iter().filter(|m| m.tier <= max_tier).collect();
    pool[rng.gen_range(0..pool.len())]
}

/// Rolls one floor item and spawns it at `pos`. Category odds follow the classic
/// Rogue drop table (food is swapped for coins); within a category every entry
/// is equally likely.
///
/// | Category | Odds |
/// |----------|------|
/// | Scrolls  | 30%  |
/// | Potions  | 27%  |
/// | Coins    | 17%  |
/// | Armor    |  8%  |
/// | Weapons  |  8%  |
/// | Wands    |  5%  |
/// | Rings    |  5%  |
fn spawn_random_item(world: &mut World, rng: &mut ChaCha12Rng, pos: Position) -> Entity {
    /// Picks one row from a catalog table uniformly and spawns it as a floor
    /// drop — enchantment, battery charge and all, whatever that category rolls.
    fn one<D: ItemDef>(world: &mut World, rng: &mut ChaCha12Rng, pos: Position, table: &[D]) -> Entity {
        table[rng.gen_range(0..table.len())].spawn_as_loot(world, rng, pos)
    }

    match rng.gen_range(0..100) {
        0..=29 => one(world, rng, pos, SCROLLS),
        30..=56 => one(world, rng, pos, POTIONS),
        57..=73 => one(world, rng, pos, COINS),
        74..=81 => one(world, rng, pos, ARMORS),
        82..=89 => one(world, rng, pos, WEAPONS),
        90..=94 => one(world, rng, pos, WANDS),
        _ => one(world, rng, pos, RINGS),
    }
}

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

        // Flood-fill this one corridor.
        let mut stack = vec![start];
        let mut blob: Vec<usize> = Vec::new();
        seen[start] = true;
        while let Some(idx) = stack.pop() {
            blob.push(idx);
            let x = (idx % width) as i32;
            let y = (idx / width) as i32;
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

        // Centroid of the blob, then the blob tile closest to it.
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
        centers.push(((best % width) as u16, (best / width) as u16));
    }

    centers
}

/// Spawns the monsters and items for a freshly built floor. The staircases are
/// carved by [`build_tiles`]. Shared by [`initialize_world`] and [`change_level`].
fn populate_level(world: &mut World, rooms: &[Rect], player_start: (u16, u16)) {
    let (player_x, player_y) = player_start;

    let mut game_rng = world.remove_resource::<GameRng>().unwrap();
    let mut occupied = HashSet::new();

    // The player's tile is already occupied.
    occupied.insert((player_x, player_y));

    // Rough danger tier: deeper floors unlock nastier letters.
    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);

    // Both the monster and trap budgets step up every three floors: `tier` is 0
    // on depth 1-3, 1 on depth 4-6, 2 on depth 7-9, and so on. Each tier grants
    // one more spawn slot and widens the odds that a given slot actually fills,
    // so the dungeon gets steadily — but smoothly — more crowded and more
    // dangerous the deeper you go.
    let tier = (depth.saturating_sub(1) / 3) as u32;

    // Monsters: three slots at the surface, +1 per tier. The first slot always
    // fills (no floor is ever completely empty); every later slot fills with a
    // probability that itself climbs one step every three floors (capped so a
    // slot is never quite certain).
    let max_monsters = 3 + tier as usize;
    let monster_chance = (0.60 + 0.12 * tier as f64).min(0.95);
    for slot in 0..max_monsters {
        if slot > 0 && !game_rng.0.gen_bool(monster_chance) {
            continue;
        }
        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(1..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);

            if occupied.insert((x, y)) {
                let def = pick_monster(depth, &mut game_rng.0);
                spawn_monster(world, def, Position { x, y });
                break;
            }
        }
    }

    // From depth 7 on, every corridor also has a small (5%) chance of hiding a
    // lurker dead centre — right where an unwary traveller runs into it.
    if depth >= 7 {
        let centers = corridor_centers(&world.resource::<Map>().tiles);
        for (cx, cy) in centers {
            if !game_rng.0.gen_bool(0.05) {
                continue;
            }
            if occupied.insert((cx, cy)) {
                let def = pick_monster(depth, &mut game_rng.0);
                spawn_monster(world, def, Position { x: cx, y: cy });
            }
        }
    }

    // Up to 3 items.
    for _ in 0..3 {
        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(1..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);

            if occupied.insert((x, y)) {
                spawn_random_item(world, &mut game_rng.0, Position { x, y });
                break;
            }
        }
    }

    // 1 floor in 10 hides an extra item in plain sight: it draws nothing and is
    // never announced until a ring of perception turns it up or the player walks
    // straight onto it ("Hey! There's something here!").
    if game_rng.0.gen_bool(0.10) {
        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(1..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);

            if occupied.insert((x, y)) {
                let item = spawn_random_item(world, &mut game_rng.0, Position { x, y });
                world.entity_mut(item).insert((Hidden, Invisible));
                break;
            }
        }
    }

    // The deepest floor: replace the down-stair with the Element of Yoord and
    // ring it with guardians — the first certain, each further one half as
    // likely. A guardian that would land in a wall cuts the ring short.
    if depth >= FINAL_DEPTH {
        if let Some((ex, ey)) = find_tile(&world.resource::<Map>().tiles, TileType::Downstairs) {
            world.resource_mut::<Map>().tiles[tile_index(ex, ey)] = TileType::Room;
            spawn_element_of_yoord(world, Position { x: ex, y: ey });
            occupied.insert((ex, ey));

            for (i, (dx, dy)) in RING_DIRS.iter().enumerate() {
                if !game_rng.0.gen_bool(0.5_f64.powi(i as i32)) {
                    continue;
                }
                let (nx, ny) = (ex as i32 + dx, ey as i32 + dy);
                if nx < 0 || ny < 0 {
                    break;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                if world.resource::<Map>().blocks(nx, ny) {
                    break;
                }
                if occupied.insert((nx, ny)) {
                    let def = pick_monster(depth, &mut game_rng.0);
                    spawn_monster(world, def, Position { x: nx, y: ny });
                }
            }
        }
    }

    // Traps: placed after the stairs, monsters and loot, before the hero drops
    // in. Like the monster budget, the trap budget steps up every three floors —
    // four slots at the surface, +1 per `tier` — and each slot's chance of
    // producing a trap climbs the same way, so the deep floors bristle with them
    // and the first floors rarely hold more than one.
    let max_traps = 4 + tier as usize;
    let trap_chance = (0.12 + 0.13 * tier as f64).min(0.75);
    for _ in 0..max_traps {
        if !game_rng.0.gen_bool(trap_chance) {
            continue;
        }
        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(0..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);
            if (x, y) == player_start {
                continue;
            }
            if world.resource::<Map>().tiles[tile_index(x, y)] != TileType::Room {
                continue;
            }
            if occupied.insert((x, y)) {
                world.spawn(crate::TrapBundle::random(&mut game_rng.0, Position { x, y }));
                break;
            }
        }
    }

    world.insert_resource(game_rng);
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
                .add("The Element of Yoord seeks the sun; it will not let you descend.");
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
        world.resource_mut::<GameLog>().add(if tile == TileType::Upstairs {
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
        world
            .resource_mut::<GameLog>()
            .add("You climb the last stair into open sky, the Element of Yoord blazing in your hands.");
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

    // Despawn every monster and every item lying on the floor. Backpack contents
    // (which carry no Position) are left untouched.
    let backpacked: HashSet<Entity> = world
        .query::<&Backpack>()
        .iter(world)
        .flat_map(|bp| bp.items.iter().copied())
        .collect();
    let to_despawn: Vec<Entity> = world
        .iter_entities()
        .filter(|e| !e.contains::<Player>() && e.contains::<Position>() && !backpacked.contains(&e.id()))
        .map(|e| e.id())
        .collect();
    for e in to_despawn {
        world.despawn(e);
    }

    // Build the next floor from the live RNG stream.
    let mut game_rng = world.remove_resource::<GameRng>().unwrap();
    let (tiles, rooms, dark) = build_tiles(&mut game_rng.0);
    world.insert_resource(game_rng);
    world.insert_resource(Map { tiles, dark });
    world.resource_mut::<BloodStains>().clear();

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

    let depth = {
        let mut depth = world.resource_mut::<Depth>();
        depth.what = if going_down {
            depth.what.saturating_add(1)
        } else {
            depth.what.saturating_sub(1).max(1)
        };
        depth.what
    };

    populate_level(world, &rooms, start);

    // A trapdoor plunge is a fall, not a rest: no arrival heal, no magic restore.
    if cause != LevelChange::Trapdoor {
        if let Some(mut fighter) = world.get_mut::<Fighter>(player_entity) {
            let heal = fighter.max_hp / 2;
            fighter.hp = (fighter.hp + heal).min(fighter.max_hp);
        }
        if let Some(mut magic) = world.get_mut::<Magic>(player_entity) {
            magic.points = magic.max_points;
        }
    }

    if let Some(mut dl) = world.get_resource_mut::<DungeonLord>() {
        dl.idle_turns = 0;
    }

    let msg = match cause {
        // Descending, it is the Dungeon Lord who wrenches you down; once you
        // carry the Element it is the Element that tears the way open upward.
        LevelChange::Portal if going_down => {
            format!("The Dungeon Lord opens a portal beneath your feet! You fall downward. (Depth {depth})")
        }
        LevelChange::Portal => {
            format!("The Element of Yoord flares and rips a portal above your head! You rise upward. (Depth {depth})")
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
    if world.get_resource::<Ending>().map(|e| e.player_dead).unwrap_or(false) {
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
    } else {
        if depth >= FINAL_DEPTH {
            world
                .resource_mut::<GameLog>()
                .add("The Dungeon Lord claws at the floor, but there is nowhere deeper to cast you.");
            return;
        }
        transition_level(world, true, LevelChange::Portal);
    }
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    world.insert_resource(Depth { what: 1 });
    world.insert_resource(BloodStains::new());
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

    // 1. Create a starting wand entity first, and roll its battery like any drop.
    let starting_wand = spawn_wand(world, WandEffect::MagicMissile, Position { x: 0, y: 0 });
    {
        let charges = crate::catalog::roll_wand_charges(&mut world.resource_mut::<GameRng>().0);
        if let Some(mut battery) = world.get_mut::<Battery>(starting_wand) {
            battery.charges = charges;
        }
    }
    let player_name = world.resource::<PlayerName>().what.clone();

    // 2. Spawn the player with the wand in their backpack
    world.spawn((
        Player,
        Name { what: player_name},
        Position { x: player_x, y: player_y },
        Renderable {
            glyph: '@',
            color: Color::Yellow,
        },
        Viewshed {
            visible_tiles: Vec::new(),
            revealed_tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            range: 12,
            dirty: true,
        },
        Fighter {
            hp: 12,
            max_hp: 12,
            armor: 2,
            power: 4,
            max_power: 4,
            armor_bonus: 0,
            power_bonus: 0,
        },
        Magic { points: 4, max_points: 4 },
        Faction::Player,
        Backpack { items: vec![starting_wand] },
        Score { value: 0 },
        Blood,
        Speed::new(SpeedKind::Normal),
    ));

    populate_level(world, &rooms, (player_x, player_y));
}