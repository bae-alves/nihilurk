//! The effect vocabulary: the one language monsters, gear and systems all speak.
//!
//! A property a creature has — burning doesn't hurt it, it sees invisible
//! things, it hits harder — is a **component on the creature**, never an enum a
//! subsystem has to recognise. A dragon born fire-immune and a player wearing a
//! ring of fire resistance both carry [`FireImmune`], so the wand-of-fire code
//! asks one question and never learns that rings exist.
//!
//! Two kinds of effect live here:
//!
//! * **Markers** ([`FireImmune`], [`SeesInvisible`], …) — a yes/no property,
//!   read as a plain query filter: `Query<&Viewshed, With<SeesInvisible>>`.
//! * **Modifiers** ([`PowerDie`], [`ArmorBonus`], …) — a number that stacks.
//!   Combat folds every equipped source together with [`equipped_total`]
//!   without caring whether it came from a sword, plate mail or a ring.
//! * **Caps** ([`MeleeCap`]) — a ceiling the dice cannot beat. Folded with
//!   `min` rather than `+`, since the strictest one wins.
//!
//! Const tables (the bestiary, the ring catalog) can't hold components
//! directly, so they name them through [`Grant`]: `Grant::of::<FireImmune>()`
//! is a const handle that knows how to attach, detach and probe one effect.
//! That single handle serves all four places an effect moves: a monster born
//! with it, a ring granting it while worn, a wand of cancellation stripping it,
//! and a save file recording it.

mod ledger;
mod markers;
mod modifiers;
mod registry;
mod status;
mod upkeep;

pub use ledger::*;
pub use markers::*;
pub use modifiers::*;
pub use registry::*;
pub use status::*;
pub use upkeep::*;

use bevy_ecs::prelude::*;
use bevy_ecs::world::{EntityRef, EntityWorldMut};
use std::any::TypeId;

use crate::components::{Fighter, GameLog, Player};
use crate::constants::combat::CHIP_DAMAGE;
