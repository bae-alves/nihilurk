ADR 0002: every crate has a package name and a library name
===========================================================

    Status         Accepted
    Audience       Anyone who opens `models/Cargo.toml` and wonders why
                   the package is `nihilurk-models` and the library is
                   `models`, or who is about to rename one of them.
    Supersedes     --
    Related        ../how-to/publish-to-crates-io.md

Every crate in this workspace has two names, and they are different on purpose.

    directory       models/
    package name    nihilurk-models     what crates.io and `cargo -p` see
    library name    models              what `use models::*;` sees


Context
-------

The directory and the library name are the short ones the code was written with. Publishing to crates.io forced the package name to change.

crates.io gives out names on a first-come basis. `engine`, `models` and `strings` were all taken by other people before nihilurk existed, so a crate published under those names would be refused. `particle-core` was free, but it was marked `publish = false`, and the crates above it cannot go out unless it does.

That last point forces the issue. `engine` depends on `models` and `strings` by `path`. crates.io does not accept a `path` dependency: it needs a version that resolves in its own index. So publishing the game means publishing the whole chain, and each link in it needs a name nobody else holds.


Decision
--------

**Rename the packages. Keep the directories and the library names.**

  1. Every crate that ships is `nihilurk` or `nihilurk-<thing>`. `engine/tests/workspace.rs` fails if a new one is not.
  2. The binary's package takes the bare name `nihilurk`, which is free. The binary was already called `nihilurk`, so `cargo install nihilurk` installs the thing it says.
  3. A package name and a library name are separate settings in Cargo, so two lines keep the library names:

    `[lib] name = "models"`              in the crate itself, so its own
                                         integration tests keep compiling.
    `models = { package = "nihilurk-models", ... }`
                                         in the crate that depends on it, so
                                         the dependency is still called
                                         `models` in the code.

  4. `compat/` is already `nihilurk-compat` and is `publish = false`. It is a test rig that builds the game for other machines, and it depends on ratatui, which the shipped binary may not. Nothing installs it, so it has no name to defend.

Rejected:

  5. **Renaming the library too.** The library name defaults to the package name with hyphens turned into underscores, so renaming the package alone would have made it `nihilurk_models` and broken every `use models::` line in the game, the tests and the docs. Renaming the library means touching every file that mentions it, a diff a hundred times the size for a gain the player never sees.
  6. **One crate.** Fold `models`, `strings` and `particle-core` into one `nihilurk` crate as modules and publish once. Two reasons against: `particle-core` is `no_std` with no dependencies so that `compat/nostd_check.sh` can compile it for an ESP32 and have that mean something about the game, and folded in it would sit beside `bevy_ecs` and the save-file code, and the proof would be about a copy. And `models` is the crate the documentation is written around: almost every page under `tutorial/`, `how-to/` and `reference/` names a file in it, so moving it is a rewrite of the docs, not a release task.


Consequences
------------

Accepted costs:

  * **A reader has to know both names.** `cargo -p` takes the package name, so it is `cargo test -p nihilurk-models` and `cargo run -p nihilurk`, while the code says `models`. This ADR is the page that explains it.

Benefits realised:

  * The game, its tests and its docs kept every `use models::` line.
  * `cargo install nihilurk` installs the game.
  * `particle-core` stays small enough for a microcontroller check to mean something.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **crates.io stops being a target.** With nothing published, the package names can go back to the short ones.
  * **A short name is released to us.** If `models` or `engine` ever becomes available, the rename is a different trade.
  * **The docs stop being written around `models`.** Then folding crates costs less than it does now.


See also
--------

  ../how-to/publish-to-crates-io.md     the release recipe
  cross-platform-testing.md             why `particle-core` stays separate
  ../../engine/tests/workspace.rs       the tests that hold the names in place
