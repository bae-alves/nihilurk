//! The dungeon floor: the terrain itself, and everything that happens to one.
//!
//! This file is the **terrain resource and nothing else** — [`Map`], the
//! [`TileType`] it is made of, and the handful of questions every other system
//! asks it ("is that a wall?", "can I cut that corner?"). Terrain is one
//! `Vec<TileType>` rather than an entity per tile: a 1,760-entity floor with a
//! `Renderable` each would cost more to iterate every frame than the whole rest
//! of the world put together.
//!
//! Everything a floor *does* lives in a submodule, the same way `crate::items`
//! is split by what the player is doing to an item:
//!
//! * [`streams`] — the three RNGs, and the per-floor ones derived from a seed.
//! * [`generate`] — carving a floor: rooms, corridors, stairs, the dark.
//! * [`special`] — carving the floors that are not rooms and corridors at all.
//! * [`population`] — what stands on one once it is carved.
//! * [`levels`] — moving between floors, and starting a run.
//! * [`overlays`] — the cosmetic mess a floor accumulates.
//!
//! All but `special`, which only `generate` calls, are re-exported here, so
//! `models::change_level` and
//! `map::BloodStains` keep resolving and a caller never has to know which file
//! a thing came from.

use crossterm::style::Color;
use fixedbitset::FixedBitSet;

use bevy_ecs::prelude::*;
use rand::Rng;
use rand_chacha::ChaCha12Rng;

mod generate;
mod levels;
mod overlays;
mod population;
mod special;
mod streams;

pub use generate::{create_map, regenerate_map};
pub use levels::*;
pub use overlays::*;
pub use population::difficulty_tier;
pub use streams::*;

pub(crate) use levels::{LevelChange, transition_level, win_with_style};

// --- Tuning constants -------------------------------------------------------
// All defined and documented in `constants.rs`; re-exported here so the old
// paths (`map::MAP_WIDTH`, `map::FINAL_DEPTH`, ...) keep resolving.
//
//   MAP_WIDTH / MAP_HEIGHT   playfield size in tiles (pins seed layout)
//   FINAL_DEPTH              deepest floor; holds the Element of Yoord
//   DUNGEON_LORD_PATIENCE    turns per level before the forced portal
pub use crate::constants::map::{HEIGHT as MAP_HEIGHT, WIDTH as MAP_WIDTH};
pub use crate::constants::progression::{DUNGEON_LORD_PATIENCE, FINAL_DEPTH};

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum TileType {
    Wall,
    Room,
    Passage,
    Door,
    Upstairs,
    Downstairs,
    /// Deep water: floor to anything that [`Swims`](crate::effects::Swims),
    /// a wall to anything that does not, and nothing at all to sight or a
    /// shot. See [`Map::walkable`].
    Water,
}

/// One of the four special room kinds `build_tiles` rolls a chance for on
/// every room past the start that holds no staircase, alongside (and
/// mutually exclusive with) the dark-room roll — see
/// `models/src/map/generate.rs::roll_special_rooms`.
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpecialRoom {
    /// Entirely coin, guarded by `tier + 1` forced dragons.
    DragonHoard,
    /// Every open tile holds a monster.
    MonsterZoo,
    /// Entirely coin but for one tile, which holds a single apis.
    TreasureHive,
    /// A normal item budget that collapses to whatever was picked first the
    /// moment anything in the room is taken.
    RedRoom,
}

/// A floor that is not Rogue's 3x3 of rooms at all. Rolled per floor from
/// the seed on depths [`SPECIAL_LEVEL_MIN_DEPTH`] up to the one above the
/// last — see `models/src/map/special.rs` for the roll and for how each kind
/// is carved. A special level never holds a [`SpecialRoom`].
///
/// [`SPECIAL_LEVEL_MIN_DEPTH`]: crate::constants::map::SPECIAL_LEVEL_MIN_DEPTH
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpecialLevel {
    /// One lit room, wall to wall: the whole floor in view from the stairs,
    /// twice the monsters and twice the loot.
    Battlefield,
    /// A maze of passages with a few loops knocked through it, and no room
    /// anywhere to light it.
    Labyrinth,
    /// A white honeycomb of rooms, each joined to every neighbour by one
    /// door, with the stairs in two of its four corners: twice the monsters,
    /// thrice the treasure.
    Vault,
    /// Land in the middle of deep water, all of it in view: swimmers in the
    /// water, the usual crowd on the shore.
    Island,
    /// One yellow cave, every monster in it an apis — as many budgets of them
    /// as the floor's difficulty tier — and thrice the treasure.
    BeeWorld,
    /// An ordinary floor with a castle in the middle of it: a 7x7 keep with a
    /// dragon inside and a walled 5x5 tower on each corner, every one of the
    /// five stocked like a whole floor of its own. It arrives without a word.
    Castle,
}

