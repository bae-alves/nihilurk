# .claude

How Claude Code is set up for nihilurk. Open the repo root in Claude Code and this loads. You need none of it to build, test, or send a patch. `CONTRIBUTING.md` says where LLM-assisted work is welcome and where it is not.

## What is here

| Path | What it does | In git |
|---|---|---|
| `CLAUDE.md` | The project instructions. Claude Code reads them at the start of every session, so they work as the system prompt. | yes |
| `settings.json` | Permissions and hooks. | yes |
| `commands/` | Slash commands. | yes |
| `skills/` | `play-nihilurk`: plays the game through a pty and prints the screen, so Claude can see a change working in the real game. | yes |
| `hooks/` | `rustfmt.lua` formats each `.rs` file Claude writes. `rustfmt_test.lua` checks it. | yes |
| `journal.local.md` | Dated notes Claude keeps about this repo. | no |
| `settings.local.json` | One person's own permissions. | no |

`.gitignore` covers both `.local` files.

## The system prompt

`CLAUDE.md` has six blocks: who bae is to Claude and how conflicts resolve, hard rules, test and debug habits, context and output habits, and prose style.

Why it reads the way it does:

- It loads every session, so each line costs tokens every turn. The rules are short, and most name an action Claude can check itself, such as "Fix fails 2x→stop, re-read top-down, name where your model was wrong."
- Conflicts have an order: correctness, then bae's goal, then repo convention. Claude does not stall, and does not pick a side without saying so.
- Architecture, frameworks and major refactors are bae's to decide. Claude asks before anything irreversible, and lays out options when more than one approach fits.
- Context is rationed. If a plan doc exists, Claude reads it. Otherwise it reads `docs/` and greps for specifics.

Why it works for this codebase:

- The repo checks itself. `cargo test` runs the whole suite, and some tests hold docs to the code (`models/tests/content_docs.rs`). `docs_style.sh` lints the pages, and the pre-commit hook runs it on staged docs once you enable `.githooks`. That makes "read `docs/` first" safe, and "failing test first" has somewhere to land.
- The pre-commit hook also refuses a comment inside a function body in `engine`, `models` and `particle-core` (`.githooks/no_body_comments.lua`). `CLAUDE.md` states the rule in one line, so Claude writes the note above the function, or above a closure, and does not need the hook to tell it.
- Rules the compiler cannot state live in tests, not prose (`models/tests/effects.rs`, `engine/tests/workspace.rs`). `CLAUDE.md` says "Enforcement=hooks/permissions; docs=guidance", and `settings.json` follows it: it denies `Task` and `Agent`, and runs `rustfmt --edition 2024` after every edit to a `.rs` file (`hooks/rustfmt.lua`). Scripts here are bash or Lua.
- The cost of denying `Agent`: Claude explores in one context, so a wide audit takes more sequential reads.

Four things name this repo or its maintainer: the `/docs` pointer, the content-table rule, the contributing paragraph, and bae's name. The rest says nothing about nihilurk. bae generalized the file for other Rust codebases, and it worked there too. Those codebases belong to bae's upcoming secret project.

## The journal

`journal.local.md` records what was verified and what was not, with the reason for each decision. `CLAUDE.md` tells Claude to search it before complex tasks and to log bugs it finds outside its task. It is 28 KB, and bae prunes it by hand now and then. Even pruned, it is a lot of context for Claude to read, and it describes one person's sessions. So it stays out of git, and each contributor's Claude keeps a journal of its own. Git ignores that one too. Earlier versions are in the git history.

## Commands and skills

| Command | What it does |
|---|---|
| `/prepare-for-nihilurk` | Sets up a new contributor. Claude reads "What you need" in `CONTRIBUTING.md`, checks the machine against it with version flags only, and asks before it installs anything. It offers to turn on the pre-commit hook and run a smoke test, then points to the onboarding page for the job: content, translation or engine. It edits no repo file, and it never commits or pushes. |
| `/careful-review` | Re-reads the code Claude just wrote or changed, looks for bugs, and fixes them. |
| `/codebase-architecture` | Runs `npx skills use` to fetch `improve-codebase-architecture` from mattpocock/skills on GitHub, then follows it. `npx` runs an npm package and needs the network. |

`skills/play-nihilurk` is the one skill the repo carries. It is a bash driver (`play.sh`) over `script`, and a Lua screen reader (`screen.lua`), each with a test next to it. `SKILL.md` says how to spawn a thing with `NIHILURK_SPAWN`, press keys and read the screen back, and what wastes a run.
