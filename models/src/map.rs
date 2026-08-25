use std::collections::HashSet;
use bevy_ecs::prelude::*;
use crossterm::style::Color;

use rand::Rng;
use rand::rngs::StdRng;

use crate::PotionBundle;
use crate::WandBundle;
use crate::rect::Rect;
use crate::components::*;
use crate::state::*;
use crate::monsters::MonsterBundle;

#[derive(Resource)]
pub struct GameRng(pub StdRng);

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
fn random_point_in_room(room: &Rect, rng: &mut StdRng) -> (u16, u16) {
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

/// Procedurally generates a random map layout, spawns map entities, and returns the player start position.
pub fn create_map(world: &mut World) -> ((u16, u16), Vec<Rect>) {
    let map_width: u16 = 80;
    let map_height: u16 = 22;

    // Take out the RNG safely without holding a long mutable borrow of `world`
    let mut game_rng = world.remove_resource::<GameRng>().unwrap();
    let rng = &mut game_rng.0;

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

    // Put the RNG back into the world before attempting to mutate world via `spawn`
    world.insert_resource(game_rng);

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
    ((start_pos.0 as u16, start_pos.1 as u16), rooms)
}

pub fn initialize_world(world: &mut World) {
    world.insert_resource(GameState::new());

    let ((player_x, player_y), rooms) = create_map(world);

    // 1. Create a starting wand entity first
    let starting_wand = world.spawn(WandBundle::magic_missile(Position { x: 0, y: 0 })).id();
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
            revealed_tiles: HashSet::new(),
            range: 16,
            dirty: true,
        },
        Fighter {
            hp: 30,
            max_hp: 30,
            armor: 2,
            power: 5,
        },
        Faction::Player,
        Backpack { items: vec![starting_wand] },
        Score { value: 0 },
    ));

    let mut game_rng = world.remove_resource::<GameRng>().unwrap();
    let mut occupied = HashSet::new();

    // The player's tile is already occupied.
    occupied.insert((player_x, player_y));

    // Up to 3 monsters.
    for _ in 0..3 {
        let mut placed = false;

        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(1..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);

            if occupied.insert((x, y)) {
                let monster = if game_rng.0.gen_bool(0.5) {
                    MonsterBundle::orc(Position { x, y })
                } else {
                    MonsterBundle::goblin(Position { x, y })
                };

                world.spawn(monster);
                placed = true;
                break;
            }
        }

        if !placed {
            break;
        }
    }

    // Up to 3 coins.
    for _ in 0..3 {
        let mut placed = false;

        for _ in 0..100 {
            let room_idx = game_rng.0.gen_range(1..rooms.len());
            let (x, y) = random_point_in_room(&rooms[room_idx], &mut game_rng.0);

            if occupied.insert((x, y)) {
                let potion = if game_rng.0.gen_bool(0.5) {
                    PotionBundle::healing(Position {x,y})
                } else {
                    PotionBundle::healing(Position {x,y})
                };

                world.spawn(potion);
                placed = true;
                break;
            }
        }

        if !placed {
            break;
        }
    }

    world.insert_resource(game_rng);
}