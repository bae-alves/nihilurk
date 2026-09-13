roog documentation
==================

This is the documentation for people who want to *put things in the dungeon*: monsters, items, traps, magic. It lives in the repository, in plain text, wide enough to `cat` and short enough to `head`.

    cat docs/how-to/add-a-monster.md


Start here
----------

| I want to...                        | Read                                    |
|-------------------------------------|-----------------------------------------|
| Add my first item, hand-held        | `tutorial/add-your-first-item.md`       |
| Add my first monster, hand-held     | `tutorial/add-your-first-monster.md`    |
| Add my first move, hand-held        | `tutorial/add-your-first-move.md`       |
| Add a monster                       | `how-to/add-a-monster.md`               |
| Add a potion, wand, weapon, ring    | `how-to/add-an-item.md`                 |
| Add an active move                  | `how-to/add-a-move.md`                  |
| Add a trap                          | `how-to/add-a-trap.md`                  |
| Add a property like "fire immune"   | `how-to/add-an-effect.md`               |
| Make something rarer, or deeper     | `how-to/tune-rarity-and-depth.md`       |
| Add a whole new *kind* of item      | `how-to/add-an-item-category.md`        |
| Put a specific thing on a specific tile | `how-to/spawn-a-thing.md`           |
| Look up a field, a type, a default  | `reference/content-tables.md`           |
| Look up a component, resource, event| `reference/components.md`               |
| Change a balance number             | `reference/constants.md`                |
| Look up a function I have to call   | `reference/spawn-api.md`                |
| Look up a flag or an env var        | `reference/cli-and-env.md`              |
| Look up how input and the turn loop work | `reference/input-and-turn-loop.md` |
| Look up how a frame gets to the screen | `reference/rendering.md`             |
| Reach an entity and change it       | `how-to/work-with-the-ecs.md`           |
| Understand why it is built this way | `explanation/data-driven-content.md`    |
| Understand how bevy_ecs is used here| `explanation/ecs-in-roog.md`            |
| Know what an effect should look like | `explanation/the-feel-layer.md`        |
| Write or edit a page in here        | `explanation/documentation-style.md`    |
| Know what good numbers look like    | `explanation/combat-and-balance.md`     |
| Know what shape to leave the code in| `explanation/code-calisthenics.md`      |
| Check I did not make it slower      | `how-to/run-the-perf-pipeline.md`       |
| Know how performance is measured    | `explanation/performance-testing.md`    |
| Check it still runs on a Pi         | `how-to/run-the-compat-pipeline.md`     |
| Know how portability is tested      | `explanation/cross-platform-testing.md` |

And one architecture decision record, on why content is compiled into the binary rather than loaded from JSON raw files:

    explanation/adr-0001-tables-not-raws.md


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
                    resolved. Starts at `explanation/ecs-in-roog.md` and
                    `how-to/work-with-the-ecs.md`, then lives in
                    `reference/` and `explanation/`.
                    `engine/` itself (input handling, rendering) is
                    thinner ground than `models/`: it carries very
                    little test coverage — two numpad tests and
                    `view.rs`'s frame geometry — so a change there is
                    mostly checked by playing the game, not by
                    `cargo test`. See `reference/input-and-turn-loop.md`
                    and `reference/rendering.md`.

The game design document (`../gdd.md`) is a third thing again: what roog is trying to *be*. It is not a spec of the code.


The rule that keeps this true
-----------------------------

Documentation ships with the change that makes it true. A pull request that adds a field, renames a table, or changes what a row means is not finished until the pages that describe it say so. Stale documentation is worse than none, because someone will believe it.

Three things make that cheap here:

  1. The docs describe *tables*, and the tables are short. There is rarely more than one page to touch.

  2. `models/tests/content.rs` enforces the claims this documentation makes about the tables -- unique names, reachable rows, depth gating, appearance pools -- and `models/tests/determinism.rs` enforces the one claim that protects everyone else's saved seeds: adding content cannot move a wall. `engine/tests/workspace.rs` is the third of that kind: it checks that a bare `cargo test` still covers every crate that ships. If a doc claim can be a test, make it a test and cite it here.

  3. `cargo run -p engine -- -content` prints the live content list. It reads the tables, so it can never go stale. Prefer pointing a reader at it over pasting a list into a page.


Quick sanity check
------------------

    ./docs_style.sh                           these pages, house style
    cargo test                                # everything still works
    cargo test --test content                 # the tables specifically
    cargo test --test determinism             # seeds still mean what they meant
    cargo test -p engine --test workspace     # the root test run still covers everything
    cargo run -p engine -- -content           # what the game knows
    ROOG_SPAWN="dragon" cargo run -p engine   # put one in front of me

And the two pipelines, which are slower and answer different questions:

    ./perf_test.sh                            # did I make it slower?
    ./compat_test.sh                          # does it still run on a Pi?
