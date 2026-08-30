//! Auto-fight: each `Tab` press takes one turn toward the deadliest foe in
//! sight and, once adjacent, strikes it. It is not a mode — one press, one
//! turn — so the player stays in the driver's seat while the tedium of walking
//! up to a monster is handled for them.
//!
//! Like [`crate::autoexplore`], the helpers here are pure: they only read the
//! world. The engine owns the terminal, the message log, and the actual attack
//! resolution (a step onto a mob's tile becomes a melee strike).

use bevy_ecs::prelude::*;

use crate::autoexplore::first_step;
use crate::components::*;
use crate::map::{tile_index, Map, MAP_HEIGHT, MAP_WIDTH};

/// Whether the player is at or below a quarter of their maximum HP — the cutoff
/// below which auto-fight refuses. Kept in integer maths: `hp * 4 <= max_hp` is
/// exactly `hp <= 25% of max_hp`.
pub fn player_too_injured(world: &mut World) -> bool {
    let mut q = world.query_filtered::<&Fighter, With<Player>>();
    q.iter(world).next().is_some_and(|f| f.hp * 4 <= f.max_hp)
}

/// The player's map position, if there is a player.
fn player_pos(world: &mut World) -> Option<(u16, u16)> {
    let mut q = world.query_filtered::<&Position, With<Player>>();
    q.iter(world).next().map(|p| (p.x, p.y))
}

/// Every hostile currently in the player's viewshed, as `(entity, (x, y), hp)`.
fn visible_enemies(world: &mut World) -> Vec<(Entity, (u16, u16), i32)> {
    let mut q = world
        .query_filtered::<(Entity, &Position, &Fighter, &Faction), (With<Mob>, Without<Hidden>)>();
    q.iter(world)
        .filter_map(|(e, p, f, faction)| {
            (*faction == Faction::Monster).then_some((e, (p.x, p.y), f.hp))
        })
        .collect()
}

/// The foe to press toward. Anything already in melee range is finished first
/// (so the player never turns their back on an adjacent enemy); otherwise the
/// whole visible pack is in play. Within that set the pick is the lowest current
/// HP, ties broken by proximity. `None` when nothing hostile is in sight.
pub fn auto_fight_target(world: &mut World) -> Option<Entity> {
    let (px, py) = player_pos(world)?;
    let enemies = visible_enemies(world);

    let chebyshev = |(x, y): (u16, u16)| -> i32 {
        (x as i32 - px as i32).abs().max((y as i32 - py as i32).abs())
    };

    let in_melee = enemies.iter().any(|&(_, pos, _)| chebyshev(pos) <= 1);

    enemies
        .into_iter()
        .filter(|&(_, pos, _)| !in_melee || chebyshev(pos) <= 1)
        .min_by_key(|&(_, pos, hp)| (hp, chebyshev(pos)))
        .map(|(e, _, _)| e)
}

/// The single `(dx, dy)` step the player should take against `target`: straight
/// onto its tile when already adjacent (the caller turns a step onto a mob into
/// an attack), otherwise the first hop of the shortest path over revealed,
/// walkable ground that no other mob is standing on. `None` if the target is
/// gone or no route reaches it.
pub fn fight_step(world: &mut World, target: Entity) -> Option<(i16, i16)> {
    let (px, py) = player_pos(world)?;
    let (tx, ty) = {
        let p = world.get::<Position>(target)?;
        (p.x, p.y)
    };

    let dx = tx as i32 - px as i32;
    let dy = ty as i32 - py as i32;
    if dx.abs() <= 1 && dy.abs() <= 1 {
        return Some((dx.signum() as i16, dy.signum() as i16));
    }

    // Other mobs are impassable: route around the pack rather than through it.
    let occupied: Vec<(u16, u16)> = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Mob>>();
        q.iter(world)
            .filter(|(e, _)| *e != target)
            .map(|(_, p)| (p.x, p.y))
            .collect()
    };

    let seen = {
        let mut q = world.query_filtered::<&Viewshed, With<Player>>();
        q.iter(world).next()?.revealed_tiles.clone()
    };
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH
            && y < MAP_HEIGHT
            && seen.contains(tile_index(x, y))
            && !map.blocks(x, y)
            && !occupied.contains(&(x, y))
    };

    // Goal: any tile bordering the target — from there the adjacency branch
    // above lands the blow next turn.
    let step_ok = |fx: u16, fy: u16, tgx: u16, tgy: u16| map.diagonal_step_ok(fx, fy, tgx, tgy);
    first_step(px, py, &open, step_ok, |x, y| {
        let ax = (x as i32 - tx as i32).abs();
        let ay = (y as i32 - ty as i32).abs();
        ax <= 1 && ay <= 1 && (ax != 0 || ay != 0)
    })
}
