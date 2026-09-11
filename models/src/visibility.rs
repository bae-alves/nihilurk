use crate::components::*;
use crate::effects::SeesInvisible;
use crate::identify::{Identified, ItemAppearances, named_display, phrase_for};
use crate::map::{MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, Map, TileType, tile_index};
use bevy_ecs::prelude::*;
use std::collections::{HashSet, VecDeque};

#[inline]
fn in_bounds(x: i16, y: i16) -> bool {
    x >= 0 && y >= 0 && (x as u16) < MAP_WIDTH && (y as u16) < MAP_HEIGHT
}

#[allow(clippy::type_complexity)] // one query covering both mobs and floor items
#[allow(clippy::too_many_arguments)]
pub fn visibility_system(
    mut commands: Commands,

    // `With<Player>` matters: without it a future monster viewshed would reveal
    // the map for the player.
    // `SeesInvisible` is asked for as a plain component. It might come from a
    // ring, from a potion, from being born that way — this system doesn't ask.
    mut viewshed_query: Query<
        (Entity, &mut Viewshed, &Position, Option<&SeesInvisible>),
        With<Player>,
    >,

    // Everything the player can "spot": monsters and floor items. `Option`s let
    // one query cover both kinds and track the per-entity spotted state.
    #[allow(clippy::type_complexity)] spot_query: Query<
        (
            Entity,
            &Position,
            Option<&Mob>,
            Option<&Invisible>,
            Option<&Name>,
            Option<&Stack>,
            Option<&Potion>,
            Option<&Scroll>,
            Option<&Wand>,
            Option<&Ring>,
            Option<&Spotted>,
        ),
        Or<(With<Mob>, With<Item>)>,
    >,

    // Hidden traps whose reveal style might trip this turn.
    mut trap_query: Query<(Entity, &Position, &mut Trap), With<Hidden>>,

    mut log: ResMut<GameLog>,

    map: Res<Map>,
    identified: Res<Identified>,
    appearances: Res<ItemAppearances>,
) {
    let any_dirty = viewshed_query.iter().any(|(_, v, _, _)| v.dirty);
    if !any_dirty {
        return;
    }

    for (_player_entity, mut viewshed, pos, sees_invisible) in viewshed_query.iter_mut() {
        if !viewshed.dirty {
            continue;
        }
        let perception = sees_invisible.is_some();
        let visible = visible_from(&map, pos);

        hide_and_announce(
            &mut commands,
            &mut log,
            &spot_query,
            &visible,
            perception,
            &identified,
            &appearances,
        );
        reveal_traps(
            &mut commands,
            &mut log,
            &mut trap_query,
            &visible,
            pos,
            perception,
        );

        if viewshed.revealed_tiles.len() < MAP_TILE_COUNT {
            viewshed.revealed_tiles.grow(MAP_TILE_COUNT);
        }
        for &(x, y) in &visible {
            if x < MAP_WIDTH && y < MAP_HEIGHT {
                viewshed.revealed_tiles.insert(tile_index(x, y));
            }
        }
        viewshed.visible_tiles = visible.into_iter().collect();
        viewshed.dirty = false;
    }
}

