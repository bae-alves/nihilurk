//! Auto-walk: the `o` command explores the floor, and `>` / `<` away from a
//! staircase you have already found walks you to it. Either way the player moves
//! one step per turn until the goal is reached or something interrupts the walk.
//!
//! The pathfinding here mostly only reads the world — so the engine can own the
//! parts that touch the terminal (polling for a keypress) and the message log,
//! and the interesting logic stays testable. [`explore_step`] is the one
//! exception: it remembers, in [`AutoExplore::frontier`], the unexplored tile
//! it's currently walking toward, so a single-step recompute doesn't abandon
//! an almost-finished approach the instant something else looks marginally
//! closer — see its doc comment.

use std::collections::{HashSet, VecDeque};

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;

use crate::components::*;
use crate::constants::items::PACK_CAPACITY;
use crate::map::{MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, Map, TileType, tile_index};

/// Hard ceiling on steps in a single auto-walk — paranoia against a
/// pathfinding loop. Defined and documented in `constants.rs`.
pub use crate::constants::travel::AUTO_EXPLORE_STEP_CAP;

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
    /// The frontier tile open-ended exploration (`target == None`) is
    /// currently committed to walking toward. [`explore_step`] keeps heading
    /// here — rather than re-picking the globally nearest frontier every
    /// single turn — until it's reached or stops being a frontier, so a room
    /// that's 95% mapped gets finished before something else takes over.
    pub frontier: Option<(u16, u16)>,
}

impl AutoExplore {
    /// Begin an auto-walk: toward `target` if given, otherwise general
    /// exploration.
    pub fn start(&mut self, target: Option<(u16, u16)>) {
        self.active = true;
        self.steps = 0;
        self.target = target;
        self.frontier = None;
    }

    /// Halt any auto-walk.
    pub fn stop(&mut self) {
        self.active = false;
        self.target = None;
        self.frontier = None;
    }
}

/// Whether an auto-explore walk detours to pick things up, toggled in-game with
/// `A`. Transient like [`AutoExplore`] itself — a fresh run, and a reloaded one,
/// starts with the detour on.
///
/// It exists because the detour is a strong rule (a spotted item outranks
/// exploring outright, see [`explore_step`]) and a strong rule needs an off
/// switch: a player clearing a floor for the stairs does not want the walk
/// doubling back for every dart it can see.
#[derive(Resource)]
pub struct AutoPickup {
    pub enabled: bool,
}

impl Default for AutoPickup {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// The eight neighbour offsets, matching the player's movement options.
const DIRS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
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
    let want = if going_down {
        TileType::Downstairs
    } else {
        TileType::Upstairs
    };
    map.tiles.iter().position(|&t| t == want).map(|i| {
        (
            (i % MAP_WIDTH as usize) as u16,
            (i / MAP_WIDTH as usize) as u16,
        )
    })
}

/// The player's position and a snapshot of the tiles they have revealed.
fn player_view(world: &mut World) -> Option<(u16, u16, FixedBitSet)> {
    let mut query = world.query_filtered::<(&Position, &Viewshed), With<Player>>();
    let (pos, viewshed) = query.iter(world).next()?;
    Some((pos.x, pos.y, viewshed.revealed_tiles.clone()))
}

/// Every tile carrying a trap the player already knows about — [`Trap`]s whose
/// [`Hidden`] tag has been lifted, whether by sight or by something else
/// springing it. Auto-walk and fast-move both route around these rather than
/// march the player onto a hazard they've already seen.
pub fn known_trap_tiles(world: &mut World) -> HashSet<(u16, u16)> {
    let mut query = world.query_filtered::<&Position, (With<Trap>, Without<Hidden>)>();
    query.iter(world).map(|p| (p.x, p.y)).collect()
}

