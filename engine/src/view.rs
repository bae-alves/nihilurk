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
    // player's tempo, CONF from the dazzle condition.
    let conditions: Vec<(&str, Color)> = {
        let mut q = world.query_filtered::<(Option<&Speed>, Option<&Confused>), With<Player>>();
        let mut v = Vec::new();
        if let Some((speed, confused)) = q.iter(world).next() {
            match speed.map(|s| s.kind) {
                Some(SpeedKind::Fast) => v.push(("FAST", Color::Cyan)),
                Some(SpeedKind::Slow) => v.push(("SLOW", Color::Green)),
                _ => {}
            }
            if confused.is_some() {
                v.push(("CONF", Color::Magenta));
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
        // The score line yields its space to condition badges when any are lit.
        if conditions.is_empty() {
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
                SnareKind::Bear => "HELD",
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
            let color = if visible_here { lit } else { Color::DarkGrey };
            screen.put(x, y + 1, glyph, color);
        }
    }

    // ---- Blood overlay ----
    // Bloody tiles are reddened in place by recolouring their glyph, only where
    // the player can currently see, and never on a tile an actor stands on (the
    // red marks the floor, not whatever is on it).
    {
        let stains = world.resource::<BloodStains>();
        for &(x, y) in &visible {
            if !stains.is_bloody(x, y) || occupied_by_actor.contains(&(x, y)) {
                continue;
            }
            screen.set_fg(x, y + 1, Color::DarkRed);
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
                screen.put(pos.x, pos.y + 1, renderable.glyph, renderable.color);
                continue;
            }
            if revealed.contains(tile_index(pos.x, pos.y)) {
                screen.put(pos.x, pos.y + 1, renderable.glyph, Color::DarkGrey);
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
            screen.put(x, y + 1, '≈', Color::Grey);
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

    // ---- Particle effects (transient; drawn over actors, only where seen) ----
    {
        let particles = world.resource::<Particles>();
        for p in &particles.live {
            if p.x >= MAP_WIDTH || p.y >= MAP_HEIGHT || !visible.contains(&(p.x, p.y)) {
                continue;
            }
            if let Some((glyph, color)) = p.current() {
                screen.put(p.x, p.y + 1, glyph, color);
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
                // disappear into the beam's own colour.
                let (ch, fg, _) = screen.get(tx, ty + 1);
                let recolor = if matches!(fg, Color::Yellow | Color::DarkYellow) {
                    Color::Black
                } else {
                    Color::Yellow
                };
                screen.put(tx, ty + 1, ch, recolor);
            } else {
                screen.put(tx, ty + 1, '*', Color::Yellow);
            }
            // The reticle's own tip gets a background too, so it doesn't read
            // as just another yellow monster along the beam.
            if (tx, ty) == targeting_tip {
                screen.set_bg(tx, ty + 1, Color::DarkBlue);
            }
        }
    }

    // ---- Travel cursor (`O`): a blinking highlight on the chosen tile ----
    // The blink phase paints the tile's *background* yellow, leaving the glyph
    // and its colour untouched so what's on the tile stays readable.
    {
        let tc = world.resource::<TravelCursor>();
        if tc.active && tc.blink_on && tc.x < MAP_WIDTH && tc.y < MAP_HEIGHT {
            screen.set_bg(tc.x, tc.y + 1, Color::Yellow);
        }
    }

    // ---- Message log (rows 22..=24) ----
    // Messages are packed onto shared lines and only wrap when the next one
    // would overflow; a message is never split across the wrap.
    {
        let log = world.resource::<GameLog>();
        let (lines, _consumed, more) = log_view(&log.unread);
        for (i, line) in lines.iter().enumerate() {
            let y = 22 + i as u16;
            let last = i + 1 == lines.len();
            screen.puts(0, y, line, log_line_color(line));
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
            "[hjkl/arrows move · Enter travel · Esc cancel]",
            Color::DarkGrey,
        );
    }

    // ---- Inventory overlay ----
    if world.resource::<PackIsOpen>().open {
        draw_inventory(world, screen);
    }

    screen.flush(stdout, offset)
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
        if !magic_map_reveal_step(world) {
            break;
        }
        render(world, stdout, screen)?;
        if poll(frame)? {
            let _ = read()?;
            finish_magic_map_reveal(world);
            break;
        }
    }

    world.resource_mut::<MagicMapReveal>().stop();
    render(world, stdout, screen)?;
    Ok(())
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

fn draw_inventory(world: &mut World, screen: &mut Screen) {
    let (selected_idx, action_mode, action_selected) = {
        let p = world.resource::<PackIsOpen>();
        (p.selected, p.action_mode, p.action_selected)
    };

    // (display name, is-equipped, is-known-cursed) for every backpack slot.
    let item_names: Vec<(String, bool, bool)> = {
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
                let name = models::display_name(world, e);
                let equipped = world.get::<Equipped>(e).is_some_and(|eq| eq.by.is_some());
                let cursed_known =
                    models::known_quality(world, e) && world.get::<Curse>(e).is_some();
                (name, equipped, cursed_known)
            })
            .collect()
    };

    let start_x: u16 = 5;
    let start_y: u16 = 3;
    let grey = Color::DarkGrey;
    let title = " INVENTORY ";
    // Wide enough for the title and every row's full text (letter, name,
    // "(E)" suffix), so a long identified name is never clipped.
    let box_width: u16 = item_names
        .iter()
        .enumerate()
        .map(|(i, (name, equipped, _))| {
            let letter = (b'a' + i as u8) as char;
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

    // Rows.
    for (i, (name, equipped, cursed_known)) in item_names.iter().enumerate() {
        let y = start_y + 1 + i as u16;
        let letter = (b'a' + i as u8) as char;
        let selected = i == selected_idx;
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

    // Use / Throw / Drop action modal, in whatever order the ActionMenu says
    // (`-dropthrow` swaps the last two).
    if let Some(action_idx) = action_mode {
        let actions = world.resource::<ActionMenu>().actions();
        let mx = start_x + box_width + 2;
        let my = start_y + 1 + action_idx as u16;
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
