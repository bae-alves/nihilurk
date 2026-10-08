nihilurk documentation
==================

This is the documentation for people who want to *put things in the dungeon*: monsters, items, traps, magic. It lives in the repository, in plain text, and reads in a terminal.

    cat docs/how-to/add-a-monster.md


Start here
----------

| I want to...                                   | Read                                            |
|------------------------------------------------|-------------------------------------------------|
| Add my first item, hand-held                   | `tutorial/add-your-first-item.md`               |
| Add my first monster, hand-held                | `tutorial/add-your-first-monster.md`            |
| Add my first spell, hand-held                  | `tutorial/add-your-first-spell.md`              |
| Add my first body, hand-held                   | `tutorial/add-your-first-body.md`               |
| Give a monster a mind, hand-held               | `tutorial/give-a-monster-a-mind.md`             |
| Add a monster                                  | `how-to/add-a-monster.md`                       |
| Add a new playable body, like the lurk         | `how-to/add-a-body.md`                          |
| Add a potion, wand, weapon, ring               | `how-to/add-an-item.md`                         |
| Add an active spell                            | `how-to/add-a-spell.md`                         |
| Add a trap                                     | `how-to/add-a-trap.md`                          |
| Add a property like "fire immune"              | `how-to/add-an-effect.md`                       |
| Add a way for a creature to think              | `how-to/add-a-rule-set.md`                      |
| Add one reflex to that thinking                | `how-to/add-a-rule.md`                          |
| Make something rarer, or deeper                | `how-to/tune-rarity-and-depth.md`               |
| Add a whole new *kind* of item                 | `how-to/add-an-item-category.md`                |
| Put a specific thing on a specific tile        | `how-to/spawn-a-thing.md`                       |
| See a change working in the real game          | `how-to/play-through-a-pty.md`                  |
| Look up a field, a type, a default             | `reference/content-tables.md`                   |
| Look up a component, resource, event           | `reference/components.md`                       |
| Change a balance number                        | `reference/constants.md`                        |
| Look up a function I have to call              | `reference/spawn-api.md`                        |
| Look up a flag or an env var                   | `reference/cli-and-env.md`                      |
| Look up how input and the turn loop work       | `reference/input-and-turn-loop.md`              |
| Look up what a monster does with its turn      | `reference/agents.md`                           |
| Look up how a frame gets to the screen         | `reference/rendering.md`                        |
| Reach an entity and change it                  | `how-to/work-with-the-ecs.md`                   |
| Understand why it is built this way            | `explanation/data-driven-content.md`            |
| Understand how bevy_ecs is used here           | `explanation/ecs-in-nihilurk.md`                |
| Understand why monsters think in rule sets     | `explanation/agents.md`                         |
| Know what an effect should look like           | `explanation/the-feel-layer.md`                 |
| Write or edit a page in here                   | `explanation/documentation-style.md`            |
| Know which pages to touch after a change       | `how-to/update-the-docs.md`                     |
| Know what good numbers look like               | `explanation/combat-and-balance.md`             |
| Know what shape to leave the code in           | `explanation/code-calisthenics.md`              |
| Check it still runs on a Pi                    | `how-to/run-the-compat-pipeline.md`             |
| Know how portability is tested                 | `explanation/cross-platform-testing.md`         |
| Cut a release, one command and a push          | `how-to/cut-a-release.md`                       |
| Publish a release to crates.io                 | `how-to/publish-to-crates-io.md`                |
| Publish a GitHub release                       | `how-to/publish-a-github-release.md`            |
| Publish the documentation site                 | `how-to/publish-the-site.md`                    |
| Know why package names differ from directories | `explanation/adr-0002-two-names-per-crate.md`   |

And architecture decision records. They keep the history the other pages leave out: what was tried, what was rejected, and when to reopen it.

    explanation/adr-0001-tables-not-raws.md                  content is compiled in, not loaded from JSON
    explanation/adr-0002-two-names-per-crate.md              package names differ from library names
    explanation/adr-0003-compat-checks-start-not-speed.md    the matrix checks that it starts, not that it is fast
    explanation/adr-0004-no-tests-of-presentation.md         presentation is checked by playing
    explanation/adr-0005-aim-resolves-before-the-mobs-move.md  a zap and a throw resolve before the mobs move
    explanation/adr-0006-effects-saved-by-id.md              effects are saved by string id
    explanation/adr-0007-aggravation-is-a-component.md       aggravation sits on top of the tactic
    explanation/adr-0008-player-actions-are-queued-intents.md  the player's step is a queued intent


How this is organised
---------------------

