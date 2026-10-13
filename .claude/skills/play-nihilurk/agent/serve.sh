#!/usr/bin/env bash
#
# Serves one nihilurk run over a socket: a line in is a key, a percept out is
# what the screen shows once the game has settled.
#
#   serve.sh listen [GAME FLAGS...]       start a run, serve it on :7777, one client at a time
#   serve.sh start DIR [-c CMD | FLAGS]   start a run in DIR (fifo, raw bytes, pid);
#                                         the binary is $NIHILURK_BIN, else `nihilurk`
#   serve.sh session DIR                  serve the run in DIR over stdin/stdout
#
# The game is the unchanged binary in a 25x80 pty, run with
# -ns -nobones -nshake -nb -anim-rate 0.1, so nothing is saved and the feel
# layer has finished before a percept is taken. `-c CMD` swaps the game for
# a stand-in, for testing this script.
#
# A line is typed through `printf %b`, as play.sh does (`\r`, `\e`, `\eq` for
# Alt+q). An empty line types nothing and gets the percept again. A percept is
# the 25 screen rows, then one `ROW COL GLYPH COLOUR` line per cell on a
# coloured background, then `end`, or `over` once the game has exited. A
# session gets a percept on connect and one per line after that. The run
# outlives the session, so a client can hang up and come back; once it is
# over, every session gets the last percept and `over`. Sessions take turns:
# one that arrives mid-session waits for the other to hang up. A run no
# session touches for $NIHILURK_IDLE seconds (default 1800, 0 never) is killed.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
screen="$here/../screen.lua"

alive() { kill -0 "$(cat "$dir/pid")" 2>/dev/null; }

# ponytail: quiet bytes stand in for "the game waits on a key". Every busy
# loop repaints well inside 250 ms; an engine-side ready signal is the upgrade
# if a stall ever outlasts it.
settle() {
    local size=-1 now quiet=0 tries=0
    while (( quiet < 5 && tries < 600 )); do
        sleep 0.05
        now="$(stat -c %s "$dir/raw")"
        if [[ "$now" == "$size" ]]; then quiet=$((quiet + 1)); else quiet=0 size="$now"; fi
        tries=$((tries + 1))
    done
}

# ponytail: screen.lua replays the whole capture each time; fine for a run,
# slow once it is many MB.
percept() {
    lua "$screen" "$dir/raw" "" 25 80
    lua "$screen" "$dir/raw" "" 25 80 --bg | sed '/^$/d'
    if alive; then echo end; else echo over; return 1; fi
}

# Kills the run once no session has touched it for $NIHILURK_IDLE seconds
# (default 1800; 0 never), so a forgotten run does not hold its pty forever.
reap_when_idle() {
    local idle="${NIHILURK_IDLE:-1800}"
    (( idle > 0 )) || return 0
    touch "$dir/seen"
    while alive; do
        sleep 1
        (( $(date +%s) - $(stat -c %Y "$dir/seen") < idle )) || kill "$(cat "$dir/pid")" 2> /dev/null || true
    done < /dev/null > /dev/null 2>&1 &
}

start() {
    dir="$1"
    shift
    local cmd
    if [[ "${1:-}" == -c ]]; then
        cmd="$2"
    else
        cmd="${NIHILURK_BIN:-nihilurk} -ns -nobones -nshake -nb -anim-rate 0.1 $(printf '%q ' "$@")"
    fi
    mkfifo "$dir/in"
    (cd "$dir" && exec script -qefc "echo \$\$ > '$dir/pid'; stty rows 25 cols 80; exec $cmd" /dev/null \
        0<> "$dir/in" > "$dir/raw" 2>&1) &
    local tries=0
    until [[ -s "$dir/pid" ]] || (( tries++ > 100 )); do sleep 0.05; done
    reap_when_idle
}

session() {
    dir="$1"
    exec 5> "$dir/lock"
    flock 5
    exec 4<> "$dir/in"
    touch "$dir/seen"
    settle
    percept || return 0
    local line
    while IFS= read -r line; do
        touch "$dir/seen"
        printf '%b' "${line%$'\r'}" >&4
        settle
        percept || return 0
    done
}

listen() {
    dir="$(mktemp -d)"
    start "$dir" "$@"
    exec socat TCP-LISTEN:7777,reuseaddr,fork,max-children=1 EXEC:"$here/serve.sh session $dir"
}

case "${1:-}" in
    start | session | listen) mode="$1"; shift; "$mode" "$@" ;;
    *) echo "usage: serve.sh listen [FLAGS...] | start DIR [-c CMD | FLAGS...] | session DIR" >&2; exit 2 ;;
esac
