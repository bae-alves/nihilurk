Contributing
============

Bug reports and code changes go through GitHub: open an issue, or send a pull
request. A pull request to `master` needs bae's approval; bae is the
maintainer.


What you need
-------------

To build the game, run the tests and send a patch:

- git.
- Rust 1.91 or newer, installed with rustup. `rust-toolchain.toml` asks for
  stable, which rustup fetches on its own. The default rustup profile includes
  `rustfmt` and `clippy`; if yours does not, run
  `rustup component add rustfmt clippy`. The floor is the `rust-version` in
  `engine/Cargo.toml`.
- bash, and Lua 5.4 or newer with `lua` on your PATH. The scripts and hooks are
  bash and Lua.
- The usual Unix tools (`grep`, `sed`, `awk`, `find`, `sort`, `xargs`), which
  `docs_style.sh` uses.
- A terminal, and a network connection for the first build, which downloads
  the crates.
- Linux. macOS and Windows are not officially supported yet: the code is not
  Linux-only and type-checks for both, but nobody has run it there.

Only for some jobs:

- Portability work (`compat/`, `particle-core/`): Docker and `cross`. See
  `docs/how-to/run-the-compat-pipeline.md`.
- Changing a workflow under `.github/`: `actionlint`.
- Claude Code users: `npx`, for the `/codebase-architecture` command only.
- Cutting a release, which is the maintainer's job: `gh` to watch CI. Lua
  runs `release/bump.lua`, and CI publishes, so no crates.io token lives on
  your machine. The local checks need a little more: the musl targets for
  `release/test_package.sh`, and `curl` and `sha256sum` for `aur_check.sh`.
  See `docs/how-to/cut-a-release.md`.

In Claude Code, `/prepare-for-nihilurk` checks your machine against this list
and fixes the gaps. It asks before it installs anything, turns on the
pre-commit hook if you say yes, and points you to the right first page for the
job you came to do. Without Claude, work down the list by hand, run
`git config core.hooksPath .githooks` once per clone, and run `cargo test` to
see it all pass.


What the code is written in
---------------------------

Rust for the game. Bash or Lua for scripts and hooks, and nothing else. CI
checks the whole tree on every pull request and fails on Python: "No python.
Lua is to be used." Once per clone, `git config core.hooksPath .githooks`
turns on the same check, and more, in the pre-commit hook, so a commit is
refused before it leaves your machine.


LLMs and this codebase
----------------------

LLM-assisted code is welcome here. The engine, the rules, the tests, the docs,
the scripts and the text are all open to it. If you use Claude Code,
`.claude/README.md` explains the project instructions, hooks and commands that
load when you open the repo.

Text is the one place with a convention. English strings in `strings/` that an
LLM wrote are placeholders. The text is part of the game's feel, so write your
own when you can. Nothing checks this, and bae is the maintainer, not anyone's
mom. Machine translation, LLM text included, is fine for starters and
placeholders in any language, and you do not have to flag it if you would
rather not. If player-facing slop slips through, someone else will fix it, or
you will. bae trusts contributors, and trusts their own eye too.

The reasoning is bae's own opinion about LLMs in code. In short:

- An LLM is built from text it took without asking. The damage of making the
  weights is done and cannot be undone, and using one costs far less. Big
  tech cannot be stopped without a revolution.
- So bae uses code assistants, and points the output at free software: GPL
  code, structured and documented so that humans can pick it up, play it and
  mod it. In bae's words, that is praxis in a scenario of calamity.
- The goal is tools for not needing LLMs. nihilurk is designed for LLM
  non-use, and so is every game bae makes after it.
- Code is not an asset. It is a liability, a debt to keep paying: to
  paraphrase Dijkstra, a line written is a line spent. Assets are different.
  The manifesto says bae will not stand for LLM-generated assets, not today
  and not ever. Placeholders are not the final text, and what players end up
  reading should be human work, localization above all.

The full manifesto is at https://bae-alves.itch.io/. bae chose itch to host it
because itch is probably the worst ecosystem for it. The aim is to bring a more
practical perspective, rooted in material reality, to the AI stigma there.


Translations
------------

Localization has to be made by humans, because LLMs do not have a culture.
More than translation, the game text is part of the game's feel, and it is the
only part of the game that bae considers art. Machine translation, LLM text
included, is fine as a starter or a placeholder, and only as that. A starter
is where you begin. Put your heart into the rest. Localization requires heart.

nihilurk ships in four languages -- English, Portuguese, Spanish, Haitian
Creole -- one binary per language, picked at build time (see
`strings/src/lib.rs`). English is the reference translation. The other three
are beta: they compile and run correctly today because `strings/src/pt.rs`,
`es.rs` and `ht.rs` each re-export English wholesale, function by function,
until someone translates that function for real. A beta binary shows a notice
saying so on startup.

Of those three, only Portuguese is done by a native speaker (the maintainer
is Brazilian). Spanish and Haitian Creole were started with machine
translation, because bae only speaks English and Portuguese. No native
speaker has checked them, and they need review as much as they need
finishing. Roguelikes have shipped in English-only for decades, but this one
should not have to.

So this is a plea. Spanish and Haitian Creole need you. Machine translation
got them started, nobody who speaks either one natively has checked a word of
them, and bae cannot do it alone. If you speak Spanish or Haitian Creole,
please open `strings/src/es.rs` or `strings/src/ht.rs` and fix one sentence.
One is enough. Read the English line, think about how it sounds in your own
mouth, and write that. Send it as a pull request, or open an issue and say you
are coming. You will have bae's gratitude, and so will every player who
finally sees their own language down in the dungeon.

Found an English sentence where a translation should be? Open an issue, or
send a PR: pick a function in `strings/src/en.rs`, translate it, and add the
translated version to the matching language file (overriding the `pub use
super::en::*` re-export for that one function -- see the comment at the top
of `strings/src/pt.rs` for the exact shape). Once every function in a
language file has a real translation, delete that language's `Some(...)` arm
in `beta_notice()` (`strings/src/lib.rs`) in the same PR -- a language stops
being beta the day it earns it, not before.

One known rough edge, flagged in a code comment where it lives, that a
translation can't paper over and nobody has designed around yet:

- `models/src/identify.rs`: articles and pluralization ("a dagger", "7
  arrows") are English grammar rules, hardcoded. Portuguese and Spanish need
  gender agreement; none of the three need "-s" for a plural in the general
  case. This needs real design, not just new strings.

That's out of scope for a translation PR. If you want to take it on, say so
in an issue first -- it's a small data-model change, not a string edit.