use bevy_ecs::prelude::*;
use crossterm::style::Color;
use std::collections::{HashSet, HashMap, VecDeque};
use crate::components::*; // Make sure components are imported

#[derive(Clone, Copy, PartialEq)]
enum TileKind {
    Room,
    Passage,
    Wall,
    Door,
}

pub fn visibility_system(
    mut commands: Commands, // 1. We need Commands to insert/remove components dynamically
    
    // 2. Add `With<Player>` here! If you don't, any monster with a viewshed 
    // in the future will accidentally reveal the map for you.
    mut viewshed_query: Query<(&mut Viewshed, &Position), With<Player>>,
    
    mut map_tiles_query: Query<(
        Entity, 
        &Position, 
        &mut Renderable, 
        Option<&Room>, 
        Option<&Passage>, 
        Option<&Wall>,
        Option<&Door>
    )>,
    
    // 3. Grab all the monsters
    mob_query: Query<(Entity, &Position), With<Mob>>,
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

    // Build the fast spatial lookup map.
    let mut map_lookup = HashMap::new();
    for (entity, pos, _, room, passage, wall, door) in map_tiles_query.iter() {
        let kind = if room.is_some() { TileKind::Room } 
        else if passage.is_some() { TileKind::Passage } 
        else if wall.is_some() { TileKind::Wall } 
        else if door.is_some() { TileKind::Door } 
        else { continue; };
        
        map_lookup.insert((pos.x, pos.y), (kind, entity));
    }

    // Process viewsheds
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

        // Rule B: Flood-fill Room logic (with leak prevention!)
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

                            // Check what kind of tile the neighbor is to prevent leaking
                            if let Some(&(neighbor_kind, _)) = map_lookup.get(&neighbor_pos) {
                                match neighbor_kind {
                                    TileKind::Room | TileKind::Door => {
                                        visible_set.insert(neighbor_pos);
                                        if visited_rooms.insert(neighbor_pos) {
                                            queue.push_back(neighbor_pos); // Continue spreading inside rooms/doors
                                        }
                                    }
                                    TileKind::Passage => {
                                        visible_set.insert(neighbor_pos); // Let players see open doors/passage entrances
                                    }
                                    TileKind::Wall => {
                                        visible_set.insert(neighbor_pos); // Allow player to see the room's enclosing walls...
                                        // BUT DO NOT push walls to the queue! This stops the flood-fill from leaking outside the room.
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Update Map Colors Directly
        for (&tile_pos, &(kind, entity)) in map_lookup.iter() {
            if let Ok((_, _, mut renderable, _, _, _, _)) = map_tiles_query.get_mut(entity) {
                if visible_set.contains(&tile_pos) {
                    renderable.color = match kind {
                        TileKind::Room => Color::Green,
                        TileKind::Passage => Color::White,
                        TileKind::Wall => Color::DarkYellow,
                        TileKind::Door => Color::Yellow,
                    };
                } else if viewshed.revealed_tiles.contains(crate::map::tile_index(tile_pos.0, tile_pos.1)) {
                    renderable.color = Color::DarkGrey; 
                } else {
                    renderable.color = Color::Black; 
                }
            }
        }

        // 4. ✨ HIDE OR REVEAL MONSTERS ✨
        for (entity, mob_pos) in mob_query.iter() {
            if visible_set.contains(&(mob_pos.x, mob_pos.y)) {
                // In sight! Remove the Hidden component so they get drawn
                commands.entity(entity).remove::<Hidden>();
            } else {
                // Out of sight! Add the Hidden component so the renderer skips them
                commands.entity(entity).insert(Hidden);
            }
        }

        if viewshed.revealed_tiles.len() < crate::map::MAP_TILE_COUNT {
            viewshed.revealed_tiles.grow(crate::map::MAP_TILE_COUNT);
        }
        for &(x, y) in visible_set.iter() {
            viewshed.revealed_tiles.insert(crate::map::tile_index(x, y));
        }
        viewshed.visible_tiles = visible_set.into_iter().collect();
        viewshed.dirty = false;
    }
}