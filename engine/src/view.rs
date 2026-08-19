use std::collections::HashSet;
use std::io::{Stdout, Write};

use bevy_ecs::prelude::*;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, SetForegroundColor},
};

use models::*;

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {
    if world.resource::<PackIsOpen>().open {
        let mut query = world.query_filtered::<&Backpack, With<Player>>();
        let backpack = query.single(world);

        let mut inventory_line = 5;

        for &item_entity in &backpack.items {
            if let Some(item) = world.get::<Item>(item_entity) {
                queue!(
                    stdout,
                    MoveTo(5, inventory_line),
                    SetForegroundColor(Color::White),
                    Print(&item.name)
                )?;

                inventory_line += 2;
            }
        }

        stdout.flush()?;
        Ok(())
    } else {
    // 1. Get player viewshed and fighter stats.
    let (visible_tiles, revealed_tiles, player_hp, player_max_hp) = {
        let mut query = world.query_filtered::<(&Viewshed, &Fighter), With<Player>>();

        if let Some((viewshed, fighter)) = query.iter(world).next() {
            (
                viewshed.visible_tiles.clone(),
                viewshed.revealed_tiles.clone(),
                fighter.hp,
                fighter.max_hp,
            )
        } else {
            (Default::default(), Default::default(), 10, 10)
        }
    };

    // 2. Collect positions occupied by actors.
    // Floor/map entities must NOT suppress items.
    let occupied_by_actor = {
        let mut occupied = HashSet::new();

        let mut query = world.query_filtered::<&Position, Or<(With<Player>, With<Mob>)>>();

        for pos in query.iter(world) {
            occupied.insert((pos.x, pos.y));
        }

        occupied
    };

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
        Option<&Item>,
        Option<&Wall>,
        Option<&Room>,
        Option<&Passage>,
    ), Without<Hidden>>();

    for (pos, renderable, item, wall, room, passage) in query.iter(world) {
        // Items lose rendering priority to players/monsters.
        if item.is_some() && occupied_by_actor.contains(&(pos.x, pos.y)) {
            continue;
        }

        let tile_coord = (pos.x, pos.y);
        let is_visible = visible_tiles.contains(&tile_coord);
        let is_revealed = revealed_tiles.contains(&tile_coord);
        let is_map_tile = wall.is_some() || room.is_some() || passage.is_some();

        let render_y = pos.y + 1;

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
        } else {
            queue!(
                stdout,
                MoveTo(pos.x as u16, render_y as u16),
                SetForegroundColor(Color::Black),
                Print(" ")
            )?;            
        }
    }

    // ==========================================
    // THE LOG & --MORE-- PROMPT (3 Fixed Rows)
    // ==========================================
    let log = world.resource::<GameLog>();
    let unread_len = log.unread.len();

    if unread_len > 0 {
        let count = unread_len.min(3);
        let mut lines = vec![String::new(), String::new(), String::new()];

        for (i, msg) in log.unread.iter().take(count).enumerate() {
            lines[i] = msg.clone();
        }

        queue!(
            stdout,
            MoveTo(0, 22),
            SetForegroundColor(Color::White),
            Print(format!("{:<80}", lines[0])),
            MoveTo(0, 23),
            SetForegroundColor(Color::White),
            Print(format!("{:<80}", lines[1]))
        )?;

        if unread_len > 3 {
            let prompt_line = format!("{:<57} --MORE-- (Press Space)", lines[2]);

            queue!(
                stdout,
                MoveTo(0, 24),
                SetForegroundColor(Color::Yellow),
                Print(format!("{:<80}", prompt_line))
            )?;
        } else {
            queue!(
                stdout,
                MoveTo(0, 24),
                SetForegroundColor(Color::White),
                Print(format!("{:<80}", lines[2]))
            )?;
        }
    } else {
        queue!(
            stdout,
            MoveTo(0, 22),
            Print(format!("{:<80}", "")),
            MoveTo(0, 23),
            Print(format!("{:<80}", "")),
            MoveTo(0, 24),
            Print(format!("{:<80}", ""))
        )?;
    }

    stdout.flush()?;
    Ok(())
    }
}