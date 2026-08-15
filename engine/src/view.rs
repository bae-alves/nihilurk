use std::io::{Stdout, Write};
use bevy_ecs::prelude::*;
use crossterm::{
    queue,
    style::{Print, SetForegroundColor, Color},
    cursor::MoveTo,
};
use models::*;

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {
    // 1. Get player viewshed and fighter stats (with safe fallbacks to prevent 0/0 viewshed failure)
    let (visible_tiles, revealed_tiles, player_hp, player_max_hp) = {
        let mut query = world.query_filtered::<(&Viewshed, &Fighter), With<Player>>();
        if let Some((viewshed, fighter)) = query.iter(world).next() {
            (viewshed.visible_tiles.clone(), viewshed.revealed_tiles.clone(), fighter.hp, fighter.max_hp)
        } else {
            (Default::default(), Default::default(), 10, 10) // Fallback defaults so viewshed doesn't break
        }
    }; // <-- Borrow on `world` cleanly ends here!

    // TOP UI
    queue!(
        stdout,
        MoveTo(0, 0),
        SetForegroundColor(Color::Cyan),
        Print(format!(" ROOG | Hits: {} / {} ", player_hp, player_max_hp))
    )?;

    // DUNGEON GRID
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

    // ==========================================
    // THE LOG & --MORE-- PROMPT (3 Fixed Rows)
    // ==========================================
    let log = world.resource::<GameLog>();
    let is_more = !log.unread.is_empty();

    if is_more {
        let count = log.unread.len().min(3);
        let mut lines = vec![String::new(), String::new(), String::new()];
        for (i, msg) in log.unread.iter().take(count).enumerate() {
            lines[i] = msg.clone();
        }

        // Line 1 (y = 22)
        queue!(
            stdout,
            MoveTo(0, 22),
            SetForegroundColor(Color::White),
            Print(format!("{:<80}", lines[0]))
        )?;

        // Line 2 (y = 23)
        queue!(
            stdout,
            MoveTo(0, 23),
            SetForegroundColor(Color::White),
            Print(format!("{:<80}", lines[1]))
        )?;

        // Line 3 (y = 24)
        if count == 3 {
            // If we have all 3 messages, share the line with --MORE-- on the right
            let prompt_line = format!("{:<57} --MORE-- (Press Space)", lines[2]);
            queue!(
                stdout,
                MoveTo(0, 24),
                SetForegroundColor(Color::Yellow),
                Print(format!("{:<80}", prompt_line))
            )?;
        } else {
            // If there are only 1 or 2 messages, print the 3rd line normally (blank) 
            // and show --MORE-- cleanly without empty text gaps.
            queue!(
                stdout,
                MoveTo(0, 24),
                SetForegroundColor(Color::Yellow),
                Print(format!("{:<80}", "--MORE-- (Press Space)"))
            )?;
        }
    } else {
        // Quiet turn: completely clear all 3 log rows so nothing lingers
        queue!(
            stdout,
            MoveTo(0, 22),
            Print(format!("{:<80}", ""))
        )?;
        queue!(
            stdout,
            MoveTo(0, 23),
            Print(format!("{:<80}", ""))
        )?;
        queue!(
            stdout,
            MoveTo(0, 24),
            Print(format!("{:<80}", ""))
        )?;
    }

    stdout.flush()?;
    Ok(())
}