/// Every tile holding an item the player has already spotted and would
/// actually take — an [`Item`] still lying on the floor (still carries
/// [`Position`]) whose [`Hidden`] tag has been lifted. [`explore_step`]
/// beelines for these ahead of frontier exploration, so a visible item doesn't
/// get left behind while the walk wanders off toward unseen ground.
///
/// Two kinds are left out, and for the same reason: walking to something the
/// arrival will refuse halts the walk, leaves the thing where it was, and sends
/// the next `o` straight back to it forever.
///
/// * A [`Pickup`] the player has no use for yet — a red coin at full health
///   ([`crate::items::would_help`]). It stays visible and stays skipped until
///   the day it would help, and then the walk goes and gets it.
/// * Everything that needs a pack slot, once the pack is full
///   ([`stowable_room`]). A coin needs no slot, which is why the pack no longer
///   calls the whole detour off.
pub fn known_item_tiles(world: &mut World) -> Vec<(u16, u16)> {
    let Some(player) = world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
    else {
        return Vec::new();
    };
    let room = stowable_room(world);
    let mut query = world.query_filtered::<(Entity, &Position), (With<Item>, Without<Hidden>)>();
    let candidates: Vec<(Entity, (u16, u16))> =
        query.iter(world).map(|(e, p)| (e, (p.x, p.y))).collect();
    candidates
        .into_iter()
        .filter(|&(item, _)| worth_the_walk(world, player, item, room))
        .map(|(_, tile)| tile)
        .collect()
}

/// Whether arriving at `item` would actually achieve something: a pickup has to
/// be one the player can use, and anything else has to have a slot to go into.
fn worth_the_walk(world: &World, player: Entity, item: Entity, room: bool) -> bool {
    match world.get::<Pickup>(item).map(|p| p.effect) {
        Some(effect) => crate::items::would_help(world, player, effect),
        None => room,
    }
}

/// Whether the pack has room for one more thing that needs a slot.
fn stowable_room(world: &mut World) -> bool {
    world
        .query_filtered::<&Backpack, With<Player>>()
        .iter(world)
        .next()
        .map_or(true, |b| b.items.len() < PACK_CAPACITY)
}

/// Whether a walk should detour for loot at all: the `A` toggle
/// ([`AutoPickup`]) and nothing else. What is *worth* detouring for is
/// [`known_item_tiles`]'s question, item by item — a full pack no longer calls
/// the whole thing off, because a coin still gets picked up with a full pack.
fn detours_for_loot(world: &mut World) -> bool {
    world
        .get_resource::<AutoPickup>()
        .map_or(true, |p| p.enabled)
}

/// Breadth-first search across tiles the caller deems `open`, from `(px, py)`,
/// for the nearest tile satisfying `goal`. Returns the first `(dx, dy)` hop of
/// the shortest route, or `None` if no such tile is reachable.
pub(crate) fn first_step<O, S, G>(
    px: u16,
    py: u16,
    open: O,
    step_ok: S,
    goal: G,
) -> Option<(i16, i16)>
where
    O: Fn(u16, u16) -> bool,
    S: Fn(u16, u16, u16, u16) -> bool,
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
            if visited[ni] || !open(nx, ny) || !step_ok(cx, cy, nx, ny) {
                continue;
            }
            visited[ni] = true;
            prev[ni] = tile_index(cx, cy);
            queue.push_back((nx, ny));
        }
    }

    first_hop(px, py, start, found?, &prev)
}

/// Breadth-first search across tiles `open`, from `(px, py)`, for the nearest
/// tile satisfying `goal` — same shortest-path search as [`first_step`], but
/// returning the tile's coordinates rather than the hop toward it.
///
/// `bias`, when given, breaks ties among a node's several open neighbours in
/// the same BFS layer by preferring whichever lies closest to that point, so
/// that when two frontiers are equally near, the one heading toward it wins.
/// [`explore_step`] uses this to favour the still-unseen downstairs.
fn nearest_open_tile<O, S, G>(
    px: u16,
    py: u16,
    open: O,
    step_ok: S,
    goal: G,
    bias: Option<(u16, u16)>,
) -> Option<(u16, u16)>
where
    O: Fn(u16, u16) -> bool,
    S: Fn(u16, u16, u16, u16) -> bool,
    G: Fn(u16, u16) -> bool,
{
    let start = tile_index(px, py);
    let mut visited = vec![false; MAP_TILE_COUNT];
    visited[start] = true;
    let mut queue: VecDeque<(u16, u16)> = VecDeque::new();
    queue.push_back((px, py));

    let dist_to_bias = |x: u16, y: u16| -> i64 {
        let Some((bx, by)) = bias else {
            return 0;
        };
        let dx = x as i64 - bx as i64;
        let dy = y as i64 - by as i64;
        dx * dx + dy * dy
    };

    while let Some((cx, cy)) = queue.pop_front() {
        if (cx != px || cy != py) && goal(cx, cy) {
            return Some((cx, cy));
        }
        let mut candidates: Vec<(u16, u16)> = Vec::new();
        for &(dx, dy) in &DIRS {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            let ni = tile_index(nx, ny);
            if visited[ni] || !open(nx, ny) || !step_ok(cx, cy, nx, ny) {
                continue;
            }
            visited[ni] = true;
            candidates.push((nx, ny));
        }
        // Closest to the bias point first: within the batch this node
        // contributes to the next layer, that one is dequeued soonest.
        candidates.sort_by_key(|&(x, y)| dist_to_bias(x, y));
        queue.extend(candidates);
    }
    None
}

