//! Scroll of magic mapping: the whole floor's layout floods into the player's
//! memory at once.
//!
//! Rather than snap the map on in a single frame, the reveal wipes down the
//! screen one row at a time — roog's take on the row-by-row `MagicMapReveal`
//! run-state from the "Rust Roguelike" tutorial (chapter 20). There is no
//! run-state machine here, so the sweep is driven the same way the particle
//! layer is: [`crate::items::apply_scroll_effect`] arms [`MagicMapReveal`] while
//! resolving the read, and once the turn is over the engine plays the wipe out
//! over a handful of frames, folding one [`magic_map_reveal_step`] into the
//! player's fog-of-war memory per frame and re-rendering between.
//!
//! The reveal only touches terrain memory (the `revealed_tiles` bitset on the
//! player's [`Viewshed`]). Monsters and floor items still only draw where the
//! player can actually *see*, so a mapped-but-unvisited room reads as bare
//! architecture — exactly like the tutorial's version.

use bevy_ecs::prelude::*;

use crate::components::{Player, Viewshed};
use crate::map::{tile_index, MAP_HEIGHT, MAP_TILE_COUNT, MAP_WIDTH};

/// Transient: armed the turn a scroll of magic mapping is read, cleared once the
/// wipe has swept past the last row. Never serialised — a save written mid-wipe
/// reloads with every row already committed to memory, so there is nothing left
/// to animate.
#[derive(Resource, Default)]
pub struct MagicMapReveal {
    pub active: bool,
    /// The next map row the wipe will commit to fog-of-war memory.
    pub next_row: u16,
}

impl MagicMapReveal {
    /// Arm the sweep, starting from the top row.
    pub fn start(&mut self) {
        self.active = true;
        self.next_row = 0;
    }

    /// Disarm the sweep.
    pub fn stop(&mut self) {
        self.active = false;
        self.next_row = 0;
    }
}

/// Fold the next row of the map into the player's revealed-tiles memory and
/// advance the sweep. Returns `true` while the wipe is still running and `false`
/// once every row has been revealed (at which point [`MagicMapReveal`] has been
/// switched back off).
///
/// The engine calls this once per animation frame, exactly like it ages the
/// particle layer one frame at a time. A no-op returning `false` when no reveal
/// is armed.
pub fn magic_map_reveal_step(world: &mut World) -> bool {
    let row = {
        let mm = match world.get_resource::<MagicMapReveal>() {
            Some(mm) if mm.active => mm,
            _ => return false,
        };
        mm.next_row
    };

    if row >= MAP_HEIGHT {
        world.resource_mut::<MagicMapReveal>().stop();
        return false;
    }

    let mut query = world.query_filtered::<&mut Viewshed, With<Player>>();
    if let Some(mut viewshed) = query.iter_mut(world).next() {
        if viewshed.revealed_tiles.len() < MAP_TILE_COUNT {
            viewshed.revealed_tiles.grow(MAP_TILE_COUNT);
        }
        for x in 0..MAP_WIDTH {
            viewshed.revealed_tiles.insert(tile_index(x, row));
        }
    }

    world.resource_mut::<MagicMapReveal>().next_row = row + 1;
    true
}

/// Reveal the whole map immediately, skipping the animation. Used when the
/// player cuts the wipe short with a keypress, or anywhere the sweep needs to be
/// forced to completion.
pub fn finish_magic_map_reveal(world: &mut World) {
    while magic_map_reveal_step(world) {}
}
