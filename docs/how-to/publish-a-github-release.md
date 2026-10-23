How to publish a GitHub release
===============================

    Audience       The maintainer, cutting a release.
    Prerequisites  Push access to the repository. The version bumped
                   and committed, as in
                   `publish-to-crates-io.md`.
    Result         A GitHub release with one archive per machine, each
                   with a checksum, built and checked by CI.

This is the manual path. `release/bump.lua` does the bump and the tag for you: see `cut-a-release.md`.

Pushing a tag is the whole procedure. `.github/workflows/release.yml` does the rest, and it cannot be undone from your side once the release is public, so check the version before you tag.


Quick commands
--------------

    cargo test                            green before you tag
    git tag vX.Y.Z                        the number in engine/Cargo.toml
    git push origin vX.Y.Z                starts the workflow

Watch it with `gh run watch`. When the `publish` job finishes, the release is at `https://github.com/bae-alves/nihilurk/releases`.

To try the packaging on your own machine before you tag, without CI:

    release/test_package.sh x86_64-unknown-linux-musl

It builds with fat LTO, so it takes a few minutes, then unpacks the tarball and runs it.


What the workflow does
----------------------

Four jobs. `check` runs first, then `build`, then `publish` and `crates-io` side by side. Any failure stops the ones after it.

  check     Fails if the tag is not `v` plus the version in
            `engine/Cargo.toml`, then runs `cargo test`.
  build     One runner per target. Runs `release/test_package.sh`, which
            calls `release/package.sh` and then plays with the result.
            Uploads the tarball and its `.sha256`.
  publish   Collects every tarball and creates the release with
            generated notes.
  crates-io Publishes the three crates to crates.io, skipping any
            version already there. See `cut-a-release.md`.

The targets:

    x86_64-unknown-linux-musl     ubuntu-24.04
    aarch64-unknown-linux-musl    ubuntu-24.04-arm
    x86_64-pc-windows-msvc        windows-latest
    aarch64-apple-darwin          macos-latest

The runner images are named, not `ubuntu-latest`. GitHub moves that label from one Ubuntu release to the next, which would change the `musl-tools` the build installs without a commit in this repository. Move to a newer image on purpose, and run `release/test_package.sh` first.

The workflows use `actions/*@v4`. They run on a forced Node 24 and print a deprecation note. They are not bumped, because only a tag exercises the release workflow, and a bump there would go untested until the release it breaks.

Windows gets a `.zip` of `.exe` files and the other targets a `.tar.gz`. `release/package.sh` and `release/test_package.sh` run in bash on all three systems, and the workflow names `shell: bash` because Windows would otherwise run a `.sh` file in PowerShell and report success. A macOS download from a browser is quarantined by Gatekeeper, since the binaries are not signed: `xattr -d com.apple.quarantine nihilurk*` clears it.

The Linux builds are musl, so each is one static file with no glibc to match.


What is in an archive
---------------------

    nihilurk-<version>-<target>/
        nihilurk          the game
        nihilurk.6        the man page
        MANUAL.md
        LICENSE

A user can put the directory anywhere, or `install` the files into `/usr/local/bin`.


What CI shows
-------------

Every tarball is unpacked and run by CI on the machine that built it: the binary has to start. That proves it launches and that the packaging is right. It is not a playtest.

Adding a target is one line in the `matrix` of `release.yml`. Windows was played by a person before it went in. The macOS build has not been played yet: CI is all it has.

When it goes wrong
------------------

  `check` says the tag disagrees        Delete the tag, bump the version,
                                        commit, tag again:
                                        `git tag -d vX.Y.Z`
                                        `git push origin :refs/tags/vX.Y.Z`
  One `build` job fails                 Read its log. The same command runs
                                        on your machine as
                                        `release/test_package.sh <target>`.
                                        Fix, delete the tag, tag again.
  `publish` fails after the builds pass Re-run the failed job from the
                                        Actions tab. The artifacts are kept
                                        for the run.
  The tag is on GitHub, no run starts   `gh run list` shows nothing, and
                                        the cause is not known. Delete
                                        the remote tag and push it again at
                                        the same commit:
                                        `git push origin :refs/tags/vX.Y.Z`
                                        `git push origin vX.Y.Z`
  A bad release is live                 `gh release delete vX.Y.Z --yes`,
                                        then `git push origin
                                        :refs/tags/vX.Y.Z`. Anyone who
                                        already downloaded it keeps it.


See also
--------

    publish-to-crates-io.md                    the other half of a release
    ../../release/package.sh                   the build, one target
    ../../release/test_package.sh              the check on that build
    ../../.github/workflows/release.yml        the workflow itself
