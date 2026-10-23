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

Tests say a rule holds. They cannot say how it looks, and nobody tests that on purpose (`../explanation/adr-0004-no-tests-of-presentation.md`). The way to check a look is to play. `.claude/skills/play-nihilurk/play.sh` plays for you: it starts the game in a pty, types the keys you list, waits, and prints the screen. It builds the game first. The game runs from a scratch directory with `-ns -nshake -nb`, so it writes no save. The scratch directory is also its data directory, so it never reads your keymap or writes to your leaderboard and morgue.

    .claude/skills/play-nihilurk/play.sh -S "rune of ice,aquator" ' ' @1 '?'


The keys
--------

Each argument is one key, typed as it stands. `printf %b` runs on it, so `'\r'`, `'\e'` and `'\t'` are Enter, Escape and Tab. Four words are not keys:

    ?       print the screen as it is now
    %       list the cells on a coloured background
    @SECS   wait that long before the next key
    #TEXT   print a note with the screens; sends nothing

The screen prints once more when the keys run out. A key waits `-d` seconds (0.4 by default) before the next one goes in. The other options are `-s SEED`, `-S LIST` (that is `NIHILURK_SPAWN`), `-L LEVEL` (that is `NIHILURK_LEVEL`), `-a "FLAGS"` for the game (`-a "-b lurk"`), `-w SECS` for the first frame, `-z RxC` for the terminal size (25x80 by default, the smallest the game draws cleanly), `-k FILE` to keep the raw bytes, and `-c CMD` to run something else in the pty, which is how the script tests itself.


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
* **The first step can kill.** A dragon one-shots nihil, and a lurk starts with fewer HP still (`lurk::START_HP` against `player::START_HP`). An orc takes most of nihil's. To test an effect on a monster, spawn an aquator: awake, hostile, 9 HP, weak hits. Use a troll or a quagga when the test needs more. A yeti is immune to cold, and a red demon and a gnome are peaceful, so a spell that needs a hostile target ignores them.
* **Aiming:** `z`, the wand's letter, direction keys to move the cursor, `'\r'` to fire, `x` to cancel.


Hold a game open
----------------

`play.sh` replays its keys from the start every time. To play one run a key at a time, start it once with `agent/serve.sh` and send keys with `agent/act.sh`:

    a=.claude/skills/play-nihilurk/agent
    d=$(mktemp -d)
    NIHILURK_BIN="$PWD/target/debug/nihilurk" NIHILURK_SPAWN=aquator $a/serve.sh start "$d" -s 1
    $a/act.sh -d "$d" ' ' l     # one call per step; the run waits in between
    kill "$(cat "$d/pid")"

`NIHILURK_BIN` is the binary to run, as an absolute path. Without it, `start` runs the `nihilurk` on `PATH`. Game flags go after the directory.

To hand the run to a player outside the machine, serve it from a container on a TCP port instead:

    cargo build -p nihilurk --target x86_64-unknown-linux-musl
    docker build -t nihilurk-agent -f $a/Dockerfile .claude/skills/play-nihilurk
    docker run --rm -d -p 127.0.0.1:7777:7777 \
        --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges --pids-limit 128 \
        -v "$PWD/target/x86_64-unknown-linux-musl/debug/nihilurk:/usr/local/bin/nihilurk:ro" \
        nihilurk-agent -s 1
    .claude/skills/play-nihilurk/agent/act.sh ' ' l l

The image holds only the server. The game is a static binary built on the host and mounted in, so it never compiles inside Docker. Arguments after the image name are game flags, and `-e NIHILURK_SPAWN=...` passes through. `act.sh` without `-d` connects to `127.0.0.1:7777`. Either way, the server, `serve.sh`, runs the game in a pty with `-ns -nshake -nb -anim-rate 0.1`, with the run's directory as its data directory. Each line a client sends is one key, in the notation above. Once the game signals that it waits on a key, the server answers with a percept: the 25 screen rows, a `ROW COL GLYPH COLOUR` line for each tinted cell, then `end`. Once the game has exited it answers `over`, and keeps answering every new connection with the last percept and `over`, until `docker stop` in the container. Sessions take turns, in the container and on the host: one that arrives mid-session waits for the other to hang up. A run that no session has touched for `NIHILURK_IDLE` seconds (default 1800, `0` never) is killed and answers `over` from then on, and a session that sends nothing for that long is hung up on. A line longer than 4096 bytes is cut into keys of that size. A client gets a percept on connect and one per line after that, so it acts only on what it has seen. It can hang up at any point, and the run waits for the next connection. `act.sh` connects through bash's `/dev/tcp`, so any line-based client works, `nc` included.

The protocol has no login: whoever reaches the port plays the run. Publish the port on `127.0.0.1`, as above, unless the player is on a network you trust. The second `docker run` line keeps the container read-only, with no capabilities and a cap on processes, and the server inside runs as an unprivileged user. `serve.sh listen` binds `127.0.0.1` unless `NIHILURK_BIND` says otherwise; the image sets it to `0.0.0.0` so Docker can map the port.

The server sets `NIHILURK_READY` for the game, which then writes a signal each time it blocks for a key (`../reference/cli-and-env.md`). Auto-explore, a paralysed turn and particles never signal, so their output reaches a percept only after they have finished, however long they take. A game that never signals, such as a build from before the signal, gets a percept after 250 ms without output instead. The signal changes nothing on screen, so a seed and the same keys give the same percepts.


Its own tests
-------------

    bash .claude/skills/play-nihilurk/play_test.sh
    lua .claude/skills/play-nihilurk/screen_test.lua
    bash .claude/skills/play-nihilurk/agent/serve_test.sh

`play_test.sh` runs the driver against `cat`, `printf` and `sleep`, so it needs no build. `screen_test.lua` checks the screen reader. `agent/serve_test.sh` runs the socket server over stdio against stand-ins, so it needs no Docker and no socat. None of them runs in CI or in the pre-commit hook.


See also
--------

  spawn-a-thing.md                   put the thing in front of you
  add-an-item.md                     the recipe whose last step is "look at it"
  ../reference/rendering.md          the status tints `%` reports
  ../reference/cli-and-env.md        the flags and `NIHILURK_SPAWN`
  ../explanation/adr-0004-no-tests-of-presentation.md  why a look is checked by playing
