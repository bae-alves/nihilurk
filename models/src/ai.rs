use std::collections::HashSet;
use bevy_ecs::prelude::*;
use crate::components::*;

pub fn ai(
    mut param_set: ParamSet<(
        Query<(&Position, &Viewshed), With<Player>>,     // p0
        Query<(&Mob, &mut Position), Without<Player>>,       // p1
        Query<&Position, With<Wall>>,                        // p2
    )>,
) {
    // 1. Bind the query so it stays alive, then get player position & viewshed
    let player_query = param_set.p0();
    let Ok(player) = player_query.get_single() else {
        return;
    };
    let player_pos = *player.0;
    
    let visible_tiles: HashSet<(u16, u16)> = player.1
        .visible_tiles
        .iter()
        .copied()
        .collect();

    // 2. Build a fast lookup set of wall coordinates from the wall query
    let walls: HashSet<(u16, u16)> = param_set.p2()
        .iter()
        .map(|pos| (pos.x, pos.y))
        .collect();

    // 3. Iterate over every mob and update their position using the mutable query
    for (mob, mut mob_pos) in param_set.p1().iter_mut() {
        if !visible_tiles.contains(&(mob_pos.x, mob_pos.y)) {
            match mob.movement_type {
                MovementType::Chase | MovementType::Flee => continue,
                _ => {}
            }
        }

        let (step_x, step_y) = match mob.movement_type {
            MovementType::Static => continue,
            MovementType::Chase => {
                let dx = player_pos.x as i16 - mob_pos.x as i16;
                let dy = player_pos.y as i16 - mob_pos.y as i16;
                (
                    if dx > 0 { 1 } else if dx < 0 { -1 } else { 0 },
                    if dy > 0 { 1 } else if dy < 0 { -1 } else { 0 },
                )
            }
            MovementType::Flee => {
                let dx = player_pos.x as i16 - mob_pos.x as i16;
                let dy = player_pos.y as i16 - mob_pos.y as i16;
                (
                    if dx > 0 { -1 } else if dx < 0 { 1 } else { 0 },
                    if dy > 0 { -1 } else if dy < 0 { 1 } else { 0 },
                )
            }
            MovementType::Confused => {
                let directions = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                let idx = (getrandom::u32().unwrap() as usize) % directions.len();
                directions[idx]
            }
        };

        let new_x = (mob_pos.x as i16 + step_x) as u16;
        let new_y = (mob_pos.y as i16 + step_y) as u16;

        // 4. Check map bounds and wall collisions
        if new_x < 80 && new_y < 22 && !walls.contains(&(new_x, new_y)) {
            mob_pos.x = new_x;
            mob_pos.y = new_y;
        }
    }
}