use std::collections::HashSet;
use std::io::Write;

use bevy_ecs::prelude::*;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{Color, Print, SetForegroundColor},
    terminal::size,
};

use models::*;

/// Screen dimensions. The classic 80x25: row 0 is the status line, rows 1..=22
/// hold the map, and rows 22..25 hold the message log.
pub const SCREEN_W: u16 = 80;
pub const SCREEN_H: u16 = 25;

const BLANK_CELL: (char, Color) = (' ', Color::Reset);

/// Double-buffered character grid. `render` paints the whole frame into `cur`;
/// `flush` then emits terminal commands only for the cells that differ from the
/// previously displayed frame, so a typical turn writes a few dozen cells
/// instead of repainting all 2000.
pub struct Screen {
    cur: Vec<(char, Color)>,
    prev: Vec<(char, Color)>,
    /// Force a full repaint on the next flush (first frame, or the centering
    /// offset changed and stale cells would otherwise be left behind).
    dirty_all: bool,
    last_offset: (u16, u16),
}

impl Screen {
    pub fn new() -> Self {
        let len = (SCREEN_W * SCREEN_H) as usize;
        Self {
            cur: vec![BLANK_CELL; len],
            prev: vec![BLANK_CELL; len],
            dirty_all: true,
            last_offset: (0, 0),
        }
    }

    fn clear(&mut self) {
        for c in &mut self.cur {
            *c = BLANK_CELL;
        }
    }

    #[inline]
    fn put(&mut self, x: u16, y: u16, ch: char, color: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize] = (ch, color);
        }
    }

    #[inline]
    fn get(&self, x: u16, y: u16) -> (char, Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize]
        } else {
            BLANK_CELL
        }
    }

    fn puts(&mut self, x: u16, y: u16, s: &str, color: Color) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as u16, y, ch, color);
        }
    }

    fn hline(&mut self, x: u16, y: u16, ch: char, n: u16, color: Color) {
        for i in 0..n {
            self.put(x + i, y, ch, color);
        }
    }

    fn flush<W: Write>(&mut self, out: &mut W, offset: (u16, u16)) -> std::io::Result<()> {
        if offset != self.last_offset {
            self.dirty_all = true;
            self.last_offset = offset;
        }

        let mut cur_color: Option<Color> = None;
        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W {
                let idx = (y * SCREEN_W + x) as usize;
                if !self.dirty_all && self.cur[idx] == self.prev[idx] {
                    continue;
                }
                let (ch, color) = self.cur[idx];
                if cur_color != Some(color) {
                    queue!(out, SetForegroundColor(color))?;
                    cur_color = Some(color);
                }
                queue!(out, MoveTo(offset.0 + x, offset.1 + y), Print(ch))?;
            }
        }
        out.flush()?;

        std::mem::swap(&mut self.cur, &mut self.prev);
        self.dirty_all = false;
        Ok(())
    }
}

// Targeting-beam trajectory, in map coordinates.
fn bresenham_line(x0: u16, y0: u16, x1: u16, y1: u16) -> Vec<(u16, u16)> {
    let mut result = Vec::new();

    let mut x = x0 as i32;
    let mut y = y0 as i32;
    let target_x = x1 as i32;
    let target_y = y1 as i32;

    let dx = (target_x - x).abs();
    let sx = if x < target_x { 1 } else { -1 };
    let dy = -(target_y - y).abs();
    let sy = if y < target_y { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        result.push((x as u16, y as u16));
        if x == target_x && y == target_y {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    result
}

pub fn render<W: Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut Screen,
) -> std::io::Result<()> {
    screen.clear();

    let is_centered = world
        .get_resource::<RenderConfig>()
        .map(|cfg| cfg.centered)
        .unwrap_or(false);
    let offset = if is_centered {
        let (term_width, term_height) = size().unwrap_or((SCREEN_W, SCREEN_H));
        (
            term_width.saturating_sub(SCREEN_W) / 2,
            term_height.saturating_sub(SCREEN_H) / 2,
        )
    } else {
        (0, 0)
    };

    // 1. Player-derived state.
    let (visible, revealed, player_hp, player_max_hp, player_pos) = {
        let mut query = world.query_filtered::<(&Viewshed, &Fighter, &Position), With<Player>>();
        if let Some((viewshed, fighter, pos)) = query.iter(world).next() {
            (
                viewshed.visible_tiles.iter().copied().collect::<HashSet<_>>(),
                viewshed.revealed_tiles.clone(),
                fighter.hp,
                fighter.max_hp,
                (pos.x, pos.y),
            )
        } else {
            (HashSet::new(), Default::default(), 10, 10, (0, 0))
        }
    };

    // 2. Targeting beam.
    let targeting = world.resource::<TargetingState>();
    let is_targeting = targeting.active;
    let target_line: HashSet<(u16, u16)> = if is_targeting {
        bresenham_line(
            player_pos.0,
            player_pos.1,
            targeting.cursor_x as u16,
            targeting.cursor_y as u16,
        )
        .into_iter()
        .collect()
    } else {
        HashSet::new()
    };

    // 3. Tiles occupied by an actor (so we don't draw a floor item under a mob).
    let occupied_by_actor: HashSet<(u16, u16)> = {
        let mut query = world.query_filtered::<&Position, Or<(With<Player>, With<Mob>)>>();
        query.iter(world).map(|pos| (pos.x, pos.y)).collect()
    };

    let player_name = world.resource::<PlayerName>().what.clone();

    // ---- Status line ----
    screen.puts(
        0,
        0,
        &format!(" {} | Hits: {} / {} ", player_name, player_hp, player_max_hp),
        Color::Cyan,
    );

    // ---- Terrain ----
    let map = world.resource::<Map>().clone();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let coord = (x, y);
            let (glyph, lit) = tile_appearance(map.tile(x, y));
            let (ch, color) = if visible.contains(&coord) {
                (glyph, lit)
            } else if revealed.contains(tile_index(x, y)) {
                (glyph, Color::DarkGrey)
            } else {
                continue; // unexplored: leave blank
            };
            screen.put(x, y + 1, ch, color);
        }
    }

    // ---- Floor items (only where currently visible and not under an actor) ----
    {
        let mut query =
            world.query_filtered::<(&Position, &Renderable), (With<Item>, Without<Hidden>)>();
        for (pos, renderable) in query.iter(world) {
            let coord = (pos.x, pos.y);
            if !visible.contains(&coord) || occupied_by_actor.contains(&coord) {
                continue;
            }
            screen.put(pos.x, pos.y + 1, renderable.glyph, renderable.color);
        }
    }

    // ---- Actors (visibility system already tags out-of-sight mobs Hidden) ----
    {
        let mut query = world
            .query_filtered::<(&Position, &Renderable), (Or<(With<Player>, With<Mob>)>, Without<Hidden>)>();
        for (pos, renderable) in query.iter(world) {
            if !visible.contains(&(pos.x, pos.y)) {
                continue;
            }
            screen.put(pos.x, pos.y + 1, renderable.glyph, renderable.color);
        }
    }

    // ---- Targeting beam overlay ----
    if is_targeting {
        for &(tx, ty) in &target_line {
            if tx >= MAP_WIDTH || ty >= MAP_HEIGHT {
                continue;
            }
            if visible.contains(&(tx, ty)) && occupied_by_actor.contains(&(tx, ty)) {
                // Keep the actor's glyph but recolour it.
                let (ch, _) = screen.get(tx, ty + 1);
                screen.put(tx, ty + 1, ch, Color::Yellow);
            } else {
                screen.put(tx, ty + 1, '*', Color::Yellow);
            }
        }
    }

    // ---- Message log (rows 22..=24) ----
    {
        let log = world.resource::<GameLog>();
        let unread_len = log.unread.len();
        let count = unread_len.min(3);
        for i in 0..count {
            let y = 22 + i as u16;
            if i == 2 && unread_len > 3 {
                let line = format!("{:<57} --MORE-- (Press Space)", log.unread[2]);
                screen.puts(0, y, &line, Color::Yellow);
            } else {
                screen.puts(0, y, &log.unread[i], Color::White);
            }
        }
    }

    // ---- Inventory overlay ----
    if world.resource::<PackIsOpen>().open {
        draw_inventory(world, screen);
    }

    screen.flush(stdout, offset)
}

