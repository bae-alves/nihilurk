Why the crates are named twice
==============================

    Audience       Anyone who opens `models/Cargo.toml` and wonders why
                   the package is `nihilurk-models` and the library is
                   `models`, or who is about to rename one of them.
    Prerequisites  You know what a Cargo workspace is.
    This is        Understanding. The recipe is
                   `../how-to/publish-to-crates-io.md`.

Every crate in this workspace has two names, and they are different on purpose.

    directory       models/
    package name    nihilurk-models     what crates.io and `cargo -p` see
    library name    models              what `use models::*;` sees

The directory and the library name are the short ones the code was written with. The package name is the one that had to change.


Why the package names changed
-----------------------------

crates.io gives out names on a first-come basis. `engine`, `models` and `strings` were all taken by other people before nihilurk existed, so a crate published under those names would be refused. `particle-core` was free, but it was marked `publish = false`, and the crates above it cannot go out unless it does.

That last point is the one that forces the issue. `engine` depends on `models` and `strings` by `path`. crates.io does not accept a `path` dependency: it needs a version that resolves in its own index. So publishing the game means publishing the whole chain, and each link in it needs a name nobody else holds. Every crate that ships is now `nihilurk` or `nihilurk-<thing>`, and `engine/tests/workspace.rs` fails if a new one is not.

The binary's package took the bare name `nihilurk`, which is free. The binary was already called `nihilurk`, so `cargo install nihilurk` installs the thing it says.


Why the library names did not
-----------------------------

A package name and a library name are separate settings in Cargo. The library name defaults to the package name with hyphens turned into underscores, so renaming the package would have renamed the library to `nihilurk_models` and broken every `use models::` line in the game, the tests and the docs.

Two settings keep that from happening:

  `[lib] name = "models"`              in the crate itself, so its own
                                       integration tests keep compiling.
  `models = { package = "nihilurk-models", ... }`
                                       in the crate that depends on it, so
                                       the dependency is still called
                                       `models` in the code.

The cost is that a reader has to know both. That is the reason for this page. The alternative was to rename the library too and touch every file that mentions it, a diff a hundred times the size for a gain the player never sees.

`cargo -p` takes the package name, so it is `cargo test -p nihilurk-models` and `cargo run -p nihilurk`. The directory and the library keep working the way they did.


Why `compat` was left alone
---------------------------

`compat/` is already `nihilurk-compat` and is `publish = false`. It is a test rig that builds the game for other machines, and it depends on ratatui, which the shipped binary may not. Nothing installs it, so it has no name to defend.


Why there is no single crate
----------------------------

The other way out of the name problem is to fold `models`, `strings` and `particle-core` into one `nihilurk` crate as modules, and publish once. It was not taken, for two reasons.

`particle-core` is `no_std` with no dependencies so that `compat/nostd_check.sh` can compile it for an ESP32 and have that mean something about the game. Folded in, it would sit beside `bevy_ecs` and the save-file code, and the proof would be about a copy.

And `models` is the crate the documentation is written around. Almost every page under `tutorial/`, `how-to/` and `reference/` names a file in it. Moving it is a rewrite of the docs, not a release task.


See also
--------

    ../how-to/publish-to-crates-io.md          the release recipe
    cross-platform-testing.md                  why `particle-core` stays separate
    ../../engine/tests/workspace.rs            the tests that hold the names in place
