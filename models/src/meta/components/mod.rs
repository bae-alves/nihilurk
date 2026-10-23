//! The ECS vocabulary — every [`Component`], [`Resource`] and [`Event`] the
//! game is made of, split by theme.
//!
//! * `identity` — who and where a thing is.
//! * `creatures` — what a creature fights with and moves by.
//! * `items` — items on the floor and in the pack, and their type keys.
//! * `conditions` — conditions, traps and the score.
//! * `events` — intents and their queues.
//! * `ui` — menus and input state.
//! * `run` — state of the run as a whole.
//!
//! Every item is re-exported from here, so `crate::components::Fighter` holds.
//!
//! This module is nouns, not verbs. A component here says *what a thing is* — it
//! has a position, it bleeds, it is a scroll of type X — and nothing about what
//! happens as a result. The verbs live in the systems (`crate::combat`,
//! `crate::ai`, `crate::items`, `crate::visibility`, and so on). If you are
//! looking for "what does a potion of healing do", it is not here; it is in
//! `crate::items` (the `potions` submodule).
//!
//! Two recurring shapes are worth knowing before you read:
//!
//! * **Marker components** carry no data — [`Player`], [`Blood`], [`Curse`],
//!   [`Confused`](crate::Confused). Their presence *is* the fact. A system asks
//!   `world.get::<Blood>(e).is_some()` and that is the whole check.
//!
//! * **Type-key components** — [`Potion`], [`Scroll`], [`Wand`], [`Ring`] — name
//!   *which* one an item is. The key is the item's identity for identification
//!   and for the save file. [`Potion`], [`Scroll`], [`Wand`] and [`Rune`] hold
//!   their catalog row ([`crate::catalog`]), and the row names the handler that
//!   does the work. [`Ring`] holds its row too, and the row turns into the
//!   components that do something.
//!
//! **Transient vs serialised.** Some fields are rebuilt from scratch every frame
//! or every load and are deliberately left out of the save format —
//! [`Viewshed::visible_tiles`], [`Speed::energy`], [`Spotted`], [`DungeonLord`].
//! Each says so in its doc comment. Everything else is expected to round-trip
//! through `crate::saveload`.
//!
//! **Ordering matters for saved types.** Every enum that derives `Serialize`
//! ([`MovementType`], [`Faction`], [`SpeedKind`], [`TrapReveal`],
//! the `*Effect` enums) is written to the save by variant position, and every
//! serialised struct by field order. Append new variants and fields; do not
//! reorder existing ones, or old saves change meaning.

mod conditions;
mod creatures;
mod events;
mod identity;
mod items;
mod run;
mod ui;

pub use conditions::*;
pub use creatures::*;
pub use events::*;
pub use identity::*;
pub use items::*;
pub use run::*;
pub use ui::*;

use crate::Color;
use bevy_ecs::prelude::*;
use fixedbitset::FixedBitSet;
use serde::{Deserialize, Serialize};

use crate::catalog::{ArmorDef, ItemDef, PotionDef, RingDef, ScrollDef, WandDef, WeaponDef};
use crate::constants::hud::LOG_HISTORY_CAP;
use crate::constants::speed::{ACTION_COST, FAST_RATE, NORMAL_RATE, QUICK_RATE, SLOW_RATE};
use crate::effects::{ColdImmune, FireImmune, Grant, Undead};

/// The most a single pack slot will hold before the overflow spills into a
/// second slot. Defined and documented in `constants.rs`; re-exported here so
/// `components::STACK_LIMIT` (and the crate-wide glob) keep resolving.
pub use crate::constants::items::STACK_LIMIT;
