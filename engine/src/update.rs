use bevy_ecs::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use models::*;

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

    if let Some(mut target_fighter) = world.get_mut::<Fighter>(target_entity) {
        target_fighter.hp -= damage;
        if target_fighter.hp <= 0 {
            world.despawn(target_entity);
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

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                world.resource_mut::<GameState>().is_running = false;
            }
            // Cardinal movements (WASD, Arrows, Vi keys)
            KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Up => turn_taken = move_player(world, 0, -1),
            KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Down => turn_taken = move_player(world, 0, 1),
            KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Left => turn_taken = move_player(world, -1, 0),
            KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Right => turn_taken = move_player(world, 1, 0),
            
            // Diagonal movements (yubn)
            KeyCode::Char('y') => turn_taken = move_player(world, -1, -1), // Up-Left
            KeyCode::Char('u') => turn_taken = move_player(world, 1, -1),  // Up-Right
            KeyCode::Char('b') => turn_taken = move_player(world, -1, 1),  // Down-Left
            KeyCode::Char('n') => turn_taken = move_player(world, 1, 1),   // Down-Right
            
            _ => {}
        }
    }

    Ok(turn_taken)
}