Reference: command line and environment
=======================================

    Audience       Anyone running the game, especially to test content.
    Prerequisites  None.
    Status         Flags as parsed in `engine/src/main.rs`.

    cargo run -p engine -- [flags] [name-or-save]


Flags
-----

Single dash, in any order. Unrecognised arguments are treated as the
positional argument, so a typo becomes a player name rather than an
error.

| Flag         | Effect                                                    |
|--------------|-----------------------------------------------------------|
| `-s <seed>`  | Start the run from a specific `u64` seed. Reproducible.   |
| `-c`         | Centre the map on the player instead of a fixed viewport. |
| `-ns`        | No save. The run is never written to disk.                |
| `-nb`        | No blood. Suppresses bloodstain rendering, and with it the flung-corpse-and-bones death animation — a kill just leaves a static grey corpse mark. |
| `-dropthrow` | Swap the pack menu order to Use / Drop / Throw.           |
| `-content`   | Print every name the content tables know, then exit.      |
| `-anim-rate <n>` | Multiplier on every animation frame's on-screen hold time (particles, the magic-mapping reveal wipe). `1.0` is the default pacing; raise it if a terminal's redraw can't keep up, lower it for snappier animations. Clamped to `0.1..=5.0`; a bad or missing value falls back to `1.0`. |

`-content` never touches the alternate screen, so it pipes:

    cargo run -p engine -- -content | grep ring
    cargo run -p engine -- -content | less

`-s` fixes the dungeon's *maps*. Every floor's walls are a pure function
of `(seed, depth)`, so floor 7 of seed 1234 is the same maze today,
tomorrow, after you reload a save, and after somebody adds a monster to
the bestiary. Its *contents* -- monsters, loot, traps -- are re-rolled
each time you enter the floor (they key off the staircase count as well),
so walking back up through floor 7 finds the same corridors freshly
stocked. How you play still does not reach into generation: two runs on
one seed that take the same staircases see the same everything.


The positional argument
-----------------------

One bare argument, meaning one of two things:

  * **A save file**, if it names a file that exists -- verbatim or with
    `.sav` appended, matched case-insensitively. The run is loaded.
  * **A player name**, otherwise. A fresh run starts under that name.

        cargo run -p engine -- roog          # loads roog.sav if it exists
        cargo run -p engine -- Bae           # otherwise: a new run as Bae

If the save is *clear data* -- a won run, which is kept rather than
deleted -- the game asks before spending it.


Environment variables
---------------------

### ROOG_SPAWN

Comma-separated content names, dropped on free tiles around the player
the moment a floor is built. The content author's shortcut: it means you
never have to play down to floor 9 to look at a floor-9 monster.

    ROOG_SPAWN="dragon" cargo run -p engine
    ROOG_SPAWN="bow,arrow,ring of protection" cargo run -p engine
    ROOG_SPAWN="dart trap, long sword" cargo run -p engine

Details:

  * Any name from `-content` works -- monsters, items, traps, the relic.
  * Whitespace around each name is trimmed; empty entries are skipped.
  * A name the tables do not know is skipped **silently**. This is a
    debug knob, not a parser. Check your spelling against `-content`.
  * Things are placed on the nearest free walkable tiles, searching
    outward in rings from the player. Nothing lands in a wall or on top
    of anything else.
  * It applies to **every floor**, not just the first. Descend and your
    dragon is waiting again.
  * Items arrive exactly as their row describes them -- unenchanted,
    uncharged, a single arrow rather than a bundle. For the randomised
    version, find one on the floor.

### ROOG_MAGICMAP

Forces the animation a scroll of magic mapping plays, instead of rolling
one. Useful when you are working on the animation itself.

| Value                                         | Style      |
|-----------------------------------------------|------------|
| `row`, `rows`, `rowbyrow`, `row-by-row`, `curtain` | Row by row |
| `spiral`, `swirl`                             | Spiral     |
| `explode`, `explosion`, `blast`, `burst`      | Explode    |

Anything unrecognised falls back to a random roll.


Recipes
-------

Look at a new monster immediately:

    ROOG_SPAWN="basilisk" cargo run -p engine

Reproduce a run someone reported:

    cargo run -p engine -- -s 1234567 -ns

Check a name before you use it:

    cargo run -p engine -- -content | grep -i staff

Test a throw build from turn one:

    ROOG_SPAWN="bow,arrow,arrow,ring of dexterity" cargo run -p engine


See also
--------

  spawn-api.md                  spawn_named, spawn_requested
  content-tables.md             what the names refer to
  ../tutorial/add-your-first-item.md   these tools in context
