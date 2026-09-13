use std::collections::HashSet;
use std::io::Write;
use std::time::Duration;

use bevy_ecs::prelude::*;
use crossterm::{
    cursor::MoveTo,
    event::{poll, read},
    queue,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::size,
};

use models::*;

/// Screen dimensions. The classic 80x25: row 0 is the status line, rows 1..=22
/// hold the map, and rows 22..25 hold the message log.
pub const SCREEN_W: u16 = 80;
pub const SCREEN_H: u16 = 25;

/// The screen row map row 0 paints on — row 0 being the status line.
///
/// Every map-space painter folds this in, which is why `render`'s map layers
/// pass a raw map coordinate instead of each carrying its own `y + 1`.
const MAP_TOP: u16 = 1;

/// A screen cell: glyph, foreground colour, background colour.
type Cell = (char, Color, Color);
const BLANK_CELL: Cell = (' ', Color::Reset, Color::Reset);

/// Double-buffered character grid. `render` paints the whole frame into `cur`;
/// `flush` then emits terminal commands only for the cells that differ from the
/// previously displayed frame, so a typical turn writes a few dozen cells
/// instead of repainting all 2000.
pub struct Screen {
    cur: Vec<Cell>,
    prev: Vec<Cell>,
    /// Force a full repaint on the next flush (first frame, or the centering
    /// offset changed and stale cells would otherwise be left behind).
    dirty_all: bool,
    last_offset: (u16, u16),
    /// Where the screen shake has thrown the map this frame, in whole cells
    /// right and down. Read only by the map-space painters below, so the
    /// status line, the message log and the pack overlay stay nailed down
    /// while the floor rocks.
    ///
    /// This is *not* a second viewport. The grid is 80x25 whatever this says,
    /// and a tile the displacement pushes past the edge of the map rows is
    /// dropped by `map_cell` rather than drawn somewhere else — the game
    /// renders no more of the map mid-shake than it does at rest, and no less
    /// of anything else.
    map_shift: (i16, i16),
}

impl Screen {
    pub fn new() -> Self {
        let len = (SCREEN_W * SCREEN_H) as usize;
        Self {
            cur: vec![BLANK_CELL; len],
            prev: vec![BLANK_CELL; len],
            dirty_all: true,
            last_offset: (0, 0),
            map_shift: (0, 0),
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
            self.cur[(y * SCREEN_W + x) as usize] = (ch, color, Color::Reset);
        }
    }

