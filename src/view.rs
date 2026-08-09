use std::io::{Stdout, Write};
use bevy_ecs::prelude::*;
use crossterm::{
    queue,
    style::{Print, SetForegroundColor},
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};
use crate::model::{Position, Renderable};

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {
    queue!(stdout, Clear(ClearType::All))?;

    let mut query = world.query::<(&Position, &Renderable)>();
    for (position, renderable) in query.iter(world) {
        queue!(
            stdout,
            MoveTo(position.x, position.y),
            SetForegroundColor(renderable.color),
            Print(renderable.glyph),
        )?;
    }

    stdout.flush()?;
    Ok(())
}
