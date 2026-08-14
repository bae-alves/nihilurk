use std::collections::HashSet;
use bevy_ecs::prelude::*;
use crossterm::style::Color;

use crate::rect::Rect;
use crate::components::*;
use crate::state::*;

#[derive(PartialEq, Copy, Clone)]
enum TileType {
    Wall,
    Room,
    Passage,
    Door,
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
fn random_point_in_room(room: &Rect) -> (u16, u16) {
    let width = (room.x2 - room.x1 + 1).max(1) as u32;
    let height = (room.y2 - room.y1 + 1).max(1) as u32;
    
    let rx = room.x1 as u32 + (getrandom::u32().unwrap() % width);
    let ry = room.y1 as u32 + (getrandom::u32().unwrap() % height);
    
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

/// Procedurally generates a random map layout, spawns map entities, and returns the player start position.
pub fn create_map(world: &mut World) -> (u16, u16) {
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
                        let pt1 = random_point_in_room(&rooms[prev_idx]);
                        let pt2 = random_point_in_room(&rooms[room_idx]);
                        
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
                        let pt1 = random_point_in_room(&rooms[prev_idx]);
                        let pt2 = random_point_in_room(&rooms[room_idx]);
                        
                        create_corridor(pt1, pt2, &mut tiles, map_width);
                    }
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
                            color: Color::Green,
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
                            color: Color::DarkYellow,
                        },
                        Wall,
                    ));
                }
                TileType::Door => {
                    world.spawn((
                        Position { x, y },
                        Renderable {
                            glyph: '+',
                            color: Color::Yellow,
                        },
                        Door,
                    ));
                }
            }
        }
    }

    // Return the center of the very first room so we can spawn the player safely away from doors
    let start_pos = rooms[0].center();
    (start_pos.0 as u16, start_pos.1 as u16)
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());
    
    let (player_x, player_y) = create_map(world);
    
    world.spawn((
        Player,
        Position { x: player_x, y: player_y },
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