/// Shared tail of [`first_step`]: walks `prev`'s predecessor chain from
/// `found` back to the tile right after the player — that first hop is the
/// move to make this turn.
fn first_hop(px: u16, py: u16, start: usize, found: usize, prev: &[usize]) -> Option<(i16, i16)> {
    let mut cur = found;
    while prev[cur] != start {
        cur = prev[cur];
        if cur == usize::MAX {
            return None; // unreachable in practice; the goal came off the search
        }
    }
    let tx = (cur % MAP_WIDTH as usize) as i16;
    let ty = (cur / MAP_WIDTH as usize) as i16;
    Some((tx - px as i16, ty - py as i16))
}

/// The single `(dx, dy)` step the player should take to keep exploring, or
/// `None` when every reachable tile has already been seen.
///
/// A known item still sitting on the floor (see [`known_item_tiles`]) always
/// wins over frontier exploration: the walk beelines straight for it, `move_player`
/// picks it up on arrival, and only once it's gone does frontier picking resume.
/// [`detours_for_loot`] is what can call that rule off, and it is the `A`
/// toggle and nothing else — whether a *particular* item is worth the walk is
/// [`known_item_tiles`]'s question, item by item.
///
/// Otherwise keeps heading toward [`AutoExplore::frontier`] — the frontier tile
/// it last committed to — for as long as that's still a real frontier, rather
/// than re-picking the globally nearest one fresh every turn. Recomputing
/// "nearest" on every step is what let auto-explore abandon a room that was
/// 95% mapped the moment something elsewhere became marginally closer, only to
/// trek back through it later; sticking to one destination until it's actually
/// reached (or made moot) avoids that. Once a new frontier needs picking, ties
/// toward the still-unseen downstairs when there's a real choice of direction.
///
/// Reads the world like the rest of this module's `*_step` functions, but
/// also writes the frontier it commits to back into [`AutoExplore`].
pub fn explore_step(world: &mut World) -> Option<(i16, i16)> {
    let traps = known_trap_tiles(world);
    let items = detours_for_loot(world)
        .then(|| known_item_tiles(world))
        .unwrap_or_default();
    let (px, py, seen) = player_view(world)?;
    // `AutoExplore` isn't inserted in every test world; treat it as having no
    // committed frontier yet rather than panicking.
    let cached_frontier = world.get_resource::<AutoExplore>().and_then(|a| a.frontier);
    let map = world.resource::<Map>().clone();

    let is_seen = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && seen.contains(tile_index(x, y))
    };
    // A tile the search may stand on and route through: revealed, walkable, and
    // not a trap the player already knows to avoid.
    let open =
        |x: u16, y: u16| -> bool { is_seen(x, y) && !map.blocks(x, y) && !traps.contains(&(x, y)) };

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

    let step_ok = |fx: u16, fy: u16, tx: u16, ty: u16| map.diagonal_step_ok(fx, fy, tx, ty);

    // A spotted item outranks frontier exploration entirely: head straight for
    // the nearest one. Once it's picked up it drops out of `items` and normal
    // exploration takes back over.
    if !items.is_empty() {
        if let Some(hop) = first_step(px, py, &open, step_ok, |x, y| items.contains(&(x, y))) {
            return Some(hop);
        }
    }

    // Still committed to a real frontier: keep walking there.
    if let Some(target) = cached_frontier {
        if is_frontier(target.0, target.1) {
            if let Some(hop) = first_step(px, py, &open, step_ok, |x, y| (x, y) == target) {
                return Some(hop);
            }
        }
    }

    // Time to pick a new one — steer toward the downstairs while they're
    // still unseen, so finishing the floor doesn't end with a separate walk
    // back to find them.
    let bias = stair_location(&map, true).filter(|&(sx, sy)| !is_seen(sx, sy));
    let next = nearest_open_tile(px, py, &open, step_ok, &is_frontier, bias)?;
    if let Some(mut auto) = world.get_resource_mut::<AutoExplore>() {
        auto.frontier = Some(next);
    }
    first_step(px, py, &open, step_ok, |x, y| (x, y) == next)
}