    /// Sets only the foreground colour of a cell, leaving its glyph untouched.
    /// Used by the blood overlay to redden a tile in place.
    #[inline]
    fn set_fg(&mut self, x: u16, y: u16, fg: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize].1 = fg;
        }
    }

    /// Sets only the background colour of a cell, leaving its glyph and
    /// foreground untouched. Used by the travel cursor to highlight a tile.
    #[inline]
    fn set_bg(&mut self, x: u16, y: u16, bg: Color) {
        if x < SCREEN_W && y < SCREEN_H {
            self.cur[(y * SCREEN_W + x) as usize].2 = bg;
        }
    }

    #[inline]
    fn get(&self, x: u16, y: u16) -> Cell {
        if x >= SCREEN_W || y >= SCREEN_H {
            return BLANK_CELL;
        }
        self.cur[(y * SCREEN_W + x) as usize]
    }

    /// The screen cell a map tile lands on, with [`Screen::map_shift`] folded
    /// in — or `None` when the shake has thrown that tile clean out of the map
    /// viewport.
    ///
    /// The clip is against the map's own rows, not the whole grid, and that is
    /// the whole safety property of the shake: a displaced map can never smear
    /// a tile up into the status line or down into the message log, and a
    /// tile pushed off the left or right edge is not drawn at all rather than
    /// wrapping onto the next row. Off-viewport is off — nothing is rendered
    /// to fill the gap it leaves, which is why the shake reads as the map
    /// moving inside a fixed frame rather than as the frame resizing.
    #[inline]
    fn map_cell(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        let sx = x as i32 + self.map_shift.0 as i32;
        let sy = y as i32 + MAP_TOP as i32 + self.map_shift.1 as i32;
        if sx < 0 || sx >= SCREEN_W as i32 {
            return None;
        }
        if sy < MAP_TOP as i32 || sy >= (MAP_TOP + MAP_HEIGHT) as i32 {
            return None;
        }
        Some((sx as u16, sy as u16))
    }

    /// [`Screen::put`], in map coordinates.
    #[inline]
    fn put_map(&mut self, x: u16, y: u16, ch: char, color: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.put(sx, sy, ch, color);
        }
    }

    /// [`Screen::set_fg`], in map coordinates.
    #[inline]
    fn fg_map(&mut self, x: u16, y: u16, fg: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.set_fg(sx, sy, fg);
        }
    }

    /// [`Screen::set_bg`], in map coordinates.
    #[inline]
    fn bg_map(&mut self, x: u16, y: u16, bg: Color) {
        if let Some((sx, sy)) = self.map_cell(x, y) {
            self.set_bg(sx, sy, bg);
        }
    }

    /// [`Screen::get`], in map coordinates. A tile the shake has pushed out of
    /// the viewport reads as blank, which is what the targeting beam's
    /// recolour wants: there is nothing under it to preserve.
    #[inline]
    fn get_map(&self, x: u16, y: u16) -> Cell {
        match self.map_cell(x, y) {
            Some((sx, sy)) => self.get(sx, sy),
            None => BLANK_CELL,
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

        let mut cur_fg: Option<Color> = None;
        let mut cur_bg: Option<Color> = None;
        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W {
                let idx = (y * SCREEN_W + x) as usize;
                if !self.dirty_all && self.cur[idx] == self.prev[idx] {
                    continue;
                }
                let (ch, fg, bg) = self.cur[idx];
                if cur_fg != Some(fg) {
                    queue!(out, SetForegroundColor(fg))?;
                    cur_fg = Some(fg);
                }
                if cur_bg != Some(bg) {
                    queue!(out, SetBackgroundColor(bg))?;
                    cur_bg = Some(bg);
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

/// The top-left offset that keeps the 80x25 frame centred in the real terminal
/// when `-c` was passed; `(0, 0)` otherwise.
pub fn centering_offset(world: &World) -> (u16, u16) {
    let is_centered = world
        .get_resource::<RenderConfig>()
        .map(|cfg| cfg.centered)
        .unwrap_or(false);
    if !is_centered {
        return (0, 0);
    }
    let (term_width, term_height) = size().unwrap_or((SCREEN_W, SCREEN_H));
    (
        term_width.saturating_sub(SCREEN_W) / 2,
        term_height.saturating_sub(SCREEN_H) / 2,
    )
}

pub fn render<W: Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut Screen,
) -> std::io::Result<()> {
    screen.clear();

    let offset = centering_offset(world);

    // How far the screen shake has thrown the map this frame. Only the map
    // layers below read it (through `put_map` and friends); the status line,
    // the message log and the pack overlay are painted in screen coordinates
    // and stay put — a "You are badly wounded!" line has to stay readable
    // while the floor that earned it is still rocking.
    screen.map_shift = world
        .get_resource::<Shake>()
        .map_or((0, 0), |shake| shake.offset());

    // 1. Player-derived state.
    let (
        visible,
        revealed,
        player_hp,
        player_max_hp,
        player_magic,
        player_max_magic,
        player_pos,
        mut pow_die,
        mut pow_flat,
        mut arm_die,
        mut arm_flat,
        player_score,
        pack_items,
        player_entity,
    ) = {
        let mut query = world.query_filtered::<(
            Entity,
            &Viewshed,
            &Fighter,
            Option<&Magic>,
            &Position,
            &Score,
            &Backpack,
        ), With<Player>>();
        match query.iter(world).next() {
            Some((entity, viewshed, fighter, magic, pos, score, backpack)) => (
                viewshed
                    .visible_tiles
                    .iter()
                    .copied()
                    .collect::<HashSet<_>>(),
                viewshed.revealed_tiles.clone(),
                fighter.hp,
                fighter.max_hp,
                magic.map_or(0, |m| m.points),
                magic.map_or(0, |m| m.max_points),
                (pos.x, pos.y),
                fighter.power,
                fighter.power_bonus,
                fighter.armor,
                fighter.armor_bonus,
                score.value,
                backpack.items.clone(),
                Some(entity),
            ),
            None => (
                HashSet::new(),
                Default::default(),
                10,
                10,
                4,
                4,
                (0, 0),
                1,
                0,
                0,
                0,
                0,
                Vec::new(),
                None,
            ),
        }
    };

    // Fold every equipped modifier into the displayed Pow. / Arm. figures — the
    // same fold combat runs, so the HUD can never drift from the real numbers.
    if let Some(pe) = player_entity {
        pow_die += equipped_total::<PowerDie>(world, pe);
        pow_flat += equipped_total::<PowerBonus>(world, pe);
        arm_die += equipped_total::<ArmorDie>(world, pe);
        arm_flat += equipped_total::<ArmorBonus>(world, pe);
    }
    let throw_flat = player_entity.map_or(0, |pe| equipped_total::<ThrowBonus>(world, pe));

    // Transient conditions, as 4-letter HUD mnemonics. FAST/SLOW come from the
    // player's tempo, STLH from a ring of stealth, CONF from the dazzle
    // condition, BLND/PARL from the two potions that take your eyes and your
    // limbs, GLOW from a scroll of monster confusion still waiting on the next
    // blow to land, PLAT/FORG from the two coins whose reward the next
    // staircase pays. A paralysed player shows both SLOW and PARL, which is
    // honest: paralysis slows you *and* eats turns.
    //
    // The tempo is read through `models::tempo` rather than off the component,
    // so gear that weighs the player down — a ring of slow digestion — reads
    // SLOW exactly like a potion of paralysis does. It *is* the same slowing.
    let tempo = player_entity.map(|pe| models::tempo(world, pe));
    let stealthy = player_entity.is_some_and(|pe| world.get::<Stealthy>(pe).is_some());
    let conditions: Vec<(&str, Color)> = {
        let mut q = world.query_filtered::<(
            Option<&Confused>,
            Option<&Blind>,
            Option<&Paralyzed>,
            Option<&ConfusingTouch>,
            Option<&Plated>,
            Option<&Forged>,
        ), With<Player>>();
        let mut v = Vec::new();
        if let Some((confused, blind, paralyzed, charmed, plated, forged)) = q.iter(world).next() {
            match tempo {
                Some(SpeedKind::Fast) => v.push(("FAST", Color::Cyan)),
                Some(SpeedKind::Slow) => v.push(("SLOW", Color::Green)),
                _ => {}
            }
            if stealthy {
                v.push(("STLH", Color::DarkGreen));
            }
            if confused.is_some() {
                v.push(("CONF", Color::Magenta));
            }
            if blind.is_some() {
                v.push(("BLND", Color::DarkGrey));
            }
            if paralyzed.is_some() {
                v.push(("PARL", Color::DarkMagenta));
            }
            if charmed.is_some() {
                v.push(("GLOW", Color::Magenta));
            }
            // The two promises. Not afflictions — they are the only badges that
            // are good news — but they are lost the same way a condition is,
            // and the player needs to know they are still holding one.
            if plated.is_some() {
                v.push(("PLAT", Color::White));
            }
            if forged.is_some() {
                v.push(("FORG", Color::DarkYellow));
            }
        }
        v
    };

    let depth = world.get_resource::<Depth>().map(|d| d.what).unwrap_or(1);
    // Carrying the Element of Yoord recolours the auto-walk badge: the descent is
    // over, every step now heads for the surface.
    let holding_element = pack_items
        .iter()
        .any(|&it| world.get::<Amulet>(it).is_some());
    let auto_label = world.get_resource::<AutoExplore>().and_then(|a| {
        a.active
            .then(|| match (holding_element, a.target.is_some()) {
                (true, _) => "ASCENDING",
                (_, true) => "TRAVELING",
                _ => "EXPLORING",
            })
    });

    // 1b. Blindness. A blinded player is down to touch: the 3x3 the visibility
    // system left them, and no colour in it — every glyph they can make out is
    // painted white, and the blood underfoot (colour and nothing else) is not
    // painted at all. Monsters are already `Hidden` by the visibility system, so
    // nothing below has to think about them.
    let blind = player_entity.is_some_and(|pe| world.get::<Blind>(pe).is_some());
    let by_touch = |color: Color| match blind {
        true => Color::White,
        false => color,
    };

    // 2. Targeting beam.
    let (is_targeting, targeting_tip) = {
        let targeting = world.resource::<TargetingState>();
        (
            targeting.active,
            (targeting.cursor_x as u16, targeting.cursor_y as u16),
        )
    };
    let target_line: HashSet<(u16, u16)> = if is_targeting {
        bresenham_line(player_pos.0, player_pos.1, targeting_tip.0, targeting_tip.1)
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

    // ---- Top HUD ----
    {
        let stat = |die: i32, flat: i32| {
            if flat != 0 {
                format!("{die}+{flat}")
            } else {
                format!("{die}")
            }
        };
        let mut fields = vec![
            player_name.to_uppercase(),
            format!("HP {}/{}", player_hp, player_max_hp),
            format!("Ma {}/{}", player_magic, player_max_magic),
            format!("Pow. {}", stat(pow_die, pow_flat)),
            format!("Arm. {}", stat(arm_die, arm_flat)),
        ];
        // What a throw is worth is only worth a HUD field once something is
        // making it worth something — a bow, a ring of dexterity. A player who
        // never throws never sees it.
        if throw_flat != 0 {
            fields.push(format!("Thr. {throw_flat:+}"));
        }
        fields.push(format!("DEPTH {}", depth));
        // The score line yields its space to condition badges when any are lit
        // — but never to a flash, which takes the scorekeeper's own place. A
        // payment is worth seeing whatever else is going on.
        let flash = world
            .get_resource::<ScoreFlash>()
            .filter(|f| f.lit())
            .map(|f| {
                (
                    f.text.clone(),
                    (0..f.text.chars().count())
                        .map(|i| f.color_at(i))
                        .collect::<Vec<_>>(),
                )
            });
        if conditions.is_empty() && flash.is_none() {
            fields.push(format!("SCORE {:06}", player_score));
        }
        let mut hx: u16 = 1;
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                screen.puts(hx, 0, " · ", Color::DarkGrey);
                hx += 3;
            }
            screen.puts(hx, 0, field, Color::Cyan);
            hx += field.chars().count() as u16;
        }
        // The scorekeeper shouting: `+700` in one bright colour, or DOUBLE a
        // letter at a time in the six of the flag.
        if let Some((text, colors)) = flash {
            screen.puts(hx, 0, " · ", Color::DarkGrey);
            hx += 3;
            for (ch, color) in text.chars().zip(colors) {
                screen.puts(hx, 0, &ch.to_string(), color);
                hx += 1;
            }
        }
        for (label, color) in &conditions {
            screen.puts(hx, 0, " · ", Color::DarkGrey);
            screen.puts(hx + 3, 0, label, *color);
            hx += 3 + label.len() as u16;
        }
        if let Some(kind) = world
            .query_filtered::<&Snare, With<Player>>()
            .iter(world)
            .next()
            .map(|s| s.kind)
        {
            let label = match kind {
                SnareKind::Bear | SnareKind::Hold => "HELD",
                SnareKind::Sleep => "ASLEEP",
            };
            screen.puts(hx, 0, " · ", Color::DarkGrey);
            screen.puts(hx + 3, 0, label, Color::Red);
            hx += 3 + label.len() as u16;
        }
        if let Some(label) = auto_label {
            screen.puts(hx, 0, " · ", Color::DarkGrey);
            let color = if holding_element {
                Color::Magenta
            } else {
                Color::Green
            };
            screen.puts(hx + 3, 0, label, color);
        }
        if world.resource::<TravelCursor>().active {
            screen.puts(hx, 0, " · ", Color::DarkGrey);
            screen.puts(hx + 3, 0, "TRAVEL?", Color::Yellow);
        }
    }

    // ---- Terrain ----
    let map = world.resource::<Map>().clone();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let coord = (x, y);
            let tile = map.tile(x, y);
            // Rogue only draws the walls that frame a room; corridor walls stay
            // dark so passages look like tunnels, not ditches.
            if tile == TileType::Wall && !map.is_room_wall(x, y) {
                continue;
            }
            let (glyph, lit) = tile_appearance(tile);
            let visible_here = visible.contains(&coord);
            if !visible_here && !revealed.contains(tile_index(x, y)) {
                continue; // unexplored: leave blank
            }
            let color = if visible_here {
                by_touch(lit)
            } else {
                Color::DarkGrey
            };
            screen.put_map(x, y, glyph, color);
        }
    }

    // ---- Blood overlay ----
    // Bloody tiles are reddened in place by recolouring their glyph, only where
    // the player can currently see, and never on a tile an actor stands on (the
    // red marks the floor, not whatever is on it). A blind player gets none of
    // it: blood is carried entirely by colour, and they have no colour.
    if !blind {
        let stains = world.resource::<BloodStains>();
        for &(x, y) in &visible {
            if !stains.is_bloody(x, y) || occupied_by_actor.contains(&(x, y)) {
                continue;
            }
            screen.fg_map(x, y, Color::DarkRed);
        }
    }

    // ---- Corpses (cosmetic `%`; same visibility rule as blood) ----
    {
        let corpses = world.resource::<Corpses>();
        for &(x, y) in &visible {
            if !corpses.has(x, y) || occupied_by_actor.contains(&(x, y)) {
                continue;
            }
            screen.put_map(x, y, '%', by_touch(Color::DarkGrey));
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
            screen.put_map(pos.x, pos.y, renderable.glyph, by_touch(renderable.color));
        }
    }

    // ---- Traps (only the ones the player has discovered) ----
    // A known trap is drawn like a discovered staircase: in colour while in
    // sight, in fog-grey once seen, and never under an actor standing on it.
    {
        let mut query =
            world.query_filtered::<(&Position, &Renderable), (With<Trap>, Without<Hidden>)>();
        for (pos, renderable) in query.iter(world) {
            let coord = (pos.x, pos.y);
            if occupied_by_actor.contains(&coord) {
                continue;
            }
            if visible.contains(&coord) {
                screen.put_map(pos.x, pos.y, renderable.glyph, by_touch(renderable.color));
                continue;
            }
            if revealed.contains(tile_index(pos.x, pos.y)) {
                screen.put_map(pos.x, pos.y, renderable.glyph, Color::DarkGrey);
            }
        }
    }

    // ---- Lingering smoke (DCSS-style; fades on its own over a few turns) ----
    // Drawn over the floor and anything lying on it, only where currently
    // visible, and never on a tile an actor stands on — like blood, it marks
    // the floor, not whatever is standing there.
    {
        let smoke = world.resource::<Smoke>();
        for &(x, y) in &visible {
            if !smoke.is_smoky(x, y) || occupied_by_actor.contains(&(x, y)) {
                continue;
            }
            screen.put_map(x, y, '≈', by_touch(Color::Grey));
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
            screen.put_map(pos.x, pos.y, renderable.glyph, by_touch(renderable.color));
        }
    }

    // ---- Monster status tints ----
    // A monster that can't fight back properly is worth seeing from across the
    // room, so its cell takes a background: dark blue for one that has lost its
    // turns outright (asleep in gas, or paralysed), dark green for one held in
    // a bear trap, dark cyan for one bound by a scroll of hold monster, dark
    // magenta for one staggering about confused. Painted after
    // the actors so it lands under a glyph that is actually drawn, and only in
    // that order of precedence — a monster that is both asleep and confused is
    // first of all asleep.
    {
        let mut query = world.query_filtered::<
            (&Position, &Mob, Option<&Snare>, Option<&Paralyzed>),
            Without<Hidden>,
        >();
        for (pos, mob, snare, paralyzed) in query.iter(world) {
            if !visible.contains(&(pos.x, pos.y)) {
                continue;
            }
            let snared = snare.map(|s| s.kind);
            let confused = matches!(mob.movement_type, MovementType::Confused);
            let tint = match (snared, paralyzed.is_some(), confused) {
                (Some(SnareKind::Sleep), _, _) | (_, true, _) => Some(Color::DarkBlue),
                (Some(SnareKind::Bear), _, _) => Some(Color::DarkGreen),
                (Some(SnareKind::Hold), _, _) => Some(Color::DarkCyan),
                (_, _, true) => Some(Color::DarkMagenta),
                _ => None,
            };
            if let Some(tint) = tint {
                screen.bg_map(pos.x, pos.y, tint);
            }
        }
    }

    // ---- Detected things (a potion of magic / monster detection) ----
    // Only where the player *can't* see: anything in view is already drawn
    // above, in its own colour and with the fog rules that apply to it. A
    // detection is a sense, not a window, so what it turns up is painted in one
    // flat magic-magenta — the glyph says what, the colour says "you are not
    // looking at this, you are feeling it".
    {
        let mut query = world.query_filtered::<(&Position, &Renderable), With<Detected>>();
        for (pos, renderable) in query.iter(world) {
            if visible.contains(&(pos.x, pos.y)) {
                continue;
            }
            screen.put_map(pos.x, pos.y, renderable.glyph, Color::DarkMagenta);
        }
    }

    // ---- Particle effects (transient; drawn over actors, only where seen) ----
    {
        let particles = world.resource::<Particles>();
        for p in &particles.live {
            if p.x >= MAP_WIDTH || p.y >= MAP_HEIGHT || !visible.contains(&(p.x, p.y)) {
                continue;
            }
            if let Some((glyph, color)) = p.current() {
                screen.put_map(p.x, p.y, glyph, color);
            }
        }
    }

    // ---- Targeting beam overlay ----
    if is_targeting {
        for &(tx, ty) in &target_line {
            if tx >= MAP_WIDTH || ty >= MAP_HEIGHT {
                continue;
            }
            if visible.contains(&(tx, ty)) && occupied_by_actor.contains(&(tx, ty)) {
                // Keep the actor's glyph, but recolour it — unless it was
                // already yellow (or close to it), in which case switch to
                // black instead, so a naturally-yellow monster doesn't just
                // disappear into the beam's own colour. `put` always resets
                // the background to the default, so a glyph recoloured to
                // black needs a background of its own here or it vanishes
                // outright — this is what used to blank the player out the
                // instant the reticle passed over their own tile.
                let (ch, fg, _) = screen.get_map(tx, ty);
                let recolor = if matches!(fg, Color::Yellow | Color::DarkYellow) {
                    Color::Black
                } else {
                    Color::Yellow
                };
                screen.put_map(tx, ty, ch, recolor);
                if recolor == Color::Black {
                    screen.bg_map(tx, ty, Color::DarkYellow);
                }
            } else {
                screen.put_map(tx, ty, '*', Color::Yellow);
            }
            // The reticle's own tip gets a background too, so it doesn't read
            // as just another yellow monster along the beam. Applied after the
            // recolour above so it wins over the black-recolour background.
            if (tx, ty) == targeting_tip {
                screen.bg_map(tx, ty, Color::DarkBlue);
            }
        }
    }

    // ---- Travel cursor (`O`): a blinking highlight on the chosen tile ----
    // The blink phase paints the tile's *background* yellow, leaving the glyph
    // and its colour untouched so what's on the tile stays readable.
    {
        let tc = world.resource::<TravelCursor>();
        if tc.active && tc.blink_on && tc.x < MAP_WIDTH && tc.y < MAP_HEIGHT {
            screen.bg_map(tc.x, tc.y, Color::Yellow);
        }
    }

    // ---- Message log (rows 22..=24) ----
    // Messages are packed onto shared lines and only wrap when the next one
    // would overflow; a message is never split across the wrap. Each keeps its
    // own colour on the line it shares — a shouting message must never repaint
    // the sentences beside it — and one of them is painted a letter at a time.
    {
        let stripes = models::pride::stripes(world);
        let log = world.resource::<GameLog>();
        let (lines, _consumed, more) = log_view(&log.unread);
        for (i, segments) in lines.iter().enumerate() {
            let y = 22 + i as u16;
            let last = i + 1 == lines.len();
            let mut x: u16 = 0;
            for message in segments {
                let paint = log_paint(message, stripes);
                for (n, ch) in message.chars().enumerate() {
                    screen.put(x, y, ch, paint.color_at(n));
                    x += 1;
                }
                // The joining space, in nobody's colour.
                x += 1;
            }
            if last && more {
                screen.puts(57, y, "--MORE-- (Press Space)", Color::Yellow);
            }
        }
    }

    // ---- Travel-cursor prompt (overrides the log rows while picking) ----
    if world.resource::<TravelCursor>().active {
        screen.puts(0, 24, "Move where?", Color::Yellow);
        screen.puts(
            12,
            24,
            "[hjkl/arrows move · Enter travel · Esc/x cancel]",
            Color::DarkGrey,
        );
    }

    // ---- Inventory overlay ----
    if world.resource::<PackIsOpen>().open {
        draw_inventory(world, screen);
    }

    // ---- "Really quit?" ----
    // Last of all, so it sits over whatever else is on screen.
    if world.resource::<QuitPrompt>().open {
        draw_quit_prompt(screen);
    }

    screen.flush(stdout, offset)
}

