Reference: command line and environment
=======================================

    Audience       Anyone running the game, especially to test content.
    Prerequisites  None.
    Status         Describes the flags as parsed in
                   `engine/src/main.rs`. If this page and the source
                   disagree, the source is right and this page is a bug.

    cargo run -p nihilurk -- [flags] [name-or-save]

After `cargo install nihilurk`, run `nihilurk [flags] [name-or-save]` instead. If the shell cannot find it, `~/.cargo/bin` is not on your `PATH`; the README's install steps shows how to add it.


Flags
-----

Single dash, in any order. Unrecognised arguments are treated as the positional argument, so a typo becomes a player name rather than an error.

| Flag         | Effect                                                    |
|--------------|-----------------------------------------------------------|
| `-s <seed>`  | Start the run from a specific `u64` seed. Reproducible.   |
| `-c`         | Centre the map on the terminal instead of on top left. |
| `-ns`        | No save. The run is never written to disk.                |
| `-nb`        | No blood. Suppresses bloodstain rendering, and with it the flung-corpse-and-bones death animation — a kill just leaves a static grey corpse mark. |
| `-nshake`    | No screen shake. The map never leaves its moorings — nothing arms one for the rest of the run. For anyone who would rather the terminal held still; `-anim-rate` can only make a shake *slower*, which is the wrong direction. |
| `-nobones`   | Skip the bones mechanic entirely: a death never writes a `bones-N.sav`, and an ascent never reads one. Not the corpse-fling animation `-nb` mentions above — this is the NetHack-style "a past run's ghost, guarding its own cursed gear" (`models::bones`). |
| `-endless`   | No Element of Yoord ever spawns, so there is no way to win: the dungeon keeps going down past `FINAL_DEPTH`. Sets the `Endless` resource (`models/src/map.rs`); the depth-`FINAL_DEPTH` checks in `traps.rs`, `monsters.rs` and `saveload.rs` read it. |
| `-content`   | Print every name the content tables know, then exit.      |
| `-scores`    | Print the leaderboard (top 10 scores ever recorded), then exit. |
| `-anim-rate <n>` | Multiplier on every animation frame's on-screen hold time (particles, the magic-mapping reveal wipe, the screen shake). `1.0` is the default pacing; raise it if a terminal's redraw can't keep up, lower it for snappier animations. Clamped to `0.1..=5.0`; a bad or missing value falls back to `1.0`. |
| `-b <body>` | Play as `nihil` (the default) or `lurk`. See below. |
| `-am <species>` | Play *as* a monster: any bestiary name (`-am dragon`). See below. |

### `-b <body>`

Which of the two written-to-be-played creatures descends. `-b nihil` is the
default and changes nothing.

`-b lurk` is the other one: quadruped, fanged, clawed, furred, and a magenta
`@`. Lower stats and the only thing it can put on is rings. What it has instead:

  * `SpeedKind::Quick`, half again as fast as `Normal` and the only creature
    in the game born at that tempo. Two monster rounds bought per three player
    turns; shown as `QUIK` on the status line.
  * The estoc's lunge (`Lunges`), without the estoc's double time.
  * The rapier's momentum (`BuildsMomentum`), built on the creature rather
    than on a blade it does not have.
  * A ring of stealth's quiet (`Stealthy`).
  * Bide in its spellset.
  * **Growth.** Every creature that dies on the floor is rolled against tuning
    constant `lurk::GROWTH_CHANCE`, and a hit puts one point on one of the four
    numbers on the status line, drawn at random: `FEAR THE LURK!` Rolled per
    kill rather than counted toward a tenth one, so there is no counter to
    pace against and nothing for a save file to remember.

Its tempo is deliberately lumpy: two monster rounds per three player turns
means the free turn always lands third, and pacing yourself against that beat
is how lurk is played.

### `-am <species>`

The run starts in that species' body instead of nihil's or lurk's. The
name must be a bestiary row exactly as `-content` prints it;
anything else stops the game before it starts rather than quietly starting
you as nihil. `-b` and `-am` are the same choice asked two ways,
so passing both is refused too.

What the body changes is what the bestiary row says: the glyph and its colour,
the hit points, the attack and armour dice and their bonuses, the tempo (`-am
wraith` is permanently hasted), invisibility (`-am phantom`), and the innate
magic -- a dragon is immune to fire and flies, a rattlesnake's bite is
venomous, a slime splits when hurt -- into a *hostile* copy of you, carrying
your hit points, which is working as intended. None of that is a special case
for the player: the bestiary row is the same one a dragon on floor 10 is built
from, and the on-hit abilities are the same table.

