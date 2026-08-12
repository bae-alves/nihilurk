use bevy_ecs::prelude::*;
use crossterm::style::Color;
use std::collections::{HashSet, HashMap, VecDeque};
use crate::model::*;

#[derive(Clone, Copy, PartialEq)]
enum TileKind {
    Room,
    Passage,
    Wall,
    Door,
}

pub fn visibility_system(
    // Note: If monsters get Viewsheds later, you should add `With<Player>` 
    // to this query so only the player's FOV changes the map colors!
    mut viewshed_query: Query<(&mut Viewshed, &Position)>,
    // We combine the map queries into one, grabbing Entity and Renderable
    mut map_tiles_query: Query<(
        Entity, 
        &Position, 
        &mut Renderable, 
        Option<&Room>, 
        Option<&Passage>, 
        Option<&Wall>,
        Option<&Door>
    )>,
) {
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

    // 1. Build the fast spatial lookup map.
    // Because we queried `Entity`, we can save it in the map for instant access later!
    let mut map_lookup = HashMap::new();
    for (entity, pos, _, room, passage, wall, door) in map_tiles_query.iter() {
        let kind = if room.is_some() {
            TileKind::Room
        } else if passage.is_some() {
            TileKind::Passage
        } else if wall.is_some() {
            TileKind::Wall
        } else if door.is_some() {
            TileKind::Door
        } else {
            continue; // Skip entities that aren't map tiles
        };
        map_lookup.insert((pos.x, pos.y), (kind, entity));
    }

    // 2. Process viewsheds
    for (mut viewshed, pos) in viewshed_query.iter_mut() {
        if !viewshed.dirty {
            continue;
        }

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
        if let Some(&(TileKind::Room, _)) | Some(&(TileKind::Door, _)) = map_lookup.get(&(pos.x, pos.y)) {
            let mut queue = VecDeque::new();
            let mut visited_rooms = HashSet::new();

            queue.push_back((pos.x, pos.y));
            visited_rooms.insert((pos.x, pos.y));

            while let Some((cx, cy)) = queue.pop_front() {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        
                        let nx = cx as i16 + dx;
                        let ny = cy as i16 + dy;
                        
                        if nx >= 0 && ny >= 0 {
                            let neighbor_pos = (nx as u16, ny as u16);
                            visible_set.insert(neighbor_pos);

                            if let Some(&(TileKind::Room, _)) = map_lookup.get(&neighbor_pos) {
                                if visited_rooms.insert(neighbor_pos) {
                                    queue.push_back(neighbor_pos);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. ✨ THE MAGIC: Update Map Colors Directly ✨
        for (&tile_pos, &(kind, entity)) in map_lookup.iter() {
            // Grab the mutable renderable component using the Entity ID
            if let Ok((_, _, mut renderable, _, _, _, _)) = map_tiles_query.get_mut(entity) {
                if visible_set.contains(&tile_pos) {
                    // It's currently visible - render bright original colors
                    renderable.color = match kind {
                        TileKind::Room => Color::Green,
                        TileKind::Passage => Color::White,
                        TileKind::Wall => Color::DarkYellow,
                        TileKind::Door => Color::Yellow,
                    };
                } else if viewshed.revealed_tiles.contains(&tile_pos) {
                    // It's not visible, but we remember it - render dark grey
                    renderable.color = Color::DarkGrey; 
                } else {
                    // Never seen - hide it completely
                    renderable.color = Color::Black; // Or Color::NONE, depending on your Bevy version
                }
            }
        }

        // 4. Update the viewshed memory and clean the dirty flag
        viewshed.revealed_tiles.extend(visible_set.iter().copied());
        viewshed.visible_tiles = visible_set.into_iter().collect();
        viewshed.dirty = false;
    }
}