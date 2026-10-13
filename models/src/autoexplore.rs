//! Auto-walk: the `o` command explores the floor, and `>` / `<` away from a
//! staircase you have already found walks you to it. Either way the player moves
//! one step per turn until the goal is reached or something interrupts the walk.
//!
//! The pathfinding here only reads the world — so the engine can own the parts
//! that touch the terminal (polling for a keypress) and the message log, and
//! the interesting logic stays testable. What it remembers between steps is
//! [`LeftBehind`]: the items the player turned down, which the walk no longer
//! goes for.

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
    /// Whether an auto-walk is under way.
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

    /// Whether `o` is under way: an auto-walk with no fixed tile to reach.
    fn exploring(&self) -> bool {
        self.active && self.target.is_none()
    }
}

/// The items auto-explore walks past: ones the player dropped, and ones they
/// saw in a room and walked out of it without. Transient like [`AutoExplore`]:
/// never saved, so a reload forgets it and the walk wants everything again. A
/// new floor starts it empty ([`forget_left_behind`]).
#[derive(Resource, Default)]
pub struct LeftBehind {
    items: HashSet<Entity>,
    /// The floor tiles of the room the player last stood in, until they walk
    /// out of it. A doorway does not count as out.
    room: Option<FixedBitSet>,
}

impl LeftBehind {
    /// Whether the walk leaves `item` where it lies.
    pub fn holds(&self, item: Entity) -> bool {
        self.items.contains(&item)
    }
}

/// The player put `item` down: the walk leaves it there.
pub fn leave_behind(world: &mut World, item: Entity) {
    if let Some(mut left) = world.get_resource_mut::<LeftBehind>() {
        left.items.insert(item);
    }
}

/// The player picked `item` up: should it reach the floor again by a throw or
/// a thief, the walk wants it again.
pub fn take_back(world: &mut World, item: Entity) {
    if let Some(mut left) = world.get_resource_mut::<LeftBehind>() {
        left.items.remove(&item);
    }
}

/// A new floor: nothing on it has been turned down yet, and the player stands
/// in none of the old floor's rooms.
pub fn forget_left_behind(world: &mut World) {
    if let Some(mut left) = world.get_resource_mut::<LeftBehind>() {
        *left = LeftBehind::default();
    }
}

/// Notes the player's step from `from` to `to`. A step out of a room, onto a
/// corridor or into another room, leaves behind every item spotted in it that
/// still lies there, unless `o` took the step: the walk passing an item on its
/// way to a nearer frontier has not turned it down. Call it after the step's
/// pickup, so what the step picked up is not on the floor.
pub fn note_room_step(world: &mut World, from: (u16, u16), to: (u16, u16)) {
    let Some(held) = world
        .get_resource_mut::<LeftBehind>()
        .map(|mut left| left.room.take())
    else {
        return;
    };
    let (now, walked_out_of) = room_step(world.resource::<Map>(), held, from, to);
    let exploring = world
        .get_resource::<AutoExplore>()
        .is_some_and(AutoExplore::exploring);
    let turned_down: Vec<Entity> = match walked_out_of.filter(|_| !exploring) {
        Some(room) => world
            .query_filtered::<(Entity, &Position), (With<Item>, Without<Hidden>)>()
            .iter(world)
            .filter(|(_, p)| room.contains(tile_index(p.x, p.y)))
            .map(|(item, _)| item)
            .collect(),
        None => Vec::new(),
    };
    let mut left = world.resource_mut::<LeftBehind>();
    left.room = now;
    left.items.extend(turned_down);
}

/// The room the player stands in after a step from `from` to `to`, given
/// `held`, the one they stood in before, and the room the step walked out of,
/// if it did. A doorway and water are neither in nor out.
fn room_step(
    map: &Map,
    held: Option<FixedBitSet>,
    from: (u16, u16),
    to: (u16, u16),
) -> (Option<FixedBitSet>, Option<FixedBitSet>) {
    let held = match held {
        Some(room) if room.contains(tile_index(from.0, from.1)) => Some(room),
        held => room_floor(map, from).or(held),
    };
    match map.tile(to.0, to.1) {
        TileType::Door | TileType::Water | TileType::Wall => (held, None),
        _ if held
            .as_ref()
            .is_some_and(|room| room.contains(tile_index(to.0, to.1))) =>
        {
            (held, None)
        }
        _ => (room_floor(map, to), held),
    }
}