/// Transient UI state for the `O` command: a free-floating cursor the player
/// steers over already-seen ground to pick a travel destination. Confirming
/// hands the tile to [`AutoExplore`] as a travel target. Never serialised.
#[derive(Resource, Default)]
pub struct TravelCursor {
    pub active: bool,
    pub x: u16,
    pub y: u16,
    /// Highlight blink phase; the engine flips it on a timer while active.
    pub blink_on: bool,
}

impl TravelCursor {
    /// Open the cursor at `(x, y)` — normally the player's own tile.
    pub fn open(&mut self, x: u16, y: u16) {
        self.active = true;
        self.x = x;
        self.y = y;
        self.blink_on = true;
    }

    /// Close the cursor.
    pub fn close(&mut self) {
        self.active = false;
    }
}

/// Whether the player has revealed `(x, y)` — i.e. whether the `O` cursor is
/// allowed to rest on it. Pure: reads the world, never mutates it.
pub fn tile_is_revealed(world: &mut World, x: u16, y: u16) -> bool {
    if x >= MAP_WIDTH || y >= MAP_HEIGHT {
        return false;
    }
    let mut q = world.query_filtered::<&Viewshed, With<Player>>();
    q.iter(world)
        .next()
        .is_some_and(|v| v.revealed_tiles.contains(tile_index(x, y)))
}

/// The single `(dx, dy)` step toward `target` over already-revealed, walkable
/// ground, or `None` if the player is already there or no known path reaches it.
pub fn travel_step(world: &mut World, target: (u16, u16)) -> Option<(i16, i16)> {
    let traps = known_trap_tiles(world);
    let (px, py, seen) = player_view(world)?;
    if (px, py) == target {
        return None;
    }
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH
            && y < MAP_HEIGHT
            && seen.contains(tile_index(x, y))
            && !map.blocks(x, y)
            && !traps.contains(&(x, y))
    };

    let step_ok = |fx: u16, fy: u16, tx: u16, ty: u16| map.diagonal_step_ok(fx, fy, tx, ty);
    first_step(px, py, &open, step_ok, |x, y| (x, y) == target)
}

/// The revealed, walkable tile reachable from the player that lies closest to
/// `target` — which may itself be a wall, or in a spot the player cannot get to.
/// Returns the player's own tile when nothing better is reachable. Pure.
pub fn nearest_reachable(world: &mut World, target: (u16, u16)) -> Option<(u16, u16)> {
    let traps = known_trap_tiles(world);
    let (px, py, seen) = player_view(world)?;
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH
            && y < MAP_HEIGHT
            && seen.contains(tile_index(x, y))
            && !map.blocks(x, y)
            && !traps.contains(&(x, y))
    };
    let dist2 = |x: u16, y: u16| -> i64 {
        let dx = x as i64 - target.0 as i64;
        let dy = y as i64 - target.1 as i64;
        dx * dx + dy * dy
    };

    // Flood every open tile the player can reach, remembering the one that ends
    // up nearest the target.
    let start = tile_index(px, py);
    let mut visited = vec![false; MAP_TILE_COUNT];
    visited[start] = true;
    let mut queue: VecDeque<(u16, u16)> = VecDeque::new();
    queue.push_back((px, py));

    let mut best = (px, py);
    let mut best_d = dist2(px, py);

    while let Some((cx, cy)) = queue.pop_front() {
        for &(dx, dy) in &DIRS {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            let ni = tile_index(nx, ny);
            if visited[ni] || !open(nx, ny) || !map.diagonal_step_ok(cx, cy, nx, ny) {
                continue;
            }
            visited[ni] = true;
            let d = dist2(nx, ny);
            if d < best_d {
                best_d = d;
                best = (nx, ny);
            }
            queue.push_back((nx, ny));
        }
    }

    Some(best)
}
