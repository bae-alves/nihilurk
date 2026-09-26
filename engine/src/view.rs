//! Everything that goes *in* the terminal grid: the map layers, the HUD, the
//! overlays and the playback loops that pace an animation across frames.
//!
//! The grid itself — `Screen`, its diffing and its flush — lives in its own
//! `view` crate, so `perf/` measures the renderer the game actually has
//! rather than a copy of it. [`render`] is the whole frame, drawn layer by
//! layer (terrain, blood, items, actors, overlays); [`play_particles`],
//! [`play_magic_map`] and [`play_shake`] are the three ways a turn's
//! aftermath gets spread across more than one of them.

use std::collections::HashSet;
use std::io::Write;
use std::time::Duration;

use bevy_ecs::prelude::*;
use crossterm::{
    event::{Event, KeyEventKind, poll, read},
    style::Color,
    terminal::size,
};

use models::*;

pub use view::{MAP_TOP, SCREEN_H, SCREEN_W, Screen};

/// A targeting beam's trajectory from `(x0, y0)` to `(x1, y1)`, in map
/// coordinates.
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

/// What the message log shows this frame: the unread messages packed onto its
/// three lines, and whether the `--MORE--` prompt goes up under them. One
/// `log_view` for the pair, since the prompt is a fact about the same packing
/// that produced the lines.
///
/// The prompt needs more than a backlog — it also waits for the effect layer.
/// [`play_particles`] reads *any* keypress as "skip the animation", so a
/// prompt up while motes are still on screen asks for the one key that throws
/// the rest of the batch away — and a batch is ordered: a trick shot's blast
/// is queued *behind* the missile's flight (`Particles::hold_ms`), so that
/// keypress eats the explosion and leaves the flight looking fine. A trick
/// shot raises the prompt every time — it shouts, kills, drops the dead one's
/// gear and then says "Very clever." — which is why it was the shot nobody
/// could get to explode.
///
/// Only the prompt waits. The lines that fit are painted throughout, and the
/// backlog is out of reach for no longer than the animation the player is
/// already watching.
fn log_panel(world: &World) -> (Vec<Vec<LogEntry>>, bool) {
    let animating = world.resource::<Particles>().any_alive();
    let (lines, _consumed, more) = log_view(&world.resource::<GameLog>().unread);
    (lines, more && !animating)
}

