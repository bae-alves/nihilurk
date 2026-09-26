//! Every player-facing string in nihilurk, one module per language.
//!
//! The rest of the workspace never writes a literal sentence to the player.
//! `models` and `engine` call a function here (`strings::pack_full()`,
//! `strings::you_drop(name)`, ...) and never know which language answered --
//! that is decided once, at compile time, by which `lang-*` feature built the
//! binary (`nihilurk-en`, `nihilurk-pt`, `nihilurk-es`, `nihilurk-ht`; see
//! `engine/Cargo.toml`). There is no runtime locale switch and no bundling of
//! every language into one binary.
//!
//! `en` is the only module with real content right now. `pt`, `es` and `ht`
//! each re-export it wholesale -- they compile and run correctly today, in
//! English, under their own binaries, until someone actually translates them
//! function by function. That is deliberate: a stub that compiles is honest
//! about what has and hasn't been translated, and it means adding a fourth
//! language never has to wait on translating the third.
//!
//! Content-table names (a monster's, item's or trap's `name` field) are a
//! separate case from everything else here: they are also the stable id that
//! `-content`, `-am`, `NIHILURK_SPAWN` and save files match against, so they
//! cannot simply be replaced with translated text. [`content_name`] is the
//! one function that stands between an id and what the player sees for it;
//! see its own doc comment.

// Always compiled, feature or no: `pt`/`es`/`ht` build their placeholder on
// top of it (`pub use super::en::*;`), so it has to exist even in a binary
// that never re-exports it at the crate root itself.
mod en;
#[cfg(feature = "lang-en")]
pub use en::*;

#[cfg(feature = "lang-pt")]
mod pt;
#[cfg(feature = "lang-pt")]
#[allow(unused_imports)]
pub use pt::*;

#[cfg(feature = "lang-es")]
mod es;
#[cfg(feature = "lang-es")]
#[allow(unused_imports)]
pub use es::*;

#[cfg(feature = "lang-ht")]
mod ht;
#[cfg(feature = "lang-ht")]
#[allow(unused_imports)]
pub use ht::*;

/// Shown once, folded into the run's opening log lines, only in a binary
/// whose translation isn't finished. `None` for English, which is the
/// reference translation, not a beta of itself. Defined once here rather
/// than per-locale module — a per-module override of a name the same module
/// glob-imports from `en` is an ambiguous re-export as far as rustc is
/// concerned, not a shadow (see `pt.rs`'s history for the error this used to
/// be). A locale earning a real translation deletes its `Some` arm here the
/// same day, rather than the notice quietly rotting into a lie.
#[cfg(feature = "lang-en")]
pub fn beta_notice() -> Option<&'static str> {
    None
}
#[cfg(feature = "lang-pt")]
pub fn beta_notice() -> Option<&'static str> {
    Some(
        "Tradução em beta — a maior parte do texto ainda está em inglês. \
         Encontrou um problema? Abra uma issue ou envie um pull request.",
    )
}
#[cfg(feature = "lang-es")]
pub fn beta_notice() -> Option<&'static str> {
    Some(
        "Traducción en beta — la mayor parte del texto todavía está en inglés. \
         ¿Encontraste un problema? Abre un issue o envía un pull request.",
    )
}
#[cfg(feature = "lang-ht")]
pub fn beta_notice() -> Option<&'static str> {
    Some(
        "Tradiksyon an an beta — pifò tèks la toujou an anglè. \
         Ou jwenn yon pwoblèm? Ouvri yon issue oswa voye yon pull request.",
    )
}
