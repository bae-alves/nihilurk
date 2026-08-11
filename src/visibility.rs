use bevy_ecs::prelude::*;
use std::collections::{HashSet, HashMap, VecDeque};
use crate::model::{Viewshed, Position, Room, Passage, Wall};

// A lightweight enum to help us map the ECS components into a quick 2D lookup
#[derive(Clone, Copy, PartialEq)]
enum TileKind {
    Room,
    Passage,
    Wall,
}

pub fn visibility_system(
    mut viewshed_query: Query<(&mut Viewshed, &Position)>,
    room_query: Query<&Position, With<Room>>,
    passage_query: Query<&Position, With<Passage>>,
    wall_query: Query<&Position, With<Wall>>,
) {
    // 1. Bail out early if nothing moved. 
    // This saves us from building the spatial lookup if no viewsheds are dirty.
    let mut any_dirty = false;
    for (viewshed, _) in viewshed_query.iter() {
        if viewshed.dirty {
            any_dirty = true;
            break;
        }
    }

    if !any_dirty {
        return;
    }

    // 2. Build a fast spatial lookup map. 
    // Querying the ECS inside a flood-fill while-loop is an anti-pattern that will 
    // crush performance. Converting the relevant map data to a HashMap first gives us O(1) lookups.
    let mut map_lookup = HashMap::new();
    for pos in room_query.iter() {
        map_lookup.insert((pos.x, pos.y), TileKind::Room);
    }
    for pos in passage_query.iter() {
        map_lookup.insert((pos.x, pos.y), TileKind::Passage);
    }
    for pos in wall_query.iter() {
        map_lookup.insert((pos.x, pos.y), TileKind::Wall);
    }

    // 3. Process viewsheds
    for (mut viewshed, pos) in viewshed_query.iter_mut() {
        if !viewshed.dirty {
            continue;
        }

        viewshed.visible_tiles.clear();
        let mut visible_set = HashSet::new();
        
        let center_x = pos.x as i16;
        let center_y = pos.y as i16;

        // Rule A: Always see a 1-tile radius (3x3 grid)
        for dx in -1..=1 {
            for dy in -1..=1 {
                let vx = center_x + dx;
                let vy = center_y + dy;
                if vx >= 0 && vy >= 0 {
                    visible_set.insert((vx as u16, vy as u16));
                }
            }
        }

        // Rule B: If standing in a Room, flood-fill to reveal the whole room
        if let Some(&TileKind::Room) = map_lookup.get(&(pos.x, pos.y)) {
            let mut queue = VecDeque::new();
            let mut visited_rooms = HashSet::new();

            queue.push_back((pos.x, pos.y));
            visited_rooms.insert((pos.x, pos.y));

            while let Some((cx, cy)) = queue.pop_front() {
                // Check all 8 neighboring tiles
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        
                        let nx = cx as i16 + dx;
                        let ny = cy as i16 + dy;
                        
                        if nx >= 0 && ny >= 0 {
                            let neighbor_pos = (nx as u16, ny as u16);
                            
                            // Any tile touching a visited Room tile becomes visible 
                            // (this catches the enclosing walls and passage entrances)
                            visible_set.insert(neighbor_pos);

                            // If the neighbor is also a Room tile, add it to the queue
                            if let Some(&TileKind::Room) = map_lookup.get(&neighbor_pos) {
                                if visited_rooms.insert(neighbor_pos) {
                                    queue.push_back(neighbor_pos);
                                }
                            }
                        }
                    }
                }
            }
        }

        viewshed.visible_tiles = visible_set.into_iter().collect();
        viewshed.dirty = false;
    }
}