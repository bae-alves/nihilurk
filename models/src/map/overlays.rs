//! The mess a floor accumulates: blood, corpses and smoke.
//!
//! Three map-sized overlays, all purely cosmetic, all rebuilt per floor, and
//! **none of them saved** — see the exclusions at the top of
//! `crate::saveload`. Nothing here blocks movement or sight; the renderer
//! paints them and gameplay never asks.

use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;

use super::{MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH, tile_index};

/// Per-tile record of where a bleeding creature (anything with [`Blood`]) has
/// been hurt. Purely cosmetic: the renderer paints these tiles with a red
/// background while they are in the player's viewshed. Rebuilt per floor and not
/// saved, like the map itself.
#[derive(Resource)]
pub struct BloodStains {
    tiles: FixedBitSet,
    /// When `false` (the `-nb` flag) no tile is ever stained.
    pub enabled: bool,
}

impl BloodStains {
    pub fn new() -> Self {
        Self {
            tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            enabled: true,
        }
    }

    /// Marks the tile at `(x, y)` bloody (unless blood is disabled).
    pub fn stain(&mut self, x: u16, y: u16) {
        if self.enabled && x < MAP_WIDTH && y < MAP_HEIGHT {
            self.tiles.insert(tile_index(x, y));
        }
    }

    pub fn is_bloody(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.tiles.contains(tile_index(x, y))
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

impl Default for BloodStains {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a dead creature's corpse has come to rest: a decorative `%` with
/// nothing behind it — not lootable, not steppable-on-specially, just a mark
/// left by [`crate::helpers::death_burst`]. Rebuilt per floor and never saved,
/// like [`BloodStains`]; with blood switched off (`-nb`) this is the *entire*
/// death effect, since the animation that would otherwise fling a corpse here
/// is skipped along with the RNG it would spend.
#[derive(Resource)]
pub struct Corpses {
    tiles: FixedBitSet,
}

impl Corpses {
    pub fn new() -> Self {
        Self {
            tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        }
    }

    /// Marks `(x, y)` as holding a corpse.
    pub fn mark(&mut self, x: u16, y: u16) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            self.tiles.insert(tile_index(x, y));
        }
    }

    pub fn has(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.tiles.contains(tile_index(x, y))
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
    }
}

impl Default for Corpses {
    fn default() -> Self {
        Self::new()
    }
}

/// Lingering smoke from a fire blast — unlike a [`BloodStains`] mark, a puff
/// fades on its own after a few turns instead of staying for the floor's
/// life, DCSS-style. Purely cosmetic: it never blocks movement or sight.
/// Rebuilt per floor and not saved, like the map itself.
#[derive(Resource)]
pub struct Smoke {
    /// Turns left before each tile's puff burns off; 0 means clear.
    turns_left: Vec<u8>,
}

impl Smoke {
    pub fn new() -> Self {
        Self {
            turns_left: vec![0; MAP_TILE_COUNT],
        }
    }

    /// Lays (or refreshes) a puff of smoke on `(x, y)`, good for `turns` more
    /// calls to [`Smoke::tick`].
    pub fn puff(&mut self, x: u16, y: u16, turns: u8) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            let slot = &mut self.turns_left[tile_index(x, y)];
            *slot = (*slot).max(turns);
        }
    }

    pub fn is_smoky(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.turns_left[tile_index(x, y)] > 0
    }

    /// Ages every puff down by one turn — one call per game turn.
    pub fn tick(&mut self) {
        for slot in &mut self.turns_left {
            *slot = slot.saturating_sub(1);
        }
    }

    pub fn clear(&mut self) {
        self.turns_left.fill(0);
    }
}

impl Default for Smoke {
    fn default() -> Self {
        Self::new()
    }
}

/// Schedule step: ages every lingering smoke puff down by one turn.
pub fn smoke_system(world: &mut World) {
    world.resource_mut::<Smoke>().tick();
}
