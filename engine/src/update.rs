use bevy_ecs::prelude::*;
use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use models::*;

fn move_player(world: &mut World, dx: i16, dy: i16) -> bool {
    // 1. Calculate target position
    let mut target_pos = None;
    if let Some((pos, _)) = world.query::<(&Position, With<Player>)>().iter(world).next() {
        target_pos = Some((
            pos.x.saturating_add_signed(dx),
            pos.y.saturating_add_signed(dy),
        ));
    }
    let Some((new_x, new_y)) = target_pos else { return false; };
    
    // 2. Check if any Wall or Mob component exists at target coordinates
    let is_blocked = world
        .query::<(&Position, Or<(With<Wall>, With<Mob>)>)>()
        .iter(world)
        .any(|(pos, _)| pos.x == new_x && pos.y == new_y);
        
    // 3. Move player if path is clear
    if !is_blocked {
        if let Some((mut pos, _)) = world.query::<(&mut Position, With<Player>)>().iter_mut(world).next() {
            pos.x = new_x;
            pos.y = new_y;
        }
        if let Some((mut viewshed, _)) = world.query::<(&mut Viewshed, With<Player>)>().iter_mut(world).next() {
            viewshed.dirty = true;
        }
        return true; // The player successfully moved, consuming a turn
    }
    
    false // The player bumped into a wall or mob, turn is NOT consumed
}

pub fn process_input_and_update(world: &mut World) -> std::io::Result<bool> {
    // read() halts the thread until an event occurs
    let event = read()?;
    let mut turn_taken = false;

    if let Event::Key(key) = event {
        // Ignore key release events to prevent double-turns on a single press
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