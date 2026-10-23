//! Every tuning knob in the model, in one place.
//!
//! nihilurk's balance lives in a few dozen numbers scattered across a dozen
//! modules. This folder gathers the ones worth turning — the ones a person
//! rebalancing the game reaches for — so that job is "read one folder, edit one
//! file" instead of a treasure hunt.
//!
//! The topics (`potions`, `rings`, `map`, ...) sit in six files grouped by what
//! they tune: `world_tuning`, `creature_tuning`, `item_tuning`, `combat_tuning`,
//! `player_tuning` and `meta_tuning`. Each header names its topics. A topic
//! keeps its path, `constants::potions::X`, whichever file holds it.
//!
//! Each constant is defined here and re-exported from the module that uses it
//! (often under its historical name), so call sites and doc links elsewhere in
//! the tree keep working. Change the value here; nothing else needs to move
//! unless a doc comment below says so.
//!
//! ## What is *not* here
//!
//! * **Animation timing** (`particles.rs` — bolt/blast/ripple milliseconds) and
//!   **magic-map wave counts** (`magicmap.rs`). These are presentation feel,
//!   wound tightly around the algorithms that read them, and no one balances
//!   the game by touching them. The shake table is the exception: [`shake`].
//! * **Content-table numbers** — a monster's HP, a weapon's die, a potion's
//!   colour. Those are data, one row per thing, and live in `catalog.rs` /
//!   `monsters.rs` / `traps.rs`.
//! * **RNG salts** and the neighbour-offset tables. Structural, not balance.
//!   The grid and special-level dials are here ([`layout`], [`special_levels`]),
//!   with the caveat that moving one moves every layout on every seed.
//! * A few one-off numbers still sit where they fire: spikemail's prick
//!   (`abilities::spike_prick`), the relic's `Value` (`spawn_element_of_yoord`),
//!   and the `-anim-rate` clamp (`engine/src/main.rs`).

mod combat_tuning;
mod creature_tuning;
mod item_tuning;
mod meta_tuning;
mod player_tuning;
mod world_tuning;

pub use combat_tuning::*;
pub use creature_tuning::*;
pub use item_tuning::*;
pub use meta_tuning::*;
pub use player_tuning::*;
pub use world_tuning::*;
