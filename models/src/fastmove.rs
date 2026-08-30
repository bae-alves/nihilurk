//! NetHack-style fast movement ("running" / "zooming"). Shift + a direction
//! either bolts the player in a straight line until something interesting
//! happens, or — when a feature lies roughly that way — beelines to the nearest
//! one, preferring stairs, then doors, then loose items.
//!
//! Unlike auto-explore, the walk is not animated: the engine runs every step
//! back to back and only repaints once the run is over, so it reads as a single
//! jump. Each step still costs exactly one turn.
//!
//! Like [`crate::autoexplore`], the helpers here are pure — they only read the
//! world. The engine owns the terminal, the turn schedule, and the keypress
//! poll that aborts a run.

use std::collections::HashSet;

use bevy_ecs::prelude::*;

use crate::autoexplore::{monster_in_sight, travel_step};
use crate::components::*;
use crate::map::{Map, TileType, MAP_HEIGHT, MAP_WIDTH};

/// Transient flag: set while a fast-move run is in progress. Never serialised,
/// so a reload always starts idle.
#[derive(Resource, Default)]
pub struct FastMove {
    pub active: bool,
    /// The pressed direction; drives a straight-line run.
    pub dx: i16,
    pub dy: i16,
    /// `Some` while beelining to a fixed feature tile; `None` for a straight run.
    pub target: Option<(u16, u16)>,
    /// Steps taken on the current run — a guard against a runaway loop.
    pub steps: u32,
}

impl FastMove {
    /// Begin a run in direction `(dx, dy)`: toward `target` if given, otherwise
    /// a straight line until something interrupts.
    pub fn start(&mut self, dx: i16, dy: i16, target: Option<(u16, u16)>) {
        self.active = true;
        self.dx = dx;
        self.dy = dy;
        self.target = target;
        self.steps = 0;
    }

    /// Halt any run.
    pub fn stop(&mut self) {
        self.active = false;
        self.target = None;
    }
}

/// Hard ceiling on steps in a single run. A straight shot crosses the map in
/// ~80 steps and the longest sane beeline is a few hundred; this is paranoia
/// against an unforeseen loop.
pub const FAST_MOVE_STEP_CAP: u32 = 1000;

/// What a Shift + direction press should do from where the player stands.
pub enum FastMovePlan {
    /// A creature is in view — running is refused.
    MonsterInSight,
    /// Nothing to run toward and the first step that way is blocked.
    Blocked,
    /// Bolt in a straight line in the pressed direction.
    Straight,
    /// Beeline to this feature tile (stairs / door / item).
    Travel((u16, u16)),
}

fn player_pos(world: &mut World) -> Option<(u16, u16)> {
    let mut q = world.query_filtered::<&Position, With<Player>>();
    q.iter(world).next().map(|p| (p.x, p.y))
}

/// Whether `(vx, vy)` — a vector from the player to some feature — points within
/// the cone around the pressed direction `(dx, dy)`: a 90° wedge for a cardinal
/// press, the matching quadrant for a diagonal one.
fn in_cone(vx: i32, vy: i32, dx: i16, dy: i16) -> bool {
    if vx == 0 && vy == 0 {
        return false;
    }
    match (dx.signum() as i32, dy.signum() as i32) {
        (0, sy) => vy.signum() == sy && vx.abs() <= vy.abs(),
        (sx, 0) => vx.signum() == sx && vy.abs() <= vx.abs(),
        (sx, sy) => vx.signum() == sx && vy.signum() == sy,
    }
}

/// Decide what a Shift + `(dx, dy)` press does right now. Pure: reads the world,
/// never mutates it.
pub fn fast_move_plan(world: &mut World, dx: i16, dy: i16) -> FastMovePlan {
    if monster_in_sight(world) {
        return FastMovePlan::MonsterInSight;
    }

    let Some((px, py)) = player_pos(world) else {
        return FastMovePlan::Blocked;
    };

    // The tiles the player can see this instant.
    let visible: Vec<(u16, u16)> = {
        let mut q = world.query_filtered::<&Viewshed, With<Player>>();
        match q.iter(world).next() {
            Some(v) => v.visible_tiles.clone(),
            None => Vec::new(),
        }
    };

    // Floor items currently in view.
    let item_tiles: HashSet<(u16, u16)> = {
        let mut q = world.query_filtered::<&Position, (With<Item>, Without<Hidden>)>();
        q.iter(world).map(|p| (p.x, p.y)).collect()
    };

    // Best feature roughly in the pressed direction: lowest priority number wins
    // (0 = stairs, 1 = door, 2 = item), ties broken by Chebyshev distance.
    let best: Option<(u8, i32, (u16, u16))> = {
        let map = world.resource::<Map>();
        let mut best: Option<(u8, i32, (u16, u16))> = None;
        for &(x, y) in &visible {
            if (x, y) == (px, py) {
                continue;
            }
            let vx = x as i32 - px as i32;
            let vy = y as i32 - py as i32;
            if !in_cone(vx, vy, dx, dy) {
                continue;
            }
            let prio = match map.tile(x, y) {
                TileType::Upstairs | TileType::Downstairs => 0,
                TileType::Door => 1,
                _ if item_tiles.contains(&(x, y)) => 2,
                _ => continue,
            };
            let dist = vx.abs().max(vy.abs());
            if best.is_none_or(|(bp, bd, _)| (prio, dist) < (bp, bd)) {
                best = Some((prio, dist, (x, y)));
            }
        }
        best
    };

    // Take the beeline only if a known path actually reaches it; otherwise fall
    // through to a straight run.
    match best {
        Some((_, _, tile)) if travel_step(world, tile).is_some() => {
            return FastMovePlan::Travel(tile);
        }
        _ => {}
    }

    // No feature that way: a straight run, if the first step is clear.
    let nx = px as i32 + dx as i32;
    let ny = py as i32 + dy as i32;
    if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
        return FastMovePlan::Blocked;
    }
    if world.resource::<Map>().blocks(nx as u16, ny as u16) {
        return FastMovePlan::Blocked;
    }
    FastMovePlan::Straight
}

/// One step of a straight-line run: the pressed direction, or `None` when the
/// tile straight ahead is off-map or a wall.
pub fn straight_step(world: &mut World) -> Option<(i16, i16)> {
    let (px, py) = player_pos(world)?;
    let (dx, dy) = {
        let fm = world.resource::<FastMove>();
        (fm.dx, fm.dy)
    };
    let nx = px as i32 + dx as i32;
    let ny = py as i32 + dy as i32;
    if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
        return None;
    }
    if world.resource::<Map>().blocks(nx as u16, ny as u16) {
        return None;
    }
    Some((dx, dy))
}

/// Whether a straight-line run should halt now that the player has landed on
/// their latest tile: true on a door or staircase, or at a corridor branch.
pub fn straight_stop_here(world: &mut World) -> bool {
    let Some((px, py)) = player_pos(world) else {
        return true;
    };
    let (dx, dy) = {
        let fm = world.resource::<FastMove>();
        (fm.dx, fm.dy)
    };
    let map = world.resource::<Map>();

    match map.tile(px, py) {
        TileType::Door | TileType::Upstairs | TileType::Downstairs => return true,
        // In a corridor, stop where a side passage opens up.
        TileType::Passage => {
            for &(qx, qy) in &[(-dy, dx), (dy, -dx)] {
                let nx = px as i32 + qx as i32;
                let ny = py as i32 + qy as i32;
                if nx >= 0
                    && ny >= 0
                    && nx < MAP_WIDTH as i32
                    && ny < MAP_HEIGHT as i32
                    && !map.blocks(nx as u16, ny as u16)
                {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}
