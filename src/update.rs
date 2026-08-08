use crossterm::event::{poll, read, Event, KeyCode};
use std::time::Duration;
use crate::model::GameState;

pub fn process_input_and_update(state: &mut GameState) -> std::io::Result<()> {
    // Wait up to 16ms for an input event (approx 60 FPS)
    if poll(Duration::from_millis(16))? {
        if let Event::Key(key) = read()? {
            match key.code {
                // Movement
                KeyCode::Char('w') | KeyCode::Up => state.player_y = state.player_y.saturating_sub(1),
                KeyCode::Char('s') | KeyCode::Down => state.player_y = state.player_y.saturating_add(1),
                KeyCode::Char('a') | KeyCode::Left => state.player_x = state.player_x.saturating_sub(1),
                KeyCode::Char('d') | KeyCode::Right => state.player_x = state.player_x.saturating_add(1),
                
                // Quit
                KeyCode::Char('q') | KeyCode::Esc => state.is_running = false,
                
                _ => {} // Ignore other keys
            }
        }
    }
    
    Ok(())
}