/// Ages the screen shake by one animation frame.
///
/// Every blocking frame loop in this file calls it, so a shake armed mid-turn
/// goes on decaying whichever animation happens to be on screen at the time —
/// which is the common case, not an edge one: the blast that arms the medium
/// shake queues a particle batch in the same breath, and the two are meant to
/// be seen together.
fn age_shake(world: &mut World, frame_ms: u64) {
    world.resource_mut::<Shake>().advance(frame_ms as f32);
}

/// Puts the map back on its moorings.
///
/// Called wherever a keypress has cut an animation short. The player is acting;
/// the frame they act on is never a skewed one, and a shake never outlives the
/// moment it was decorating.
fn settle_shake(world: &mut World) {
    world.resource_mut::<Shake>().settle();
}

/// Play out whatever hit / beam / blast particles the turn just queued.
///
/// The turn is already fully resolved — this only animates the aftermath — so it
/// is safe to freeze here for a couple hundred milliseconds the way NetHack and
/// DCSS freeze for a bolt. Each ~33 ms frame ages the effect layer and repaints
/// the map; the loop ends when the last mote dies or the player hits a key
/// (that key is swallowed, exactly like the auto-explore interrupt). A no-op
/// when nothing was queued.
pub fn play_particles<W: Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut Screen,
) -> std::io::Result<()> {
    if !world.resource::<Particles>().pending {
        return Ok(());
    }
    world.resource_mut::<Particles>().pending = false;

    const BASE_FRAME_MS: u64 = 33;
    let frame_ms = world.resource::<AnimRate>().scale(BASE_FRAME_MS);
    loop {
        age_shake(world, frame_ms);
        {
            let mut fx = world.resource_mut::<Particles>();
            fx.advance(frame_ms as f32);
            if !fx.any_alive() {
                break;
            }
        }
        render(world, stdout, screen)?;
        // The frame delay doubles as an "abort on keypress" poll.
        if poll(Duration::from_millis(frame_ms))? {
            let _ = read()?;
            // Skipping the sparks skips the shake with them: they are one
            // effect, and half of it left rocking after the other half was
            // dismissed reads as a bug.
            settle_shake(world);
            break;
        }
    }

    world.resource_mut::<Particles>().clear();
    Ok(())
}

