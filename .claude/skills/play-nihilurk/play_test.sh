#!/usr/bin/env bash
# bash .claude/skills/play-nihilurk/play_test.sh
#
# play.sh against stand-ins for the game, so it needs no build: `cat` echoes
# what is typed, `printf` paints a fixed cell, `sleep` proves the child is
# killed when the keys run out.

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
play="$here/play.sh"

fail() { echo "FAIL: $1" >&2; exit 1; }

# 1. Keys reach the program, and the screen shows what it did with them.
out="$("$play" -w 0.3 -d 0.2 -c cat 'hi' '?' '!')"
grep -q '^== screen ==$' <<< "$out" || fail "no mid-run screen"
grep -q '^hi' <<< "$out" || fail "typed keys not on the screen: $out"
grep -q '^== final screen ==$' <<< "$out" || fail "no final screen"

# 2. Escapes in a key are real escapes, and a pause and a note send nothing.
out="$("$play" -w 0.3 -d 0.2 -c cat 'a\rb' '@0.2' '#a note' '?')"
grep -q '^== a note ==$' <<< "$out" || fail "note not printed"
grep -qx 'a' <<< "$out" && grep -q '^b' <<< "$out" || fail "\\r did not break the line: $out"

# 3. The pty has the size asked for, and absolute moves land where they say.
out="$("$play" -w 0.5 -z 10x40 -c "sh -c 'printf \"\\033[3;4Hok\"; stty size; sleep 5'")"
grep -qE '^   ok' <<< "$out" || fail "cursor move not applied: $out"
grep -q '10 40' <<< "$out" || fail "terminal size not 10x40: $out"

# 4. The program is gone when play.sh returns.
"$play" -w 0.3 -c 'sleep 31337' > /dev/null
sleep 0.3
! pgrep -f 'sleep 31337' > /dev/null || { pkill -f 'sleep 31337'; fail "the child outlived play.sh"; }

# 5. -k keeps the raw bytes.
keep="$(mktemp)"
"$play" -w 0.3 -d 0.2 -k "$keep" -c cat 'zz' > /dev/null
grep -q 'zz' "$keep" || fail "-k kept nothing"
rm -f "$keep"

# 6. `%` lists the cells on a coloured background, which is how a tint shows.
out="$("$play" -w 0.5 -c "sh -c 'printf \"\\033[48;5;14mX\\033[49m\"; sleep 5'" '%')"
grep -q '^== backgrounds ==$' <<< "$out" || fail "no backgrounds header: $out"
grep -q '^1 1 X Cyan$' <<< "$out" || fail "background cell not listed: $out"

echo "play_test: ok"
