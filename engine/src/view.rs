use std::io::{Stdout, Write};
use bevy_ecs::prelude::*;
use crossterm::{
    queue,
    style::{Print, SetForegroundColor, Color},
    cursor::MoveTo,
};
use models::*;

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {

    // 1. Get player viewshed and fighter stats in a single query, then drop world borrow
    let (visible_tiles, revealed_tiles, player_hp, player_max_hp) = {
        let mut query = world.query_filtered::<(&Viewshed, &Fighter), With<Player>>();
        if let Some((viewshed, fighter)) = query.iter(world).next() {
            (viewshed.visible_tiles.clone(), viewshed.revealed_tiles.clone(), fighter.hp, fighter.max_hp)
        } else {
            (Default::default(), Default::default(), 0, 0) // Automatically matches the viewshed collection types!
        }
    }; // <-- The borrow on `world` cleanly ends here!<-- The borrow on `world` cleanly ends here! <-- Borrow on `world` ends here!

    queue!(
        stdout,
        MoveTo(0, 0),
        SetForegroundColor(Color::Cyan),
        Print(format!(" ROOG | Hits: {} / {} ", player_hp, player_max_hp))
    )?;

    let mut query = world.query_filtered::<(
        &Position,
        &Renderable,
        Option<&Wall>,
        Option<&Room>,
        Option<&Passage>,
    ), Without<Hidden>>();

    for (pos, renderable, wall, room, passage) in query.iter(world) {
        let tile_coord = (pos.x, pos.y);
        let is_visible = visible_tiles.contains(&tile_coord);
        let is_revealed = revealed_tiles.contains(&tile_coord);
        let is_map_tile = wall.is_some() || room.is_some() || passage.is_some();

        let render_y = pos.y + 1; // Shift down by 1 for the top UI line

        if is_visible {
            queue!(
                stdout,
                MoveTo(pos.x as u16, render_y as u16),
                SetForegroundColor(renderable.color),
                Print(renderable.glyph)
            )?;
        } else if is_map_tile && is_revealed {
            queue!(
                stdout,
                MoveTo(pos.x as u16, render_y as u16),
                SetForegroundColor(Color::DarkGrey),
                Print(renderable.glyph)
            )?;
        }
    }

    queue!(
        stdout,
        MoveTo(0, 22),
        SetForegroundColor(Color::Green),
        Print("Welcome to ROOG!")
    )?;

    stdout.flush()?;
    Ok(())
}