/// Play a scroll of magic mapping's reveal out over a handful of frames,
/// re-rendering between each wave so the layout wipes in — as a falling curtain,
/// an outward spiral, or a bursting shell, depending on the rolled
/// [`MagicMapStyle`].
///
/// Mirrors [`play_particles`]: the turn is already resolved, so freezing input
/// here for a few hundred ms is fine, and any keypress skips straight to the
/// finished map (the key is swallowed). A no-op when no reveal is armed.
pub fn play_magic_map<W: Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut Screen,
) -> std::io::Result<()> {
    if !world.resource::<MagicMapReveal>().active {
        return Ok(());
    }

    let base_frame_ms = world.resource::<MagicMapReveal>().frame_ms();
    let frame = Duration::from_millis(world.resource::<AnimRate>().scale(base_frame_ms));
    loop {
        age_shake(world, frame.as_millis() as u64);
        if !magic_map_reveal_step(world) {
            break;
        }
        render(world, stdout, screen)?;
        if poll(frame)? {
            let _ = read()?;
            finish_magic_map_reveal(world);
            settle_shake(world);
            break;
        }
    }

    world.resource_mut::<MagicMapReveal>().stop();
    render(world, stdout, screen)?;
    Ok(())
}

/// Play out whatever screen shake the turn just armed: the recoil of an
/// excellent hit, the thump of a blast, the long lurch of being knocked into
/// the red.
///
/// **This one does not block, and that is the entire design.** [`play_particles`]
/// and [`play_magic_map`] freeze input the way NetHack freezes for a bolt,
/// which they can afford to because they animate an aftermath the player asked
/// for. A shake is armed by the dungeon, at the exact moments a player is most
/// likely to be typing ahead — mid-fight, half a second from death — and a
/// flourish that eats a keystroke there is a flourish that gets a flag turned
/// off. So this runs only in the gap where nothing is waiting to be read: the
/// moment `poll` reports a key it settles the map, repaints one steady frame,
/// and returns **without consuming the key**, leaving it for the input handler
/// that was about to block on it anyway.
///
/// The map is therefore never left skewed while the game blocks. Either the
/// shake runs out here, or a keypress settles it here; nothing else in the loop
/// has to know.
///
/// A no-op when nothing armed one, and under `-nshake` nothing ever does.
pub fn play_shake<W: Write>(
    world: &mut World,
    stdout: &mut W,
    screen: &mut Screen,
) -> std::io::Result<()> {
    if !world.resource::<Shake>().active() {
        return Ok(());
    }

    // Paced off the same base frame as the particle layer, and scaled by the
    // same `-anim-rate`, so a terminal retuned for one is retuned for both.
    const BASE_FRAME_MS: u64 = 33;
    let frame_ms = world.resource::<AnimRate>().scale(BASE_FRAME_MS);
    loop {
        age_shake(world, frame_ms);
        if !world.resource::<Shake>().active() {
            break;
        }
        render(world, stdout, screen)?;
        if poll(Duration::from_millis(frame_ms))? {
            settle_shake(world);
            break;
        }
    }

    // The settling frame: whatever happens above, the map goes home before the
    // player's next keystroke is read.
    render(world, stdout, screen)
}

