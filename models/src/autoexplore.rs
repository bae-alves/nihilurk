//! Auto-walk: the `o` command explores the floor, and `>` / `<` away from a
//! staircase you have already found walks you to it. Either way the player moves
//! one step per turn until the goal is reached or something interrupts the walk.
//!
//! The pathfinding here is deliberately pure — the `*_step` functions only read
//! the world — so the engine can own the parts that touch the terminal (polling
//! for a keypress) and the message log, and the interesting logic stays testable.

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;

use crate::components::*;
use crate::map::{tile_index, Map, TileType, MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH};

/// Transient flag: set while the player is auto-walking. Never serialised, so a
/// reload always starts idle.
#[derive(Resource, Default)]
pub struct AutoExplore {
    pub active: bool,
    /// Steps taken on the current run. A guard against a pathologically long
    /// walk: [`AUTO_EXPLORE_STEP_CAP`] stops it dead.
    pub steps: u32,
    /// `Some` while travelling to a fixed tile (a known staircase); `None` for
    /// open-ended exploration.
    pub target: Option<(u16, u16)>,
}

impl AutoExplore {
    /// Begin an auto-walk: toward `target` if given, otherwise general
    /// exploration.
    pub fn start(&mut self, target: Option<(u16, u16)>) {
        self.active = true;
        self.steps = 0;
        self.target = target;
    }

    /// Halt any auto-walk.
    pub fn stop(&mut self) {
        self.active = false;
        self.target = None;
    }
}

/// Hard ceiling on steps in a single auto-walk. A full 80x22 floor is a few
/// hundred steps at most; this is pure paranoia against an unforeseen loop.
pub const AUTO_EXPLORE_STEP_CAP: u32 = 5000;

/// The eight neighbour offsets, matching the player's movement options.
const DIRS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1, 0),           (1, 0),
    (-1, 1),  (0, 1),  (1, 1),
];

/// Whether any monster is currently inside the player's viewshed (i.e. drawn on
/// screen). Auto-walk refuses to start, and halts, while this is true.
pub fn monster_in_sight(world: &mut World) -> bool {
    let mut query = world.query_filtered::<(), (With<Mob>, Without<Hidden>)>();
    query.iter(world).next().is_some()
}

/// The map coordinate of the floor's staircase, up or down. Every floor carries
/// exactly one of each (see `build_tiles`).
pub fn stair_location(map: &Map, going_down: bool) -> Option<(u16, u16)> {
    let want = if going_down { TileType::Downstairs } else { TileType::Upstairs };
    map.tiles.iter().position(|&t| t == want).map(|i| {
        ((i % MAP_WIDTH as usize) as u16, (i / MAP_WIDTH as usize) as u16)
    })
}

/// The player's position and a snapshot of the tiles they have revealed.
fn player_view(world: &mut World) -> Option<(u16, u16, FixedBitSet)> {
    let mut query = world.query_filtered::<(&Position, &Viewshed), With<Player>>();
    let (pos, viewshed) = query.iter(world).next()?;
    Some((pos.x, pos.y, viewshed.revealed_tiles.clone()))
}

/// Breadth-first search across tiles the caller deems `open`, from `(px, py)`,
/// for the nearest tile satisfying `goal`. Returns the first `(dx, dy)` hop of
/// the shortest route, or `None` if no such tile is reachable.
fn first_step<O, G>(px: u16, py: u16, open: O, goal: G) -> Option<(i16, i16)>
where
    O: Fn(u16, u16) -> bool,
    G: Fn(u16, u16) -> bool,
{
    let start = tile_index(px, py);
    let mut prev = vec![usize::MAX; MAP_TILE_COUNT];
    let mut visited = vec![false; MAP_TILE_COUNT];
    visited[start] = true;
    let mut queue: VecDeque<(u16, u16)> = VecDeque::new();
    queue.push_back((px, py));

    let mut found = None;
    'bfs: while let Some((cx, cy)) = queue.pop_front() {
        if (cx != px || cy != py) && goal(cx, cy) {
            found = Some(tile_index(cx, cy));
            break 'bfs;
        }
        for &(dx, dy) in &DIRS {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            let ni = tile_index(nx, ny);
            if visited[ni] || !open(nx, ny) {
                continue;
            }
            visited[ni] = true;
            prev[ni] = tile_index(cx, cy);
            queue.push_back((nx, ny));
        }
    }

    // Walk the predecessor chain back until we sit on the tile right after the
    // player: that first hop is the move to make this turn.
    let mut cur = found?;
    while prev[cur] != start {
        cur = prev[cur];
        if cur == usize::MAX {
            return None; // unreachable in practice; the goal came off the queue
        }
    }
    let tx = (cur % MAP_WIDTH as usize) as i16;
    let ty = (cur / MAP_WIDTH as usize) as i16;
    Some((tx - px as i16, ty - py as i16))
}

/// The single `(dx, dy)` step the player should take toward the closest tile that
/// borders unexplored ground, or `None` when every reachable tile has already
/// been seen. Pure: reads the world, never mutates it.
pub fn explore_step(world: &mut World) -> Option<(i16, i16)> {
    let (px, py, seen) = player_view(world)?;
    let map = world.resource::<Map>();

    let is_seen = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && seen.contains(tile_index(x, y))
    };
    // A tile the search may stand on and route through: revealed and walkable.
    let open = |x: u16, y: u16| -> bool { is_seen(x, y) && !map.blocks(x, y) };

    // An `open` tile that touches at least one still-unseen tile.
    let is_frontier = |x: u16, y: u16| -> bool {
        open(x, y)
            && DIRS.iter().any(|&(dx, dy)| {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                nx >= 0
                    && ny >= 0
                    && nx < MAP_WIDTH as i32
                    && ny < MAP_HEIGHT as i32
                    && !is_seen(nx as u16, ny as u16)
            })
    };

    first_step(px, py, &open, &is_frontier)
}

/// The single `(dx, dy)` step toward `target` over already-revealed, walkable
/// ground, or `None` if the player is already there or no known path reaches it.
pub fn travel_step(world: &mut World, target: (u16, u16)) -> Option<(i16, i16)> {
    let (px, py, seen) = player_view(world)?;
    if (px, py) == target {
        return None;
    }
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && seen.contains(tile_index(x, y)) && !map.blocks(x, y)
    };

    first_step(px, py, &open, |x, y| (x, y) == target)
}
