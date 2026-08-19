use bevy_ecs::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use models::*;
use models::{GameState, components::GameLog}; 

fn move_player(world: &mut World, dx: i16, dy: i16) -> bool {
    // 1. Get player entity and calculate target position
    let mut player_data = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Player>>();
        if let Some((entity, pos)) = query.iter(world).next() {
            player_data = Some((
                entity,
                pos.x.saturating_add_signed(dx),
                pos.y.saturating_add_signed(dy),
            ));
        }
    }
    let Some((player_entity, new_x, new_y)) = player_data else {
        return false;
    };
    
    // 2. Check if a Wall exists at the target coordinates
    let is_wall = world
        .query_filtered::<&Position, With<Wall>>()
        .iter(world)
        .any(|pos| pos.x == new_x && pos.y == new_y);

    if is_wall {
        return false; // Bumped into a wall, turn is NOT consumed
    }

    // 3. Check if a Mob exists at the target coordinates to attack
    let mut target_mob_entity = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Mob>>();
        for (entity, pos) in query.iter(world) {
            if pos.x == new_x && pos.y == new_y {
                target_mob_entity = Some(entity);
                break;
            }
        }
    }

    if let Some(target_entity) = target_mob_entity {
        player_attack(world, player_entity, target_entity);
        return true; // Attacking consumes a turn
    }

    // 4. Move player if the path is completely clear
    if let Some(mut pos) = world.get_mut::<Position>(player_entity) {
        pos.x = new_x;
        pos.y = new_y;
    }
    if let Some(mut viewshed) = world.get_mut::<Viewshed>(player_entity) {
        viewshed.dirty = true;
    }

    //5. Query world to see if there is an item at the new position and pick it up if so
    let mut item_entity_to_pickup = None;
    {
        let mut query = world.query_filtered::<(Entity, &Position), With<Item>>();
        for (entity, pos) in query.iter(world) {
            if pos.x == new_x && pos.y == new_y {
                item_entity_to_pickup = Some(entity);
                break;
            }
        }
    }
    if let Some(item_entity) = item_entity_to_pickup {
        if let Some(mut backpack) = world.get_mut::<Backpack>(player_entity) {
            backpack.items.push(item_entity);
            world.entity_mut(item_entity).remove::<Position>();
            let mut log = world.resource_mut::<GameLog>();
            log.add("You pick up an item!".to_string());
        }
    }

    true // Successfully moved, consuming a turn
}

fn player_attack(world: &mut World, attacker_entity: Entity, target_entity: Entity) {
    let attacker_power = world
        .get::<Fighter>(attacker_entity)
        .map(|f| f.power)
        .unwrap_or(1);

    let target_armor = world
        .get::<Fighter>(target_entity)
        .map(|f| f.armor)
        .unwrap_or(0);

    let damage = (attacker_power - target_armor).max(0);
    let mut log = world.resource_mut::<GameLog>();
    if damage > 0 {
        log.add(format!("You smack the monster for {} damage!", damage));
    } else {
        log.add("You swing wildly and miss!".to_string());
    }
    drop(log); // Drop borrow before despawning below
    if let Some(mut target_fighter) = world.get_mut::<Fighter>(target_entity) {
        target_fighter.hp -= damage;
        if target_fighter.hp <= 0 {
            world.despawn(target_entity);
            let mut log = world.resource_mut::<GameLog>();
            log.add("The monster is dead!".to_string());
        }
    }
}

pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool> {
    let event = read()?;
    let mut turn_taken = false;

    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }
        
        {
            let mut log = world.resource_mut::<GameLog>();
            
            // 1. Only block and require a spacebar IF there are more than 3 messages.
            if log.unread.len() > 3 {
                if key.code == KeyCode::Char(' ') || key.code == KeyCode::Enter {
                    let to_remove = log.unread.len().min(3);
                    log.unread.drain(0..to_remove);
                }
                return Ok(false); 
            }
        } // Block ends, log borrow is dropped cleanly!

        // 2. Normal game input
        let mut dx = 0;
        let mut dy = 0;
        let mut action_attempted = false;
        
        // Map the key strictly once!
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                world.resource_mut::<GameState>().is_running = false;
            }
            KeyCode::Char('i') => {
                let player_entity = world
                    .query_filtered::<Entity, With<Player>>()
                    .iter(world)
                    .next();

                let is_empty = if let Some(entity) = player_entity {
                    world.get::<Backpack>(entity).map_or(true, |bp| bp.items.is_empty())
                } else {
                    true
                };

                if is_empty {
                    let mut log = world.resource_mut::<GameLog>();
                    log.add("You have no items.");
                    world.resource_mut::<PackIsOpen>().open = false;
                } else {
                    let mut pack_open = world.resource_mut::<PackIsOpen>();
                    pack_open.open = !pack_open.open;
                }
                return Ok(false);
            }
            KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Up => { dy = -1; action_attempted = true; }
            KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Down => { dy = 1; action_attempted = true; }
            KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Left => { dx = -1; action_attempted = true; }
            KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Right => { dx = 1; action_attempted = true; }
            KeyCode::Char('y') => { dx = -1; dy = -1; action_attempted = true; }
            KeyCode::Char('u') => { dx = 1; dy = -1; action_attempted = true; }
            KeyCode::Char('b') => { dx = -1; dy = 1; action_attempted = true; }
            KeyCode::Char('n') => { dx = 1; dy = 1; action_attempted = true; }
            _ => {} // Unrecognized key; do nothing
        }

        // 3. If they successfully pressed a movement key, clear the logs and execute
        if action_attempted {
            world.resource_mut::<GameLog>().unread.clear();
            turn_taken = move_player(world, dx, dy);
        }
    }

    Ok(turn_taken)
}