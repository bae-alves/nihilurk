---
name: play-nihilurk
description: Play nihilurk through a pty to see a change working in the real game: spawn an item or monster with NIHILURK_SPAWN, press keys, read the screen back. Use when asked to run the game, try a content row live, reproduce a gameplay bug, or confirm a change in the real TUI and not only in tests.
---

# Play nihilurk through a pty

`play.sh` runs the game in a pty, types the keys you give it and prints the screen as a player sees it. `screen.lua` rebuilds that screen from the bytes the game wrote. No tmux, no Python. Linux only (`script` is util-linux's).

    .claude/skills/play-nihilurk/play.sh [options] KEY...

A KEY is typed as it stands, with `printf %b` escapes (`'\r'`, `'\e'`, `'\t'`). Four words are not keys: `?` prints the screen now, `%` lists the cells on a coloured background, `@SECS` waits, `#TEXT` prints a note. The screen always prints once more at the end. Options: `-s SEED` (default 1), `-S LIST` (`NIHILURK_SPAWN`), `-a "FLAGS"` (e.g. `-a "-b lurk"`), `-d SECS` pause per key, `-w SECS` wait for the first frame, `-k FILE` keep the raw bytes. `play.sh` builds the game first and runs it from a scratch directory with `-ns -nobones -nshake -nb`, so it writes no save, bones or leaderboard.

Its tests: `bash .claude/skills/play-nihilurk/play_test.sh` and `lua .claude/skills/play-nihilurk/screen_test.lua`.

## Try a content row

A seed fixes the floor, so look first, then plan the walk and replay it.

    # 1. See where the items landed. The first --MORE-- needs a Space.
    play.sh -S "rune of ice,orc" ' ' @1 '?'

    # 2. Step onto the item (auto-pickup), read it, look.
    play.sh -S "rune of ice,orc" ' ' @1 y @2 ' ' r f @4 '?'

`NIHILURK_SPAWN` drops each name on the nearest free tile. `nihilurk -content` lists the names.

## What costs a wasted run

- **`--MORE--` eats keys.** Send `' '` after anything that logs more than one message, and `@2`-`@4` after anything animated. The log paints letter by letter, so a screen taken too early shows half a sentence, and stale letters can linger in the log row. Read effects off state (HP, the badge row, the pack, the map, and `%` for a monster's status tint: paralysed is Cyan, asleep DarkBlue, held DarkGreen) and use the log as a hint.
- **`o` (explore) picks items up, and a key pressed during it interrupts it.** Put `@2` after every `o`. It refuses while a hostile is in view.
- **Letters in the read, zap and pack menus are the pack letters.** Nihil starts with `a`-`e`, so the first spawned item is `f`. A lurk (`-a "-b lurk"`) starts empty, so it is `a`. Open `i` to be sure.
- **Aiming:** `z`, the wand's letter, direction keys to move the cursor, `'\r'` to fire, `x` to cancel.
- **You cannot pass a turn.** To let monsters act, walk away from them.
- **The first step can kill.** A dragon one-shots nihil, and a lurk has 6 HP. Pick monsters that are awake and hostile but survivable: orc, troll, quagga. A yeti and an ice monster are immune to cold, and a red demon is peaceful until the spirits turn, so a spell that needs a hostile target skips it.
- **Compare against a control.** Run the same keys with the reading step left out, so a number that moved can be blamed on the item.

`screen.lua FILE [END] [ROWS COLS] [--bg]` prints the screen (or, with `--bg`, the coloured-background cells) after the first END bytes of a `-k` capture, which is how to see the frame before a key.
