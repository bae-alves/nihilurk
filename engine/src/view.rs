use std::io::{Stdout, Write};
use bevy_ecs::prelude::*;
use crossterm::{
    queue,
    style::{Print, SetForegroundColor, Color},
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};
use models::*;

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {
    queue!(stdout, Clear(ClearType::All))?;

    // 1. Get the player's viewshed data and immediately drop the world borrow 
    // by enclosing it in a block scope. Clone both visible and revealed tiles.
    let (visible_tiles, revealed_tiles) = {
        let mut query = world.query::<(&Position, &Viewshed)>();
        if let Some((_, viewshed)) = query.iter(world).next() {
            (viewshed.visible_tiles.clone(), viewshed.revealed_tiles.clone())
        } else {
            return Ok(()); // No player/viewshed found, bail out early
        }
    }; // <-- The borrow on `world` ends here!

    // 2. Query all renderable entities along with optional map components
    let mut query = world.query::<(
        &Position,
        &Renderable,
        Option<&Wall>,
        Option<&Room>,
        Option<&Passage>,
    )>();

    for (pos, renderable, wall, room, passage) in query.iter(world) {
        let tile_coord = (pos.x, pos.y);
        let is_visible = visible_tiles.contains(&tile_coord);
        let is_revealed = revealed_tiles.contains(&tile_coord);
        let is_map_tile = wall.is_some() || room.is_some() || passage.is_some();

        if is_visible {
            // Currently in line of sight: Draw with full brightness
            queue!(
                stdout,
                MoveTo(pos.x as u16, pos.y as u16),
                SetForegroundColor(renderable.color),
                Print(renderable.glyph)
            )?;
        } else if is_map_tile && is_revealed {
            // Not visible right now, but remembered: Draw as dark grey
            queue!(
                stdout,
                MoveTo(pos.x as u16, pos.y as u16),
                SetForegroundColor(Color::DarkGrey),
                Print(renderable.glyph)
            )?;
        }
        // If it's an actor/item that isn't visible, or a tile never seen, we do nothing (it stays hidden)
    }

    stdout.flush()?;
    Ok(())
}