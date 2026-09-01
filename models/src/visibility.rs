use bevy_ecs::prelude::*;
use std::collections::{HashSet, VecDeque};
use crate::components::*;
use crate::map::{tile_index, Map, TileType, MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH};
use crate::traps::{Trap, TrapReveal};

#[inline]
fn in_bounds(x: i16, y: i16) -> bool {
    x >= 0 && y >= 0 && (x as u16) < MAP_WIDTH && (y as u16) < MAP_HEIGHT
}

pub fn visibility_system(
    mut commands: Commands,

    // `With<Player>` matters: without it a future monster viewshed would reveal
    // the map for the player.
    mut viewshed_query: Query<(&mut Viewshed, &Position), With<Player>>,

    // Everything the player can "spot": monsters and floor items. `Option`s let
    // one query cover both kinds and track the per-entity spotted state.
    spot_query: Query<
        (Entity, &Position, Option<&Mob>, Option<&Name>, Option<&Spotted>),
        Or<(With<Mob>, With<Item>)>,
    >,

    // Hidden traps whose reveal style might trip this turn.
    mut trap_query: Query<(Entity, &Position, &mut Trap), With<Hidden>>,

    mut log: ResMut<GameLog>,

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

        // Rule B: Flood-fill Room logic (with leak prevention!). A dark room is
        // skipped entirely — inside one you see only the always-on 3x3, as if it
        // were a passage, until a wand of light clears its `dark` bits.
        if !map.is_dark(pos.x, pos.y)
            && matches!(map.tile(pos.x, pos.y), TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs) {
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

                        // Never see into (or spread through) an unlit dark room.
                        if map.is_dark(neighbor_pos.0, neighbor_pos.1) {
                            continue;
                        }

                        match map.tile(neighbor_pos.0, neighbor_pos.1) {
                            TileType::Room | TileType::Door | TileType::Downstairs | TileType::Upstairs => {
                                visible_set.insert(neighbor_pos);
                                if visited_rooms.insert(neighbor_pos) {
                                    queue.push_back(neighbor_pos); // keep spreading inside rooms/doors/stairs
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

        // Hide/reveal monsters and announce anything freshly in view.
        for (entity, target_pos, mob, name, spotted) in spot_query.iter() {
            let in_view = visible_set.contains(&(target_pos.x, target_pos.y));

            if mob.is_some() {
                if in_view {
                    commands.entity(entity).remove::<Hidden>();
                } else {
                    commands.entity(entity).insert(Hidden);
                }
            }

            if in_view && spotted.is_none() {
                match name {
                    Some(name) => log.add(format!("you spotted {} {}", name.article(), name.what)),
                    None => log.add("you spotted something"),
                }
                commands.entity(entity).insert(Spotted);
            } else if !in_view && spotted.is_some() {
                commands.entity(entity).remove::<Spotted>();
            }
        }

        // Bring hidden traps to light: a `Sight` trap the instant its tile is in
        // view, an `Adjacent` trap once the player is standing next to it. A
        // `Triggered` trap stays invisible until something sets it off. Once
        // revealed it latches (Hidden removed for good).
        for (trap_entity, tpos, mut trap) in trap_query.iter_mut() {
            if trap.revealed {
                continue;
            }
            let found = match trap.reveal {
                TrapReveal::Sight => visible_set.contains(&(tpos.x, tpos.y)),
                TrapReveal::Adjacent => {
                    (tpos.x as i32 - pos.x as i32).abs() <= 1
                        && (tpos.y as i32 - pos.y as i32).abs() <= 1
                }
                TrapReveal::Triggered => false,
            };
            if found {
                trap.revealed = true;
                commands.entity(trap_entity).remove::<Hidden>();
                log.add(format!("you spot {} {}", trap.effect.label_article(), trap.effect.label()));
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