/// Rough vertical centring helper for the full-screen end panels.
fn centered_x(text: &str) -> u16 {
    (SCREEN_W.saturating_sub(text.chars().count() as u16)) / 2
}

/// The transient "You die..." panel. It is deliberately sparse: a single line
/// and a `--MORE--` prompt the player must acknowledge before the tombstone.
pub fn render_you_died<W: Write>(
    stdout: &mut W,
    screen: &mut Screen,
    offset: (u16, u16),
) -> std::io::Result<()> {
    screen.clear();
    let y = SCREEN_H / 2;
    let msg = "You die...";
    screen.puts(centered_x(msg), y, msg, Color::Red);
    let more = "--MORE-- (Press Space)";
    screen.puts(centered_x(more), y + 2, more, Color::Yellow);
    screen.dirty_all = true;
    screen.flush(stdout, offset)
}

/// The tombstone. Shown once the player has acknowledged the death prompt.
pub fn render_tombstone<W: Write>(
    stdout: &mut W,
    screen: &mut Screen,
    offset: (u16, u16),
    player_name: &str,
    cause: &str,
    score: i32,
) -> std::io::Result<()> {
    screen.clear();

    const GRAVESTONE: [&str; 8] = [
        "       .-'\"\"\"\"\"'-.       ",
        "     .'           '.     ",
        "    /     R.I.P.    \\    ",
        "   |  _            _  |   ",
        "   | (_)          (_) |   ",
        "   |    HERE LIES     |   ",
        "   |       YOU        |   ",
        "   |__________________|   ",
    ];

    let top = 3u16;
    for (i, line) in GRAVESTONE.iter().enumerate() {
        screen.puts(centered_x(line), top + i as u16, line, Color::White);
    }

    let mut y = top + GRAVESTONE.len() as u16 + 2;
    let epitaph = "DEATH AND THE DUNGEON HAVE TAKEN THEE";
    screen.puts(centered_x(epitaph), y, epitaph, Color::Red);
    y += 2;

    let name_line = player_name.to_uppercase();
    screen.puts(centered_x(&name_line), y, &name_line, Color::Cyan);
    y += 1;
    screen.puts(centered_x(cause), y, cause, Color::Grey);
    y += 2;

    let score_line = format!("SCORE {:06}", score);
    screen.puts(centered_x(&score_line), y, &score_line, Color::Yellow);
    y += 3;

    let prompt = "Press any key to depart.";
    screen.puts(centered_x(prompt), y, prompt, Color::DarkGrey);

    screen.dirty_all = true;
    screen.flush(stdout, offset)
}

