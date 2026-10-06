How to update the docs after you add something
==============================================

    Audience       Anyone who just changed the game and is about to
                   open a pull request.
    Prerequisites  None.
    Result         You know which pages your change made false, and
                   you fix them before review.

Most additions need no docs edit. The tables are short, `cargo run -p nihilurk -- -content` prints them live, and no page lists a monster, a weapon or a trap by name. This page lists the exceptions: the places where a page keeps a hand-written list of something. Find your change below, touch those pages, run the check.

Paths are relative to this page, except `MANUAL.md`, `doc/`, `strings/` and `engine/`, which start at the repo root.


Look it up
----------

| You added or changed              | Touch                                                                 | Because                                      |
|-----------------------------------|-----------------------------------------------------------------------|----------------------------------------------|
| A monster, weapon, armour, coin, ammo, launcher or treat | nothing, unless it brings a new mechanic (see below)   | `-content` prints every row                  |
| A ring                            | `../reference/components.md`, the `RingEffect` line                      | the enum is pasted there                     |
| A potion                          | `../reference/components.md`, the `PotionEffect` line                    | the enum is pasted there                     |
| A scroll                          | `../reference/components.md`, the `ScrollEffect` line; `../explanation/the-feel-layer.md`, "What a scroll looks like" | the enum is pasted there; each scroll has a flourish |
| A wand                            | `../reference/components.md`, the `WandEffect` line; `../reference/content-tables.md`, the wand table; `../explanation/the-feel-layer.md`, "What a wand looks like" and the thrown-wand list | one table row per wand; a thrown wand blasts by kind |
| A spell                           | nothing, unless it adds a `constants::` module (see below)            | `SPELLS` is not in `-content`, but no page lists spells |
| A trap                            | `../reference/components.md`, the `TrapEffect` line                      | the enum is pasted there                     |
| An effect                         | `../reference/content-tables.md`, one row in the `EFFECTS` table         | the table has one row per id                 |
| An ability                        | nothing                                                               | the `ABILITIES` page describes fields, not rows |
| A playable body                   | `../reference/cli-and-env.md`, the `-b` section; `../reference/components.md`; `../reference/constants.md`; `MANUAL.md`, "Who goes down"; `doc/nihilurk.6` | each says what the bodies are |
| A rule                            | `../reference/agents.md`, the rules table                                | one table row per rule                       |
| A rule set                        | `../reference/agents.md`, the sets table                                 | one table row per set                        |
| A whole item category             | `../reference/content-tables.md`, "Where everything is", a section for the new `Def`, and the `DROPS` weights table; `add-an-item.md`, "At a glance"; `tune-rarity-and-depth.md`, the `DROPS` listing | each lists the categories |
| A kind of entity that is not an item, monster or trap | `../reference/content-tables.md`, a section for its table; `../reference/components.md`, anything new it carries | the table and the components are the register |
| A `constants::` module            | `../reference/constants.md`, one row in the modules table                | nothing else checks it                       |
| A component, resource or event    | `../reference/components.md`                                             | it is the register                           |
| A system in the turn schedule     | `../reference/input-and-turn-loop.md`, the schedule list                 | the order is the page                        |
| A key                             | `MANUAL.md`, "Useful commands" and the quick reference; `doc/nihilurk.6`, "GAME CONTROLS"; `../reference/input-and-turn-loop.md`, the key table | three places list keys |
| A flag or an env var              | `../reference/cli-and-env.md`; `doc/nihilurk.6`; `print_help` in `engine/src/main.rs` | the `-h` text is separate from the page     |
| A player-facing sentence          | a new function in `strings/src/en.rs`                                 | see "Player-facing text" below               |
| A new page                        | `../README.md`, one row in the index; a `See also` block | an unlisted page is not found              |

A secret (the `T` key, the `-pride` flags) stays out of `MANUAL.md` and `../reference/cli-and-env.md`. See `../explanation/documentation-style.md`.


When the change is a mechanic
-----------------------------

If your change alters what a player can do or has to know (polymorph lends powers, a staff changes what spells cost, a trap now fires for monsters), the rule belongs in `MANUAL.md` in the player's voice, and in the reference page for the system it touches. `../../gdd.md` changes only when the design does, not when the code does.

If your change settles or reverses a design question, write an ADR in the shape of `../explanation/adr-0001-tables-not-raws.md` and add it to the list in `../README.md`. The history goes there, and nowhere else.


Player-facing text
------------------

Every sentence the player reads is a function in `strings/src/en.rs`. `es.rs`, `pt.rs` and `ht.rs` re-export English until someone translates the function, so a new English function needs no other file. If a language stops building, run `lua .githooks/stub_missing_translations.lua`. English you write starts unreviewed: the pre-commit hook tags each new `en.rs` function `// TODO: placeholder English; needs a human's pass.`, and whoever has read it and stands behind it deletes the tag, reworded or not.


The three habits
----------------

1. **Name the constant, not its value.** A number copied into prose goes stale on the first rebalance. The same goes for counts: "nine tables" and "all six traps" were each wrong within a release. Say "every table".
2. **Search for what you renamed.** `docs_style.sh` checks paths and links, not identifiers. After a rename or a removed mechanic, run `grep -rn 'old_name' docs MANUAL.md doc`.
3. **Cite the test.** If a claim can be a test, make it one and name it in the page.


Check
-----

    ./docs_style.sh                    # house style, links, paths
    cargo test --test content          # the claims the how-tos make about the tables
    grep -rn 'old_name' docs MANUAL.md doc   # what your rename left behind

With `git config core.hooksPath .githooks` on, the pre-commit hook runs `../../.githooks/docs_nudge.lua` and names the pages from the table above that your staged change leaves out. It reminds and never refuses. A row you add to the table above needs a rule in `../../.githooks/docs_nudge_rules.lua`, or a stated reason no diff can see it; `../../.githooks/docs_nudge_test.lua` refuses the commit while the two disagree.


See also
--------

  add-an-item.md                   the recipes this page follows up
  ../explanation/documentation-style.md   the shape and voice of a page
  ../README.md                     the index, and the rule that keeps docs true
  ../reference/constants.md        the modules table you add a row to
