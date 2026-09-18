//! Where content meets the dungeon.
//!
//! The catalogs next door say what things *are* — a bestiary row, a ring row, a
//! trap row. This module holds the two questions the game asks about them:
//!
//! 1. **What turns up here?** [`DROPS`] is the loot table: one line per
//!    category, a weight and a debut depth. [`roll_item`] draws from it.
//!    Monsters and traps carry the same two dials on their own rows
//!    ([`crate::MonsterDef::pick`], [`crate::TrapDef::pick`]).
//! 2. **Give me one of those.** [`spawn_named`] takes any name the game knows —
//!    `"dragon"`, `"long sword"`, `"scroll of identify"`, `"dart trap"` — and
//!    builds it. It is the only function that needs to exist for a test, a
//!    debug command or a future script to reach every piece of content.
//!
//! Neither knows what a potion is. Both walk the tables, and a table is the
//! only thing you edit to add content.
//!
//! ## The weights are relative, not percentages
//!
//! A weight means nothing on its own; it means something next to its
//! table-mates. Ten is the baseline everything sits at unless it asks
//! otherwise, so a row at 5 is half as common as its neighbours and one at 20
//! is twice. You can add a row without touching any other number — which is the
//! whole reason the loot table is not a list of percentages that must total 100
//! any more.

use std::collections::HashSet;

use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

use crate::catalog::{
    AMMO, ARMORS, COINS, ItemDef, LAUNCHERS, POTIONS, RINGS, SCROLLS, WANDS, WEAPONS,
    spawn_element_of_yoord,
};
use crate::components::{Position, TrapReveal};
use crate::map::Map;
use crate::monsters::{MonsterDef, spawn_monster};
use crate::traps::{TrapBundle, TrapDef};

/// The relic's name, so [`spawn_named`] and the save file agree on it.
pub const ELEMENT_OF_YOORD: &str = "The Element of Yoord";

// ---------------------------------------------------------------------------
// Weighted draws
// ---------------------------------------------------------------------------

/// Picks an index from `weights` in proportion to them. `None` when there is
/// nothing to pick — an empty slice, or every weight zero.
///
/// One roll of the RNG whatever the table's length, so a table can grow without
/// shifting the stream a seeded run depends on any more than adding a row
/// already does.
pub fn pick_weighted(weights: &[u32], rng: &mut ChaCha12Rng) -> Option<usize> {
    let total: u32 = weights.iter().sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.gen_range(0..total);
    for (i, &w) in weights.iter().enumerate() {
        if roll < w {
            return Some(i);
        }
        roll -= w;
    }
    None
}

// ---------------------------------------------------------------------------
// Generic table operations
// ---------------------------------------------------------------------------