/// The monster-status tint's precedence, as a pure function of the five
/// conditions it cares about: asleep, paralysed, held-down (a bear trap or
/// hold monster), staggering (confused or fleeing), slowed. The first that's
/// true wins.
fn status_tint(
    asleep: bool,
    paralyzed: bool,
    held_down: bool,
    staggering: bool,
    slowed: bool,
) -> Option<Color> {
    match (asleep, paralyzed, held_down, staggering, slowed) {
        (true, _, _, _, _) => Some(Color::DarkBlue),
        (_, true, _, _, _) => Some(Color::DarkYellow),
        (_, _, true, _, _) => Some(Color::DarkGreen),
        (_, _, _, true, _) => Some(Color::Yellow),
        (_, _, _, _, true) => Some(Color::Grey),
        _ => None,
    }
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

    // Everything the HUD and the map layers below read off the player, in one
    // query. The `None` arm is what lets a headless or mid-teardown world still
    // paint a frame instead of panicking.
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

    // Fold every equipped modifier into the displayed Pow. / Arm. / Thr.
    // figures — the same fold combat runs, so the HUD can never drift from the
    // real numbers. One pass over the player's gear, not one per field: this
    // runs on every frame of every animation, and there are five fields.
    let worn = player_entity
        .map(|pe| models::loadout(world, pe))
        .unwrap_or_default();
    pow_die += worn.power_die;
    pow_flat += worn.power_bonus;
    arm_die += worn.armor_die;
    arm_flat += worn.armor_bonus;
    let throw_flat = worn.throw_bonus;

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
            Option<&MagicWard>,
            Option<&Bided>,
        ), With<Player>>();
        let mut v = Vec::new();
        if let Some((confused, blind, paralyzed, charmed, plated, forged, warded, bided)) =
            q.iter(world).next()
        {
            match tempo {
                Some(SpeedKind::Fast) => v.push(("FAST", Color::Cyan)),
                // The lurk's own tempo, so it is on the HUD from turn one
                // rather than only after a potion — a permanent tag, in the
                // lurk's own magenta rather than haste's cyan.
                Some(SpeedKind::Quick) => v.push(("QUIK", Color::Magenta)),
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
            // The two spells that leave something on you: a shield up for the
            // rest of the floor, and a blow coiled and waiting to land.
            if warded.is_some() {
                v.push(("WARD", Color::Cyan));
            }
            if bided.is_some() {
                v.push(("BIDE", Color::DarkYellow));
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
                (true, _) => strings::badge_ascending(),
                (_, true) => strings::badge_traveling(),
                _ => strings::badge_exploring(),
            })
    });

    // Blindness. A blinded player is down to touch: the 3x3 the visibility
    // system left them, and no colour in it — every glyph they can make out is
    // painted white, and the blood underfoot (colour and nothing else) is not
    // painted at all. Monsters are already `Hidden` by the visibility system, so
    // nothing below has to think about them.
    let blind = player_entity.is_some_and(|pe| world.get::<Blind>(pe).is_some());
    let by_touch = |color: Color| match blind {
        true => Color::White,
        false => color,
    };

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

    // Tiles an actor is standing on, so no later layer draws the floor under
    // one: blood, corpses, items, traps and smoke all mark the ground, not
    // whatever is on it.
    let occupied_by_actor: HashSet<(u16, u16)> = {
        let mut query = world.query_filtered::<&Position, Or<(With<Player>, With<Mob>)>>();
        query.iter(world).map(|pos| (pos.x, pos.y)).collect()
    };

    let player_name = world.resource::<PlayerName>().what.clone();
    // The same cutoff auto-fight refuses under, so the HP field can never
    // disagree with whether `Tab` will actually swing.
    let too_injured = player_too_injured(world);

    // ---- Top HUD (row 0) ----
    // The dungeon's half of the status: what is on the player, where they
    // are, what it has been worth. Badges run out from the left, the depth
    // sits in the middle and the scorekeeper is pinned to the right edge, so
    // no two of them can ever run each other off the line.
    {
        let mut badges = conditions.clone();
        let held = world
            .query_filtered::<(
                Option<&Petrified>,
                Option<&Asleep>,
                Option<&Pinned>,
                Option<&Rooted>,
            ), With<Player>>()
            .iter(world)
            .next()
            .map(|(s, a, p, r)| (s.is_some(), a.is_some(), p.is_some() || r.is_some()));
        // Worst first: stone costs the turn and everything else besides, sleep
        // costs the turn, a bear trap costs the step. A player who is two of
        // them is told the worst one.
        if let Some(label) = match held {
            Some((true, _, _)) => Some(strings::badge_stone()),
            Some((_, true, _)) => Some(strings::badge_asleep()),
            Some((_, _, true)) => Some(strings::badge_held()),
            _ => None,
        } {
            badges.push((label, Color::Red));
        }
        if let Some(label) = auto_label {
            badges.push((
                label,
                match holding_element {
                    true => Color::Magenta,
                    false => Color::Green,
                },
            ));
        }
        if world.resource::<TravelCursor>().active {
            badges.push((strings::badge_travel_query(), Color::Yellow));
        }
        let mut hx: u16 = 1;
        for (i, (label, color)) in badges.iter().enumerate() {
            if i > 0 {
                screen.puts(hx, 0, " · ", Color::DarkGrey);
                hx += 3;
            }
            screen.puts(hx, 0, label, *color);
            hx += label.chars().count() as u16;
        }

        let label = strings::depth_label();
        let depth_text = format!("{label} {depth}");
        let dx = centered_x(&depth_text);
        screen.puts(dx, 0, label, Color::Magenta);
        let number_x = dx + label.chars().count() as u16 + 1;
        screen.puts(number_x, 0, &depth.to_string(), Color::White);

        // The scorekeeper, right-aligned. A payment no longer takes its place:
        // the flash shouts in the gutter under it (row 1, below), so the
        // running total is readable through the frame that pays it.
        let score_line = strings::score_line(&score_text(player_score));
        let sx = SCREEN_W.saturating_sub(1 + score_line.chars().count() as u16);
        screen.puts(sx, 0, &score_line, Color::White);
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
    // room, so its cell takes a background: dark blue for one asleep, dark
    // yellow for one paralysed, dark green for one held down (a bear trap or a
    // scroll of hold monster), yellow for one staggering about confused or
    // bolting in a panic, grey for one merely slowed. The glyph itself
    // repaints black over the tint so it stays readable against every one of
    // those backgrounds. Painted after the actors so it lands under a glyph
    // that is actually drawn, and only in that order of precedence — a monster
    // that is both asleep and confused is first of all asleep.
    //
    // Every one of these is also the garrote's whole "tell" — the set of
    // conditions `crate::combat::garrote_vorpal` calls helpless, so a tinted
    // monster is always one it can end in a single stroke. Fleeing is in that
    // set too even though it isn't truly helpless: it's a free garrote by
    // design, the reward for having scared something off.
    {
        let mut query = world.query_filtered::<(
            &Position,
            &Mob,
            Option<&Asleep>,
            Option<&Pinned>,
            Option<&Rooted>,
            Option<&Paralyzed>,
            Option<&Speed>,
        ), Without<Hidden>>();
        for (pos, mob, asleep, pinned, rooted, paralyzed, speed) in query.iter(world) {
            if !visible.contains(&(pos.x, pos.y)) {
                continue;
            }
            let confused = matches!(mob.movement_type, MovementType::Confused);
            let fleeing = matches!(mob.movement_type, MovementType::Flee);
            let slowed = speed.is_some_and(|s| s.kind == SpeedKind::Slow);
            let tint = status_tint(
                asleep.is_some(),
                paralyzed.is_some(),
                pinned.is_some() || rooted.is_some(),
                confused || fleeing,
                slowed,
            );
            if let Some(tint) = tint {
                screen.bg_map(pos.x, pos.y, tint);
                screen.fg_map(pos.x, pos.y, Color::Black);
            }
        }
    }

    // ---- Trick-shot potential ----
    // A monster standing on something a shot would set off — a trap the player
    // has found, a coin, or the Element of Yoord — is drawn on bright magenta.
    // It is an offer: put a missile in that tile and the thing underneath goes
    // off with it (see `models::traps::detonate_at`). A trap still hidden is
    // not in here, because an aimed shot passes straight over one.
    //
    // Painted after the status tints, so it outranks them. A sleeping monster
    // on a bear trap is worth reading as the shot, not as the nap — and the
    // two tints never disagree about anything, since a helpless monster on a
    // live trap is both at once.
    {
        let mut live: HashSet<(u16, u16)> = world
            .query_filtered::<&Position, (With<Trap>, Without<Hidden>)>()
            .iter(world)
            .map(|p| (p.x, p.y))
            .collect();
        live.extend(
            world
                .query_filtered::<&Position, Or<(With<Pickup>, With<Amulet>)>>()
                .iter(world)
                .map(|p| (p.x, p.y)),
        );
        let mut query = world.query_filtered::<&Position, (With<Mob>, Without<Hidden>)>();
        for pos in query.iter(world) {
            let coord = (pos.x, pos.y);
            if visible.contains(&coord) && live.contains(&coord) {
                screen.bg_map(pos.x, pos.y, Color::Magenta);
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
            // Open ground under the beam is a plain yellow spark. An actor
            // keeps its own glyph and only takes the beam's colour, so the
            // player can still see what they are aiming at.
            let over_actor = visible.contains(&(tx, ty)) && occupied_by_actor.contains(&(tx, ty));
            if !over_actor {
                screen.put_map(tx, ty, '*', Color::Yellow);
            }
            if over_actor {
                // Unless it was already yellow (or close to it), in which case
                // recolour to black instead, so a naturally-yellow monster
                // doesn't disappear into the beam's own colour. `put` resets
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

    // ---- Player line (row 22, directly under the viewport) ----
    // Who the player is and what they are made of, on the row nearest the
    // thing it describes. The map's own bottom row is what pays for it — the
    // log's first line used to sit here and cost exactly the same one.
    // Each field's label carries the colour; the figure beside it stays white,
    // so the line reads as one row of numbers with a colour-coded key. HP is
    // the exception — it colours its own number, and turns dark red at the
    // cutoff where auto-fight starts refusing.
    {
        let hp_color = match too_injured {
            true => Color::DarkRed,
            false => Color::Yellow,
        };
        // A poisoned dart and a potion of poison both take the attack die below
        // its ceiling, and nothing else on this line would say so: a damaged
        // Pow. reads light green until a potion of restore strength puts it
        // back. Read off `Fighter` rather than the displayed figure, which has
        // the gear folded in and so can sit above the baseline while the arm
        // behind it is still poisoned.
        let pow_color = match player_entity
            .and_then(|pe| world.get::<Fighter>(pe))
            .is_some_and(|f| f.power < f.max_power)
        {
            true => Color::Green,
            false => Color::White,
        };
        let mut fields = vec![
            (player_name.to_uppercase(), Color::White, String::new()),
            (
                strings::hp_abbr().into(),
                hp_color,
                format!("{}/{}", player_hp, player_max_hp),
            ),
            (
                strings::magic_abbr().into(),
                Color::DarkCyan,
                format!("{}/{}", player_magic, player_max_magic),
            ),
            (strings::power_abbr().into(), Color::Red, stat(pow_die, pow_flat)),
            (strings::armor_abbr().into(), Color::Cyan, stat(arm_die, arm_flat)),
        ];
        // What a throw is worth is only worth a field once something is making
        // it worth something — a bow, a ring of sharpshooting. A player who
        // never throws never sees it.
        if throw_flat != 0 {
            fields.push((strings::skill_abbr().into(), Color::Green, format!("{throw_flat:+}")));
        }
        let mut px: u16 = 1;
        for (i, (label, color, value)) in fields.iter().enumerate() {
            if i > 0 {
                screen.puts(px, 22, " · ", Color::DarkGrey);
                px += 3;
            }
            screen.puts(px, 22, label, *color);
            px += label.chars().count() as u16;
            if !value.is_empty() {
                let number = match label.as_str() {
                    s if s == strings::hp_abbr() => hp_color,
                    s if s == strings::power_abbr() => pow_color,
                    _ => Color::White,
                };
                screen.puts(px + 1, 22, value, number);
                px += 1 + value.chars().count() as u16;
            }
        }
    }

    // ---- The scorekeeper's flash (row 1, the gutter under the HUD) ----
    // A payment shouts under the score rather than over it: `+700` in one
    // bright colour, `COMBO! +2400` with the word in the flag's stripes, or
    // DOUBLE. Painted here, after the map layers, because the row it lands on
    // is the map's own top row — always wall, never anything to read.
    if let Some((text, colors)) = world
        .get_resource::<ScoreFlash>()
        .filter(|f| f.lit())
        .map(|f| {
            (
                f.text.clone(),
                (0..f.text.chars().count())
                    .map(|i| f.color_at(i))
                    .collect::<Vec<_>>(),
            )
        })
    {
        let sx = SCREEN_W.saturating_sub(1 + text.chars().count() as u16);
        for (i, (ch, color)) in text.chars().zip(colors).enumerate() {
            screen.put(sx + i as u16, 1, ch, color);
        }
    }

    // ---- Message log (rows 23..=24) ----
    // Messages are packed onto shared lines and only wrap when the next one
    // would overflow; a message is never split across the wrap. Each keeps its
    // own colour on the line it shares — a shouting message must never repaint
    // the sentences beside it — and one of them is painted a letter at a time.
    {
        let stripes = models::pride::stripes(world);
        let (lines, more) = log_panel(world);
        for (i, segments) in lines.iter().enumerate() {
            let y = 23 + i as u16;
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
                screen.puts(57, y, strings::more_prompt(), Color::Yellow);
            }
        }
    }

    // ---- Travel-cursor prompt (overrides the log rows while picking) ----
    if world.resource::<TravelCursor>().active {
        screen.puts(0, 24, strings::travel_cursor_prompt(), Color::Yellow);
        screen.puts(12, 24, strings::travel_cursor_hint(), Color::DarkGrey);
    }

    // ---- Inventory overlay ----
    if world.resource::<PackIsOpen>().open {
        draw_inventory(world, screen);
    }

    // ---- Moves overlay ----
    if world.resource::<SpellsMenu>().open {
        draw_spells(world, screen);
    }

    // ---- "Really quit?" ----
    // Last of all, so it sits over whatever else is on screen.
    if world.resource::<QuitPrompt>().open {
        draw_quit_prompt(screen);
    }

    screen.flush(stdout, offset)?;
    Ok(())
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

/// Throw away every event already sitting in the queue.
///
/// Called once before an animation starts playing, because the loops below
/// treat *any* pending event as "the player wants this skipped" — and the key
/// that triggered the animation can still be echoing. A terminal that reports
/// key releases (and some do) hands us the release of the very keypress that
/// zapped the wand, which used to kill the bolt on its first frame, so a zap
/// looked like it drew nothing at all. Keys typed ahead are swallowed here
/// instead of one frame later, which is where they were going anyway.
fn drain_input() -> std::io::Result<()> {
    while poll(Duration::from_millis(0))? {
        let _ = read()?;
    }
    Ok(())
}

/// Wait up to `frame` for the player to cut an animation short, swallowing the
/// key that does it. Only an actual key *press* counts: a release, a resize or
/// a mouse report is not someone asking to skip anything, and counting one as
/// a skip is the bug [`drain_input`] describes, arriving a frame later.
fn interrupted(frame: Duration) -> std::io::Result<bool> {
    if !poll(frame)? {
        return Ok(false);
    }
    Ok(matches!(read()?, Event::Key(k) if k.kind == KeyEventKind::Press))
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
    drain_input()?;

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
        if interrupted(Duration::from_millis(frame_ms))? {
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

    drain_input()?;
    let base_frame_ms = world.resource::<MagicMapReveal>().frame_ms();
    let frame = Duration::from_millis(world.resource::<AnimRate>().scale(base_frame_ms));
    loop {
        age_shake(world, frame.as_millis() as u64);
        if !magic_map_reveal_step(world) {
            break;
        }
        render(world, stdout, screen)?;
        if interrupted(frame)? {
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

/// A die and its flat bonus, as the HUD prints them: `10`, `10+2`, `10-2`.
///
/// The sign comes from the number, never from a `+` glued on in front of it —
/// which is what used to render a cursed weapon as `Pow. 10+-2`. A bonus of
/// zero is not shown at all: a plain weapon is a die and nothing else, and a
/// trailing `+0` on every field would be four characters of noise on a status
/// line that has to fit eight of them.
fn stat(die: i32, flat: i32) -> String {
    match flat {
        0 => format!("{die}"),
        _ => format!("{die}{flat:+}"),
    }
}

/// The scorekeeper's number, in the one width the HUD and both end panels have
/// room for: six digits, zero-padded, arcade-style.
///
/// A score that outgrows six digits is no longer a number anybody reads digit
/// by digit, so it gets an order of magnitude instead — `4.09M`, `1.31B`,
/// `9.44T` — and past a thousand trillion, where the suffixes run out, the
/// exponent itself. Truncated, never rounded: a score should never read higher
/// than it is. A score that has saturated (`models::score` pins rather than
/// wraps) is not a number at all any more, so it gets the word.
fn score_text(score: i64) -> String {
    const M: i64 = 1_000_000;
    match score {
        ..M => format!("{score:06}"),
        ..1_000_000_000 => scaled(score, M, 'M'),
        ..1_000_000_000_000 => scaled(score, 1_000 * M, 'B'),
        ..1_000_000_000_000_000 => scaled(score, 1_000_000 * M, 'T'),
        i64::MAX => "MAXIMUM".to_string(),
        _ => format!("{score:.2e}"),
    }
}

/// `score` in units of `unit`, two decimals, truncated.
fn scaled(score: i64, unit: i64, suffix: char) -> String {
    format!(
        "{}.{:02}{suffix}",
        score / unit,
        score % unit / (unit / 100)
    )
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
    let msg = strings::you_die();
    screen.puts(centered_x(msg), y, msg, Color::Red);
    let more = strings::more_prompt();
    screen.puts(centered_x(more), y + 2, more, Color::Yellow);
    screen.dirty_all = true;
    screen.flush(stdout, offset)?;
    Ok(())
}

/// The defeat panel. Shown once the player has acknowledged the death prompt:
/// the word, the name, what did it, the number. No stone and no epitaph — a
/// run that ended is a fact, not a ceremony.
pub fn render_lose<W: Write>(
    stdout: &mut W,
    screen: &mut Screen,
    offset: (u16, u16),
    player_name: &str,
    cause: &str,
    score: i64,
) -> std::io::Result<()> {
    screen.clear();

    let mut y = SCREEN_H / 2 - 2;
    let title = strings::lose_title();
    screen.puts(centered_x(title), y, title, Color::Red);
    y += 2;

    let name_line = player_name.to_uppercase();
    screen.puts(centered_x(&name_line), y, &name_line, Color::Cyan);
    y += 1;
    screen.puts(centered_x(cause), y, cause, Color::Grey);
    y += 2;

    let score_line = strings::score_line(&score_text(score));
    screen.puts(centered_x(&score_line), y, &score_line, Color::Yellow);
    y += 2;

    let prompt = strings::press_any_key_to_depart();
    screen.puts(centered_x(prompt), y, prompt, Color::DarkGrey);

    screen.dirty_all = true;
    screen.flush(stdout, offset)?;
    Ok(())
}

/// The victory panel. Shown when the player carries the Element of Yoord up the
/// final stair. The counterpart to [`render_lose`], and as plain: the dungeon
/// does not congratulate anyone.
pub fn render_win<W: Write>(
    stdout: &mut W,
    screen: &mut Screen,
    offset: (u16, u16),
    player_name: &str,
    score: i64,
) -> std::io::Result<()> {
    screen.clear();

    let mut y = SCREEN_H / 2 - 2;
    let title = strings::win_title();
    screen.puts(centered_x(title), y, title, Color::Green);
    y += 2;

    let name_line = player_name.to_uppercase();
    screen.puts(centered_x(&name_line), y, &name_line, Color::Cyan);
    y += 2;

    let score_line = strings::score_line(&score_text(score));
    screen.puts(centered_x(&score_line), y, &score_line, Color::Yellow);
    y += 2;

    let prompt = strings::press_any_key_to_depart();
    screen.puts(centered_x(prompt), y, prompt, Color::DarkGrey);

    screen.dirty_all = true;
    screen.flush(stdout, offset)?;
    Ok(())
}

/// One inventory row's full text: `" a) +1 ring mail (E) "`. The one place
/// this shape lives, so sizing the box and drawing a row can never disagree.
fn row_text(letter: char, name: &str, equipped: bool) -> String {
    let suffix = if equipped { strings::equipped_suffix() } else { "" };
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
    let question: &str = strings::quit_question();
    let answers: &str = strings::quit_answers();

    let inner = answers.chars().count() as u16 + 4;
    let x = (SCREEN_W - inner) / 2 - 1;
    let y = MAP_TOP + MAP_HEIGHT / 2 - 2;
    let grey = Color::DarkGrey;

    screen.put(x, y, '┌', grey);
    screen.hline(x + 1, y, '─', inner, grey);
    screen.put(x + 1 + inner, y, '┐', grey);
    for (row, (text, color)) in [(question, Color::Yellow), (answers, Color::White)]
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
                // A wand says how many zaps are left; nothing else carries a
                // Battery, and the count is pack-only so messages stay clean.
                let name = match world.get::<Battery>(e) {
                    Some(b) => format!("{} ({})", models::display_name(world, e), b.charges),
                    None => models::display_name(world, e),
                };
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

/// The `Z` spells box: up to four rows, lettered `a`-`d` like the pack's, each
/// the spell's name and its [`Magic`] cost. Drawn the same way the inventory
/// box is, just with no sub-menu — picking a row goes straight to the aiming
/// reticle.
fn draw_spells(world: &mut World, screen: &mut Screen) {
    let selected = world.resource::<SpellsMenu>().selected;
    let (player, slots) = {
        let mut q = world.query_filtered::<(Entity, &Spellset), With<Player>>();
        q.iter(world)
            .next()
            .map(|(e, m)| (e, m.slots.clone()))
            .unwrap_or((Entity::PLACEHOLDER, Vec::new()))
    };

    let rows: Vec<String> = slots
        .iter()
        .enumerate()
        .map(|(i, &effect)| {
            let def = SpellDef::of(effect);
            let cost = models::spell_cost(world, player, effect);
            let letter = (b'a' + i as u8) as char;
            format!(" {letter}) {} ({} Ma) ", def.display_name(), cost)
        })
        .collect();

    let start_x: u16 = 5;
    let start_y: u16 = 3;
    let grey = Color::DarkGrey;
    let title = strings::spells_menu_title();
    let box_width = rows
        .iter()
        .map(|r| r.chars().count() as u16)
        .chain([title.len() as u16])
        .max()
        .unwrap_or(0)
        .max(20);

    screen.put(start_x, start_y, '┌', grey);
    screen.hline(start_x + 1, start_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, start_y, '┐', grey);
    screen.puts(
        start_x + box_width / 2 - title.len() as u16 / 2,
        start_y,
        title,
        Color::Yellow,
    );

    for (row, text) in rows.iter().enumerate() {
        let y = start_y + 1 + row as u16;
        let color = if row == selected {
            Color::Yellow
        } else {
            Color::White
        };
        screen.put(start_x, y, '│', grey);
        screen.puts(
            start_x + 1,
            y,
            &format!("{:<w$}", text, w = box_width as usize),
            color,
        );
        screen.put(start_x + 1 + box_width, y, '│', grey);
    }

    let bottom_y = start_y + 1 + rows.len() as u16;
    screen.put(start_x, bottom_y, '└', grey);
    screen.hline(start_x + 1, bottom_y, '─', box_width, grey);
    screen.put(start_x + 1 + box_width, bottom_y, '┘', grey);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world holding only what the log panel reads. Deliberately not a
    /// mirror of `main`'s startup: this asserts a rule, not a frame, and the
    /// last end-to-end render test here died of hand-copying twenty-two
    /// resources — see `docs/explanation/the-feel-layer.md`.
    fn gate_world() -> World {
        let mut w = World::new();
        w.init_resource::<GameLog>();
        w.init_resource::<Particles>();
        w
    }

    /// More unread messages than the log's three lines can hold — what a trick
    /// shot leaves behind once it has shouted, killed and dropped their gear.
    fn flood_the_log(w: &mut World) {
        let mut log = w.resource_mut::<GameLog>();
        log.unread.clear();
        for i in 0..12 {
            log.unread.push(LogEntry::plain(format!(
                "Message number {i} is a fairly long one."
            )));
        }
        assert!(
            log_view(&w.resource::<GameLog>().unread).2,
            "the fixture did not actually give the log a backlog to prompt about"
        );
    }

    #[test]
    fn the_scorekeeper_shortens_a_score_it_cannot_print_in_six_digits() {
        assert_eq!(score_text(140), "000140", "the arcade format, padded");
        assert_eq!(score_text(999_999), "999999", "six digits is the limit");
        assert_eq!(score_text(1_000_000), "1.00M");
        assert_eq!(score_text(4_098_000), "4.09M", "truncated, never rounded");
        assert_eq!(score_text(999_999_999), "999.99M", "and never rounded up");
        assert_eq!(score_text(1_310_000_000), "1.31B");
        assert_eq!(score_text(9_440_000_000_000), "9.44T");
        assert_eq!(score_text(4_611_686_018_427_387_900), "4.61e18");
        // A run that has pinned the ceiling has stopped counting; say so.
        assert_eq!(score_text(i64::MAX), "MAXIMUM");
    }

    #[test]
    fn a_backlog_holds_its_prompt_until_the_animation_is_over() {
        let mut w = gate_world();
        flood_the_log(&mut w);
        // A blast still ripening behind a missile in flight: the batch a trick
        // shot queues, and the moment the prompt used to ask for the keypress
        // that would throw the blast away.
        {
            let mut fx = w.resource_mut::<Particles>();
            fx.hurl(&[(1, 1), (2, 1)], '\u{2191}', Color::Grey);
            fx.explosion(&[(2, 1, 0.0)], BlastPalette::Force);
        }
        assert!(
            !log_panel(&w).1,
            "the prompt asked for a key while a key would have skipped the blast"
        );

        w.resource_mut::<Particles>().clear();
        assert!(
            log_panel(&w).1,
            "the animation is over — the prompt has to come up now, or the backlog is unreachable"
        );
    }

    #[test]
    fn an_animation_alone_raises_no_prompt() {
        let mut w = gate_world();
        w.resource_mut::<Particles>().hit_spark(1, 1);
        assert!(
            !log_panel(&w).1,
            "nothing is waiting to be read: the gate has nothing to hold back"
        );
    }

    #[test]
    fn status_tint_takes_asleep_over_everything_else() {
        assert_eq!(
            status_tint(true, true, true, true, true),
            Some(Color::DarkBlue),
            "asleep outranks paralysed, held-down, staggering, slowed"
        );
    }

    #[test]
    fn status_tint_gives_paralysis_its_own_colour() {
        assert_eq!(
            status_tint(false, true, true, true, true),
            Some(Color::DarkYellow),
            "paralysed outranks held-down, staggering, slowed, and reads as its own colour, not asleep's"
        );
    }

    #[test]
    fn status_tint_puts_a_bear_trap_and_hold_monster_on_the_same_colour() {
        assert_eq!(
            status_tint(false, false, true, false, false),
            Some(Color::DarkGreen)
        );
        assert_eq!(
            status_tint(false, false, true, true, true),
            Some(Color::DarkGreen),
            "held-down outranks staggering and slowed"
        );
    }

    #[test]
    fn status_tint_marks_confused_and_fleeing_the_same_yellow() {
        assert_eq!(
            status_tint(false, false, false, true, false),
            Some(Color::Yellow)
        );
        assert_eq!(
            status_tint(false, false, false, true, true),
            Some(Color::Yellow),
            "staggering outranks merely slowed"
        );
    }

    #[test]
    fn status_tint_falls_back_to_slowed_then_nothing() {
        assert_eq!(
            status_tint(false, false, false, false, true),
            Some(Color::Grey)
        );
        assert_eq!(status_tint(false, false, false, false, false), None);
    }
}
