use bevy_ecs::prelude::*;
use std::collections::{HashSet, VecDeque};
use crate::components::*;
use crate::map::{tile_index, Map, TileType, MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH};

#[inline]
fn in_bounds(x: i16, y: i16) -> bool {
    x >= 0 && y >= 0 && (x as u16) < MAP_WIDTH && (y as u16) < MAP_HEIGHT
}

pub fn visibility_system(
    mut commands: Commands,

    // `With<Player>` matters: without it a future monster viewshed would reveal
    // the map for the player.
    mut viewshed_query: Query<(&mut Viewshed, &Position), With<Player>>,

    mob_query: Query<(Entity, &Position), With<Mob>>,

    map: Res<Map>,
) {
    let any_dirty = viewshed_query.iter().any(|(v, _)| v.dirty);
    if !any_dirty {
        return;
    }

    for (mut viewshed, pos) in viewshed_query.iter_mut() {
        if !viewshed.dirty {
            continue;
        }

        let mut visible_set: HashSet<(u16, u16)> = HashSet::new();
        let center_x = pos.x as i16;
        let center_y = pos.y as i16;

        // Rule A: Always see a 1-tile radius (3x3 grid)
        for dx in -1..=1 {
            for dy in -1..=1 {
                let vx = center_x + dx;
                let vy = center_y + dy;
                if in_bounds(vx, vy) {
                    visible_set.insert((vx as u16, vy as u16));
                }
            }
        }

        // Rule B: Flood-fill Room logic (with leak prevention!)
        if matches!(map.tile(pos.x, pos.y), TileType::Room | TileType::Door) {
            let mut queue = VecDeque::new();
            let mut visited_rooms = HashSet::new();

            queue.push_back((pos.x, pos.y));
            visited_rooms.insert((pos.x, pos.y));

            while let Some((cx, cy)) = queue.pop_front() {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = cx as i16 + dx;
                        let ny = cy as i16 + dy;
                        if !in_bounds(nx, ny) {
                            continue;
                        }
                        let neighbor_pos = (nx as u16, ny as u16);

                        match map.tile(neighbor_pos.0, neighbor_pos.1) {
                            TileType::Room | TileType::Door => {
                                visible_set.insert(neighbor_pos);
                                if visited_rooms.insert(neighbor_pos) {
                                    queue.push_back(neighbor_pos); // keep spreading inside rooms/doors
                                }
                            }
                            TileType::Passage => {
                                visible_set.insert(neighbor_pos); // see passage entrances
                            }
                            TileType::Wall => {
                                // See the room's enclosing walls, but never queue
                                // them — that is what stops the fill from leaking out.
                                visible_set.insert(neighbor_pos);
                            }
                        }
                    }
                }
            }
        }

        // Hide or reveal monsters based on the fresh visibility set.
        for (entity, mob_pos) in mob_query.iter() {
            if visible_set.contains(&(mob_pos.x, mob_pos.y)) {
                commands.entity(entity).remove::<Hidden>();
            } else {
                commands.entity(entity).insert(Hidden);
            }
        }

        if viewshed.revealed_tiles.len() < MAP_TILE_COUNT {
            viewshed.revealed_tiles.grow(MAP_TILE_COUNT);
        }
        for &(x, y) in visible_set.iter() {
            if x < MAP_WIDTH && y < MAP_HEIGHT {
                viewshed.revealed_tiles.insert(tile_index(x, y));
            }
        }
        viewshed.visible_tiles = visible_set.into_iter().collect();
        viewshed.dirty = false;
    }
}
