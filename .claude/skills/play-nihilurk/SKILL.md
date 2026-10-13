---
name: play-nihilurk
description: Play nihilurk to see a change working in the real game: start a run that stays open, send it keys one at a time, and read back what the screen shows, with NIHILURK_SPAWN putting an item or monster in front of you. Use when asked to run the game, try a content row live, reproduce a gameplay bug, or confirm a change in the real TUI and not only in tests.
---

# Play nihilurk

`agent/serve.sh` holds one run open in a 25x80 pty. `agent/act.sh` types keys into it and prints what the game shows once it settles: a **percept**. The run lasts between calls, so each step is one `act.sh` call, and you choose the next key after reading the screen. Linux only (`script` is util-linux's).

    a=.claude/skills/play-nihilurk/agent
    cargo build -q -p nihilurk
    d=$(mktemp -d)
    NIHILURK_BIN="$PWD/target/debug/nihilurk" NIHILURK_SPAWN="rune of ice,aquator" $a/serve.sh start "$d" -s 1
    $a/act.sh -d "$d"              # the screen now
    $a/act.sh -d "$d" ' ' y        # a key at a time; the last percept prints
    kill "$(cat "$d/pid")"; rm -rf "$d"

`start` takes game flags after the directory (`-s 7`, `-b lurk`, `-am dragon`) and runs the game from that directory with `-ns -nobones -nshake -nb -anim-rate 0.1`, so it writes no save, bones or leaderboard, and the feel layer is gone. `NIHILURK_BIN` must be an absolute path. Without it, `start` runs the `nihilurk` on `PATH`, which is not your change. `NIHILURK_SPAWN` and `NIHILURK_LEVEL` (battlefield, labyrinth, vault, bee, castle, island) pass through.

A KEY is typed with `printf %b` escapes (`'\r'`, `'\e'`, `'\t'`, `'\eq'` for Alt+q). `?` prints the percept at that point in the list. An empty KEY (`''`) types nothing and gets the screen again. A percept is the 25 screen rows, then a `ROW COL GLYPH COLOUR` line for each cell on a coloured background (a status tint: paralysed Cyan, asleep DarkBlue, held DarkGreen), then `end`, or `over` once the game has exited.

A percept is sent once the game has written nothing for 250 ms. Animations, auto-explore and a paralysed turn all repaint faster than that, so a percept never catches them half done: no `@` waits. A seed and the same keys give the same percepts.

Tests: `bash .claude/skills/play-nihilurk/agent/serve_test.sh`.

## Try a content row

    $a/act.sh -d "$d" ' '          # the first --MORE-- needs a Space; find the rune and the aquator
    $a/act.sh -d "$d" y ' ' r f    # step onto the rune (auto-pickup), read it

`NIHILURK_SPAWN` drops each name on the nearest free tile, on **every** floor, so descending brings them back. It ignores depth gates. The game skips an unknown name silently, so check names with `target/debug/nihilurk -content | grep -i NAME` (a summon-only monster can be missing there and still spawn).

Gear takes dressing, in this order: `[cursed] [+N|-N] name [xN]`.

    NIHILURK_SPAWN="cursed -2 long sword,+3 ring mail,arrow x13,+3 ring of protection"

`cursed` blocks removal until remove curse, `+N` is the item's bonus (the whole number on a numeric ring), `xN` is the stack size. Quality stays hidden until worn or identified. Dressing is ignored on things that cannot use it.

Spawn order is placement order, nearest first, so list the thing under test first when space is tight. Spawn several of a kind (`"aquator,aquator"`) for crowd behaviour. Use `NIHILURK_LEVEL=island` and friends to start on a special level without hunting for a seed. `-am dragon` plays a monster: the pack starts empty and innate spells cost 0 Ma.

**Loop:** start, read the screen, act, read, act. For a control, start a second run with the same seed and spawn but without the change (or without the step under test), and send the same keys. Keep the key list in a shell variable so the control is exact.

## What wastes a step

- **`--MORE--` eats keys.** When the bottom line says `--MORE--`, send `' '` before anything else. Read effects off state (HP, the badge row, the pack, the map, the tint lines) and use the log as a hint.
- **`o` (explore) picks items up.** It refuses while a hostile is in view.
- **Menu letters are pack letters.** Nihil starts with `a`-`e`, so the first spawned item is `f`. A lurk (`-b lurk`) starts empty, so it is `a`. Open `i` to be sure.
- **Aiming:** `z`, the wand's letter, direction keys to move the cursor, `'\r'` to fire, `x` to cancel.
- **You cannot pass a turn.** To let monsters act, walk away from them.
- **The first step can kill.** A dragon one-shots nihil, an orc takes most of 12 HP, and a lurk has 6. To test an effect on a monster, spawn an **aquator**: awake, hostile, 9 HP, weak hits. Use a troll or quagga only when the test needs more. A yeti and an ice monster are immune to cold, a red demon is peaceful until the spirits turn, and a gnome is a peaceful spirit, so a spell that needs a hostile target skips them.
- **Kill the run when done** with `kill "$(cat "$d/pid")"`. A forgotten run dies on its own after `NIHILURK_IDLE` seconds without a call (default 1800, `0` never; set it on `start`), and then answers `over`.
- **Calls take turns.** Two `act.sh` calls on one run never interleave: the second waits for the first to finish. That also means a stuck call blocks the next one.

## Hand a run to another player

The same server runs in a container and listens on TCP port 7777, for a player that is not you (a script, another model, a person with `nc`):

    cargo build -q -p nihilurk --target x86_64-unknown-linux-musl
    docker build -t nihilurk-agent -f $a/Dockerfile .claude/skills/play-nihilurk
    docker run --rm -d -p 127.0.0.1:7777:7777 -e NIHILURK_SPAWN=aquator \
        -v "$PWD/target/x86_64-unknown-linux-musl/debug/nihilurk:/usr/local/bin/nihilurk:ro" \
        nihilurk-agent -s 1
    $a/act.sh ' ' l                # no -d: talks to 127.0.0.1:7777 (-H, -p to change)

The protocol is one key per line in, one percept out, plus one percept on connect. The run outlives a connection. Once it is over, every connection gets the last percept and `over` until `docker stop`. The image holds no game: it is the server, socat and lua, about 10 MiB running. The game is a static musl build from the host, mounted in at `docker run`, so a change needs a `cargo build` and a new container, not a new image. Rebuild the image only when `serve.sh` or `screen.lua` changes.

## One-shot replay: play.sh

`play.sh [options] KEY...` starts a fresh run, types the keys with a pause after each, prints the final screen, and kills the run. Use it for what the held run cannot do: another terminal size (`-z RxC`) or the raw bytes (`-k FILE`, read back with `screen.lua FILE [END] [ROWS COLS] [--bg]` for the frame before a key). Options: `-s SEED`, `-S LIST`, `-L LEVEL`, `-a "FLAGS"`, `-d SECS` pause per key, `-w SECS` first-frame wait. Words that are not keys: `?` screen, `%` tints, `@SECS` wait, `#TEXT` note. Its waits are guesses, so add `@2`-`@4` after anything animated. Tests: `play_test.sh`, `screen_test.lua`.