/// The victory starfield. Shown when the player carries the Element of Yoord up
/// the final stair. The counterpart to [`render_tombstone`].
pub fn render_victory<W: Write>(
    stdout: &mut W,
    screen: &mut Screen,
    offset: (u16, u16),
    player_name: &str,
    score: i32,
) -> std::io::Result<()> {
    screen.clear();

    const STAR: [&str; 9] = [
        "           *           ",
        "     .     |     .     ",
        "      '.   |   .'      ",
        "        '. | .'        ",
        "*  --  --  *  --  --  *",
        "        .' | '.        ",
        "      .'   |   '.      ",
        "     '     |     '     ",
        "           *           ",
    ];

    let top = 2u16;
    for (i, line) in STAR.iter().enumerate() {
        screen.puts(centered_x(line), top + i as u16, line, Color::Yellow);
    }

    let mut y = top + STAR.len() as u16 + 2;
    let banner = "YOU WIN";
    screen.puts(centered_x(banner), y, banner, Color::Green);
    y += 2;

    // The blessing, wrapped so a long name can't run off the panel.
    let name = player_name.to_uppercase();
    let blessing = format!(
        "WITH THE ELEMENT, {name} AND EVERYONE WHO BASKED IN ITS LIGHT LIVED HAPPILY EVER AFTER"
    );
    for line in wrap_words(&blessing, 68) {
        screen.puts(centered_x(&line), y, &line, Color::Cyan);
        y += 1;
    }
    y += 2;

    let score_line = format!("SCORE {:06}", score);
    screen.puts(centered_x(&score_line), y, &score_line, Color::Yellow);
    y += 3;

    let prompt = "Press any key to depart.";
    screen.puts(centered_x(prompt), y, prompt, Color::DarkGrey);

    screen.dirty_all = true;
    screen.flush(stdout, offset)
}

/// Greedily breaks `text` into lines no wider than `width` on word boundaries. A
/// single word longer than `width` gets its own overflowing line.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
            continue;
        }
        if cur.chars().count() + 1 + word.chars().count() <= width {
            cur.push(' ');
            cur.push_str(word);
            continue;
        }
        lines.push(std::mem::take(&mut cur));
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// One inventory row's full text: `" a) +1 ring mail (E) "`. The one place
/// this shape lives, so sizing the box and drawing a row can never disagree.
fn row_text(letter: char, name: &str, equipped: bool) -> String {
    let suffix = if equipped { " (E)" } else { "" };
    format!(" {letter}) {name}{suffix} ")
}

/// The "Really quit?" confirmation: a small box in the middle of the map, drawn
/// over everything else.
///
/// Deliberately plain and deliberately in the way. It is the only modal in the
/// game that exists to *slow the player down* — every other one is a menu — so
/// it sits in the centre rather than off in a corner where a key could be
/// answered by reflex, and it spells out both answers instead of leaning on
/// "any key".
fn draw_quit_prompt(screen: &mut Screen) {
    const QUESTION: &str = "Really quit?";
    const ANSWERS: &str = "[y] yes    [n] no";

    let inner = ANSWERS.chars().count() as u16 + 4;
    let x = (SCREEN_W - inner) / 2 - 1;
    let y = MAP_TOP + MAP_HEIGHT / 2 - 2;
    let grey = Color::DarkGrey;

    screen.put(x, y, '┌', grey);
    screen.hline(x + 1, y, '─', inner, grey);
    screen.put(x + 1 + inner, y, '┐', grey);
    for (row, (text, color)) in [(QUESTION, Color::Yellow), (ANSWERS, Color::White)]
        .into_iter()
        .enumerate()
    {
        let ty = y + 1 + row as u16;
        screen.put(x, ty, '│', grey);
        screen.hline(x + 1, ty, ' ', inner, grey);
        let tx = x + 1 + (inner - text.chars().count() as u16) / 2;
        screen.puts(tx, ty, text, color);
        screen.put(x + 1 + inner, ty, '│', grey);
    }
    screen.put(x, y + 3, '└', grey);
    screen.hline(x + 1, y + 3, '─', inner, grey);
    screen.put(x + 1 + inner, y + 3, '┘', grey);
}