/// Every room-floor tile 4-connected to `at` through room floor: one room.
/// `None` off room floor.
fn room_floor(map: &Map, at: (u16, u16)) -> Option<FixedBitSet> {
    let floor = |x: u16, y: u16| {
        matches!(
            map.tile(x, y),
            TileType::Room | TileType::Upstairs | TileType::Downstairs
        )
    };
    if !floor(at.0, at.1) {
        return None;
    }
    let mut room = FixedBitSet::with_capacity(MAP_TILE_COUNT);
    room.insert(tile_index(at.0, at.1));
    let mut stack = vec![at];
    while let Some((x, y)) = stack.pop() {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= MAP_WIDTH as i32 || ny >= MAP_HEIGHT as i32 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            if floor(nx, ny) && !room.put(tile_index(nx, ny)) {
                stack.push((nx, ny));
            }
        }
    }
    Some(room)
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
    /// Whether a walk detours to pick up what it spots.
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
/// screen). Auto-walk refuses to start, and halts, while this is true. The
/// player's [`Helper`] and any other ally are drawn too, and are no reason to
/// stop.
pub fn monster_in_sight(world: &mut World) -> bool {
    let mut query = world
        .query_filtered::<&Faction, (With<Mob>, Without<Hidden>, Without<crate::ice::IceCube>)>();
    query.iter(world).any(|&f| f != Faction::Ally)
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
/// [`Position`]) whose [`Hidden`] tag has been lifted. [`explore_step`] counts
/// these as goals alongside the frontier, nearest first.
///
/// Two kinds are left out, and for the same reason: walking to something the
/// arrival will refuse halts the walk, leaves the thing where it was, and sends
/// the next `o` straight back to it forever.
///
/// * A [`Pickup`] the player has no use for yet — a red coin at full health
///   ([`crate::items::would_help`]). It stays visible and stays skipped until
///   the day it would help, and then the walk goes and gets it.
/// * Everything that needs a pack slot, once the pack is full
///   (`stowable_room`). A coin needs no slot, which is why the pack no longer
///   calls the whole detour off.
///
/// So is what the player turned down, [`LeftBehind`]: there the arrival would
/// take the thing, but the player has said they do not want it.
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
/// Nothing the player has turned down ([`LeftBehind`]) is.
fn worth_the_walk(world: &World, player: Entity, item: Entity, room: bool) -> bool {
    if world
        .get_resource::<LeftBehind>()
        .is_some_and(|left| left.holds(item))
    {
        return false;
    }
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
///
/// `bias`, when given, breaks ties between a node's several open neighbours —
/// several are equally short routes toward `goal` in Chebyshev distance —
/// toward whichever lies closest to that point, same trick as
/// [`nearest_open_tile`]. Without it a mob chasing straight down an open
/// corridor can zig into a side room and back for no reason: any wobble off
/// the direct line is still a shortest path by hop count alone, so plain BFS
/// tie-breaks on queue order, not geometry. Pass the goal itself when there is
/// a single concrete target; `None` where `goal` matches several tiles and no
/// one point is the "right" one to bias toward.
pub(crate) fn first_step<O, S, G>(
    px: u16,
    py: u16,
    open: O,
    step_ok: S,
    goal: G,
    bias: Option<(u16, u16)>,
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

    let dist_to_bias = |x: u16, y: u16| -> i64 {
        let Some((bx, by)) = bias else {
            return 0;
        };
        let dx = x as i64 - bx as i64;
        let dy = y as i64 - by as i64;
        dx * dx + dy * dy
    };

    let mut found = None;
    'bfs: while let Some((cx, cy)) = queue.pop_front() {
        if (cx != px || cy != py) && goal(cx, cy) {
            found = Some(tile_index(cx, cy));
            break 'bfs;
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
            prev[ni] = tile_index(cx, cy);
            candidates.push((nx, ny));
        }
        candidates.sort_by_key(|&(x, y)| dist_to_bias(x, y));
        queue.extend(candidates);
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
            return None;
        }
    }
    let tx = (cur % MAP_WIDTH as usize) as i16;
    let ty = (cur / MAP_WIDTH as usize) as i16;
    Some((tx - px as i16, ty - py as i16))
}

/// The single `(dx, dy)` step the player should take to keep exploring, or
/// `None` when every reachable tile has already been seen.
///
/// Every step heads for the nearest goal from where the player stands now: a
/// frontier tile, or a known item still sitting on the floor (see
/// [`known_item_tiles`]), which `move_player` picks up on arrival. Nothing is
/// remembered from the step before, so a player who moves nearer another goal
/// goes there. `detours_for_loot` takes the items out of the running, and it
/// is the `A` toggle and nothing else — whether a *particular* item is worth
/// the walk is [`known_item_tiles`]'s question, item by item. Goals equally
/// near tie toward the still-unseen downstairs, so finishing the floor does
/// not end with a separate walk back to find them.
pub fn explore_step(world: &mut World) -> Option<(i16, i16)> {
    let traps = known_trap_tiles(world);
    let items = detours_for_loot(world)
        .then(|| known_item_tiles(world))
        .unwrap_or_default();
    let (px, py, seen) = player_view(world)?;
    let swims = crate::helpers::player_swims(world);
    let map = world.resource::<Map>().clone();

    let is_seen = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && seen.contains(tile_index(x, y))
    };
    // A tile the search may stand on and route through: revealed, walkable, and
    // not a trap the player already knows to avoid.
    let open = |x: u16, y: u16| -> bool {
        is_seen(x, y) && map.walkable(x, y, swims) && !traps.contains(&(x, y))
    };

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
    let is_goal = |x: u16, y: u16| items.contains(&(x, y)) || is_frontier(x, y);

    let step_ok = |fx: u16, fy: u16, tx: u16, ty: u16| map.diagonal_step_ok(fx, fy, tx, ty);
    let stairs_bias = stair_location(&map, true).filter(|&(sx, sy)| !is_seen(sx, sy));
    let next = nearest_open_tile(px, py, &open, step_ok, is_goal, stairs_bias)?;
    first_step(px, py, &open, step_ok, |x, y| (x, y) == next, Some(next))
}

