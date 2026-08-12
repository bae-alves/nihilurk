use std::collections::HashSet;

use crate::rect::Rect;
use bevy_ecs::prelude::*;
use crossterm::style::Color;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct Wall;

#[derive(Component)]
pub struct Passage;

#[derive(Component)]
pub struct Room;

#[derive(Component)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

#[derive(Component)]
pub struct Renderable {
    pub glyph: char,
    pub color: Color,
}

#[derive(Component)]
pub struct Viewshed {
    pub visible_tiles: Vec<(u16, u16)>,
    pub revealed_tiles: HashSet<(u16, u16)>,
    pub range: u16,
    pub dirty: bool,
}

#[derive(Resource)]
pub struct GameState {
    pub is_running: bool,
}

#[derive(PartialEq, Copy, Clone)]
enum TileType {
    Wall,
    Room,
    Passage,
}

impl GameState {
    pub fn new() -> Self {
        Self { is_running: true }
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

/// Helper function to create a corridor between two points
fn create_corridor(from: (u16, u16), to: (u16, u16), tiles: &mut [TileType], map_width: u16) {
    let mut x = from.0;
    let mut y = from.1;

    // Horizontal movement
    while x != to.0 {
        let idx = (y * map_width + x) as usize;
        if tiles[idx] != TileType::Room {
            tiles[idx] = TileType::Passage;
        }
        if x < to.0 {
            x += 1;
        } else {
            x -= 1;
        }
    }

    // Vertical movement
    while y != to.1 {
        let idx = (y * map_width + x) as usize;
        if tiles[idx] != TileType::Room {
            tiles[idx] = TileType::Passage;
        }
        if y < to.1 {
            y += 1;
        } else {
            y -= 1;
        }
    }
}

/// Procedurally generates a random map layout with walls and floor tiles, then spawn entities in the world based on that layout.
pub fn create_map(world: &mut World) {
    let map_width = 80;
    let map_height = 22;

    // Initialize map
    let mut tiles = vec![TileType::Wall; (map_width * map_height) as usize];
    let mut rooms: Vec<Rect> = Vec::new();

    // THE ROGUE GENERATION ALGORITHM
    let section_width = map_width / 3;
    let section_height = map_height / 3;

    let gone_sections_count = (getrandom::u32().unwrap() % 4) as usize;

    let mut sections = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    for i in 0..gone_sections_count {
        let swap_idx = i + (getrandom::u32().unwrap() as usize % (9 - i));
        sections.swap(i, swap_idx);
    }
    let gone_sections = &sections[..gone_sections_count];

    let avail_width: u16 = section_width - 2_u16;
    let avail_height: u16 = section_height - 2_u16;
    let mut grid_rooms: [Option<usize>; 9] = [None; 9];

    for section_y in 0_u16..3_u16 {
        for section_x in 0_u16..3_u16 {
            let section_index = (section_y * 3_u16 + section_x) as usize;
            if gone_sections.contains(&section_index) {
                continue;
            }

            let width_range = avail_width.saturating_sub(4_u16).max(1_u16);
            let room_width: u16 = 5_u16 + (getrandom::u32().unwrap() as u16 % width_range);

            let height_range = avail_height.saturating_sub(4_u16).max(1_u16);
            let room_height: u16 = 5_u16 + (getrandom::u32().unwrap() as u16 % height_range);

            let base_x = section_x * section_width;
            let base_y = section_y * section_height;

            let x_range = (avail_width.saturating_sub(room_width) + 1_u16).max(1_u16);
            let room_x = base_x + (getrandom::u32().unwrap() as u16 % x_range);

            let y_range = (avail_height.saturating_sub(room_height) + 1_u16).max(1_u16);
            let room_y = base_y + (getrandom::u32().unwrap() as u16 % y_range);

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

    // Connect Adjacent Grid Rooms orthogonally (replaces the random shuffle)
    // This strictly prevents corridors from cutting through other rooms and prevents duplicate paths.

    // 1. Horizontal connections (Left to Right)
    for y in 0..3 {
        let mut prev_room: Option<usize> = None;
        for x in 0..3 {
            if let Some(room_idx) = grid_rooms[y * 3 + x] {
                if let Some(prev_idx) = prev_room {
                    let center1 = rooms[prev_idx].center();
                    let center2 = rooms[room_idx].center();
                    create_corridor(
                        (center1.0 as u16, center1.1 as u16),
                        (center2.0 as u16, center2.1 as u16),
                        &mut tiles,
                        map_width,
                    );
                }
                prev_room = Some(room_idx);
            }
        }
    }

    // 2. Vertical connections (Top to Bottom)
    for x in 0..3 {
        let mut prev_room: Option<usize> = None;
        for y in 0..3 {
            if let Some(room_idx) = grid_rooms[y * 3 + x] {
                if let Some(prev_idx) = prev_room {
                    let center1 = rooms[prev_idx].center();
                    let center2 = rooms[room_idx].center();
                    create_corridor(
                        (center1.0 as u16, center1.1 as u16),
                        (center2.0 as u16, center2.1 as u16),
                        &mut tiles,
                        map_width,
                    );
                }
                prev_room = Some(room_idx);
            }
        }
    }

    // 4. Spawn exactly one entity per tile coordinate
    for y in 0..map_height {
        for x in 0..map_width {
            let idx = (y * map_width + x) as usize;
            match tiles[idx] {
                TileType::Room => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '.',
                            color: Color::Cyan,
                        },
                        Room,
                    ));
                }
                TileType::Passage => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '▒',
                            color: Color::White,
                        },
                        Passage,
                    ));
                }
                TileType::Wall => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '#',
                            color: Color::Green,
                        },
                        Wall,
                    ));
                }
            }
        }
    }
}

//Helper function to create a test map layout
pub fn _create_test_map(world: &mut World) {
    let map_width = 80;
    let map_height = 22;

    // 1. Initialize empty floor grid
    let mut tiles = vec![TileType::Room; (map_width * map_height) as usize];

    // 2. Set border walls
    for y in 0..map_height {
        for x in 0..map_width {
            if x == 0 || x == map_width - 1 || y == 0 || y == map_height - 1 {
                let idx = (y * map_width + x) as usize;
                tiles[idx] = TileType::Wall;
            }
        }
    }

    // 3. Add random walls into the grid
    for _ in 0..400 {
        let x = 1 + (getrandom::u32().unwrap() as u16 % (map_width - 1));
        let y = 1 + (getrandom::u32().unwrap() as u16 % (map_height - 1));
        let idx = (y * map_width + x) as usize;
        tiles[idx] = TileType::Wall;
    }

    // 4. Spawn exactly ONE entity per coordinate
    for y in 0..map_height {
        for x in 0..map_width {
            let idx = (y * map_width + x) as usize;
            match tiles[idx] {
                TileType::Room | TileType::Passage => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '.',
                            color: Color::White,
                        },
                    ));
                }
                TileType::Wall => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '#',
                            color: Color::Green,
                        },
                        Wall,
                    ));
                }
            }
        }
    }
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    create_map(world);
    world.spawn((
        Player,
        Position { x: 10, y: 10 },
        Renderable {
            glyph: '@',
            color: Color::Yellow,
        },
        Viewshed {
            visible_tiles: Vec::new(),
            revealed_tiles: HashSet::new(),
            range: 16,
            dirty: true,
        },
    ));
}
