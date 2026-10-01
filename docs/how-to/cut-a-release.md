How to cut a release
====================

    Audience       The maintainer.
    Prerequisites  A clean `master` with green CI, and crates.io
                   Trusted Publishing set up once for each of the
                   four crates (see "Before the first automated
                   release").
    Result         A new version on GitHub, with tarballs for two
                   Linux machines, and on crates.io, as four crates.
                   One command and one push.

Everything a release used to need by hand is now `release/bump.lua` and a tag. The script edits the files, runs the checks and tags the commit. CI does the publishing when the tag is pushed. The push is the one step that cannot be taken back, and the script never does it for you.


Quick commands
--------------

    lua release/bump.lua patch             bump, test, commit, tag
    git push origin master v0.1.3          starts the release
    gh run watch                           watch CI

Use `patch`, `minor`, `major` or an exact `X.Y.Z`. Two flags:

    --dry-run      show every line that would change, and change nothing
    --no-cargo     skip the cargo steps, if you have just run them yourself

The script prints the exact push command when it finishes, with the version filled in.


What the script does
--------------------

It refuses, and edits nothing, if the working tree has uncommitted changes, if you are not on `master`, if the tag already exists, or if the new version is not greater than the current one. Then it edits:

    the four published crates     `version`, and every `path` pin on
                                  a dependency line
    doc/nihilurk.6                the version and date in the `.TH` line
    aur/PKGBUILD                  `pkgver`, and `pkgrel` back to 1

All four crates move to the same number, so there is one version to think about. `compat/` is a test rig and is left alone.

Then, unless `--no-cargo`: `cargo update --workspace` refreshes `Cargo.lock`, `cargo test --locked` runs the suite, and `cargo publish --workspace --dry-run` proves all four packages build. If a step fails, the edits stay in the working tree and `git checkout .` puts them back. When they pass, it commits `Release vX.Y.Z` and tags it.

`release/bump_test.lua` runs the script in a throwaway repository. Run it with `lua release/bump_test.lua` after you change the script.


What CI does
------------

Pushing the tag starts `.github/workflows/release.yml`:

  check        The tag has to be `v` plus the version in
               `engine/Cargo.toml`. Then `cargo test`.
  build        One runner per machine. Builds, unpacks and plays
               each tarball.
  publish      Creates the GitHub release with the tarballs.
  crates-io    Publishes the four crates, in dependency order. It
               skips any version crates.io already has, so a
               failed run can be run again.

`publish` and `crates-io` run in parallel once `build` is green. A failure in `check` or `build` stops both, and nothing reaches GitHub or crates.io.

`crates-io` uses Trusted Publishing: the job trades GitHub's identity token for a short-lived publish token. No secret is stored in the repository.


Before the first automated release
----------------------------------

Do this once. It is the one part CI cannot do for you. For each of `nihilurk`, `nihilurk-models`, `nihilurk-strings` and `nihilurk-particle-core`, open the crate's settings on crates.io and add a trusted publisher with:

    GitHub user         bae-alves
    Repository          nihilurk
    Workflow filename   release.yml
    Environment         (leave empty)

A crate has to exist before it can have a trusted publisher. All four do.


When it goes wrong
------------------

  The script refuses                  Read the message. It names the
                                      cause and changed nothing.
  A cargo step in the script fails    Fix the cause, then
                                      `git checkout .` and run it again.
  `check` or `build` fails            Nothing was published. Fix it,
                                      delete the tag locally and on
                                      GitHub, and run the script again:
                                      `git tag -d v0.1.3`
                                      `git push origin :refs/tags/v0.1.3`
  `crates-io` fails part way          Re-run the failed job from the
                                      Actions tab. It skips what is
                                      already published.
  The tag is on GitHub, no run starts Delete the remote tag and push it
                                      again at the same commit.
  A bad version is live               `cargo yank --version 0.1.3
                                      nihilurk` for each crate. A
                                      yank stops new projects from
                                      picking it up. It does not
                                      delete it.

The manual versions of these steps are in `publish-to-crates-io.md` and `publish-a-github-release.md`, for the day CI cannot publish.


Adding another place to publish
-------------------------------

Each place is one job in `release.yml`. Give it `needs: build` (or `needs: publish` if it uses the GitHub release), keep its credentials in repository secrets, and gate it on a repository variable such as `vars.AUR_ENABLED == 'true'` until the account behind it exists. A new job does not change the script, because the script only edits files and tags.

The AUR package is not published by CI yet. The script bumps `pkgver`, and the repo's PKGBUILD carries `SKIP` for the sha. Until an AUR job exists, publishing there is a hand step after the tag: see "Updating the AUR package" in `publish-a-github-release.md`.


See also
--------

    ../../release/bump.lua                the script
    ../../release/bump_test.lua           its test
    ../../.github/workflows/release.yml   what the tag starts
    publish-to-crates-io.md               the manual path to crates.io
    publish-a-github-release.md           the manual path to a GitHub release