/// Transient UI state for the `O` command: a free-floating cursor the player
/// steers over already-seen ground to pick a travel destination. Confirming
/// hands the tile to [`AutoExplore`] as a travel target. Never serialised.
#[derive(Resource, Default)]
pub struct TravelCursor {
    /// Whether the cursor is up and taking the keyboard.
    pub active: bool,
    /// The cursor's column, as a map tile coordinate.
    pub x: u16,
    /// The cursor's row, as a map tile coordinate.
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
    let swims = crate::helpers::player_swims(world);
    if (px, py) == target {
        return None;
    }
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH
            && y < MAP_HEIGHT
            && seen.contains(tile_index(x, y))
            && map.walkable(x, y, swims)
            && !traps.contains(&(x, y))
    };

    let step_ok = |fx: u16, fy: u16, tx: u16, ty: u16| map.diagonal_step_ok(fx, fy, tx, ty);
    first_step(
        px,
        py,
        &open,
        step_ok,
        |x, y| (x, y) == target,
        Some(target),
    )
}

/// The revealed, walkable tile reachable from the player that lies closest to
/// `target` — which may itself be a wall, or in a spot the player cannot get to.
/// Returns the player's own tile when nothing better is reachable. Pure.
pub fn nearest_reachable(world: &mut World, target: (u16, u16)) -> Option<(u16, u16)> {
    let traps = known_trap_tiles(world);
    let (px, py, seen) = player_view(world)?;
    let swims = crate::helpers::player_swims(world);
    let map = world.resource::<Map>();

    let open = |x: u16, y: u16| -> bool {
        x < MAP_WIDTH
            && y < MAP_HEIGHT
            && seen.contains(tile_index(x, y))
            && map.walkable(x, y, swims)
            && !traps.contains(&(x, y))
    };
    let dist2 = |x: u16, y: u16| -> i64 {
        let dx = x as i64 - target.0 as i64;
        let dy = y as i64 - target.1 as i64;
        dx * dx + dy * dy
    };

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

/// What a travel-cursor jump key snaps to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CursorJump {
    /// The up staircase.
    UpStairs,
    /// The down staircase.
    DownStairs,
    /// Monsters in view.
    Monsters,
    /// Items in view.
    Items,
}

/// The next (or previous) known target of `kind`, nearest the
/// player first and wrapping at the ends. A cursor that isn't on a target
/// starts at the nearest (or farthest). Returns the new cursor tile, or `None`
/// when there is none. Monsters and items count only while in view, as the
/// map draws them; stairs count on any revealed tile.
pub fn cursor_jump(
    world: &mut World,
    from: (u16, u16),
    kind: CursorJump,
    forward: bool,
) -> Option<(u16, u16)> {
    let (px, py, _) = player_view(world)?;
    let seen: HashSet<(u16, u16)> = world
        .query_filtered::<&Viewshed, With<Player>>()
        .iter(world)
        .flat_map(|v| v.visible_tiles.iter().copied())
        .collect();
    let mut targets: Vec<(u16, u16)> = match kind {
        CursorJump::UpStairs | CursorJump::DownStairs => {
            stair_location(world.resource::<Map>(), kind == CursorJump::DownStairs)
                .into_iter()
                .collect()
        }
        CursorJump::Items => world
            .query_filtered::<&Position, (With<Item>, Without<Hidden>)>()
            .iter(world)
            .map(|p| (p.x, p.y))
            .filter(|t| seen.contains(t))
            .collect(),
        CursorJump::Monsters => world
            .query_filtered::<&Position, (With<Mob>, Without<Hidden>, Without<Player>)>()
            .iter(world)
            .map(|p| (p.x, p.y))
            .filter(|t| seen.contains(t))
            .collect(),
    };
    targets.retain(|&(x, y)| tile_is_revealed(world, x, y));
    targets.sort_by_key(|&(x, y)| {
        let (dx, dy) = (x as i32 - px as i32, y as i32 - py as i32);
        (dx * dx + dy * dy, y, x)
    });
    targets.dedup();
    if targets.is_empty() {
        return None;
    }
    let n = targets.len();
    let i = match targets.iter().position(|&t| t == from) {
        Some(i) if forward => (i + 1) % n,
        Some(i) => (i + n - 1) % n,
        None if forward => 0,
        None => n - 1,
    };
    Some(targets[i])
}
