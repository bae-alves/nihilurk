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
    /// Which stained tiles are green (a slime's [`crate::effects::GreenBlood`])
    /// rather than the default red. A subset of `tiles`.
    green: FixedBitSet,
    /// When `false` (the `-nb` flag) no tile is ever stained.
    pub enabled: bool,
}

impl BloodStains {
    /// An empty floor with staining switched on.
    pub fn new() -> Self {
        Self {
            tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            green: FixedBitSet::with_capacity(MAP_TILE_COUNT),
            enabled: true,
        }
    }

    /// Marks the tile at `(x, y)` bloody (unless blood is disabled).
    pub fn stain(&mut self, x: u16, y: u16) {
        self.stain_colored(x, y, false);
    }

    /// Marks the tile at `(x, y)` bloody, `green` choosing a slime's ichor
    /// over the default red (unless blood is disabled).
    ///
    /// ```
    /// use models::BloodStains;
    ///
    /// let mut blood = BloodStains::new();
    /// blood.stain_colored(3, 4, true);
    /// assert!(blood.is_bloody(3, 4) && blood.is_green(3, 4));
    ///
    /// blood.enabled = false; // what `-nb` does
    /// blood.stain(5, 5);
    /// assert!(!blood.is_bloody(5, 5));
    /// ```
    pub fn stain_colored(&mut self, x: u16, y: u16, green: bool) {
        if self.enabled && x < MAP_WIDTH && y < MAP_HEIGHT {
            let idx = tile_index(x, y);
            self.tiles.insert(idx);
            if green {
                self.green.insert(idx);
            }
        }
    }

    /// Whether `(x, y)` is stained, red or green.
    pub fn is_bloody(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.tiles.contains(tile_index(x, y))
    }

    /// Whether the stain at `(x, y)` is a slime's rather than red.
    pub fn is_green(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.green.contains(tile_index(x, y))
    }

    /// Wipes every stain, for a new floor.
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.green.clear();
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
    /// A floor with no corpses on it.
    pub fn new() -> Self {
        Self {
            tiles: FixedBitSet::with_capacity(MAP_TILE_COUNT),
        }
    }

    /// Marks `(x, y)` as holding a corpse.
    ///
    /// ```
    /// use models::Corpses;
    ///
    /// let mut corpses = Corpses::new();
    /// corpses.mark(2, 2);
    /// assert!(corpses.has(2, 2));
    /// corpses.clear();
    /// assert!(!corpses.has(2, 2));
    /// ```
    pub fn mark(&mut self, x: u16, y: u16) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            self.tiles.insert(tile_index(x, y));
        }
    }

    /// Whether a corpse lies on `(x, y)`.
    pub fn has(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.tiles.contains(tile_index(x, y))
    }

    /// Removes every corpse, for a new floor.
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
    /// A floor with no smoke on it.
    pub fn new() -> Self {
        Self {
            turns_left: vec![0; MAP_TILE_COUNT],
        }
    }

    /// Lays (or refreshes) a puff of smoke on `(x, y)`, good for `turns` more
    /// calls to [`Smoke::tick`].
    ///
    /// ```
    /// use models::Smoke;
    ///
    /// let mut smoke = Smoke::new();
    /// smoke.puff(10, 5, 2);
    /// smoke.tick();
    /// assert!(smoke.is_smoky(10, 5));
    /// smoke.tick();
    /// assert!(!smoke.is_smoky(10, 5));
    /// ```
    pub fn puff(&mut self, x: u16, y: u16, turns: u8) {
        if x < MAP_WIDTH && y < MAP_HEIGHT {
            let slot = &mut self.turns_left[tile_index(x, y)];
            *slot = (*slot).max(turns);
        }
    }

    /// Whether a puff still hangs on `(x, y)`.
    pub fn is_smoky(&self, x: u16, y: u16) -> bool {
        x < MAP_WIDTH && y < MAP_HEIGHT && self.turns_left[tile_index(x, y)] > 0
    }

    /// Ages every puff down by one turn — one call per game turn.
    pub fn tick(&mut self) {
        for slot in &mut self.turns_left {
            *slot = slot.saturating_sub(1);
        }
    }

    /// Clears every puff, for a new floor.
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
///
/// Assumes nothing: it is first in the schedule, and a puff's own age is the
/// only state it touches.
pub fn smoke_system(world: &mut World) {
    world.resource_mut::<Smoke>().tick();
}
