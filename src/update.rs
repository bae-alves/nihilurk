use bevy_ecs::prelude::*;
use crossterm::event::{poll, read, Event, KeyCode};
use std::time::Duration;
use crate::model::{GameState, Player, Position, Wall};

fn move_player(world: &mut World, dx: i16, dy: i16) {
    // 1. Calculate target position
    let mut target_pos = None;
    if let Some((pos, _)) = world.query::<(&Position, With<Player>)>().iter(world).next() {
        target_pos = Some((
            pos.x.saturating_add_signed(dx),
            pos.y.saturating_add_signed(dy),
        ));
    }
    let Some((new_x, new_y)) = target_pos else { return };
    // 2. Check if any Wall component exists at target coordinates
    let is_blocked = world
        .query::<(&Position, With<Wall>)>()
        .iter(world)
        .any(|(pos, _)| pos.x == new_x && pos.y == new_y);
    // 3. Move player if path is clear
    if !is_blocked {
        if let Some((mut pos, _)) = world.query::<(&mut Position, With<Player>)>().iter_mut(world).next() {
            pos.x = new_x;
            pos.y = new_y;
        }
    }
}

pub fn process_input_and_update(world: &mut World) -> std::io::Result<()> {
    if poll(Duration::from_millis(16))? {
        if let Event::Key(key) = read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    world.resource_mut::<GameState>().is_running = false;
                }
                KeyCode::Char('w') | KeyCode::Up => move_player(world, 0, -1),
                KeyCode::Char('s') | KeyCode::Down => move_player(world, 0, 1),
                KeyCode::Char('a') | KeyCode::Left => move_player(world, -1, 0),
                KeyCode::Char('d') | KeyCode::Right => move_player(world, 1, 0),
                _ => {}
            }
        }
    }

    Ok(())
}
