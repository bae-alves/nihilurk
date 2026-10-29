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

In any order. Every flag has a single-dash form; the shorthand ones (`-s`, `-c`, `-ns`, `-nb`, `-nshake`, `-mouse`, `-b`, `-am`, `-scores`) also have a double-dash longhand (`--seed`, `--centered`, `--no-save`, `--no-blood`, `--no-shake`, `--mouse`, `--body`, `--as-monster`, `--leaderboard`), as do `-endless`, `-anim-rate`, `-keymap` and `-content`. Any other argument that starts with `-` is refused, and so is a flag whose value is missing or bad (`-s -5`, `-s abc`, `-anim-rate fast`, `-b` with nothing after it); the message quotes the flag and its value: `invalid argument ARG. Please see -help for more information`. A player name therefore cannot start with `-`.

| Flag         | Effect                                                    |
|--------------|-----------------------------------------------------------|
| `-s <seed>`, `--seed` | Start the run from a specific `u64` seed. Reproducible.   |
| `-c`, `--centered` | Centre the map on the terminal instead of on top left. |
| `-ns`, `--no-save` | No save. The run is never written to disk.                |
| `-nb`, `--no-blood` | No blood. Suppresses bloodstain rendering, and with it the flung-corpse death animation — a kill just leaves a static grey corpse mark. |
| `-nshake`, `--no-shake` | No screen shake. The map never leaves its moorings — nothing arms one for the rest of the run. For anyone who would rather the terminal held still; `-anim-rate` can only make a shake *slower*, which is the wrong direction. |
| `-mouse`, `--mouse` | Turn on mouse capture. Left click reads a tile (look) and opens its Info popup; right click travels there, like `O` then Enter. On the aiming reticle, left moves the cursor and right moves it and fires; on the `O` cursor, left moves it and right goes. Menus and the `--MORE--` prompt ignore clicks. Off by default, because capture takes the terminal's own text selection (Shift+drag copies). |
| `-endless`, `--endless` | No Element of Yoord ever spawns, so there is no way to win: the dungeon keeps going down past `FINAL_DEPTH`. Sets the `Endless` resource (`models/src/world/map.rs`), which the save records (`SaveGame::endless`); refused together with `-fast`; everything that would treat `FINAL_DEPTH` as the bottom (floor building and population, the relic, the leaderboard's board) reads it. |
| `-fast`, `--fast`, `--bullet`, `--quick` (also `-bullet`, `-quick`) | A short dungeon of `FAST_FLOORS` floors (`engine/src/main.rs`): the Element of Yoord sits on the last one, and every floor from 4 to the one before it is a special level (`Floors::forces_special`). Sets the `Floors` resource (`models/src/world/map.rs`); floor `n` draws monsters, traps and items as the full-length depth it stands for (`Floors::content_depth`), and every floor runs its item budget twice (`Floors::item_multiplier`). The save records the run length (`SaveGame::floors`), so on a load the flag is ignored and the saved length wins; passing it with a save says so and asks `Load anyway? [y/N]` (same for `-endless`). Refused together with `-endless`. |
| `-content`, `--content` | Print every name the content tables know, then exit.      |
| `-scores`, `--leaderboard` | Print the leaderboard (the top `LEADERBOARD_STORE_LIMIT` scores ever recorded), then exit. Add `-fast` or `-endless` for that mode's board. |
| `-anim-rate <n>`, `--anim-rate <n>` | Multiplier on every animation frame's on-screen hold time (particles, the magic-mapping reveal wipe, the screen shake). `1.0` is the default pacing; raise it if a terminal's redraw can't keep up, lower it for snappier animations. Clamped to `0.1..=5.0`; a bad, non-finite or missing value is refused as an invalid argument. |
| `-keymap <file>`, `--keymap` | Remap keys from the file; without it, `keymap.txt` in the data directory is read if it exists. See [Key remapping](#key-remapping). A bad line or an unreadable file is named on stderr and the game does not start. |
| `-b <body>`, `--body` | Play as `nihil` (the default) or `lurk`. See below. |
| `-am <species>`, `--as-monster` | Play *as* a monster: any bestiary name (`-am dragon`). See below. |

### `-b <body>`

Which of the two written-to-be-played creatures descends. `-b nihil` is the
default and changes nothing.

`-b lurk` is the other one: quadruped, fanged, clawed, furred, and a magenta
`@`. Lower stats and the only thing it can put on is rings. What it has instead:

  * `SpeedKind::Quick`, half again as fast as `Normal` and the only creature
    in the game born at that tempo. Two monster rounds bought per three player
    turns; shown as `QUIK` on the status line.
  * The estoc's lunge (`Lunges`), and a second snap of the jaws after any bite that lands (`Fencer`, `lurk::NUMBER_OF_ATTACKS`).
  * The rapier's momentum (`BuildsMomentum`), built on the creature rather
    than on a blade it does not have.
  * A ring of stealth's quiet (`Stealthy`).
  * Bide in its spellset.
  * **Growth.** Every creature that dies on the floor is rolled against tuning
    constant `lurk::GROWTH_CHANCE`, and a hit puts `lurk::GROWTH_STEP` on one of
    the four numbers on the status line, drawn at random: `FEAR THE LURK!` Rolled per
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
so passing both is refused too. A new player named after a body, in any
case (`nihilurk lurk`, `nihilurk NIHIL`), is that body, as `-b` would make
them, so the name refuses `-am` or a different `-b` the same way. Loading a
save by that name keeps the body the save knows.

What the body changes is what the bestiary row says: the glyph and its colour,
the hit points, the attack and armour dice and their bonuses, the tempo (`-am
wraith` is permanently hasted), invisibility (`-am phantom`), and the innate
magic -- a dragon is immune to fire and flies, a rattlesnake's bite is
venomous, a slime splits when hurt -- into a *hostile* copy of you, carrying
your hit points, which is working as intended. None of that is a special case
for the player: the bestiary row is the same one a dragon deep in the dungeon
is built from, and the on-hit abilities are the same table.

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
    own `ItemUser` mark -- the orc, the centaur and the vampire are among the
    rows with hands; a dragon has claws and is told so when it tries.
  * **A species' spells land in your spellset, and cost your Ma.** A dragon
    knows Fireball, the same spell a hero coin teaches, cast through the
    same reticle at the same price out of your own pool. The dragons in the
    dungeon breathe it too, from a small pool of their own that never
    refills.
  * **The log tells you what you were born with.** One "You feel..." line
    per grant the body carries:
    "You feel venomous." Tags the HUD already shows are not repeated.
**Try `-am dragon`.** You start on floor 1 with Fireball in your spell bar
and claws that hold no sword.

**This will never be a balanced mode of play.** A kestral from the first
floor and a dragon from the deep are both one `-am` away, and nothing gates which one you're
allowed to start as, or scales the dungeon to match your pick. That's
deliberate: `-am` is a costume, not a difficulty setting, and every row in
the bestiary is tuned to be one thing a `@` fights, never to be the `@`
fighting everything else. Play it for the thrills, not for a fair fight.

### Bodies and species

Cancellation draws a line between the two, on purpose.

A **body** cannot be cancelled. It is the creature the run is about, and no
wand in the dungeon is a wand of being something else: a lurk stays a lurk,
keeps its shape, and keeps the rule about rings.

A **species** can, apart from any part its row marks as an identity effect (the dog's five grants). A player wearing a bestiary row keeps the shape and the
dice -- those are not effects at all -- but the magic is only on loan: a
cancelled dragon-bodied player loses fire immunity exactly as a real dragon
would, and a cancelled orc-bodied one loses `ItemUser`, which is to say the
wits to work a buckle, and can no longer equip what it was wearing a moment
ago. That is the bargain of wearing something else's magic.

The body is saved with the run and comes back on load: a worn species as its row name (`SaveGame::monster_body`), a lurk through its `Lurk` effect, and nihil as what is left. Two argument shapes are refused up front, both before the terminal is touched:

    nihilurk -am orc mysave     # the save already knows what body it is in
    nihilurk -b lurk Bae        # a name is the first argument or it is not a name

A monster's hit points are its own, so several rows are a one or two-hit death: that
is intended, and `-s` is how you retry it.

`-h`, `-help`, and `--help` print a short guide and exit before the terminal is configured. This page is the full reference.

The game needs a terminal of at least `SCREEN_W` columns by `SCREEN_H` rows (`engine/src/constants.rs`). In a smaller one the message log draws garbled letters. That is a known limit and will not be fixed.

`-content` never touches the alternate screen, so it pipes:

    cargo run -p nihilurk -- -content | grep ring
    cargo run -p nihilurk -- -content | less

`-scores` reads the same way, straight off `leaderboard.sav` in the data directory (`-scores -fast` reads `leaderboard-fast.sav`, `-scores -endless` reads `leaderboard-endless.sav`; each mode keeps its own board) (see `NIHILURK_DATA` below) -- a small versioned postcard file, like a save, holding the `LEADERBOARD_STORE_LIMIT` highest scores ever recorded across every run that ended in a win or a death (a quit-and-save doesn't count; the run isn't over). Each line it prints is `RANK. NAME - OUTCOME - SCORE (WHEN)` followed by one `conduct::MARK` per conduct that run kept (see `conducts.md`), where `OUTCOME` is `WIN`, `LOSE (Asc.)` (dead on the way back out, carrying the Element), or `LOSE (Desc.)` (dead on the way down), and `WHEN` is the UTC date and time the run ended. A version 1 leaderboard, from before conducts, is read and upgraded on the next write. A leaderboard it can't read, damaged or from another version, is named on stderr and never overwritten: no score is recorded until it is moved away. Two runs ending at once take turns at the file through `leaderboard.sav.lock`.

A run that ends in a win or a death also leaves a run dump, `morgue/NAME-YYYYMMDD-HHMMSS.txt` in the data directory (UTC; any character of `NAME` that is not a letter, a digit, `-` or `_` becomes `_`). It is plain text for sharing: the outcome, cause, depth and score, the final screen, the pack, every kill by name (most first), for a win the conducts kept, and the message log. The path is printed once the terminal is back. `-ns` does not stop it -- a dump is a record of the run, like the leaderboard, not a save. A dump never overwrites another, and one that can't be written is skipped without a word.

`-s` fixes the dungeon's *maps*. Every floor's walls are a pure function of `(seed, depth)`, so floor 7 of seed 1234 is the same maze today, tomorrow, after you reload a save, and after somebody adds a monster to the bestiary. Its *contents* -- monsters, loot, traps -- are re-rolled each time you enter the floor (they key off the staircase count as well), so walking back up through floor 7 finds the same corridors freshly stocked. How you play still does not reach into generation: two runs on one seed that take the same staircases see the same everything.


The positional argument
-----------------------

One bare argument, meaning one of two things:

  * **A save file**, if it is the path of a file as typed, or names one in the data directory -- as is or with `.sav` appended, matched case-insensitively. The run is loaded, and saves back to that same file.
  * **A player name**, otherwise. A fresh run starts under that name, saved as `<name>.sav` in the data directory. A name holds at most `NAME_MAX` characters, no control characters (an escape, a tab, a newline) and is not a Windows device name (`con`, `prn`, `aux`, `nul`, `com1`-`com9`, `lpt1`-`lpt9`, in any case); anything else is refused with a message and nothing is written. In file names (the save, `morgue/`) every character that is not a letter, digit, `-` or `_` becomes `_`, so `a b` saves as `a_b.sav` and `../x` cannot leave the data directory.

        cargo run -p nihilurk -- nihilurk          # loads nihilurk.sav from the data directory if it is there
        cargo run -p nihilurk -- ./old.sav         # loads that file, wherever it is
        cargo run -p nihilurk -- bae           # otherwise: a new run as bae

If the save is *clear data* -- a won run, which is kept rather than deleted -- the game asks before spending it.


Environment variables
---------------------

### NIHILURK_SPAWN

Comma-separated content names, dropped on free tiles around the player the moment a floor is built. The content author's shortcut: it means you never have to play down to a deep floor to look at a deep monster.

    NIHILURK_SPAWN="dragon" cargo run -p nihilurk
    NIHILURK_SPAWN="bow,arrow,ring of protection" cargo run -p nihilurk
    NIHILURK_SPAWN="dart trap, long sword" cargo run -p nihilurk
    NIHILURK_SPAWN="cursed -2 long sword,+3 ring mail,arrow x13" cargo run -p nihilurk

Details:

  * Any name from `-content` works -- monsters, items, traps, the Element itself.
  * Whitespace around each name is trimmed; empty entries are skipped.
  * Gear and ammunition take optional dressing, written around the name in this order: `[cursed|megacursed] [+N|-N] name [xN]`.

    | Piece    | Example             | Effect |
    |----------|---------------------|--------|
    | `cursed` | `cursed ring mail`  | Adds the `Curse` tag: once worn or wielded it will not come off until a scroll of remove curse. |
    | `megacursed` | `megacursed ring mail` | Adds `Curse` and `MegaCurse`: nothing lifts it or destroys it. |
    | `+N`/`-N`| `+3 long sword`     | Adds N to the item's flat bonus: a weapon's hit roll, armour's guard, or a launcher's throw (the arrows it looses). On a numeric ring (protection, strength, increase damage, sharpshooting, health) N is the ring's whole number, so `+3 ring of protection` is a +3 ring. Same code as the dungeon's own enchantment roll (`catalog::apply_bonus`). |
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

### Key remapping

`keymap.txt` is one pair per line, two characters with no separator (Brogue's format): `hj` makes pressing `h` act as `j`. List `hj` and `jh` to swap; lookup is a single pass, so pairs never chain. A key with no line acts as itself, so `hl` alone leaves `l` live too. An uppercase letter with no line of its own follows its lowercase one, so Shift-run keeps working. Blank lines and lines starting `#` are skipped. A line of any other length, a space as a key, or a key listed twice is refused with its line number (`models::parse_keymap`).

The file is read from the data directory, or from the path given to `-keymap`. A missing auto-loaded file is fine; a missing `-keymap` file is not. F1 and the sidebar hints show the key that now does each job (word keys such as `Tab` stay as they are). A command whose own letter is remapped away and that nothing maps to shows its old letter.

### NIHILURK_READY

Set and not empty, the game writes an OSC string, `ESC ] nihilurk;ready BEL`, each time it blocks waiting for a key: the turn loop, the `O` travel cursor, and the death and victory screens. A script that drives the game through a pty reads it as "the last key has done all it will": the play agent's `serve.sh` sets it and answers a key the moment it sees the signal, instead of waiting for a quiet spell with no output (`engine::ready`).

    NIHILURK_READY=1 cargo run -p nihilurk

Details:

  * It comes after the frame it settles, on the same stream, so a reader that has the signal has the frame.
  * Nothing is written before a read that only lets a key skip an animation or stop a walk, or while a key is already waiting to be read: the game is not waiting there.
  * A terminal ignores an OSC string it does not know, so the game looks the same with it set. Unset, the game does not write it or look at the input for it.

### NIHILURK_DATA

The directory nihilurk keeps its files in: saves, `leaderboard.sav`, `leaderboard-fast.sav`, `leaderboard-endless.sav` and the run dumps in `morgue/`. Set, it wins. Unset or empty, the OS convention decides (`models::data_dir`):

| OS              | Data directory                                              |
|-----------------|-------------------------------------------------------------|
| Linux, the BSDs | `$XDG_DATA_HOME/nihilurk`, or `~/.local/share/nihilurk`     |
| macOS           | `~/Library/Application Support/nihilurk`                    |
| Windows         | `%APPDATA%\nihilurk`                                        |

  * A relative `XDG_DATA_HOME` is ignored, as the XDG spec says.
  * With no home to go on at all, it is the current directory.
  * The directory is made on the first write, not before.

    NIHILURK_DATA=. cargo run -p nihilurk        # keep everything in the checkout, the old way


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

  conducts.md                   the conducts behind the marks on -scores
  input-and-turn-loop.md        where these flags are read (`engine/src/main.rs`)
  rendering.md                  `-c`, `-anim-rate` and `-nshake` in the renderer
  ../tutorial/add-your-first-item.md   these tools in context
