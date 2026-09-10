roog documentation
==================

This is the documentation for people who want to *put things in the
dungeon*: monsters, items, traps, magic. It lives in the repository, in
plain text, wide enough to `cat` and short enough to `head`.

    cat docs/how-to/add-a-monster.md


Start here
----------

| I want to...                        | Read                                |
|-------------------------------------|-------------------------------------|
| Add my first thing, hand-held       | `tutorial/add-your-first-item.md`   |
| Add a monster                       | `how-to/add-a-monster.md`           |
| Add a potion, wand, weapon, ring    | `how-to/add-an-item.md`             |
| Add a trap                          | `how-to/add-a-trap.md`              |
| Add a property like "fire immune"   | `how-to/add-an-effect.md`           |
| Make something rarer, or deeper     | `how-to/tune-rarity-and-depth.md`   |
| Add a whole new *kind* of item      | `how-to/add-an-item-category.md`    |
| Look up a field, a type, a default  | `reference/content-tables.md`       |
| Look up a component, resource, event| `reference/components.md`           |
| Change a balance number             | `reference/constants.md`            |
| Look up a function I have to call   | `reference/spawn-api.md`            |
| Look up a flag or an env var        | `reference/cli-and-env.md`          |
| Understand why it is built this way | `explanation/data-driven-content.md`|
| Know what good numbers look like    | `explanation/combat-and-balance.md` |

And one architecture decision record, on why content is compiled into the
binary rather than loaded from JSON raw files:

    explanation/adr-0001-tables-not-raws.md


How this is organised
---------------------

Four kinds of document, kept strictly apart, because a person adding a
monster at 1am and a person deciding whether to fork the project need very
different pages.

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


Who this is for
---------------

Every page names its own audience and prerequisites in a header. Broadly:

  Content author    Adds monsters, items, traps. Edits tables. Needs to
                    know Rust well enough to copy a line and change the
                    numbers. Lives in `tutorial/` and `how-to/`.

  Engine developer  Adds systems, changes how content is spawned or
                    resolved. Lives in `reference/` and `explanation/`.

The game design document (`../gdd.md`) is a third thing again: what roog
is trying to *be*. It is not a spec of the code.


The rule that keeps this true
-----------------------------

Documentation ships with the change that makes it true. A pull request
that adds a field, renames a table, or changes what a row means is not
finished until the pages that describe it say so. Stale documentation is
worse than none, because someone will believe it.

Three things make that cheap here:

  1. The docs describe *tables*, and the tables are short. There is
     rarely more than one page to touch.

  2. `models/tests/content.rs` enforces the claims this documentation
     makes about the tables -- unique names, reachable rows, depth
     gating, appearance pools -- and `models/tests/determinism.rs`
     enforces the one claim that protects everyone else's saved seeds:
     adding content cannot move a wall. If a doc claim can be a test,
     make it a test and cite it here.

  3. `cargo run -p engine -- -content` prints the live content list.
     It reads the tables, so it can never go stale. Prefer pointing a
     reader at it over pasting a list into a page.


Quick sanity check
------------------

    cargo test                                # everything still works
    cargo test --test content                 # the tables specifically
    cargo test --test determinism             # seeds still mean what they meant
    cargo run -p engine -- -content           # what the game knows
    ROOG_SPAWN="dragon" cargo run -p engine   # put one in front of me
