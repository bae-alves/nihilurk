Contributing
============

Bug reports and code changes go through GitHub: open an issue, or send a pull
request. A pull request to `master` needs bae's approval; bae is the
maintainer. GitHub holds a first-time contributor's workflow run until the
maintainer approves it, so CI starts a little late on your first pull request.


Scope and limits
----------------

bae maintains nihilurk alone, in spare time. A reply can take weeks, and there
are no release dates. Silence is not a no.

Send a pull request straight away for:

- a bug fix, with a test that fails without it;
- a new content row (a monster, item, trap or spell) that follows its how-to
  under `docs/how-to/`;
- a fix to the docs;
- a platform report, through the issue template.

Open an issue first, and wait for a yes, before you write code for:

- a new mechanic, or a change to what `gdd.md` describes;
- a new dependency;
- a new platform, package or place to publish.

A pull request for one of these with no issue behind it may be closed unread.

Not taken: translations, and scripts in any language but bash or Lua. An issue
outside this scope gets closed. Reopen it, or open a new one, if you have more
to add.


What you need
-------------

To build the game, run the tests and send a patch:

- git.
- Rust 1.91 or newer, installed with rustup. `rust-toolchain.toml` asks for
  stable, which rustup fetches on its own. The default rustup profile includes
  `rustfmt` and `clippy`; if yours does not, run
  `rustup component add rustfmt clippy`. Rust from a distro package, without
  rustup, is not supported. The floor is the `rust-version` in
  `engine/Cargo.toml`.
- bash, and Lua 5.4 or newer with `lua` on your PATH. The scripts and hooks are
  bash and Lua.
- A terminal, and a network connection for the first build, which downloads
  the crates.
- Linux. The game also runs on Windows (a tester built and played it), but the
  scripts and hooks here are bash and Lua, so develop on Linux. macOS builds
  and passes its tests in CI, but nobody has played it there yet.

Only for some jobs:

- Changing a workflow under `.github/`: `actionlint`.
- Claude Code users: `npx`, for the `/codebase-architecture` command only.
- Playing the game from a script (`.claude/skills/play-nihilurk`): `script`, from util-linux, which Linux has. See `docs/how-to/play-through-a-pty.md`.

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


Comments
--------

In `engine` and `models`, a comment goes above the thing it
describes and never inside a function body. These stay: doc comments, a note
over a signature, a file header, a note on its own line directly above a
closure, and the notes over a catalog table (`BESTIARY` and its kin) that say
how to add a row. Inside a body, name the thing better or split the function.

The pre-commit hook refuses a staged body comment and names the file and line
(`.githooks/no_body_comments.lua`). It reads Rust as `cargo fmt` writes it, so
run that first; plain Rust needs nothing else from you. CI does not check this
one, so a clone without the hook can break it, and review catches that.


LLMs and this codebase
----------------------

LLM-assisted code is welcome here. The engine, the rules, the tests, the docs,
the scripts and the text are all open to it. If you use Claude Code,
`.claude/README.md` explains the project instructions, hooks and commands that
load when you open the repo.

Text is the one place with a convention. English strings in `strings/` that an
LLM wrote are placeholders. The text is part of the game's feel, so write your
own when you can. Every English string carries `// TODO: placeholder English;
needs a human's pass.`, and the pre-commit hook tags the new ones for you.
When you have read a string and stand behind it, delete its tag, reworded or not. Machine translation, LLM
text included, is fine for starters and placeholders in any language. bae is
the maintainer, not anyone's mom, and trusts contributors, and trusts their
own eye too.

The reasoning is bae's own opinion about LLMs in code. In short:

- An LLM is built from text it took without asking. The damage of making the
  weights is done and cannot be undone. Big tech cannot be stopped without a
  revolution.
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

The aim is to bring a more practical perspective, rooted in material reality,
to the AI stigma in independent game development.
