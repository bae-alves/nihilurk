//! Everything the game is made of, flattened onto one crate root.
//!
//! `engine` drives the turn loop and the terminal; every rule, component and
//! system that decides what a turn *does* lives here instead, re-exported so
//! callers write `models::Fighter` rather than reaching into a submodule. Most
//! modules are public for that reason. The exceptions — `combat`, `helpers`,
//! `identify`, `items`, `saveload` — are internal wiring between the public
//! modules, kept private and re-exported piece by piece where a caller
//! actually needs one — see the `helpers` re-export below.
//!
//! # Where things live
//!
//! The source is seven folders. Each has a header that lists its modules.
//!
//! | Folder | Holds |
//! |---|---|
//! | `world` | the map and its generation, visibility, traps, ice, spawning |
//! | `creatures` | the bestiary, bodies, abilities, conditions, spirits, the pack screen |
//! | `mind` | monster AI and rule sets, auto-explore and auto-fight |
//! | `items` | the item catalog and what each kind of item does when used |
//! | `combat` | the opposed roll, deaths, and the effect vocabulary and ledger |
//! | `player` | the player's own turn, fast movement, keys, conducts |
//! | `meta` | components, tuning constants, the save file, score, HUD state |
//!
//! The folders are for the reader. The crate root is for the caller: every
//! module below is re-exported here, so `models::map::Map` and `models::Map`
//! both hold, and moving a file between folders never breaks `engine`.
//!
//! # Where to start
//!
//! * To add content (an item, a monster, a trap, a spell), start from the
//!   recipes in `docs/how-to/`. Content is rows in a table, not code.
//! * To find what a component means, read `meta/components/`. It holds nouns only.
//! * To find what a turn does, read `meta/schedule.rs`: it lists the systems in
//!   order, and each name leads to its module.
//! * To change a number, read `meta/constants/`.

// Every public item says what it is for. Coverage reached 100% and this keeps
// it there: a new `pub` item with no doc is a warning, not a surprise on docs.rs.
#![warn(missing_docs)]

// --- The seven folders ------------------------------------------------------
// Each folder is a private module whose `mod.rs` declares its files. They are
// private because callers never name them: the re-exports below put every file
// at the crate root instead.

mod combat;
mod creatures;
mod items;
mod meta;
mod mind;
pub mod player;
mod world;

// --- Modules at the crate root ----------------------------------------------
// One line per folder, so `models::map` and `models::components` resolve
// without the folder in the path.

pub use creatures::{abilities, body, companion, conditions, pack, spirits};
pub use items::{catalog, equipment};
pub use meta::{
    color, components, constants, hud, leaderboard, particles, pride, schedule, score, shake, state,
};
pub use mind::{agents, ai, autoexplore, autofight};
pub use player::{conduct, fastmove, keymap};
pub use world::{ice, magicmap, map, rect, spawn, traps, visibility};

// Internal wiring: reachable as `crate::saveload` and so on inside the crate,
// but not part of the public surface. `monsters` is exposed through its glob
// below, not by name.
pub(crate) use creatures::monsters;
pub(crate) use items::identify;
#[cfg(test)]
pub(crate) use meta::scratch;
pub(crate) use meta::{particle_core, saveload};

// `helpers` is plumbing and stays private, with three exceptions. `apply_hit`
// and `Hit` are the one place mitigation is decided — every source of harm in
// the game goes through them — so they are part of the crate's surface rather
// than an internal detail.
pub use combat::helpers::{Hit, apply_hit, chebyshev, mark_moved, mob_at};

// --- Items at the crate root ------------------------------------------------
// Globs where a whole module is the API; named lists where only a few items are.

pub use abilities::*;
pub use agents::*;
pub use ai::*;
pub use autoexplore::*;
pub use autofight::*;
pub use body::*;
pub use catalog::*;
pub use color::Color;
pub use combat::*;
pub use companion::*;
pub use components::*;
pub use conditions::*;
pub use conduct::{Conduct, Conducts};
pub use equipment::*;
pub use fastmove::*;
pub use hud::*;
pub use ice::{ColdSlain, IceCube, kick_ice_cube};
pub use identify::*;
pub use items::*;
pub use keymap::*;
pub use leaderboard::*;
pub use magicmap::*;
pub use map::*;
pub use monsters::*;
pub use pack::*;
pub use particles::*;
// The player module is mostly systems the schedule owns; these are the entry
// points `engine` calls when a key is pressed.
pub use player::{
    Hold, StepPlan, announce_special_room_entry, maybe_stumble, pick_up_here, plan_step,
    player_action_system, queue_cast, queue_charge, queue_drop, queue_reach_attack, queue_stairs,
    queue_step, queue_swap_weapon, queue_throw, queue_use, queue_willed_teleport,
};
pub use pride::*;
pub use rect::*;
pub use saveload::*;
pub use schedule::turn_schedule;
pub use score::*;
pub use shake::*;
pub use spawn::*;
pub use spirits::*;
pub use state::*;
pub use traps::*;
pub use visibility::*;

// `effects` lives inside `combat`, which is glob-exported above, so it arrives
// at the root as `models::effects` and its items as `models::Grant` and so on.
pub use effects::*;

// `engine` seeds its worlds with these and should not need its own `rand` pin.
pub use rand::SeedableRng;
pub use rand_chacha::ChaCha12Rng;