impl SpecialLevel {
    /// The kind a `NIHILURK_LEVEL` value names, however it was cased.
    pub fn named(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "battlefield" => Some(Self::Battlefield),
            "labyrinth" => Some(Self::Labyrinth),
            "vault" => Some(Self::Vault),
            "bee" | "beeworld" | "bee world" => Some(Self::BeeWorld),
            "castle" => Some(Self::Castle),
            "island" => Some(Self::Island),
            _ => None,
        }
    }
}

/// The lines the log gets on arriving on a special level — none for the
/// kinds that say enough by what they look like. `rng` only picks which quote
/// the bee world greets you with, so it is the cosmetic [`FxRng`]'s.
pub fn special_level_arrival(level: SpecialLevel, rng: &mut ChaCha12Rng) -> Vec<&'static str> {
    match level {
        SpecialLevel::Labyrinth => vec![strings::labyrinth_arrival()],
        SpecialLevel::Vault => vec![strings::vault_arrival()],
        SpecialLevel::BeeWorld => {
            let quotes = strings::bee_world_quotes();
            vec![
                strings::bee_world_arrival(),
                quotes[rng.gen_range(0..quotes.len())],
            ]
        }
        SpecialLevel::Battlefield | SpecialLevel::Castle | SpecialLevel::Island => Vec::new(),
    }
}

/// The tint a special level paints its walls and floor, if it has one.
fn special_level_color(level: SpecialLevel) -> Option<Color> {
    match level {
        SpecialLevel::Vault => Some(Color::White),
        SpecialLevel::BeeWorld => Some(Color::Yellow),
        SpecialLevel::Battlefield
        | SpecialLevel::Labyrinth
        | SpecialLevel::Castle
        | SpecialLevel::Island => None,
    }
}

/// The wall/floor tint a special room's kind paints its tiles — distinct from
/// each other and from the default wall (`DarkYellow`) and floor (`Green`).
fn special_room_color(kind: SpecialRoom) -> Color {
    match kind {
        SpecialRoom::DragonHoard => Color::Grey,
        SpecialRoom::MonsterZoo => Color::DarkGreen,
        SpecialRoom::TreasureHive => Color::Yellow,
        SpecialRoom::RedRoom => Color::Red,
    }
}

/// The dungeon terrain for the current floor: one [`TileType`] per coordinate,
/// row-major (see [`tile_index`]). This is the single source of truth for the
/// map — terrain is no longer stored as one ECS entity per tile.
#[derive(Resource, Clone)]
pub struct Map {
    pub tiles: Vec<TileType>,
    /// One bit per tile: set on the floor of a "dark" room. Visibility inside a
    /// dark room is cut to the always-on 3x3 (as if it were a passage) until a
    /// wand of light is zapped there, which clears the bits for the whole room.
    /// Rolled deterministically from the seed in [`build_tiles`]; the cleared
    /// state is persisted in the save file.
    pub dark: FixedBitSet,
    /// One [`SpecialRoom`] per tile, `Some` only on that room's own floor
    /// tiles. Rolled deterministically from the seed in [`build_tiles`] the
    /// same way `dark` is, and — unlike `dark` — never mutated after, so it
    /// never needs a line in the save file: `regenerate_map` rebuilds it
    /// exactly on load, same as it rebuilds `tiles` itself.
    pub special: Vec<Option<SpecialRoom>>,
    /// Which [`SpecialLevel`] this floor is, if any. Rebuilt from the seed
    /// with the rest of the layout, so — like `special` — it never needs a
    /// line in the save file.
    pub level: Option<SpecialLevel>,
}

