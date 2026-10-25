//! What lives in the dungeon: bodies, the bestiary, and the parts that make one
//! creature act differently from another.
//!
//! * `monsters` — the bestiary table (`BESTIARY`) and how a row spawns.
//! * `body` — the playable bodies (`-b`, `-am`).
//! * `abilities` — the effects that act on their own, one table row each.
//! * `conditions` — what a creature is afflicted with, and lifting it.
//! * `spirits` — the peaceful `Faction::Spirits` and their events.
//! * `companion` — the boon companion; `pack` — the pack screen and its menus.
//!
//! Each bestiary row is data. A spirit's event is a function in its row, not a match
//! arm anywhere: see `docs/explanation/data-driven-content.md`.
//!
//! Every module is re-exported at the crate root (`models::body`), so callers never
//! write `models::creatures::body`.

pub mod abilities;
pub mod body;
pub mod companion;
pub mod conditions;
pub(crate) mod monsters;
pub mod pack;
pub mod spirits;
