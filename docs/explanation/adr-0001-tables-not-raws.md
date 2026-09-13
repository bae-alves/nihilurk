ADR 0001: content lives in Rust tables, not in raw data files
=============================================================

    Status         Accepted
    Audience       Engine developer evaluating or revisiting this
                   decision. Assumes Rust, ECS and serialisation
                   background. Not for content authors -- you want
                   `../how-to/add-a-monster.md`.
    Supersedes     --
    Related        data-driven-content.md


Context
-------

roog's content had grown into a shape that was already half data: one `const` table per item kind, a bestiary table, a row per thing. Three things were still hand-written code, and each was a place where adding content meant editing something unrelated to the content:

  * The loot table was a `match` over hand-computed percentage ranges in `map.rs` (now `map/population.rs`). Adding a category meant recomputing every boundary.
  * Monster depth gating was a `tier` field decoded by a formula (`(depth - 1) / 2`, capped at 3) that lived in the spawner, not on the row.
  * Traps had no table at all -- six enum variants, a `label()` match, a `const ALL` array, and a uniform pick.

There was also no single entry point for "give me one of X by name". Tests and any future scripting had to know which of nine `spawn_*` helpers to call.

The reference for fixing this is chapter 45 of *Roguelike Tutorial - In Rust* (bfnightly.bracketproductions.com/chapter_45.html), which argues for moving entity definitions out of source entirely and into external JSON "raw" files, loaded at startup into a `RawMaster` registry indexed by name, with weighted spawn tables carrying `min_depth`.

The question was how much of that to adopt.


Decision
--------

**Adopt the chapter's structural ideas. Reject its storage format.**

Adopted:

  1. **One spawn entry point keyed by name.** `spawn_named(world, name, pos)` resolves any name the game knows -- monster, item, trap, relic -- and returns `None` on a miss. This is `spawn_named_entity` from the chapter.

  2. **Spawn tables as data, decoupled from definitions.** `DROPS` is a weighted, depth-gated list of categories. `MonsterDef` and `TrapDef` carry `weight` and `min_depth` on the row. `pick_weighted` is the one draw function all three share.

  3. **A registry that reads the tables.** `content_names()` enumerates everything, which backs the `-content` listing and the table tests.

Rejected:

  4. **External JSON/RON raw files parsed at run time.** Content stays in `const` tables in Rust source.


Rationale for rejecting external raws
-------------------------------------

**Portability.** roog is meant to run on anything with a terminal, and ships as one static binary with nothing beside it. Raw files introduce a data-directory lookup -- relative to the binary? the working directory? an install prefix? -- and a class of failure ("could not find `raws/spawns.json`") that a single binary simply does not have. Embedding the files with `include_str!` avoids that but throws away the runtime editability that was the whole point.

**Compile-time checking is worth more here than late binding.** A typo in a `Color`, a missing field, a `Grant` naming a component that does not exist -- all of these are compile errors today. In JSON they become startup panics at best and silent defaults at worst. The tables are edited by the same people who edit the engine; giving up the type system to serve a designer who does not exist yet is a bad trade.

**`Grant` cannot survive the trip.** The effect system's central trick is that a `const` table names a *component* -- `Grant::of::<FireImmune>()` is a const handle holding function pointers that attach, detach and probe one type. JSON can only carry the string `"FireImmune"`, so a raw-file port would need a name-to-`Grant` lookup table maintained by hand: a second list of every effect, which is exactly the kind of thing this architecture exists to avoid.

**The save format is coupled to the tables by name.** Items are saved as a name and rebuilt via `restore_from_catalog`; monsters recover their innate grants by bestiary lookup. That coupling is good -- it keeps saves small -- but it means a raw file and a save file can disagree. Compiled tables cannot drift from the binary that reads them.

**The iteration win is smaller than it looks.** The argument for raws is "edit and rerun without a rebuild". A content-only rebuild here is about **1.2 seconds**. Against that, `ROOG_SPAWN` removes the far larger cost, which was never compilation -- it was playing down to floor 9 to see a floor-9 monster.

**Three dependencies and a parse step, for nothing measurable.** A raw port would add a format crate, a loader, an error path, and a startup cost, to serve a workflow already served.


Consequences
------------

Accepted costs:

  * **Adding content requires a Rust toolchain and a rebuild.** Someone who wants to add a monster must be able to run `cargo build`. This rules out non-programmer modders and end-user modding entirely.

  * **No runtime content.** No downloadable content packs, no user-supplied bestiaries, no reloading a table without restarting.

  * **Content changes are code changes**, so they go through code review and version control. This is also a benefit, and it is why the tables read like data files: `#[rustfmt::skip]` and column alignment are there so a row can be diffed and reviewed as a row.

Benefits realised:

  * Adding a weapon, armour, coin, ammunition or launcher is one line and no other edit. Adding a monster or a trap is one line plus, for a trap, a mechanic the compiler demands.

  * The loot table has no arithmetic in it. Category weights are relative, so a new category cannot invalidate an existing one.

  * `content_names()` and `spawn_named` gave `-content`, `ROOG_SPAWN` and a table-driven test suite for one function each.

  * Zero new dependencies. The binary is still self-contained.


When to revisit
---------------

Reopen this decision if any of these becomes true:

  * **Modding becomes a goal.** If players are meant to add content, they cannot be asked to install Rust. That alone overturns this.

  * **A designer without Rust joins.** The trade above assumes the people editing tables are the people editing the engine.

  * **The tables outgrow review.** At a few hundred rows they still read well. At a few thousand -- procedural generation of content, say -- a data format with its own tooling starts to pay for itself.

  * **Content needs to change without a release.** Live tuning, remote balance patches, A/B testing a drop rate.

The migration path is deliberately short. Every table is already a flat list of plain-data rows behind a `pick`/`lookup`/`spawn` interface, and every consumer goes through those. Swapping the backing store for a parsed file means reimplementing `MonsterDef::lookup`, `TrapDef::lookup` and `one_named` -- and the one genuinely hard part, `Grant`, which would need a name-to-handle map. Nothing else in the codebase would notice.


See also
--------

  data-driven-content.md           what the architecture buys, in prose
  ../reference/spawn-api.md        the interface this ADR is about
  bracketproductions.com/chapter_45.html  the raws design this rejects