impl Map {
    /// The tile at `(x, y)`. Out-of-bounds coordinates read as solid [`TileType::Wall`]
    /// so callers can probe freely without bounds checks.
    #[inline]
    pub fn tile(&self, x: u16, y: u16) -> TileType {
        if x >= MAP_WIDTH || y >= MAP_HEIGHT {
            return TileType::Wall;
        }
        self.tiles[tile_index(x, y)]
    }

    /// Whether `(x, y)` blocks movement.
    #[inline]
    pub fn blocks(&self, x: u16, y: u16) -> bool {
        self.tile(x, y) == TileType::Wall
    }

    /// Whether a walker can stand on `(x, y)` — deep water only if it
    /// `swims`. [`Map::blocks`] answers for sight and for anything thrown;
    /// this answers for feet.
    #[inline]
    pub fn walkable(&self, x: u16, y: u16, swims: bool) -> bool {
        match self.tile(x, y) {
            TileType::Wall => false,
            TileType::Water => swims,
            _ => true,
        }
    }

    /// Whether `(x, y)` is the floor of a still-unlit dark room.
    #[inline]
    pub fn is_dark(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.dark.contains(tile_index(x, y))
    }

    /// Clears the dark flag for a single tile (a wand of light sweeping a room).
    #[inline]
    pub fn light_tile(&mut self, x: u16, y: u16) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            self.dark.set(tile_index(x, y), false);
        }
    }

    /// Whether a single step from `(fx, fy)` to `(tx, ty)` is allowed by the
    /// diagonal-movement rule. Orthogonal steps always pass; a diagonal step is
    /// only allowed between two tiles of the same kind — you can round a bend in
    /// a passage, but you can't cut the corner of a doorway or slip diagonally
    /// between a room and a corridor. Says nothing about walls or occupants;
    /// combine with [`Map::blocks`].
    #[inline]
    pub fn diagonal_step_ok(&self, fx: u16, fy: u16, tx: u16, ty: u16) -> bool {
        if fx == tx || fy == ty {
            return true; // orthogonal (or no move)
        }
        tile_kind(self.tile(fx, fy)) == tile_kind(self.tile(tx, ty))
    }

    /// Whether the wall at `(x, y)` bounds a room (or a doorway into one). Rogue
    /// only draws these; the loose walls hugging a corridor are left as blank
    /// space so passages read as tunnels through the dark rather than trenches.
    pub fn is_room_wall(&self, x: u16, y: u16) -> bool {
        if self.tile(x, y) != TileType::Wall {
            return false;
        }
        // Only room floor (and the stairs that sit on it) makes a wall worth
        // drawing. A wall that merely touches a Door — but no room tile — is
        // outside the room, hugging the corridor, and stays dark.
        NEIGHBOUR_DIRS.iter().any(|&(dx, dy)| {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            nx >= 0
                && ny >= 0
                && matches!(
                    self.tile(nx as u16, ny as u16),
                    TileType::Room | TileType::Upstairs | TileType::Downstairs
                )
        })
    }

    /// The [`SpecialRoom`] kind of the room `(x, y)`'s floor belongs to, or
    /// `None` off one entirely. Bounds-checked the same way [`Map::tile`] is.
    #[inline]
    pub fn special_kind(&self, x: u16, y: u16) -> Option<SpecialRoom> {
        if x >= MAP_WIDTH || y >= MAP_HEIGHT {
            return None;
        }
        self.special[tile_index(x, y)]
    }

    /// Every tile 4-connected to `(x, y)` through the same [`SpecialRoom`]
    /// kind — one physical room, not every tile of that kind on the floor.
    /// `(x, y)` itself must already be that kind, or this returns empty.
    pub fn special_room_tiles(&self, x: u16, y: u16) -> Vec<(u16, u16)> {
        let Some(kind) = self.special_kind(x, y) else {
            return Vec::new();
        };
        let mut seen = vec![false; self.special.len()];
        let mut stack = vec![(x, y)];
        seen[tile_index(x, y)] = true;
        let mut room = Vec::new();
        while let Some((cx, cy)) = stack.pop() {
            room.push((cx, cy));
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                if nx < 0 || ny < 0 || nx as u16 >= MAP_WIDTH || ny as u16 >= MAP_HEIGHT {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                let nidx = tile_index(nx, ny);
                if !seen[nidx] && self.special[nidx] == Some(kind) {
                    seen[nidx] = true;
                    stack.push((nx, ny));
                }
            }
        }
        room
    }

    /// The tint a special level paints `(x, y)` with if it is a wall or floor
    /// tile of one, else the tint a special room paints it with, whether it's
    /// one of the room's own floor tiles or a wall bounding it (reusing
    /// [`Map::is_room_wall`]'s neighbour scan) — or `None` off both, which
    /// leaves the tile at its ordinary [`tile_appearance`].
    pub fn special_tint(&self, x: u16, y: u16) -> Option<Color> {
        let level_color = self.level.and_then(special_level_color);
        if level_color.is_some() && matches!(self.tile(x, y), TileType::Wall | TileType::Room) {
            return level_color;
        }
        if let Some(kind) = self.special_kind(x, y) {
            return Some(special_room_color(kind));
        }
        if !self.is_room_wall(x, y) {
            return None;
        }
        NEIGHBOUR_DIRS
            .iter()
            .find_map(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 {
                    return None;
                }
                self.special_kind(nx as u16, ny as u16)
            })
            .map(special_room_color)
    }
}