fn draw_inventory(world: &mut World, screen: &mut Screen) {
    let (mode, selected_idx, action_mode, action_selected) = {
        let p = world.resource::<PackIsOpen>();
        (p.mode, p.selected, p.action_mode, p.action_selected)
    };

    // (backpack row, display name, is-equipped, is-known-cursed) for the rows
    // this mode shows — `pack_rows` is the only thing that decides which those
    // are, so the box can never draw a row the cursor can't reach or vice versa.
    let item_names: Vec<(usize, String, bool, bool)> = {
        let entities: Vec<Entity> = {
            let mut query = world.query_filtered::<&Backpack, With<Player>>();
            match query.iter(world).next() {
                Some(backpack) => backpack.items.clone(),
                None => Vec::new(),
            }
        };
        models::pack_rows(world, mode)
            .into_iter()
            .filter_map(|i| entities.get(i).map(|&e| (i, e)))
            .map(|(i, e)| {
                let name = models::display_name(world, e);
                let equipped = world.get::<Equipped>(e).is_some_and(|eq| eq.by.is_some());
                let cursed_known =
                    models::known_quality(world, e) && world.get::<Curse>(e).is_some();
                (i, name, equipped, cursed_known)
            })
            .collect()
    };

    let start_x: u16 = 5;
    let start_y: u16 = 3;
    let grey = Color::DarkGrey;
    let title = mode.title();
    // Wide enough for the title and every row's full text (letter, name,
    // "(E)" suffix), so a long identified name is never clipped.
    let box_width: u16 = item_names
        .iter()
        .map(|(i, name, equipped, _)| {
            let letter = (b'a' + *i as u8) as char;
            row_text(letter, name, *equipped).chars().count() as u16
        })
        .chain([title.len() as u16])
        .max()
        .unwrap_or(0)
        .max(30);

    // Top border + title.
    screen.put(start_x, start_y, '┌', grey);
    screen.hline(start_x + 1, start_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, start_y, '┐', grey);
    screen.puts(
        start_x + box_width / 2 - title.len() as u16 / 2,
        start_y,
        title,
        Color::Yellow,
    );

    // Rows. The letter is the item's place in the *pack*, not its place in this
    // list: a filtered menu shows `c) a potion of healing` as `c` even when it
    // is the only row on screen.
    for (row, (i, name, equipped, cursed_known)) in item_names.iter().enumerate() {
        let y = start_y + 1 + row as u16;
        let letter = (b'a' + *i as u8) as char;
        let selected = *i == selected_idx;
        let color = match (selected, *cursed_known, *equipped) {
            (true, true, _) => Color::Red,
            (false, true, _) => Color::DarkRed,
            (true, false, _) => Color::Yellow,
            (false, false, true) => Color::Cyan,
            (false, false, false) => Color::White,
        };
        let text = row_text(letter, name, *equipped);
        screen.put(start_x, y, '│', grey);
        screen.puts(
            start_x + 1,
            y,
            &format!("{:<w$}", text, w = box_width as usize),
            color,
        );
        screen.put(start_x + 1 + box_width, y, '│', grey);
    }

    // Bottom border.
    let bottom_y = start_y + 1 + item_names.len() as u16;
    screen.put(start_x, bottom_y, '└', grey);
    screen.hline(start_x + 1, bottom_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, bottom_y, '┘', grey);

    // Use / Throw / Drop action modal. Only `PackMode::Browse` ever opens it,
    // and that mode shows every row, so the item's pack index is also its row
    // on screen — `position` rather than a bare cast anyway, so a filtered mode
    // that ever grows a modal floats it next to the right line.
    if let Some(action_idx) = action_mode {
        let actions = ItemAction::MENU;
        let row = item_names
            .iter()
            .position(|(i, ..)| *i == action_idx)
            .unwrap_or(action_idx);
        let mx = start_x + box_width + 2;
        let my = start_y + 1 + row as u16;
        screen.puts(mx, my, "┌────────┐", grey);
        for (row, action) in actions.iter().enumerate() {
            let y = my + 1 + row as u16;
            screen.put(mx, y, '│', grey);
            screen.puts(
                mx + 1,
                y,
                action.label(),
                if action_selected == row {
                    Color::Yellow
                } else {
                    Color::White
                },
            );
            screen.put(mx + 9, y, '│', grey);
        }
        screen.puts(mx, my + 1 + actions.len() as u16, "└────────┘", grey);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The map rows, as everything above assumes them: `MAP_TOP` through the
    /// last row the map is allowed to touch.
    const LAST_MAP_ROW: u16 = MAP_TOP + MAP_HEIGHT - 1;

    fn shaken(shift: (i16, i16)) -> Screen {
        let mut screen = Screen::new();
        screen.map_shift = shift;
        screen
    }

    #[test]
    fn at_rest_a_map_tile_lands_one_row_down_for_the_status_line() {
        let screen = shaken((0, 0));
        assert_eq!(screen.map_cell(0, 0), Some((0, MAP_TOP)));
        assert_eq!(
            screen.map_cell(MAP_WIDTH - 1, MAP_HEIGHT - 1),
            Some((MAP_WIDTH - 1, LAST_MAP_ROW))
        );
    }

    #[test]
    fn a_shake_never_smears_the_map_into_the_status_line_or_the_log() {
        // The whole safety property. Every displacement the shake can produce,
        // over every map tile: whatever comes back is inside the map rows.
        let shifts = (-2i16..=2).flat_map(|dy| (-2i16..=2).map(move |dx| (dx, dy)));
        let tiles: Vec<(u16, u16)> = (0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .collect();
        for (dx, dy) in shifts {
            let screen = shaken((dx, dy));
            for &(x, y) in &tiles {
                let Some((sx, sy)) = screen.map_cell(x, y) else {
                    continue;
                };
                assert!(
                    (MAP_TOP..=LAST_MAP_ROW).contains(&sy),
                    "shift ({dx},{dy}) put map tile ({x},{y}) on row {sy}"
                );
                assert!(sx < SCREEN_W, "shift ({dx},{dy}) ran off the right edge");
            }
        }
    }

    #[test]
    fn a_tile_thrown_past_the_edge_is_dropped_rather_than_wrapped() {
        // What the user asked for in as many words: the shake does not change
        // the resolution, and what leaves the viewport is not drawn. A wrap
        // would show column 79's tile at column 0, one row down — the classic
        // way a shifted terminal grid goes wrong.
        let left = shaken((-1, 0));
        assert_eq!(left.map_cell(0, 5), None, "left column falls off");
        assert_eq!(left.map_cell(1, 5), Some((0, 5 + MAP_TOP)));

        let right = shaken((1, 0));
        assert_eq!(
            right.map_cell(MAP_WIDTH - 1, 5),
            None,
            "right column falls off"
        );

        let up = shaken((-1, -1));
        assert_eq!(up.map_cell(4, 0), None, "top row falls off");

        let down = shaken((0, 1));
        assert_eq!(
            down.map_cell(4, MAP_HEIGHT - 1),
            None,
            "bottom row falls off"
        );
    }

    #[test]
    fn a_dropped_tile_paints_nothing_at_all() {
        // `put_map` must no-op on a clipped tile, not clamp it to the edge —
        // clamping would pile the whole off-screen column onto column 0.
        let mut screen = shaken((-1, 0));
        screen.put_map(0, 3, '#', Color::Red);
        assert_eq!(
            screen.get(0, 3 + MAP_TOP),
            BLANK_CELL,
            "a clipped tile was clamped onto the edge instead of dropped"
        );
    }

    /// A world with everything `render` reaches for, and nothing it doesn't.
    /// Mirrors `main`'s startup, which is the only other place this list lives.
    fn rendered_world() -> World {
        let mut world = World::new();
        world.insert_resource(GameRng(ChaCha12Rng::seed_from_u64(9)));
        world.insert_resource(RngSeed(9));
        world.init_resource::<PackIsOpen>();
        world.insert_resource(RenderConfig { centered: false });
        world.insert_resource(TargetingState {
            active: false,
            item: None,
            throwing: false,
            cursor_x: 0,
            cursor_y: 0,
        });
        world.init_resource::<QuitPrompt>();
        world.insert_resource(PlayerName {
            what: "TESTER".to_string(),
        });
        world.insert_resource(Depth { what: 1 });
        world.init_resource::<DungeonLord>();
        world.init_resource::<Ending>();
        world.init_resource::<AutoExplore>();
        world.init_resource::<FastMove>();
        world.init_resource::<TravelCursor>();
        world.init_resource::<MagicMapReveal>();
        world.init_resource::<AttackQueue>();
        world.init_resource::<UseQueue>();
        world.init_resource::<ThrowQueue>();
        world.init_resource::<PlayerTempo>();
        world.init_resource::<GameLog>();
        world.insert_resource(Particles::new());
        world.insert_resource(Shake::new());
        world.insert_resource(AnimRate(1.0));
        initialize_world(&mut world);
        // A real viewshed, so the frame actually has tiles and a player in it.
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(visibility_system);
        schedule.run(&mut world);
        world
    }

    /// One row of the frame `flush` just emitted. The buffers swap at the end
    /// of a flush, so the frame on screen is the one in `prev`.
    fn row(screen: &Screen, y: u16) -> String {
        (0..SCREEN_W)
            .map(|x| screen.prev[(y * SCREEN_W + x) as usize].0)
            .collect()
    }

    fn map_rows(screen: &Screen) -> Vec<String> {
        (MAP_TOP..=LAST_MAP_ROW).map(|y| row(screen, y)).collect()
    }

    #[test]
    fn a_shaking_frame_moves_the_map_and_nothing_else() {
        // The end-to-end check: not that the arithmetic is right (particle-core
        // covers that) but that `render` actually reads it, that it reaches the
        // map layers, and that it reaches nothing else.
        let mut world = rendered_world();
        let mut screen = Screen::new();
        let mut out: Vec<u8> = Vec::new();

        render(&mut world, &mut out, &mut screen).unwrap();
        let (still_hud, still_map) = (row(&screen, 0), map_rows(&screen));
        assert!(
            still_map.iter().any(|r| r.contains('@')),
            "the resting frame drew no player, so this test proves nothing"
        );

        world.resource_mut::<Shake>().kick(ShakeKind::Heavy);
        render(&mut world, &mut out, &mut screen).unwrap();

        assert_ne!(map_rows(&screen), still_map, "the map did not move");
        assert_eq!(row(&screen, 0), still_hud, "the status line moved with it");

        // ...and it goes home again on its own.
        world
            .resource_mut::<Shake>()
            .advance(ShakeKind::Heavy.duration_ms());
        render(&mut world, &mut out, &mut screen).unwrap();
        assert_eq!(map_rows(&screen), still_map, "the map did not settle back");
        assert_eq!(row(&screen, 0), still_hud);
    }

    #[test]
    fn a_shaking_frame_is_the_same_size_as_a_still_one() {
        // The constraint the shake was asked to respect: it does not change the
        // resolution. Every row is 80 cells and there are 25 of them, shaken or
        // not, and the log rows keep whatever the log put there.
        let mut world = rendered_world();
        let mut screen = Screen::new();
        let mut out: Vec<u8> = Vec::new();

        render(&mut world, &mut out, &mut screen).unwrap();
        let still_log: Vec<String> = (22..SCREEN_H).map(|y| row(&screen, y)).collect();

        world.resource_mut::<Shake>().kick(ShakeKind::Wounded);
        for _ in 0..12 {
            render(&mut world, &mut out, &mut screen).unwrap();
            assert_eq!(screen.prev.len(), (SCREEN_W * SCREEN_H) as usize);
            for y in 0..SCREEN_H {
                assert_eq!(row(&screen, y).chars().count(), SCREEN_W as usize);
            }
            let log: Vec<String> = (22..SCREEN_H).map(|y| row(&screen, y)).collect();
            assert_eq!(log, still_log, "the message log moved with the map");
            world.resource_mut::<Shake>().advance(33.0);
        }
    }

    #[test]
    fn the_whole_map_still_fits_when_it_is_not_shaking() {
        // No displacement means no tile is ever dropped: the resting frame is
        // byte-for-byte the frame the game drew before the shake existed.
        let screen = shaken((0, 0));
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                assert!(screen.map_cell(x, y).is_some(), "({x},{y}) went missing");
            }
        }
    }
}