fn draw_inventory(world: &mut World, screen: &mut Screen) {
    let (selected_idx, action_mode, action_selected) = {
        let p = world.resource::<PackIsOpen>();
        (p.selected, p.action_mode, p.action_selected)
    };

    // (display name, is-equipped) for every backpack slot.
    let item_names: Vec<(String, bool)> = {
        let entities: Vec<Entity> = {
            let mut query = world.query_filtered::<&Backpack, With<Player>>();
            match query.iter(world).next() {
                Some(backpack) => backpack.items.clone(),
                None => Vec::new(),
            }
        };
        entities
            .iter()
            .map(|&e| {
                let name = world.get::<Item>(e).map(|it| it.name.clone()).unwrap_or_default();
                let equipped = world.get::<Wield>(e).is_some_and(|w| w.wielder.is_some())
                    || world.get::<Wear>(e).is_some_and(|w| w.wearer.is_some());
                (name, equipped)
            })
            .collect()
    };

    let box_width: u16 = 30;
    let start_x: u16 = 5;
    let start_y: u16 = 3;
    let grey = Color::DarkGrey;

    // Top border + title.
    screen.put(start_x, start_y, '┌', grey);
    screen.hline(start_x + 1, start_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, start_y, '┐', grey);
    let title = " INVENTORY ";
    screen.puts(
        start_x + box_width / 2 - title.len() as u16 / 2,
        start_y,
        title,
        Color::Yellow,
    );

    // Rows.
    for (i, (name, equipped)) in item_names.iter().enumerate() {
        let y = start_y + 1 + i as u16;
        let letter = (b'a' + i as u8) as char;
        let color = if i == selected_idx {
            Color::Yellow
        } else if *equipped {
            Color::Cyan
        } else {
            Color::White
        };
        let suffix = if *equipped { " (E)" } else { "" };
        let text = format!(" {}) {}{} ", letter, name, suffix);
        screen.put(start_x, y, '│', grey);
        screen.puts(start_x + 1, y, &format!("{:<w$}", text, w = box_width as usize), color);
        screen.put(start_x + 1 + box_width, y, '│', grey);
    }

    // Bottom border.
    let bottom_y = start_y + 1 + item_names.len() as u16;
    screen.put(start_x, bottom_y, '└', grey);
    screen.hline(start_x + 1, bottom_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, bottom_y, '┘', grey);

    // Use / Drop action modal.
    if let Some(action_idx) = action_mode {
        let mx = start_x + box_width + 2;
        let my = start_y + 1 + action_idx as u16;
        screen.puts(mx, my, "┌────────┐", grey);
        screen.put(mx, my + 1, '│', grey);
        screen.puts(
            mx + 1,
            my + 1,
            " Use    ",
            if action_selected == 0 { Color::Yellow } else { Color::White },
        );
        screen.put(mx + 9, my + 1, '│', grey);
        screen.put(mx, my + 2, '│', grey);
        screen.puts(
            mx + 1,
            my + 2,
            " Drop   ",
            if action_selected == 1 { Color::Yellow } else { Color::White },
        );
        screen.put(mx + 9, my + 2, '│', grey);
        screen.puts(mx, my + 3, "└────────┘", grey);
    }
}
