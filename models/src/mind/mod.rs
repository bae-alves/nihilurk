//! How anything decides what to do.
//!
//! * `agents` — what a monster perceives and which rule set it plays.
//! * `ai` — what every monster on the floor does with its turn.
//! * `autoexplore`, `autofight` — the player's own automation (`o` and `Tab`).
//!
//! A monster's tactic is its `MovementType`; `agents::rule_set_for` is the one
//! exhaustive match on it. See `docs/explanation/agents.md`.
//!
//! Every module is re-exported at the crate root (`models::ai`), so callers never
//! write `models::mind::ai`.

pub mod agents;
pub mod ai;
pub mod autoexplore;
pub mod autofight;
