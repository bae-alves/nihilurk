//! Every player-facing string in nihilurk.
//!
//! The rest of the workspace never writes a literal sentence to the player.
//! `models` and `engine` call a function here (`strings::pack_full()`,
//! `strings::you_drop(name)`, ...) and never know which language answered.
//! English is the only one. A translation is a fork that replaces `en.rs`.
//!
//! Content-table names (a monster's, item's or trap's `name` field) are a
//! separate case from everything else here: they are also the stable id that
//! `-content`, `-am`, `NIHILURK_SPAWN` and save files match against, so they
//! cannot simply be replaced with translated text. [`content_name`] is the
//! one function that stands between an id and what the player sees for it;
//! see its own doc comment.

mod en;
pub use en::*;
