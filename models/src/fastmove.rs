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

use crate::autoexplore::{known_trap_tiles, monster_in_sight, travel_step};
use crate::components::*;
use crate::constants::travel::CHARGE_RANGE;
use crate::effects::{Charges, Grant, Lifetime, Vuln, lend};
use crate::helpers::{chebyshev, get_line, mob_at};
use crate::map::{MAP_HEIGHT, MAP_WIDTH, Map, TileType};

/// Hard ceiling on steps in a single run — paranoia against a pathfinding
/// loop. Defined and documented in `constants.rs`.
pub use crate::constants::travel::FAST_MOVE_STEP_CAP;

/// Transient flag: set while a fast-move run is in progress. Never serialised,
/// so a reload always starts idle.
#[derive(Resource, Default)]
pub struct FastMove {
    /// Whether a run is in progress.
    pub active: bool,
    /// The pressed direction; drives a straight-line run.
    pub dx: i16,
    /// The vertical half of the pressed direction; see `dx`.
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

/// What a Shift + direction press should do from where the player stands.
pub enum FastMovePlan {
    /// A creature is in view — running is refused.
    MonsterInSight,
    /// A creature stands in the pressed direction with open ground between:
    /// close the gap and strike, see [`charge`].
    Charge(Entity),
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

/// The creature a charge in `(dx, dy)` would hit: the nearest hostile in view
/// inside the pressed direction's wedge, between two and [`CHARGE_RANGE`] tiles
/// away, with open ground on the straight line to it. A peaceful spirit is no
/// one's foe. `None` for anyone without
/// [`Charges`].
fn charge_target(world: &mut World, dx: i16, dy: i16) -> Option<Entity> {
    let me = world
        .query_filtered::<(Entity, &Position), With<Charges>>()
        .iter(world)
        .find(|(e, _)| world.get::<Player>(*e).is_some())
        .map(|(_, p)| *p)?;
    let mut q = world.query_filtered::<(Entity, &Position, &Faction), (
        With<Mob>,
        Without<Hidden>,
        Without<Helper>,
        Without<crate::ice::IceCube>,
    )>();
    let spirits_at_peace = !world.resource::<crate::components::SpiritsHostile>().0;
    let mut foes: Vec<(i32, Entity)> = q
        .iter(world)
        .filter(|&(_, _, &f)| f == Faction::Monster || (f == Faction::Spirits && !spirits_at_peace))
        .filter(|&(_, &p, _)| {
            (2..=CHARGE_RANGE).contains(&chebyshev(me, p))
                && in_cone(p.x as i32 - me.x as i32, p.y as i32 - me.y as i32, dx, dy)
        })
        .map(|(e, &p, _)| (chebyshev(me, p), e))
        .collect();
    foes.sort_by_key(|&(d, _)| d);
    foes.into_iter()
        .map(|(_, e)| e)
        .find(|&e| charge_landing(world, me, e).is_some())
}

/// The tile a charge from `from` ends on to strike `target`: the one before it
/// on the straight line, provided every tile up to there is open floor, empty
/// of creatures. A trap on the landing tile is sprung; one jumped over is not.
fn charge_landing(world: &mut World, from: Position, target: Entity) -> Option<Position> {
    let to = *world.get::<Position>(target)?;
    let line = get_line(from, to);
    let landing = *line.get(line.len().checked_sub(2)?)?;
    let swims = crate::helpers::player_swims(world);
    for pair in line[..line.len() - 1].windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let clear = {
            let map = world.resource::<Map>();
            map.walkable(b.x, b.y, swims) && map.diagonal_step_ok(a.x, a.y, b.x, b.y)
        };
        if !clear || mob_at(world, b).is_some() {
            return None;
        }
    }
    Some(landing)
}

/// Whether the player could charge `target` right now: a clear straight line
/// to the tile beside it. Reads the world and changes nothing.
pub fn can_charge(world: &mut World, target: Entity) -> bool {
    let Some(from) = world
        .query_filtered::<&Position, With<Player>>()
        .iter(world)
        .next()
        .copied()
    else {
        return false;
    };
    charge_landing(world, from, target).is_some()
}

/// CHARGE!: carries the player to the tile beside `target`, strikes it, and
/// leaves them [`Vuln`] for the creatures' turn that follows. Returns whether
/// it fired; the turn is spent either way it does.
///
/// [`Vuln`] is lent for two ticks: the first runs at the top of the very turn
/// this charge spends, before the creatures move, so one tick would end it
/// before it cost anything.
pub fn charge(world: &mut World, target: Entity) -> bool {
    if !can_charge(world, target) {
        return false;
    }
    let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    else {
        return false;
    };
    let Some(from) = world.get::<Position>(player).copied() else {
        return false;
    };
    let Some(landing) = charge_landing(world, from, target) else {
        return false;
    };
    *world.get_mut::<Position>(player).unwrap() = landing;
    crate::helpers::mark_moved(world, player);
    world.resource_mut::<GameLog>().add(strings::charge());
    lend(world, player, Grant::of::<Vuln>(), Lifetime::Turns(2));
    crate::combat::melee_attack(world, player, target);
    true
}

/// Decide what a Shift + `(dx, dy)` press does right now. Pure: reads the world,
/// never mutates it.
pub fn fast_move_plan(world: &mut World, dx: i16, dy: i16) -> FastMovePlan {
    if monster_in_sight(world) {
        return match charge_target(world, dx, dy) {
            Some(target) => FastMovePlan::Charge(target),
            None => FastMovePlan::MonsterInSight,
        };
    }

    let Some((px, py)) = player_pos(world) else {
        return FastMovePlan::Blocked;
    };

    let visible: Vec<(u16, u16)> = {
        let mut q = world.query_filtered::<&Viewshed, With<Player>>();
        match q.iter(world).next() {
            Some(v) => v.visible_tiles.clone(),
            None => Vec::new(),
        }
    };

    let item_tiles: HashSet<(u16, u16)> = {
        let mut q = world.query_filtered::<&Position, (With<Item>, Without<Hidden>)>();
        q.iter(world).map(|p| (p.x, p.y)).collect()
    };

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

    match best {
        Some((_, _, tile)) if travel_step(world, tile).is_some() => {
            return FastMovePlan::Travel(tile);
        }
        _ => {}
    }

    let nx = px as i32 + dx as i32;
    let ny = py as i32 + dy as i32;
    if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
        return FastMovePlan::Blocked;
    }
    {
        let swims = crate::helpers::player_swims(world);
        let map = world.resource::<Map>();
        if !map.walkable(nx as u16, ny as u16, swims)
            || !map.diagonal_step_ok(px, py, nx as u16, ny as u16)
        {
            return FastMovePlan::Blocked;
        }
    }
    if known_trap_tiles(world).contains(&(nx as u16, ny as u16)) {
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
    {
        let swims = crate::helpers::player_swims(world);
        let map = world.resource::<Map>();
        if !map.walkable(nx as u16, ny as u16, swims)
            || !map.diagonal_step_ok(px, py, nx as u16, ny as u16)
        {
            return None;
        }
    }
    if known_trap_tiles(world).contains(&(nx as u16, ny as u16)) {
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