A row with gear chances (`.equip(...)`) starts you with all of it, every roll
hit, worn if the body may wear it (`ItemUser`) and in the pack if not. A bow
comes with a full stack of arrows. Only the start of a run does this; a
polymorph hands out nothing.

What it does not change is who you are. You keep the `@`'s side of the fight,
your viewshed, your pack, your score and your magic points; `ai` never gets
hold of you.

Four consequences worth knowing before you pick a monster:

  * **You start with nothing.** The ring mail, mace, short bow, quiver and
    potion are *nihil's* kit.
  * **Only a species that uses items can wear them.** That is the bestiary's
    own `ItemUser` mark -- the orc, hobgoblin, centaur, medusa, nymph,
    leprechaun, troll, vampire and ur-vile have hands; a dragon has claws and
    is told so when it tries.
  * **A species' spells land in your spellset, and cost your Ma.** A dragon
    knows Fireball, the same spell a hero coin teaches, cast through the
    same reticle at the same price out of your own pool. The dragons on
    floor 10 breathe it too, from a small pool of their own that never
    refills.
  * **The log tells you what you were born with.** One "You feel..." line
    per grant the body carries, a spirit's two random boons included:
    "You feel venomous." Tags the HUD already shows are not repeated.
**Try `-am dragon`.** You start on floor 1 with Fireball in your spell bar
and claws that hold no sword.

**This will never be a balanced mode of play.** A floor-1 kestral and a
floor-10 dragon are both one `-am` away, and nothing gates which one you're
allowed to start as, or scales the dungeon to match your pick. That's
deliberate: `-am` is a costume, not a difficulty setting, and every row in
the bestiary is tuned to be one thing a `@` fights, never to be the `@`
fighting everything else. Play it for the thrills, not for a fair fight.

### Bodies and species

Cancellation draws a line between the two, on purpose.

A **body** cannot be cancelled. It is the creature the run is about, and no
wand in the dungeon is a wand of being something else: a lurk stays a lurk,
keeps its shape, and keeps the rule about rings.

