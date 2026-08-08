use std::io::{Stdout, Write};
use crossterm::{
    queue,
    style::{Color, Print, SetForegroundColor},
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};
use crate::model::GameState;

pub fn render(state: &GameState, stdout: &mut Stdout) -> std::io::Result<()> {
    // Queue all drawing commands
    queue!(
        stdout,
        Clear(ClearType::All),
        MoveTo(state.player_x, state.player_y),
        SetForegroundColor(Color::Yellow),
        Print("@")
    )?;
    
    // Flush to terminal in one big batch
    stdout.flush()?;
    
    Ok(())
}