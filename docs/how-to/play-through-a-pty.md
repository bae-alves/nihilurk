How to play the game from a script
==================================

    Audience       Anyone who changed the game and wants to see it work
                   in the real terminal, with no terminal in front of
                   them: a content author, an engine developer, or
                   Claude Code.
    Prerequisites  A Linux box with `script` (util-linux), `lua` and a
                   Rust toolchain. `spawn-a-thing.md` says what
                   `NIHILURK_SPAWN` does.
    Result         The screen the game showed after your keys, and the
                   cells that sit on a coloured background, printed as
                   text.

Tests say a rule holds. They cannot say how it looks, and nobody tests that on purpose (`../explanation/adr-0004-no-tests-of-presentation.md`). The way to check a look is to play. `.claude/skills/play-nihilurk/play.sh` plays for you: it starts the game in a pty, types the keys you list, waits, and prints the screen. It builds the game first. The game runs from a scratch directory with `-ns -nobones -nshake -nb`, so it writes no save, bones or leaderboard into the repo.

    .claude/skills/play-nihilurk/play.sh -S "rune of ice,aquator" ' ' @1 '?'


The keys
--------

Each argument is one key, typed as it stands. `printf %b` runs on it, so `'\r'`, `'\e'` and `'\t'` are Enter, Escape and Tab. Four words are not keys:

    ?       print the screen as it is now
    %       list the cells on a coloured background
    @SECS   wait that long before the next key
    #TEXT   print a note with the screens; sends nothing

The screen prints once more when the keys run out. A key waits `-d` seconds (0.4 by default) before the next one goes in. The other options are `-s SEED`, `-S LIST` (that is `NIHILURK_SPAWN`), `-a "FLAGS"` for the game (`-a "-b lurk"`), `-w SECS` for the first frame, `-z RxC` for the terminal size (25x80 by default, the smallest the game draws cleanly), and `-k FILE` to keep the raw bytes.


A walk-through: read a rune next to an aquator
------------------------------------------

A seed fixes the floor, so look first, plan the walk from what you see, then replay it.

    play.sh -S "rune of ice,aquator" ' ' @1 '?'

The first frame has a `--MORE--` that needs a Space. The map shows the rune `'` up and to the left of you and the aquator `A` right of it. Step onto the rune (items are picked up as you walk over them), read it with `r` and the pack letter, and look:

    play.sh -S "rune of ice,aquator" ' ' @1 y @2 ' ' r f @4 '#after the nova' '?' '%'

`f` is the first spawned item because nihil starts with `a` to `e`. A lurk starts empty, so there it is `a`; press `i` to be sure.


Reading colour
--------------

The screen is text. Colour is how the game marks a monster that cannot fight at full strength, and `%` prints it as `ROW COL GLYPH COLOUR` in the names `crossterm` uses:

    == backgrounds ==
    4 19 U Cyan

That line is an ur-vile on a cyan square: paralysed. The tints and what each one means are in `../reference/rendering.md`. Run the same keys without the step that should cause the tint, and the list is empty: that is your control.


What wastes a run
-----------------

* **`--MORE--` eats keys.** Send a Space after anything that logs several messages, and wait (`@2` to `@4`) after anything animated. The log paints letter by letter, so an early screen shows half a sentence. Read the effect off HP, the badge row, the pack, the map and `%`, and use the log as a hint.
* **`o` explores and picks things up, and any key pressed during it stops it.** Put `@2` after it. It refuses while a hostile is in view.
* **You cannot pass a turn.** To give monsters time, walk away from them.
* **The first step can kill.** A dragon one-shots nihil, and a lurk has 6 HP. An orc takes most of nihil's 12 HP. To test an effect on a monster, spawn an aquator: awake, hostile, 9 HP, weak hits. Use a troll or a quagga when the test needs more. A yeti is immune to cold, and a red demon and a gnome are peaceful, so a spell that needs a hostile target ignores them.
* **Aiming:** `z`, the wand's letter, direction keys to move the cursor, `'\r'` to fire, `x` to cancel.


Its own tests
-------------

    bash .claude/skills/play-nihilurk/play_test.sh
    lua .claude/skills/play-nihilurk/screen_test.lua

`play_test.sh` runs the driver against `cat`, `printf` and `sleep`, so it needs no build. `screen_test.lua` checks the screen reader. Neither runs in CI or in the pre-commit hook.


See also
--------

  spawn-a-thing.md                   put the thing in front of you
  add-an-item.md                     the recipe whose last step is "look at it"
  ../reference/rendering.md          the status tints `%` reports
  ../reference/cli-and-env.md        the flags and `NIHILURK_SPAWN`
  ../explanation/adr-0004-no-tests-of-presentation.md  why a look is checked by playing