/// The one-line flavor a special room announces the instant the player steps
/// onto its floor from anywhere else — `None` past the threshold (repeat
/// steps inside the same room stay silent) and `None` for [`SpecialRoom::MonsterZoo`],
/// which was never given one: getting swarmed says enough on its own.
pub fn special_room_entry_message(
    map: &Map,
    old: (u16, u16),
    new: (u16, u16),
) -> Option<&'static str> {
    let kind = map.special_kind(new.0, new.1)?;
    if map.special_kind(old.0, old.1) == Some(kind) {
        return None;
    }
    match kind {
        SpecialRoom::DragonHoard => Some(strings::dragon_hoard_enter()),
        SpecialRoom::MonsterZoo => None,
        SpecialRoom::TreasureHive => Some(strings::treasure_hive_enter()),
        SpecialRoom::RedRoom => Some(strings::red_room_enter()),
    }
}

/// Coarse grouping of tiles for the diagonal-movement rule: room floor, the
/// staircases standing on it and the water lapping at it count as one kind,
/// so a diagonal step onto stairs — or off a shore — still works. Walls get their own bucket but never matter — [`Map::blocks`]
/// rejects them first.
fn tile_kind(t: TileType) -> u8 {
    match t {
        TileType::Room | TileType::Upstairs | TileType::Downstairs | TileType::Water => 0,
        TileType::Passage => 1,
        TileType::Door => 2,
        TileType::Wall => 3,
    }
}

/// The glyph and lit colour used to draw a terrain tile.
pub fn tile_appearance(t: TileType) -> (char, Color) {
    match t {
        TileType::Room => ('.', Color::Green),
        TileType::Passage => ('▒', Color::White),
        TileType::Wall => ('#', Color::DarkYellow),
        TileType::Door => ('+', Color::Yellow),
        TileType::Downstairs => ('>', Color::Cyan),
        TileType::Upstairs => ('<', Color::Cyan),
        TileType::Water => ('≈', Color::Blue),
    }
}

/// A floor's rooms as population sees them: each one nothing but its own
/// floor tiles, the start room first — whatever shape carved the floor.
pub type Rooms = Vec<Vec<(u16, u16)>>;

/// Total tile count; the length of a fog-of-war bitset.
pub const MAP_TILE_COUNT: usize = MAP_WIDTH as usize * MAP_HEIGHT as usize;

/// Flattens a tile coordinate into a bitset/array index.
#[inline]
pub const fn tile_index(x: u16, y: u16) -> usize {
    y as usize * MAP_WIDTH as usize + x as usize
}

/// The eight neighbouring offsets, in row-major read order.
const NEIGHBOUR_DIRS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
