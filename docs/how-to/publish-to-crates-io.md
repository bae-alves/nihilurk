How to publish nihilurk to crates.io
====================================

    Audience       The maintainer, cutting a release.
    Prerequisites  A crates.io account and an API token
                   (`cargo login`). A clean working tree, and the
                   release committed and tagged.
    Result         Four crates on crates.io, and `cargo install
                   nihilurk` gives a stranger the game.

This is the manual path. A release is `lua release/bump.lua` and a tag push, and CI publishes: see `cut-a-release.md`. Use this page when CI cannot.

Publishing is the one step in this repository that cannot be taken back. A version, once uploaded, can be yanked but never deleted, and a crate name is held for good. Everything before the last command is a dry run. Do those first, every time.


Quick commands
--------------

    cargo test                                   workspace still green
    cargo publish --workspace --dry-run          package and build all four
    cargo publish --workspace                    the real upload, in order

`--workspace` sends the crates in dependency order. `compat` is skipped on its own, because it is `publish = false`.


What goes out
-------------

Four crates, in the order cargo sends them:

    nihilurk-particle-core   particle-core/   the `no_std` arithmetic
    nihilurk-strings         strings/         every sentence, per language
    nihilurk-models          models/          the game
    nihilurk                 engine/          the binary

The directories keep their short names. Only the package names carry the game's name, and `[lib] name` keeps the short library names, so `use models::*;` still compiles. `../explanation/adr-0002-two-names-per-crate.md` has the reasoning.

`compat/` does not go out. It pulls in ratatui, which the shipped binary may not depend on, and nobody installs a test rig.


Before the first upload
-----------------------

1. Pick a version no tag has used. Bump every crate that changed, and the `version = "..."` on each `path` dependency that points at it:

        engine/Cargo.toml    version, and models = { ..., version = "X.Y.Z", ... }
        models/Cargo.toml    version

    `engine/tests/workspace.rs` fails if a `path` dependency has no version at all. It cannot tell you the version is stale, and cargo cannot either until the upload has half happened.

2. Check the page crates.io will show. Description, keywords, categories, `readme` and `rust-version` are frozen into each version, so a wrong one costs a new number. `engine/tests/workspace.rs` holds the limits crates.io enforces (five keywords, twenty characters each), and fails on an integration test that reads outside its crate without an `exclude` line, because that test cannot pass from an unpacked download. The floors differ by crate: `nihilurk-particle-core` and `nihilurk-strings` build on 1.85, `nihilurk-models` and `nihilurk` need 1.91. Raise one if you add code that needs a newer compiler, and build it on that compiler first (`cargo +1.85.0 check -p nihilurk-strings`).

3. Commit, then tag: `git tag v<version>`. The GitHub release and the AUR package both build from that tag. Pushing the tag starts the release workflow, so see `publish-a-github-release.md` first.

4. Run the three commands under Quick commands, in order. A dry run that says `aborting upload due to dry run` for all four crates is a pass.


After the upload
----------------

A stranger installs the English game with:

    cargo install nihilurk

and another language by naming its feature:

    cargo install nihilurk --no-default-features --features lang-pt

That installs one binary, `nihilurk`, in one language, into `~/.cargo/bin`. A stranger whose shell lacks that directory on `PATH` gets "command not found", so the README's install steps carries the fix; keep it there when you edit the install lines.

The forwarder that picks a language from `$LANG` (`nihilurk-dispatch`) is behind the `dispatch` feature and is not installed, because it execs a `nihilurk-<lang>` next to itself and an install from crates.io has none. `release/package.sh` and `aur/PKGBUILD` ask for it by name. `--no-default-features` is not optional for a second language: leaving it off builds English and the requested language together, and `strings` then fails to compile.

Check the install once, in a scratch directory, before you announce anything:

    cargo install nihilurk --root /tmp/nih-check
    /tmp/nih-check/bin/nihilurk -content | sed -n 1,8p


When it goes wrong
------------------

  The upload stops after two crates      Fix the cause and bump the
                                         versions of the crates not yet
                                         sent. Send each with
                                         `cargo publish -p <crate>`, in the
                                         order under "What goes out". I have
                                         not tested a partial re-run of
                                         `--workspace`, so do not rely on it.
  A bad version is live                  `cargo yank --version <v> <crate>`.
                                         It stops new projects from picking
                                         the version up. It does not delete
                                         it, and it does not undo an
                                         install.

See also
--------

    publish-a-github-release.md                        the other half of a release
    ../explanation/adr-0002-two-names-per-crate.md     why the names differ from the directories
    ../reference/cli-and-env.md                        the flags of the installed binary
    ../../engine/tests/workspace.rs                    the tests that guard the manifests
    ../../aur/PKGBUILD                                 the other way a release gets installed
