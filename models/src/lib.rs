//! Everything the game is made of, flattened onto one crate root.
//!
//! `engine` drives the turn loop and the terminal; every rule, component and
//! system that decides what a turn *does* lives here instead, re-exported so
//! callers write `models::Fighter` rather than reaching into a submodule. Most
//! modules are public for that reason. The exceptions — `combat`, `helpers`,
//! `identify`, `items`, `saveload` — are internal wiring between the public
//! modules, kept private and re-exported piece by piece where a caller
//! actually needs one — see the `helpers` re-export below.

pub mod abilities;
pub mod ai;
pub mod autoexplore;
pub mod autofight;
pub mod body;
pub mod catalog;
pub mod components;
pub mod conditions;
pub mod constants;
pub mod effects;
pub mod equipment;
pub mod fastmove;
pub mod hud;
pub mod magicmap;
pub mod map;
mod monsters;
pub mod pack;
pub mod particles;
pub mod pride;
pub mod rect;
pub mod score;
pub mod shake;
pub mod spawn;
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
pub use helpers::{Hit, apply_hit, chebyshev, mob_at};
mod identify;
mod items;
mod saveload;

pub use abilities::*;
pub use ai::*;
pub use autoexplore::*;
pub use autofight::*;
pub use catalog::*;
pub use combat::*;
pub use components::*;
pub use conditions::*;
pub use effects::*;
pub use equipment::*;
pub use fastmove::*;
pub use hud::*;
pub use identify::*;
pub use items::*;
pub use magicmap::*;
pub use map::*;
pub use pack::*;
pub use particles::*;
pub use pride::*;
pub use rect::*;
pub use saveload::*;
pub use score::*;
pub use shake::*;
pub use spawn::*;
pub use state::*;
pub use traps::*;
pub use visibility::*;

pub use rand::SeedableRng;
pub use rand_chacha::ChaCha12Rng;
