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

    // Draw all entities within the visible tiles of the player's viewshed
    if let Some((player_pos, player_viewshed)) = world
        .query::<(&Position, &crate::model::Viewshed)>()
        .iter(world)
        .next()
    {
        // Clone needed data so the borrow on `world` ends before the next query
        let visible_tiles = player_viewshed.visible_tiles.clone();

        for (pos, renderable) in world.query::<(&Position, &Renderable)>().iter(world) {
            if visible_tiles.contains(&(pos.x, pos.y)) {
                queue!(
                    stdout,
                    MoveTo(pos.x as u16, pos.y as u16),
                    SetForegroundColor(renderable.color),
                    Print(renderable.glyph)
                )?;
            }
        }
    }

    stdout.flush()?;
    Ok(())
}