A **species** can, apart from any part its row marks as an identity effect (the dog's five grants, see `content-tables.md`, "Identity effects"). A player wearing a bestiary row keeps the shape and the
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

A monster's hit points are its own, so several rows are a one or two-hit death: that
is intended, and `-s` is how you retry it.

`-h`, `-help`, and `--help` print a short guide and exit before the terminal is configured. The full reference is `nihilurk(6)`. The prebuilt tarball and the AUR package install it, so `man nihilurk` works there; `cargo install` installs the binary only, so from a clone read it with:

    man ./doc/nihilurk.6

The game needs a terminal of at least 80 columns by 25 rows. In a smaller one the message log draws garbled letters. That is a known limit and will not be fixed.

`-content` never touches the alternate screen, so it pipes:

    cargo run -p nihilurk -- -content | grep ring
    cargo run -p nihilurk -- -content | less

`-scores` reads the same way, straight off `leaderboard.sav` in the current directory -- a small postcard file, same shape as a save, holding the ten highest scores ever recorded across every run that ended in a win or a death (a quit-and-save doesn't count; the run isn't over). Each line it prints is `RANK. NAME - OUTCOME - SCORE (WHEN)`, where `OUTCOME` is `WIN`, `LOSE (Asc.)` (dead on the way back out, carrying the Element), or `LOSE (Desc.)` (dead on the way down), and `WHEN` is the UTC date and time the run ended.

`-s` fixes the dungeon's *maps*. Every floor's walls are a pure function of `(seed, depth)`, so floor 7 of seed 1234 is the same maze today, tomorrow, after you reload a save, and after somebody adds a monster to the bestiary. Its *contents* -- monsters, loot, traps -- are re-rolled each time you enter the floor (they key off the staircase count as well), so walking back up through floor 7 finds the same corridors freshly stocked. How you play still does not reach into generation: two runs on one seed that take the same staircases see the same everything.


The positional argument
-----------------------

One bare argument, meaning one of two things:

  * **A save file**, if it names a file that exists -- verbatim or with `.sav` appended, matched case-insensitively. The run is loaded.
  * **A player name**, otherwise. A fresh run starts under that name.

        cargo run -p nihilurk -- nihilurk          # loads nihilurk.sav if it exists
        cargo run -p nihilurk -- bae           # otherwise: a new run as bae

If the save is *clear data* -- a won run, which is kept rather than deleted -- the game asks before spending it.


Language
--------

The language is a build-time choice, not a flag. `engine` forwards one `lang-*` feature (`en` by default, `pt`, `es`, `ht`) to `strings`, `models` and `view`, and the binary carries that language only:

    cargo run -p nihilurk --no-default-features --features lang-pt

The installed `nihilurk` command is `nihilurk-dispatch`, which reads `LC_ALL`, `LANG` or `LANGUAGE`, and `exec`s the matching `nihilurk-<lang>` beside it, passing every argument through. `--lang <en|pt|es|ht>` as the first two arguments overrides that and is stripped before forwarding. Anything it does not recognise gets English.


Environment variables
---------------------

### NIHILURK_SPAWN

Comma-separated content names, dropped on free tiles around the player the moment a floor is built. The content author's shortcut: it means you never have to play down to floor 9 to look at a floor-9 monster.

    NIHILURK_SPAWN="dragon" cargo run -p nihilurk
    NIHILURK_SPAWN="bow,arrow,ring of protection" cargo run -p nihilurk
    NIHILURK_SPAWN="dart trap, long sword" cargo run -p nihilurk
    NIHILURK_SPAWN="cursed -2 long sword,+3 ring mail,arrow x13" cargo run -p nihilurk

Details:

  * Any name from `-content` works -- monsters, items, traps, the Element itself.
  * Whitespace around each name is trimmed; empty entries are skipped.
  * Gear and ammunition take optional dressing, written around the name in this order: `[cursed] [+N|-N] name [xN]`.

    | Piece    | Example             | Effect |
    |----------|---------------------|--------|
    | `cursed` | `cursed ring mail`  | Adds the `Curse` tag: once worn or wielded it will not come off until a scroll of remove curse. |
    | `+N`/`-N`| `+3 long sword`     | Adds N to the item's flat bonus: a weapon's hit roll, armour's guard, or a launcher's throw (the arrows it looses). On a numeric ring (protection, strength, increase damage, sharpshooting) N is the ring's whole number, so `+3 ring of protection` is a +3 ring. Same code as the dungeon's own enchantment roll (`catalog::apply_bonus`). |
    | `xN`     | `arrow x13`         | Sets the stack size, clamped to `1..=STACK_LIMIT`. `x0` gives 1, and anything over the limit gives the limit. |

    A curse and a minus are independent: `-2 long sword` is not cursed, and `cursed +2 long sword` is. The quality is hidden until worn or identified, as for any gear.
  * A modifier the thing cannot use (a plus on a ring that is not a number, a stack on a dragon) is ignored, and the entry still counts as recognised.
  * A malformed modifier (`+x sword`) is not a modifier: it stays part of the name, so the entry is skipped like any unknown name.
  * A name the tables do not know is skipped **silently**. This is a debug knob, not a parser. Check your spelling against `-content`.
  * Things are placed on the nearest free walkable tiles, searching outward in rings from the player. Nothing lands in a wall or on top of anything else.
  * It applies to **every floor**, not just the first. Descend and your dragon is waiting again.
  * Items arrive exactly as their row describes them -- unenchanted, uncursed, a single arrow rather than a bundle -- unless you dress them as above. For the randomised version, find one on the floor.

### NIHILURK_LEVEL

Makes every floor one kind of special level instead of rolling for it. Most special levels turn up at their own small chance (`constants::map`), from `SPECIAL_LEVEL_MIN_DEPTH` down; this is how you look at one without hunting for a seed that has it.

    NIHILURK_LEVEL=island cargo run -p nihilurk

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

    NIHILURK_SPAWN="basilisk" cargo run -p nihilurk

Reproduce a run someone reported:

    cargo run -p nihilurk -- -s 1234567 -ns

Walk around an island from turn one:

    NIHILURK_LEVEL=island cargo run -p nihilurk

Check a name before you use it:

    cargo run -p nihilurk -- -content | grep -i staff

Test a throw build from turn one:

    NIHILURK_SPAWN="short bow,arrow,arrow,ring of sharpshooting" cargo run -p nihilurk


See also
--------

  spawn-api.md                  spawn_named, spawn_requested
  content-tables.md             what the names refer to
  input-and-turn-loop.md        where these flags are read (`engine/src/main.rs`)
  rendering.md                  `-c`, `-anim-rate` and `-nshake` in the renderer
  ../tutorial/add-your-first-item.md   these tools in context
