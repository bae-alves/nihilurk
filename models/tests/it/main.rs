//! The integration tests, built as one binary.
//!
//! Cargo builds every file directly under `tests/` as its own binary, and each
//! one links all of `models` with its debug info. Dozens of those cost tens of
//! gigabytes in `target/` and a link step apiece, so the tests live here as
//! modules of one binary instead. Run one file's tests with a filter:
//! `cargo test --test it combat::`. The modules mirror the folders of `src/`
//! (`world`, `creatures`, `mind`, `items`, `combat`, `player`, `meta`), and a
//! filter is a substring of the full path, so the file name alone still works.
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

mod combat;
mod creatures;
mod items;
mod meta;
mod mind;
mod player;
mod world;
