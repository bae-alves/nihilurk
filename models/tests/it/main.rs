//! The integration tests, built as one binary.
//!
//! Cargo builds every file directly under `tests/` as its own binary, and each
//! one links all of `models` with its debug info. Dozens of those cost tens of
//! gigabytes in `target/` and a link step apiece, so the tests live here as
//! modules of one binary instead. Run one file's tests with a filter:
//! `cargo test --test it combat::`.
//!
//! A test that reads outside the crate stays a file of its own under `tests/`,
//! so `Cargo.toml` can `exclude` it without dropping this whole binary from the
//! package.
//!
//! A test that only needs what `models` exports lives here. A test that needs
//! a private item stays in its module, inline at the bottom, as
//! `#[cfg(test)] mod tests { ... }`, the same as in `engine`. Its scratch files go through `crate::scratch::Scratch`,
//! the unit-test twin of `common::SaveFile`.

mod common;

mod abilities;
mod agents;
mod autoexplore;
mod autofight;
mod body;
mod bones_combat;
mod bones_encounter;
mod charge;
mod combat;
mod companions;
mod conducts;
mod content;
mod creature_casts;
mod decks;
mod determinism;
mod equip;
mod fast;
mod fastmove;
mod fencer;
mod hud;
mod icecube;
mod identify;
mod invisibility;
mod item_info;
mod keymap;
mod last_stand;
mod level_feeling;
mod loot;
mod lurk;
mod magicmap;
mod megacurse;
mod missiles;
mod morgue;
mod moved;
mod names;
mod pack;
mod pickups;
mod player_actions;
mod player_rules;
mod player_step;
mod polymorph;
mod potions;
mod rings;
mod runes;
mod saved_enums;
mod saveload;
mod schedule;
mod scrolls;
mod sightings;
mod soak;
mod special_levels;
mod special_rooms;
mod spells;
mod spirits;
mod stairs;
mod stale_attack;
mod throw;
mod tile_info;
mod traps;
mod travel_cursor;
mod wands;
mod worn;
