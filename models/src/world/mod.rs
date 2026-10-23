//! The place the game happens in: the map and how it is built, what can be seen on
//! it, and what lies in wait there.
//!
//! * `map` — the terrain, level changes (`levels`), generation (`generate`,
//!   `population`, `special`, `streams`, `overlays`).
//! * `rect`, `visibility`, `magicmap` — rooms, what the player sees, and the reveal.
//! * `traps` — the trap table and what each trap does.
//! * `ice` — the cube a cold kill leaves behind.
//! * `spawn` — put a named thing into the world (`spawn_named`).
//!
//! `map::generate` builds a floor and `map::levels` decides when. Nothing here
//! decides a fight: that is `combat`.
//!
//! Every module is re-exported at the crate root (`models::map`), so callers never
//! write `models::world::map`.

pub mod ice;
pub mod magicmap;
pub mod map;
pub mod rect;
pub mod spawn;
pub mod traps;
pub mod visibility;
