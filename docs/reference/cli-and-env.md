Reference: command line and environment
=======================================

    Audience       Anyone running the game, especially to test content.
    Prerequisites  None.
    Status         Describes the flags as parsed in
                   `engine/src/main.rs`. If this page and the source
                   disagree, the source is right and this page is a bug.

    cargo run -p engine -- [flags] [name-or-save]


Flags
-----

Single dash, in any order. Unrecognised arguments are treated as the positional argument, so a typo becomes a player name rather than an error.

| Flag         | Effect                                                    |
|--------------|-----------------------------------------------------------|
| `-s <seed>`  | Start the run from a specific `u64` seed. Reproducible.   |
| `-c`         | Centre the map on the player instead of a fixed viewport. |
| `-ns`        | No save. The run is never written to disk.                |
| `-nb`        | No blood. Suppresses bloodstain rendering, and with it the flung-corpse-and-bones death animation — a kill just leaves a static grey corpse mark. |
| `-nshake`    | No screen shake. The map never leaves its moorings — nothing arms one for the rest of the run. For anyone who would rather the terminal held still; `-anim-rate` can only make a shake *slower*, which is the wrong direction. |
| `-nobones`   | Skip the bones mechanic entirely: a death never writes a `bones-N.sav`, and an ascent never reads one. Not the corpse-fling animation `-nb` mentions above — this is the NetHack-style "a past run's ghost, guarding its own cursed gear" (`models::bones`). |
| `-content`   | Print every name the content tables know, then exit.      |
| `-scores`    | Print the leaderboard (top 10 scores ever recorded), then exit. |
| `-anim-rate <n>` | Multiplier on every animation frame's on-screen hold time (particles, the magic-mapping reveal wipe, the screen shake). `1.0` is the default pacing; raise it if a terminal's redraw can't keep up, lower it for snappier animations. Clamped to `0.1..=5.0`; a bad or missing value falls back to `1.0`. |
| `-b <body>` | Play as `nihil` (the default) or `lurk`. See below. |
| `-am <species>` | Play *as* a monster: any bestiary name (`-am dragon`). See below. |

### `-b <body>`

Which of the two written-to-be-played creatures descends. `-b nihil` is the
default and changes nothing.

`-b lurk` is the other one: quadruped, fanged, clawed, furred, and a magenta
`@`. Eight hit points and two magic against nihil's twelve and four; the same
bare attack and defence dice nihil starts with, and no way to ever add to them
with gear -- it carries nothing, and the only thing it can put on is a ring.

What it has instead:

  * `SpeedKind::Quick`, half again as fast as `Normal` and the only creature
    in the game born at that tempo. Two monster rounds bought per three player
    turns; shown as `QUIK` on the status line.
  * The estoc's lunge (`Lunges`), without the estoc's double time.
  * The rapier's momentum (`BuildsMomentum`), built on the creature rather
    than on a blade it does not have.
  * A ring of stealth's quiet (`Stealthy`).
  * Bide in the spell bar, paid for at the usual cost -- it is learned, not
    innate.
  * **Growth.** Every creature that dies on the floor is rolled against
    `lurk::GROWTH_CHANCE` (15%), and a hit puts one point on one of the four
    numbers on the status line, drawn at random: `FEAR THE WOLF!` Rolled per
    corpse rather than counted toward a tenth one, so there is no counter to
    pace against and nothing for a save file to remember.

Its tempo is deliberately lumpy: two monster rounds per three player turns
means the free turn always lands third, and pacing yourself against that beat
is how a lurk is played.