/// Every row of `table` a floor at `depth` is allowed to produce.
fn eligible<D: ItemDef>(table: &'static [D], depth: u8) -> Vec<&'static D> {
    table
        .iter()
        .filter(|d| d.min_depth() <= depth.max(1))
        .collect()
}

/// Rolls one row of `table` as a floor drop — enchantment, battery charge,
/// bundle size and all, whatever that category rolls for itself.
fn roll_one<D: ItemDef>(
    world: &mut World,
    rng: &mut ChaCha12Rng,
    depth: u8,
    pos: Position,
    table: &'static [D],
) -> Entity {
    let pool = eligible(table, depth);
    let weights: Vec<u32> = pool.iter().map(|d| d.weight()).collect();
    let idx = pick_weighted(&weights, rng)
        .expect("a category in DROPS always has a row at its own min_depth");
    pool[idx].spawn_as_loot(world, rng, pos)
}

/// Spawns the row of `table` called `name`, exactly as the row describes it.
fn one_named<D: ItemDef>(
    world: &mut World,
    name: &str,
    pos: Position,
    table: &'static [D],
) -> Option<Entity> {
    table
        .iter()
        .find(|d| d.name() == name)
        .map(|d| d.spawn(world, pos))
}

/// The names of every row of `table` a floor at `depth` can produce.
fn names_of<D: ItemDef>(table: &'static [D], depth: u8) -> Vec<&'static str> {
    eligible(table, depth)
        .into_iter()
        .map(|d| d.name())
        .collect()
}

// ---------------------------------------------------------------------------
// The loot table
// ---------------------------------------------------------------------------

/// One share of what a dungeon floor drops: a catalog table, how big its share
/// is, and the first floor it has a share at all.
///
/// The three function pointers are the category's table with its element type
/// erased — a `const` can hold `fn`s but not a `&dyn ItemDef`. Nobody writes
/// them by hand; [`category!`] fills all three in from the table's name.
pub struct DropCategory {
    /// What this category is called, for the `--content` listing and for
    /// error messages. Not a row name — no item is called "scroll".
    pub name: &'static str,
    /// This category's share of a floor's drops, against the other categories.
    pub weight: u32,
    /// The shallowest floor this category drops on at all.
    pub min_depth: u8,
    /// Roll one drop from this category's table.
    roll: fn(&mut World, &mut ChaCha12Rng, u8, Position) -> Entity,
    /// Spawn the named row from this category's table, if it holds one.
    by_name: fn(&mut World, &str, Position) -> Option<Entity>,
    /// The rows of this category's table available at a depth.
    rows: fn(u8) -> Vec<&'static str>,
}

impl DropCategory {
    /// Whether a floor at `depth` can draw anything at all from this category.
    fn available(&self, depth: u8) -> bool {
        self.min_depth <= depth.max(1) && !(self.rows)(depth).is_empty()
    }

    /// Every row this category can produce at `depth`.
    pub fn rows(&self, depth: u8) -> Vec<&'static str> {
        (self.rows)(depth)
    }
}

/// Builds a [`DropCategory`] row from a catalog table's name. The closures are
/// non-capturing, so they are plain `fn` pointers and the whole table stays
/// `const`.
macro_rules! category {
    ($name:literal, $weight:expr, $min_depth:expr, $table:ident) => {
        DropCategory {
            name: $name,
            weight: $weight,
            min_depth: $min_depth,
            roll: |world, rng, depth, pos| roll_one(world, rng, depth, pos, $table),
            by_name: |world, name, pos| one_named(world, name, pos, $table),
            rows: |depth| names_of($table, depth),
        }
    };
}

/// What a dungeon floor drops, and how often.
///
/// The weights are Rogue's own category odds in tenths of a percent, food
/// swapped for coins, with the armoury split three ways: a melee weapon is 3.6%
/// of all drops, a bundle of ammunition 2.8%, a launcher 1.6%. Launchers are
/// the rarest on purpose — one bow is a build, two are clutter.
///
/// | Category | Weight | Share |
/// |----------|--------|-------|
/// | scroll   | 300    | 30.0% |
/// | potion   | 270    | 27.0% |
/// | coin     | 170    | 17.0% |
/// | armor    |  80    |  8.0% |
/// | wand     |  50    |  5.0% |
/// | ring     |  50    |  5.0% |
/// | weapon   |  36    |  3.6% |
/// | ammo     |  28    |  2.8% |
/// | launcher |  16    |  1.6% |
///
/// The share column is what these weights happen to work out to today; it is
/// not a thing you have to maintain. Add a category and every share moves,
/// which is the point.
#[rustfmt::skip]
pub const DROPS: &[DropCategory] = &[
    //         name        weight  min_depth  table
    category!("scroll",      300,      1,     SCROLLS),
    category!("potion",      270,      1,     POTIONS),
    category!("coin",        170,      1,     COINS),
    category!("armor",        80,      1,     ARMORS),
    category!("wand",         50,      1,     WANDS),
    category!("ring",         50,      1,     RINGS),
    category!("weapon",       36,      1,     WEAPONS),
    category!("ammo",         28,      1,     AMMO),
    category!("launcher",     16,      1,     LAUNCHERS),
];

/// Rolls one floor drop for a floor at `depth` and spawns it at `pos`: a
/// weighted draw for the category, then a weighted draw within it.
///
/// This is the only place the dungeon decides what loot exists, so a new
/// category is one line in [`DROPS`] and no arithmetic anywhere.
pub fn roll_item(world: &mut World, rng: &mut ChaCha12Rng, depth: u8, pos: Position) -> Entity {
    let pool: Vec<&DropCategory> = DROPS.iter().filter(|c| c.available(depth)).collect();
    let weights: Vec<u32> = pool.iter().map(|c| c.weight).collect();
    let idx = pick_weighted(&weights, rng).expect("DROPS always has a depth-1 category");
    (pool[idx].roll)(world, rng, depth, pos)
}

// ---------------------------------------------------------------------------
// Spawning anything by name
// ---------------------------------------------------------------------------

/// Spawns whatever the game knows by that name at `pos`, or `None` if it knows
/// nothing by it. Monsters, every item category, traps and the relic — one door
/// to all of it.
///
/// What comes out is the thing *exactly as its row describes it*: no
/// enchantment roll, no battery charge, no ammunition bundle. That is what a
/// test and a debug spawn both want. The dungeon's own randomised version is
/// [`roll_item`].
///
/// ```no_run
/// # use bevy_ecs::prelude::World;
/// # use models::{spawn_named, Position};
/// # let mut world = World::new();
/// # let pos = Position { x: 10, y: 10 };
/// let dragon = spawn_named(&mut world, "dragon", pos).unwrap();
/// let sword = spawn_named(&mut world, "long sword", pos).unwrap();
/// assert!(spawn_named(&mut world, "sandwich", pos).is_none());
/// ```
pub fn spawn_named(world: &mut World, name: &str, pos: Position) -> Option<Entity> {
    if let Some(def) = MonsterDef::lookup(name) {
        return Some(spawn_monster(world, def, pos));
    }
    for category in DROPS {
        if let Some(entity) = (category.by_name)(world, name, pos) {
            return Some(entity);
        }
    }
    if let Some(def) = TrapDef::lookup(name) {
        return Some(
            world
                .spawn(TrapBundle::from_def(def, TrapReveal::Sight, pos))
                .id(),
        );
    }
    if name == ELEMENT_OF_YOORD {
        return Some(spawn_element_of_yoord(world, pos));
    }
    None
}

/// Every name [`spawn_named`] answers to, grouped and in table order:
/// `("monster", "dragon")`, `("scroll", "scroll of identify")`, and so on.
/// Used by the `--content` listing and by the test that proves no two rows
/// share a name.
pub fn content_names() -> Vec<(&'static str, &'static str)> {
    let mut names: Vec<(&'static str, &'static str)> = crate::monsters::BESTIARY
        .iter()
        .map(|m| ("monster", m.name))
        .collect();
    for category in DROPS {
        names.extend(
            category
                .rows(u8::MAX)
                .into_iter()
                .map(|n| (category.name, n)),
        );
    }
    names.extend(crate::traps::TRAPS.iter().map(|t| ("trap", t.name)));
    names.push(("relic", ELEMENT_OF_YOORD));
    names
}

// ---------------------------------------------------------------------------
// The content author's shortcut
// ---------------------------------------------------------------------------

/// The `NIHILURK_SPAWN` dev override: a comma-separated list of names dropped
/// around the player the moment a floor is built, so a new row can be looked at
/// without playing down to the depth that would produce it.
///
/// ```text
/// NIHILURK_SPAWN="dragon,bow,arrow,ring of protection" cargo run -p engine
/// ```
///
/// A name the tables do not know is skipped in silence — this is a debug knob,
/// not a parser. Placement walks outward from `near` and takes the first free
/// walkable tile, so nothing lands in a wall or on top of anything else.
pub fn spawn_requested(world: &mut World, near: Position, occupied: &mut HashSet<(u16, u16)>) {
    let Ok(list) = std::env::var("NIHILURK_SPAWN") else {
        return;
    };
    spawn_list(world, &list, near, occupied);
}

/// The half of [`spawn_requested`] that does not read the environment: spawns
/// each comma-separated name in `list` on its own free tile around `near`, and
/// returns how many the tables recognised.
pub fn spawn_list(
    world: &mut World,
    list: &str,
    near: Position,
    occupied: &mut HashSet<(u16, u16)>,
) -> usize {
    let mut spawned = 0;
    for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let Some(pos) = free_tile_near(world, near, occupied) else {
            break;
        };
        if spawn_named(world, name, pos).is_some() {
            occupied.insert((pos.x, pos.y));
            spawned += 1;
        }
    }
    spawned
}

/// The nearest free walkable tile to `origin`, searching in widening rings.
fn free_tile_near(
    world: &World,
    origin: Position,
    occupied: &HashSet<(u16, u16)>,
) -> Option<Position> {
    let map = world.get_resource::<Map>()?;
    for radius in 0..12i32 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue; // only the ring's edge; the inside was already tried
                }
                let (x, y) = (origin.x as i32 + dx, origin.y as i32 + dy);
                if x < 0 || y < 0 {
                    continue;
                }
                let (x, y) = (x as u16, y as u16);
                if map.blocks(x, y) || occupied.contains(&(x, y)) {
                    continue;
                }
                return Some(Position { x, y });
            }
        }
    }
    None
}