Four kinds of document, kept strictly apart, because a person adding a monster at 1am and a person deciding whether to fork the project need very different pages.

  tutorial/     A lesson. Follow it start to finish and you will have
                added something that works. It teaches; it does not
                cover every option.

  how-to/       A recipe for one real task. Assumes you already know
                your way around. Short, imperative, no theory.

  reference/    The dry facts. Every table, every field, every default,
                every function signature. Never a narrative.

  explanation/  Why the code is shaped the way it is. Read when you are
                deciding something, not when you are typing.

If a page starts to be two of these at once, split it.

How a page is *written* — the header block, the width, the voice, the things a page must not do — is `explanation/documentation-style.md`, inferred from the pages already here.


Who this is for
---------------

Every page names its own audience and prerequisites in a header. Broadly:

  Content author    Adds monsters, items, traps. Edits tables. Needs to
                    know Rust well enough to copy a line and change the
                    numbers. Lives in `tutorial/` and `how-to/`.

  Engine developer  Adds systems, changes how content is spawned or
                    resolved. Starts at `explanation/ecs-in-nihilurk.md` and
                    `how-to/work-with-the-ecs.md`, then lives in
                    `reference/` and `explanation/`.
                    `engine/` itself (input handling, rendering) is
                    thinner ground than `models/`: its tests
                    cover key dispatch (`update.rs`) and the log gate
                    and status tints (`view.rs`), not what a frame looks
                    like, so a change to a frame is checked by playing
                    the game, not by `cargo test`. See `reference/input-and-turn-loop.md`
                    and `reference/rendering.md`.

The game design document (`../gdd.md`) is a third thing again: what nihilurk is trying to *be*. It is not a spec of the code.


The crates
----------

The code is a workspace of small libraries and one binary. Pages name files by their path from the repo root, so this is the map. The directory names are the short ones below; the packages are `nihilurk-models`, `nihilurk-strings`, `nihilurk-particle-core` and `nihilurk` (for `engine/`), and `cargo -p` takes those. See `explanation/adr-0002-two-names-per-crate.md`.

  models/         The game. Rules, content tables, ECS systems, save
                  files. No input, no drawing. Almost everything in
                  `tutorial/`, `how-to/` and `reference/` lives here.
  strings/        Every player-facing sentence, one file per language,
                  chosen by a `lang-*` feature at build time.
  particle-core/  The particle arithmetic. `no_std`, no dependencies.
  engine/         The binary. Key handling, the turn loop, `Screen`
                  (the cell grid and its diff) and every layer that
                  paints into it. Also
                  `nihilurk-dispatch`, which picks a language binary.
  compat/         A test rig, outside a bare `cargo build` and
                  `cargo test`. See `how-to/run-the-compat-pipeline.md`.

Dependencies run one way, toward the leaves: `engine` uses `models` and `strings`; `models` uses `strings` and `particle-core`. Nothing depends on `engine`. `models` re-exports its public API from the crate root, so `use models::*;` reaches it. `helpers` is private bar four names (`Hit`, `apply_hit`, `chebyshev`, `mob_at`), and so is anything marked `pub(crate)`. A page that names one of those says so.


The rule that keeps this true
-----------------------------

Documentation ships with the change that makes it true. A pull request that adds a field, renames a table, or changes what a row means is not finished until the pages that describe it say so. Stale documentation is worse than none, because someone will believe it.

Three things make that cheap here:

  1. The docs describe *tables*, and the tables are short. There is rarely more than one page to touch.

  2. `models/tests/content.rs` enforces the claims this documentation makes about the tables -- unique names, reachable rows, depth gating, no catalog row spawning a dud -- and `models/tests/determinism.rs` enforces the one claim that protects everyone else's saved seeds: adding content cannot move a wall. `engine/tests/workspace.rs` is the third of that kind: it checks that a bare `cargo test` still covers every crate that ships. If a doc claim can be a test, make it a test and cite it here.

  3. `cargo run -p nihilurk -- -content` prints the live content list. It reads the tables, so it can never go stale. Prefer pointing a reader at it over pasting a list into a page.


Quick sanity check
------------------

    ./docs_style.sh                           these pages, house style
    cargo test                                # everything still works
    cargo test --test content                 # the tables specifically
    cargo test --test determinism             # seeds still mean what they meant
    cargo test -p nihilurk --test workspace     # the root test run still covers everything
    cargo run -p nihilurk -- -content           # what the game knows
    NIHILURK_SPAWN="dragon" cargo run -p nihilurk   # put one in front of me

And the compatibility pipeline, which is slower and answers a different question:

    ./compat_test.sh                          # does it still run on a Pi?
