//! What the rest hangs on: the ECS vocabulary, the tuning constants, the
//! save file, the score, and the presentation state the terminal reads.
//!
//! Every module is re-exported at the crate root (`models::components`), so
//! callers never write `models::meta::components`.

pub mod color;
pub mod components;
pub mod constants;
pub mod hud;
pub mod leaderboard;
pub(crate) mod particle_core;
pub mod particles;
pub mod pride;
pub(crate) mod saveload;
pub mod schedule;
pub mod score;
#[cfg(test)]
pub(crate) mod scratch;
pub mod shake;
pub mod state;
