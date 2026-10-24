How to cut a release
====================

    Audience       The maintainer.
    Prerequisites  A clean `master` with green CI, and crates.io
                   Trusted Publishing set up once for each of the
                   three crates (see "Before the first automated
                   release").
    Result         A new version on GitHub, with archives for two
                   Linux machines, Windows and Apple Silicon Macs,
                   and on crates.io, as three crates.
                   One command and one push.

A release is `release/bump.lua` and a tag. The script edits the files, runs the checks and tags the commit. CI does the publishing when the tag is pushed. The push is the one step that cannot be taken back, and the script never does it for you.

The documentation site is separate: every push to `master` that touches the docs publishes it. See "The documentation site" below.


Quick commands
--------------

    gh workflow run ci.yml --ref master    package and play on Windows and macOS first
    lua release/bump.lua patch             bump, test, commit, tag
    git push origin master vX.Y.Z          starts the release
    gh run watch                           watch CI

Use `patch`, `minor`, `major` or an exact `X.Y.Z`. Two flags:

    --dry-run      show every line that would change, and change nothing
    --no-cargo     skip the cargo steps, if you have just run them yourself

The script prints the exact push command when it finishes, with the version filled in.

To try the packaging on your own machine, without CI:

    release/test_package.sh x86_64-unknown-linux-musl

It builds with fat LTO, so it takes a few minutes, then unpacks the tarball and runs it.


Which number to bump
--------------------

The number says what an upgrade costs a player:

    patch    0.1.3 to 0.1.4    nothing. Saves and everything else carry over.
    minor    0.1.3 to 0.2.0    old saves stop loading.
    major    0.2.0 to 1.0.0    everything from the old version is invalid.

So a release that breaks saves is a minor bump, and a release that breaks everything the old version made is a major one.

The script checks one cause. If `SAVE_VERSION` in `models/src/meta/saveload.rs` changed since the last tag, it refuses any new version that keeps the same major and minor, so a save break cannot ship as a patch. A changed save layout cannot reach that point without a new `SAVE_VERSION`: `the_save_layout_only_changes_with_the_version` fails until it has one (`add-an-item-category.md`). The other usual cause is a renamed content row, because a save stores row ids (`add-a-monster.md`). Nothing catches that, so read the diff since the last tag for renames.


What the script does
--------------------

It refuses, and edits nothing, if the working tree has uncommitted changes, if you are not on `master`, if the tag already exists, if the new version is not greater than the current one, or if `SAVE_VERSION` changed since the last tag and the new version keeps the same major and minor. Then it edits:

    the three published crates    `version`, and every `path` pin on
                                  a dependency line
    README.md                     the four download links and the
                                  version in the line above them

All three crates move to the same number, so there is one version to think about.

Then, unless `--no-cargo`: `cargo update --workspace` refreshes `Cargo.lock` (it touches only the three workspace lines, no dependency moves), `cargo test --locked` runs the suite, and `cargo publish --workspace --dry-run` proves all three packages build. If a step fails, the edits stay in the working tree and `git checkout .` puts them back. When they pass, it commits `Release vX.Y.Z` and tags it.

`release/bump_test.lua` runs the script in a throwaway repository. Run it with `lua release/bump_test.lua` after you change the script.


What CI does
------------

Pushing the tag starts `.github/workflows/release.yml`:

  check        The tag has to be `v` plus the version in
               `engine/Cargo.toml`. Then `cargo test`.
  build        One runner per target. Runs `release/test_package.sh`,
               which calls `release/package.sh`, then unpacks the
               archive and starts the binary.
  publish      Creates the GitHub release with the archives and
               generated notes.
  crates-io    Publishes the three crates, in dependency order. It
               skips any version crates.io already has, so a
               failed run can be run again.

`publish` and `crates-io` run in parallel once `build` is green. A failure in `check` or `build` stops both, and nothing reaches GitHub or crates.io.

The targets:

    x86_64-unknown-linux-musl     ubuntu-24.04
    aarch64-unknown-linux-musl    ubuntu-24.04-arm
    x86_64-pc-windows-msvc        windows-latest
    aarch64-apple-darwin          macos-latest

Each archive holds one directory:

    nihilurk-<version>-<target>/
        nihilurk          the game (`nihilurk.exe` on Windows)
        MANUAL.md
        LICENSE

Windows gets a `.zip`, the rest a `.tar.gz`, each with a `.sha256`. The Linux builds are musl, so each is one static file with no glibc to match. The macOS build is not signed, so a browser download is quarantined: `xattr -d com.apple.quarantine nihilurk*` clears it. CI starting the binary proves the packaging, not the game: Windows was played by a person, macOS has not been yet.

The Ubuntu runner images are named, not `ubuntu-latest`, so `musl-tools` cannot change without a commit here. Move to a newer image on purpose, and run `release/test_package.sh` first. The workflows stay on `actions/*@v4`, because only a tag exercises `release.yml` and a bump there would go untested until the release it breaks.

`crates-io` uses Trusted Publishing: the job trades GitHub's identity token for a short-lived publish token. No secret is stored in the repository.


Rehearsing before a tag
-----------------------

Two manual runs, neither of which publishes anything.

`gh workflow run ci.yml --ref master` runs the `package` job: the Windows and macOS archives, built and started the way `release.yml` does it. Pushes do not run it, so without this the tag is the first time those two are packaged.

`gh workflow run release.yml --ref master` runs only the `rehearsal` job. It asks crates.io for a publish token and discards it. A pass proves crates.io accepts this repository and `release.yml`. It does not prove all three crates are configured, because crates.io does not report which crates the token covers.


Before the first automated release
----------------------------------

Do this once. For each of `nihilurk`, `nihilurk-models` and `nihilurk-strings`, open the crate's settings on crates.io and add a trusted publisher with:

    GitHub user         bae-alves
    Repository          nihilurk
    Workflow filename   release.yml
    Environment         (leave empty)

A crate has to exist before it can have a trusted publisher. All three do.


The documentation site
----------------------

`.github/workflows/pages.yml` publishes `README.md`, `MANUAL.md` and `docs/` to GitHub Pages on every push to `master` that touches them, `site/` or the workflow. Nothing it makes is committed. To build it on your machine you need mdBook 0.5.4 (`cargo install mdbook --locked --version 0.5.4`):

    lua site/build.lua                     stage the pages in target/site-book/
    mdbook build target/site-book          build the site into target/site-book/book
    lua site/finish.lua                    give each built page its own tags
    mdbook serve target/site-book          read it at localhost:3000
    lua site/build_test.lua                the script's test

`site/build.lua` turns a backticked path that names a page into a link and writes the contents list, sitemap, `robots.txt` and `llms.txt`. It refuses a page with no heading. The site URL is `site-url` in `site/book.toml`, and nowhere else. `site/llms.txt` repeats facts from `README.md` and `engine/Cargo.toml` (what the game is, the install line, the license); change both when one changes, because nothing checks it.

Once, in the repository's Settings, under Pages, Source must be "GitHub Actions".


When it goes wrong
------------------

  The script refuses                  Read the message. It names the
                                      cause and changed nothing.
  A cargo step in the script fails    Fix the cause, then
                                      `git checkout .` and run it again.
  `check` or `build` fails            Nothing was published. Fix it,
                                      delete the tag locally and on
                                      GitHub, and run the script again:
                                      `git tag -d vX.Y.Z`
                                      `git push origin :refs/tags/vX.Y.Z`
                                      A failed `build` runs the same on
                                      your machine as
                                      `release/test_package.sh <target>`.
  `publish` fails after the builds    Re-run the failed job from the
                                      Actions tab. The artifacts are
                                      kept for the run.
  `crates-io` fails part way          Re-run the failed job from the
                                      Actions tab. It skips what is
                                      already published.
  The tag is on GitHub, no run starts Delete the remote tag and push it
                                      again at the same commit.
  A bad GitHub release is live        `gh release delete vX.Y.Z --yes`,
                                      then delete the tag. Anyone who
                                      already downloaded it keeps it.
  A bad version is on crates.io       `cargo yank --version X.Y.Z
                                      <crate>` for each crate. A yank
                                      stops new projects from picking
                                      it up. It does not delete it.
  The site build names a page         That page has no heading. Add one.
  mdBook moved on                     The version and its checksum are
                                      two lines in `pages.yml`. Change
                                      both, build it here first.


By hand, the day CI cannot publish
----------------------------------

After `lua release/bump.lua`, from the tagged commit:

    cargo publish --workspace --dry-run    package and build all three
    cargo publish --workspace              the real upload, in dependency order

This needs `cargo login` with a crates.io token, and a verified email on the account. If the upload stops part way, send the rest one at a time with `cargo publish -p <crate>`, in the order `nihilurk-strings`, `nihilurk-models`, `nihilurk`. For the GitHub release, run `release/package.sh <target>` on each machine and `gh release create vX.Y.Z dist/* --verify-tag --generate-notes`.

Check a crates.io release once, in a scratch directory:

    cargo install nihilurk --root /tmp/nih-check
    /tmp/nih-check/bin/nihilurk -content | sed -n 1,8p


Adding another place to publish
-------------------------------

Each place is one job in `release.yml`. Give it `needs: build` (or `needs: publish` if it uses the GitHub release), keep its credentials in repository secrets, and gate it on a repository variable until the account behind it exists. A new job does not change the script, because the script only edits files and tags.


See also
--------

    ../../release/bump.lua                the script
    ../../release/bump_test.lua           its test
    ../../.github/workflows/release.yml   what the tag starts
    ../../.github/workflows/pages.yml     what a push to master starts
    ../explanation/adr-0002-two-names-per-crate.md   why the package names differ from the directories
