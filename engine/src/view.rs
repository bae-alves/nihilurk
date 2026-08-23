use std::collections::HashSet;
use std::io::{Stdout, Write};

use bevy_ecs::prelude::*;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, SetForegroundColor},
    terminal::size,
};

use models::*; 

pub fn render(world: &mut World, stdout: &mut Stdout) -> std::io::Result<()> {
    // 1. Fetch config and calculate offsets
    let is_centered = world
        .get_resource::<RenderConfig>()
        .map(|cfg| cfg.centered)
        .unwrap_or(false);

    let (offset_x, offset_y) = if is_centered {
        let (term_width, term_height) = size().unwrap_or((80, 25));
        (
            term_width.saturating_sub(80) / 2,
            term_height.saturating_sub(25) / 2,
        )
    } else {
        (0, 0)
    };

    if world.resource::<PackIsOpen>().open {
        let mut query = world.query_filtered::<&Backpack, With<Player>>();
        let backpack = query.single(world);
        let selected_idx = world.resource::<PackIsOpen>().selected;

        let box_width = 30; // Adjust this if you have longer item names
        let num_items = backpack.items.len();
        let start_x = offset_x + 5;
        let start_y = offset_y + 3;

        // 1. Draw Top Border & Title
        let title = " INVENTORY ";
        queue!(
            stdout,
            MoveTo(start_x, start_y),
            SetForegroundColor(Color::DarkGrey),
            Print("┌"),
            Print("─".repeat(box_width)),
            Print("┐"),
            MoveTo(start_x + (box_width as u16 / 2) - (title.len() as u16 / 2), start_y),
            SetForegroundColor(Color::Yellow),
            Print(title)
        )?;
        for (i, &item_entity) in backpack.items.iter().enumerate() {
            if let Some(item) = world.get::<Item>(item_entity) {
                let letter = (b'a' + i as u8) as char;
                let is_selected = i == selected_idx;
                
                // Switch color based on selection
                let item_color = if is_selected { Color::Yellow } else { Color::White };
                
                // Format nicely: " a) Potion " and pad it to fit the box
                let item_text = format!(" {}) {} ", letter, item.name);
                let padded_text = format!("{:<width$}", item_text, width = box_width);

                queue!(
                    stdout,
                    MoveTo(start_x, start_y + 1 + i as u16),
                    SetForegroundColor(Color::DarkGrey),
                    Print("│"), // Left border
                    SetForegroundColor(item_color),
                    Print(padded_text), // Item text
                    SetForegroundColor(Color::DarkGrey),
                    Print("│") // Right border
                )?;
            }
        }

        // 3. Draw Bottom Border
        queue!(
            stdout,
            MoveTo(start_x, start_y + 1 + num_items as u16),
            SetForegroundColor(Color::DarkGrey),
            Print("└"),
            Print("─".repeat(box_width)),
            Print("┘")
        )?;

        let pack_state = world.resource::<PackIsOpen>();
        if let Some(action_idx) = pack_state.action_mode {
            let action_sel = pack_state.action_selected;
            
            // Draw it slightly to the right of the main inventory box
            let modal_x = start_x + box_width as u16 + 1;
            
            // Align the Y coordinate roughly with the item that was selected
            let modal_y = start_y + 1 + action_idx as u16; 

            queue!(
                stdout,
                MoveTo(modal_x, modal_y),
                SetForegroundColor(Color::DarkGrey),
                Print("┌────────┐"),
                
                MoveTo(modal_x, modal_y + 1),
                Print("│"),
                SetForegroundColor(if action_sel == 0 { Color::Yellow } else { Color::White }),
                Print(" Use    "),
                SetForegroundColor(Color::DarkGrey),
                Print("│"),
                
                MoveTo(modal_x, modal_y + 2),
                Print("│"),
                SetForegroundColor(if action_sel == 1 { Color::Yellow } else { Color::White }),
                Print(" Drop   "),
                SetForegroundColor(Color::DarkGrey),
                Print("│"),
                
                MoveTo(modal_x, modal_y + 3),
                SetForegroundColor(Color::DarkGrey),
                Print("└────────┘"),
            )?;
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
            MoveTo(offset_x, offset_y),
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
            if item.is_some() && occupied_by_actor.contains(&(pos.x, pos.y)) {
                continue;
            }

            let tile_coord = (pos.x, pos.y);
            let is_visible = visible_tiles.contains(&tile_coord);
            let is_revealed = revealed_tiles.contains(&tile_coord);
            let is_map_tile = wall.is_some() || room.is_some() || passage.is_some();

            let render_y = pos.y + 1;
            let screen_x = offset_x + pos.x as u16;
            let screen_y = offset_y + render_y as u16;

            if is_visible {
                queue!(
                    stdout,
                    MoveTo(screen_x, screen_y),
                    SetForegroundColor(renderable.color),
                    Print(renderable.glyph)
                )?;
            } else if is_map_tile && is_revealed {
                queue!(
                    stdout,
                    MoveTo(screen_x, screen_y),
                    SetForegroundColor(Color::DarkGrey),
                    Print(renderable.glyph)
                )?;
            } else {
                queue!(
                    stdout,
                    MoveTo(screen_x, screen_y),
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
                MoveTo(offset_x, offset_y + 22),
                SetForegroundColor(Color::White),
                Print(format!("{:<80}", lines[0])),
                MoveTo(offset_x, offset_y + 23),
                SetForegroundColor(Color::White),
                Print(format!("{:<80}", lines[1]))
            )?;

            if unread_len > 3 {
                let prompt_line = format!("{:<57} --MORE-- (Press Space)", lines[2]);

                queue!(
                    stdout,
                    MoveTo(offset_x, offset_y + 24),
                    SetForegroundColor(Color::Yellow),
                    Print(format!("{:<80}", prompt_line))
                )?;
            } else {
                queue!(
                    stdout,
                    MoveTo(offset_x, offset_y + 24),
                    SetForegroundColor(Color::White),
                    Print(format!("{:<80}", lines[2]))
                )?;
            }
        } else {
            queue!(
                stdout,
                MoveTo(offset_x, offset_y + 22),
                Print(format!("{:<80}", "")),
                MoveTo(offset_x, offset_y + 23),
                Print(format!("{:<80}", "")),
                MoveTo(offset_x, offset_y + 24),
                Print(format!("{:<80}", ""))
            )?;
        }

        stdout.flush()?;
        Ok(())
    }
}