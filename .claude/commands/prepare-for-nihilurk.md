---
description: Check this machine against nihilurk's requirements, fix what is missing, and point to the right onboarding page
---

The person running this wants to contribute to nihilurk. Set up their machine and send them to the right first page. Work through the steps in order. Ask one question at a time, as multiple choice or yes/no. Ask before you change anything outside this repo.

1. Read "What you need" in `CONTRIBUTING.md`. That section is the list. Do not work from memory, and do not add requirements it does not state. If something looks missing from it, say so and carry on.

2. Ask what they came to do: add game content, translate, change the engine, fix a bug, or something else. Ask which OS they use. `CONTRIBUTING.md` says which OSes are supported.

3. Check every requirement that applies to that job. Use version flags and `command -v` only. Nothing in this step installs or changes anything. For Rust, compare `rustc --version` with the `rust-version` in `engine/Cargo.toml`, and check that `rustfmt` and `clippy` exist.

4. Report one short table: tool, found or missing, version.

5. For each gap, give the install command for their OS and ask before you run it. Do not use `sudo` without asking. If they decline, say what will not work without it and move on.

6. Offer to turn on the pre-commit hook with `git config core.hooksPath .githooks`. It is local to this clone. Say in one line what it does: it refuses Python, lints `docs/`, and checks that every language still builds.

7. Offer a smoke test, and say first that the first build downloads crates and takes a few minutes. Run `cargo test --locked --workspace --exclude nihilurk-compat` for most jobs. For a translator, `cargo check -p nihilurk --no-default-features --features lang-pt` (swap in their language) is enough. If it fails, report the first failure and stop. Do not fix their setup by editing repo files.

8. Point them to the onboarding for their job:
   - Game content: `docs/README.md`, the "Start here" table, then `docs/tutorial/add-your-first-*.md`.
   - Translation: the "Translations" section of `CONTRIBUTING.md`, and the comment at the top of `strings/src/pt.rs`.
   - Engine: `docs/how-to/work-with-the-ecs.md`, `docs/explanation/ecs-in-nihilurk.md` and `docs/explanation/code-calisthenics.md`.
   - Playing it first: `MANUAL.md`.
   - How this repo uses Claude: `.claude/README.md`.
   - In one sentence, tell them `CONTRIBUTING.md` has the LLM and localization conventions.

9. End with what is done, what is left, and the one next step.

Rules for this command: do not edit files in the repo, do not commit, and do not push. Scripts and hooks here are bash or Lua. Say what you checked and what you did not.
