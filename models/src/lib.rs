//! Everything the game is made of, flattened onto one crate root.
//!
//! `engine` drives the turn loop and the terminal; every rule, component and
//! system that decides what a turn *does* lives here instead, re-exported so
//! callers write `models::Fighter` rather than reaching into a submodule. Most
//! modules are public for that reason. The exceptions — `combat`, `helpers`,
//! `identify`, `items`, `saveload` — are internal wiring between the public
//! modules, kept private and re-exported piece by piece where a caller
//! actually needs one — see the `helpers` re-export below.

// Every public item says what it is for. Coverage reached 100% and this keeps
// it there: a new `pub` item with no doc is a warning, not a surprise on docs.rs.
#![warn(missing_docs)]

pub mod abilities;
pub mod agents;
pub mod ai;
pub mod autoexplore;
pub mod autofight;
pub mod body;
pub mod bones;
pub mod catalog;
pub mod companion;
pub mod components;
pub mod conditions;
pub mod constants;
pub mod effects;
pub mod equipment;
pub mod fastmove;
pub mod hud;
pub mod ice;
pub mod leaderboard;
pub mod magicmap;
pub mod map;
mod monsters;
pub mod pack;
pub mod particles;
pub mod player;
pub mod pride;
pub mod rect;
pub mod schedule;
pub mod score;
pub mod shake;
pub mod spawn;
pub mod spirits;
pub mod state;
pub mod traps;
pub mod visibility;
pub use body::*;
pub use monsters::*;
mod combat;
mod helpers;
// `helpers` is plumbing and stays private, with three exceptions. `apply_hit`
// and `Hit` are the one place mitigation is decided — every source of harm in
// the game goes through them — so they are part of the crate's surface rather
// than an internal detail.
pub use helpers::{Hit, apply_hit, chebyshev, mark_moved, mob_at};
mod identify;
mod items;
mod saveload;

pub use abilities::*;
pub use agents::*;
pub use ai::*;
pub use autoexplore::*;
pub use autofight::*;
pub use catalog::*;
pub use combat::*;
pub use companion::*;
pub use components::*;
pub use conditions::*;
pub use effects::*;
pub use equipment::*;
pub use fastmove::*;
pub use hud::*;
pub use ice::{ColdSlain, IceCube, kick_ice_cube};
pub use identify::*;
pub use items::*;
pub use leaderboard::*;
pub use magicmap::*;
pub use map::*;
pub use pack::*;
pub use particles::*;
pub use player::{
    Hold, StepPlan, announce_special_room_entry, maybe_stumble, pick_up_here, plan_step,
    player_action_system, queue_charge, queue_drop, queue_reach_attack, queue_stairs, queue_step,
    queue_willed_teleport,
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

pub use rand::SeedableRng;
pub use rand_chacha::ChaCha12Rng;