A wand of cancellation strips everything on that list except what the lurk
*is* -- see [Bodies and species](#bodies-and-species).

### `-am <species>`

The run starts in that species' body instead of nihil's or the lurk's. The
name must be
a bestiary row exactly as `-content` prints it; anything else stops the game
before it starts rather than quietly starting you as nihil. `-b` and `-am`
are the same choice asked two ways, so passing both is refused too.

What the body changes is what the bestiary row says: the glyph and its colour,
the hit points, the attack and armour dice and their bonuses, the tempo (`-am
wraith` is permanently hasted), invisibility (`-am phantom`), and the innate
magic -- a dragon is immune to fire and flies, a rattlesnake's bite is
venomous, a slime splits when hurt -- into a *hostile* copy of you, carrying
your hit points, which is working as intended. None of that is a special case
for the player: the bestiary row is the same one a dragon on floor 10 is built
from, and the on-hit abilities are the same table.

What it does not change is who you are. You keep the `@`'s side of the fight,
your viewshed, your pack, your score and your magic points; `ai` never gets
hold of you.

Three consequences worth knowing before you pick a rat:

  * **You start with nothing.** The ring mail, mace, short bow, quiver and
    potion are *nihil's* kit.
  * **Only a species that uses items can wear them.** That is the bestiary's
    own `ItemUser` mark -- the orc, hobgoblin, centaur, medusa, nymph,
    leprechaun, troll, vampire and ur-vile have hands; a dragon has claws and
    is told so when it tries.
  * **Innate magic that is a spell lands in your spell bar, free.** A dragon
    knows Fireball at zero magic cost, because a dragon has no magic points
    and never did. It is the same spell a hero coin teaches, cast through the
    same reticle, and the dragons on floor 10 breathe it too.

**This will never be a balanced mode of play.** A floor-1 kestral and a
floor-10 dragon are both one `-am` away, and nothing gates which one you're
allowed to start as, or scales the dungeon to match your pick. That's
deliberate: `-am` is a costume, not a difficulty setting, and every row in
the bestiary is tuned to be one thing a `@` fights, never to be the `@`
fighting everything else. Play it for the joke, the curiosity, or the
content-author's need to see a species from the inside -- not for a fair
fight.

### Bodies and species

Cancellation draws a line between the two, on purpose.

A **body** cannot be cancelled. It is the creature the run is about, and no
wand in the dungeon is a wand of being something else: a lurk stays a lurk,
keeps its shape, and keeps the rule about rings.

A **species** can. A player wearing a bestiary row keeps the shape and the
dice -- those are not effects at all -- but the magic is only on loan: a
cancelled dragon-bodied player loses fire immunity exactly as a real dragon
would, and a cancelled orc-bodied one loses `ItemUser`, which is to say the
wits to work a buckle, and can no longer equip what it was wearing a moment
ago. That is the bargain of wearing something else's magic.

The body is saved with the run and comes back on load -- the player's *name*
is the species, and that is how the save knows. Two argument shapes are
refused up front to keep that true, both before the terminal is touched:

    nihilurk Dragon             # "Don't name yourself a monster. It's quite demeaning."
    nihilurk -am orc mysave     # the save already knows what body it is in
    nihilurk -b lurk Bae        # a name is the first argument or it is not a name

A monster's hit points are its own, so several rows are a two-hit death: that
is the joke, and `-s` is how you retry it.

`-h`, `-help`, and `--help` print a short guide and exit before the terminal is configured. The full reference is installed as `nihilurk(6)`:

    man nihilurk

`-content` never touches the alternate screen, so it pipes:

    cargo run -p engine -- -content | grep ring
    cargo run -p engine -- -content | less

`-scores` reads the same way, straight off `leaderboard.sav` in the current directory -- a small postcard file, same shape as a save, holding the ten highest scores ever recorded across every run that ended in a win or a death (a quit-and-save doesn't count; the run isn't over). Each line it prints is `RANK. NAME - OUTCOME - SCORE (WHEN)`, where `OUTCOME` is `WIN`, `LOSE (Asc.)` (dead on the way back out, carrying the Element), or `LOSE (Desc.)` (dead on the way down), and `WHEN` is the UTC date and time the run ended.

`-s` fixes the dungeon's *maps*. Every floor's walls are a pure function of `(seed, depth)`, so floor 7 of seed 1234 is the same maze today, tomorrow, after you reload a save, and after somebody adds a monster to the bestiary. Its *contents* -- monsters, loot, traps -- are re-rolled each time you enter the floor (they key off the staircase count as well), so walking back up through floor 7 finds the same corridors freshly stocked. How you play still does not reach into generation: two runs on one seed that take the same staircases see the same everything.


The positional argument
-----------------------

One bare argument, meaning one of two things:

  * **A save file**, if it names a file that exists -- verbatim or with `.sav` appended, matched case-insensitively. The run is loaded.
  * **A player name**, otherwise. A fresh run starts under that name.

        cargo run -p engine -- nihilurk          # loads nihilurk.sav if it exists
        cargo run -p engine -- Bae           # otherwise: a new run as Bae

If the save is *clear data* -- a won run, which is kept rather than deleted -- the game asks before spending it.


Environment variables
---------------------

### NIHILURK_SPAWN

Comma-separated content names, dropped on free tiles around the player the moment a floor is built. The content author's shortcut: it means you never have to play down to floor 9 to look at a floor-9 monster.

    NIHILURK_SPAWN="dragon" cargo run -p engine
    NIHILURK_SPAWN="bow,arrow,ring of protection" cargo run -p engine
    NIHILURK_SPAWN="dart trap, long sword" cargo run -p engine

Details:

  * Any name from `-content` works -- monsters, items, traps, the relic.
  * Whitespace around each name is trimmed; empty entries are skipped.
  * A name the tables do not know is skipped **silently**. This is a debug knob, not a parser. Check your spelling against `-content`.
  * Things are placed on the nearest free walkable tiles, searching outward in rings from the player. Nothing lands in a wall or on top of anything else.
  * It applies to **every floor**, not just the first. Descend and your dragon is waiting again.
  * Items arrive exactly as their row describes them -- unenchanted, uncharged, a single arrow rather than a bundle. For the randomised version, find one on the floor.

### NIHILURK_LEVEL

Makes every floor one kind of special level instead of rolling for it. Most special levels turn up on one floor in a hundred, from floor 6 down; this is how you look at one without hunting for a seed that has it.

    NIHILURK_LEVEL=island cargo run -p engine

| Value                          | Level       |
|--------------------------------|-------------|
| `battlefield`                  | Battlefield |
| `labyrinth`                    | Labyrinth   |
| `vault`                        | Vault       |
| `bee`, `beeworld`, `bee world` | Bee World   |
| `castle`                       | Castle      |
| `island`                       | Island      |

Details:

  * Case does not matter. A value that names no level is ignored, and floors roll as usual.
  * It applies to **every floor**, the first and the last included -- floors the roll itself never touches.
  * A save rebuilds its map from the seed on load, and this is read then too. Load a save with the same value it was written with, or the floor comes back a different shape under everything standing on it.

### NIHILURK_MAGICMAP

Forces the animation a scroll of magic mapping plays, instead of rolling one. Useful when you are working on the animation itself.

| Value                                         | Style      |
|-----------------------------------------------|------------|
| `row`, `rows`, `rowbyrow`, `row-by-row`, `curtain` | Row by row |
| `spiral`, `swirl`                             | Spiral     |
| `explode`, `explosion`, `blast`, `burst`      | Explode    |

Anything unrecognised falls back to a random roll.


Recipes
-------

Look at a new monster immediately:

    NIHILURK_SPAWN="basilisk" cargo run -p engine

Reproduce a run someone reported:

    cargo run -p engine -- -s 1234567 -ns

Walk around an island from turn one:

    NIHILURK_LEVEL=island cargo run -p engine

Check a name before you use it:

    cargo run -p engine -- -content | grep -i staff

Test a throw build from turn one:

    NIHILURK_SPAWN="bow,arrow,arrow,ring of sharpshooting" cargo run -p engine


See also
--------

  spawn-api.md                  spawn_named, spawn_requested
  content-tables.md             what the names refer to
  input-and-turn-loop.md        where these flags are read (`engine/src/main.rs`)
  rendering.md                  `-c`, `-anim-rate` and `-nshake` in the renderer
  ../tutorial/add-your-first-item.md   these tools in context