/// The eight neighbouring offsets, centre excluded.
const NEIGHBOUR_OFFSETS: [(i16, i16); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// The in-bounds neighbours of `(x, y)`.
fn neighbours(x: u16, y: u16) -> impl Iterator<Item = (u16, u16)> {
    NEIGHBOUR_OFFSETS.iter().filter_map(move |&(dx, dy)| {
        let (nx, ny) = (x as i16 + dx, y as i16 + dy);
        in_bounds(nx, ny).then_some((nx as u16, ny as u16))
    })
}

/// Every tile the player at `pos` can currently see: the always-on 3x3, plus —
/// when standing in a lit room — a flood-fill of that room out to its walls. A
/// dark room gives only the 3x3, as if it were a passage, until a wand of light
/// clears its `dark` bits.
fn visible_from(map: &Map, pos: &Position) -> HashSet<(u16, u16)> {
    let mut visible = HashSet::new();
    let (cx, cy) = (pos.x as i16, pos.y as i16);

    for dy in -1..=1 {
        for dx in -1..=1 {
            if in_bounds(cx + dx, cy + dy) {
                visible.insert(((cx + dx) as u16, (cy + dy) as u16));
            }
        }
    }

    let in_lit_room = !map.is_dark(pos.x, pos.y)
        && matches!(
            map.tile(pos.x, pos.y),
            TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
        );
    if in_lit_room {
        flood_fill_room(map, (pos.x, pos.y), &mut visible);
    }
    visible
}

/// Breadth-first fill through connected room / door / stair tiles from `start`,
/// adding every tile it reaches — plus the enclosing walls and passage mouths —
/// to `visible`. Walls and passages are seen but never spread through, which is
/// what keeps the fill from leaking out of the room.
fn flood_fill_room(map: &Map, start: (u16, u16), visible: &mut HashSet<(u16, u16)>) {
    let mut queue = VecDeque::from([start]);
    let mut visited: HashSet<(u16, u16)> = HashSet::from([start]);

    while let Some((cx, cy)) = queue.pop_front() {
        for (nx, ny) in neighbours(cx, cy) {
            if map.is_dark(nx, ny) {
                continue; // never see into (or through) an unlit dark room
            }
            visible.insert((nx, ny));
            let spreads = matches!(
                map.tile(nx, ny),
                TileType::Room | TileType::Door | TileType::Upstairs | TileType::Downstairs
            );
            if spreads && visited.insert((nx, ny)) {
                queue.push_back((nx, ny));
            }
        }
    }
}

/// Hides or reveals every mob and floor item against what the player can see,
/// and logs the first sighting of anything new.
#[allow(clippy::type_complexity)]
fn hide_and_announce(
    commands: &mut Commands,
    log: &mut GameLog,
    spot_query: &Query<
        (
            Entity,
            &Position,
            Option<&Mob>,
            Option<&Invisible>,
            Option<&Name>,
            Option<&Stack>,
            Option<&Potion>,
            Option<&Scroll>,
            Option<&Wand>,
            Option<&Ring>,
            Option<&Spotted>,
        ),
        Or<(With<Mob>, With<Item>)>,
    >,
    visible: &HashSet<(u16, u16)>,
    perception: bool,
    identified: &Identified,
    appearances: &ItemAppearances,
) {
    for (entity, pos, mob, invisible, name, stack, potion, scroll, wand, ring, spotted) in
        spot_query.iter()
    {
        let in_view = visible.contains(&(pos.x, pos.y));
        let perceptible = in_view && (invisible.is_none() || perception);

        if mob.is_some() && perceptible {
            commands.entity(entity).remove::<Hidden>();
        }
        if mob.is_some() && !perceptible {
            commands.entity(entity).insert(Hidden);
        }
        if mob.is_none() && invisible.is_some() && perceptible {
            // A perception ring turns up an invisibly-stashed item for good.
            commands.entity(entity).remove::<Hidden>();
            commands.entity(entity).remove::<Invisible>();
            commands.entity(entity).insert(Spotted);
            log.add("Hey! There's something here!".to_string());
        }

        // Never announce something still out of the player's senses, nor an
        // item that hasn't been turned up yet (still `Invisible`).
        let announce = perceptible && !(mob.is_none() && invisible.is_some());
        if announce && spotted.is_none() {
            let seen_name = named_display(
                potion,
                scroll,
                wand,
                ring,
                name,
                stack,
                identified,
                appearances,
            );
            log.add(spotted_line(&seen_name));
            commands.entity(entity).insert(Spotted);
        }
        if !announce && spotted.is_some() {
            commands.entity(entity).remove::<Spotted>();
        }
    }
}

/// The sighting line for a freshly spotted thing, using its identification-aware
/// display name rather than its (possibly still-secret) true [`Name`].
fn spotted_line(seen_name: &str) -> String {
    format!("You spotted {}.", phrase_for(seen_name))
}

/// Brings hidden traps to light: a `Sight` trap the instant its tile is in
/// view, an `Adjacent` trap once the player is next to it, a `Triggered` trap
/// not until something sets it off — but a ring of perception reveals every
/// trap on the floor at once. Revealing latches (`Hidden` removed for good).
fn reveal_traps(
    commands: &mut Commands,
    log: &mut GameLog,
    trap_query: &mut Query<(Entity, &Position, &mut Trap), With<Hidden>>,
    visible: &HashSet<(u16, u16)>,
    player: &Position,
    perception: bool,
) {
    for (entity, tpos, mut trap) in trap_query.iter_mut() {
        if trap.revealed {
            continue;
        }
        if !perception && !trap_tripped(trap.reveal, tpos, player, visible) {
            continue;
        }
        trap.revealed = true;
        commands.entity(entity).remove::<Hidden>();
        log.add(format!(
            "You spot {} {}.",
            trap.effect.label_article(),
            trap.effect.label()
        ));
    }
}

/// Whether a trap's own reveal style is satisfied this turn. The ring of
/// perception is a separate short-circuit, checked by the caller.
fn trap_tripped(
    reveal: TrapReveal,
    tpos: &Position,
    player: &Position,
    visible: &HashSet<(u16, u16)>,
) -> bool {
    match reveal {
        TrapReveal::Sight => visible.contains(&(tpos.x, tpos.y)),
        TrapReveal::Adjacent => {
            (tpos.x as i32 - player.x as i32).abs() <= 1
                && (tpos.y as i32 - player.y as i32).abs() <= 1
        }
        TrapReveal::Triggered => false,
    }
}
