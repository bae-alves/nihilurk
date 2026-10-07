#!/usr/bin/env bash
#
# Plays nihilurk through a pty and prints what a player would see.
#
#   play.sh [options] KEY...
#
# Each KEY is typed into the game as it stands (`printf %b`, so `\r`, `\e` and
# `\t` work). Four words are not keys:
#
#   ?       print the screen as it is now
#   %       list the cells on a coloured background (`ROW COL GLYPH COLOUR`):
#           how a tint shows, since the screen itself carries no colour
#   @SECS   wait that long before the next key (a slow animation, a --MORE--)
#   #TEXT   a note, printed with the screens; sends nothing
#
# The screen is always printed once more at the end.
#
# Options:
#   -s SEED    reproducible run (default 1)
#   -S LIST    NIHILURK_SPAWN: names dropped around the player on floor 1
#   -a ARGS    more game flags, one quoted string (e.g. "-b lurk")
#   -d SECS    pause after every key (default 0.4)
#   -w SECS    wait for the first frame (default 1.5)
#   -z RxC     terminal size (default 25x80)
#   -c CMD     run CMD in the pty instead of the game: for testing this script
#   -k FILE    keep the raw bytes the game wrote, for screen.lua
#
# The game runs from a scratch directory with -ns -nobones -nshake -nb, so it
# writes no save, bones or leaderboard into the repo, and it is killed when the
# keys run out. Linux: `script` is util-linux's.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
bin="$root/target/debug/nihilurk"

seed=1 spawn="" extra="" pause=0.4 wait_first=1.5 size=25x80 custom="" keep=""
while getopts "s:S:a:d:w:z:c:k:" opt; do
    case "$opt" in
        s) seed="$OPTARG" ;;
        S) spawn="$OPTARG" ;;
        a) extra="$OPTARG" ;;
        d) pause="$OPTARG" ;;
        w) wait_first="$OPTARG" ;;
        z) size="$OPTARG" ;;
        c) custom="$OPTARG" ;;
        k) keep="$OPTARG" ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))

rows="${size%x*}" cols="${size#*x}"

if [[ -z "$custom" ]]; then
    (cd "$root" && cargo build -q -p nihilurk)
    command="exec '$bin' -s $seed -ns -nobones -nshake -nb $extra"
else
    command="exec $custom"
fi

work="$(mktemp -d)"
cleanup() {
    exec 3>&- 2>/dev/null || true
    [[ -s "$work/pid" ]] && kill "$(cat "$work/pid")" 2>/dev/null || true
    wait 2>/dev/null || true
    [[ -n "$keep" && -f "$work/raw" ]] && cp "$work/raw" "$keep"
    rm -rf "$work"
}
trap cleanup EXIT

mkfifo "$work/in"
cd "$work"
NIHILURK_SPAWN="$spawn" script -qefc "echo \$\$ > '$work/pid'; stty rows $rows cols $cols; $command" /dev/null \
    < "$work/in" > "$work/raw" 2>&1 &
exec 3> "$work/in"
sleep "$wait_first"

screen() { lua "$here/screen.lua" "$work/raw" "" "$rows" "$cols"; }

for key in "$@"; do
    case "$key" in
        '?') echo "== screen =="; screen ;;
        '%') echo "== backgrounds =="; lua "$here/screen.lua" "$work/raw" "" "$rows" "$cols" --bg ;;
        @*) sleep "${key#@}" ;;
        '#'*) echo "== ${key#\#} ==" ;;
        *) printf '%b' "$key" >&3; sleep "$pause" ;;
    esac
done

echo "== final screen =="
screen
