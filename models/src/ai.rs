use std::collections::HashMap;
use bevy_ecs::prelude::*;
use crate::components::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Debug)]
enum TileType {
    Room,
    Passage,
    Wall,
    Door,
}

pub fn ai(
    mut param_set: ParamSet<(
        Query<(Entity, &Position, &Viewshed, &Faction), With<Player>>,     // p0
        Query<(Entity, &Mob, &mut Position, &Faction), Without<Player>>,   // p1
        Query<(Entity, &Position), With<Wall>>,                            // p2
        Query<(&Position, Option<&Room>, Option<&Passage>, Option<&Wall>, Option<&Door>), Without<Player>>, // p3
    )>,
    mut attack_queue: ResMut<AttackQueue>,
) {
    // 1. Get player info and clone visible_tiles so p0 can be dropped immediately
    let player_data = {
        let p0 = param_set.p0();
        let Ok(player) = p0.get_single() else { return; };
        
        let visible_tiles: HashSet<(u16, u16)> = player.2
            .visible_tiles
            .iter()
            .copied()
            .collect();

        (player.0, *player.1, visible_tiles, *player.3)
    };
    let (player_entity, player_pos, visible_tiles, player_faction) = player_data;

    // 2. Build map lookups (walls and tile types)
    let mut spatial_map: HashMap<(u16, u16), (Entity, Faction)> = HashMap::new();

    let walls: HashSet<(u16, u16)> = param_set.p2()
        .iter()
        .map(|(_, pos)| (pos.x, pos.y))
        .collect();

    let mut tile_map: HashMap<(u16, u16), TileType> = HashMap::new();
    for (pos, room, passage, wall, door) in param_set.p3().iter() {
        let t_type = if room.is_some() {
            TileType::Room
        } else if passage.is_some() {
            TileType::Passage
        } else if wall.is_some() {
            TileType::Wall
        } else if door.is_some() {
            TileType::Door
        } else {
            continue;
        };
        tile_map.insert((pos.x, pos.y), t_type);
    }

    // Insert player into spatial map
    spatial_map.insert((player_pos.x, player_pos.y), (player_entity, player_faction));

    // Insert all mobs into spatial map
    {
        let mob_query = param_set.p1();
        for (entity, _, pos, faction) in mob_query.iter() {
            spatial_map.insert((pos.x, pos.y), (entity, *faction));
        }
    }

    // 3. Iterate over every mob and update position or attack
    for (mob_entity, mob, mut mob_pos, mob_faction) in param_set.p1().iter_mut() {
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
                let idx = match getrandom::u32() {
                    Ok(val) => (val as usize) % directions.len(),
                    Err(_) => 0,
                };
                directions[idx]
            }
        };

        let new_x = (mob_pos.x as i16 + step_x) as u16;
        let new_y = (mob_pos.y as i16 + step_y) as u16;

        // Check map bounds
        if new_x >= 80 || new_y >= 22 {
            continue;
        }

        // Check wall collision
        if walls.contains(&(new_x, new_y)) {
            continue;
        }

        // ==========================================
        // [!] ROOM LEASH: Prevent chasing player into corridors
        // ==========================================
        if matches!(mob.movement_type, MovementType::Chase) {
            let current_tile = tile_map.get(&(mob_pos.x, mob_pos.y)).copied();
            let target_tile = tile_map.get(&(new_x, new_y)).copied();
            
            // If the monster is in a Room and tries to step into a Door or Passage, block it!
            if matches!(current_tile, Some(TileType::Room)) 
                && matches!(target_tile, Some(TileType::Passage | TileType::Door)) 
            {
                continue;
            }
        }

        // Check entity collision / interaction
        if let Some(&(target_entity, target_faction)) = spatial_map.get(&(new_x, new_y)) {
            let is_hostile = match (*mob_faction, target_faction) {
                (Faction::Monster, Faction::Player) | (Faction::Monster, Faction::Ally) => true,
                (Faction::Ally, Faction::Monster) => true,
                _ => false,
            };

            if is_hostile {
                attack_queue.attacks.push(WantsToAttack {
                    attacker: mob_entity,
                    target: target_entity,
                });
            }
            continue;
        }

        // 4. Path is completely clear: Move the mob and update spatial map
        spatial_map.remove(&(mob_pos.x, mob_pos.y));
        mob_pos.x = new_x;
        mob_pos.y = new_y;
        spatial_map.insert((new_x, new_y), (mob_entity, *mob_faction));
    }
}