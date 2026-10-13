#!/usr/bin/env bash
#
# Serves one nihilurk run over a socket: a line in is a key, a percept out is
# what the screen shows once the game has settled.
#
#   serve.sh listen [GAME FLAGS...]       start a run, serve it on :7777, one client at a time,
#                                         bound to $NIHILURK_BIND (default 127.0.0.1)
#   serve.sh start DIR [-c CMD | FLAGS]   start a run in DIR (fifo, raw bytes, pid, born), which
#                                         you own and others cannot write to;
#                                         the binary is $NIHILURK_BIN, else `nihilurk`
#   serve.sh session DIR                  serve the run in DIR over stdin/stdout
#
# The game is the unchanged binary in a 25x80 pty, run with
# -ns -nobones -nshake -nb -anim-rate 0.1, so nothing is saved and the feel
# layer has finished before a percept is taken. Its data directory is DIR, so
# it never reads your keymap or writes to your leaderboard and morgue. `-c CMD`
# swaps the game for a stand-in, for testing this script.
#
# A line is typed through `printf %b`, as play.sh does (`\r`, `\e`, `\eq` for
# Alt+q); a line over 4096 bytes is cut into keys of that size. An empty line
# types nothing and gets the percept again. A percept is the 25 screen rows,
# then one `ROW COL GLYPH COLOUR` line per cell on a coloured background, then
# `end`, or `over` once the game has exited. A session gets a percept on
# connect and one per line after that. The run outlives the session, so a
# client can hang up and come back; once it is over, every session gets the
# last percept and `over`. Sessions take turns: one that arrives mid-session
# waits for the other to hang up. A run no session touches for $NIHILURK_IDLE
# seconds (set on start; default 1800, 0 never) is killed, and a session that
# sends no line for that long is hung up on.

set -euo pipefail
umask 077

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
screen="$here/../screen.lua"

# When process $1 started, in clock ticks since boot (field 22 of its stat), or
# nothing once it is gone. A pid can be reused; a pid and its start time cannot.
born() {
    local stat fields
    [[ "$1" =~ ^[0-9]+$ ]] || return 0
    stat="$(cat "/proc/$1/stat" 2>/dev/null)" || return 0
    read -ra fields <<< "${stat##*) }"
    echo "${fields[19]}"
}

alive() {
    local now
    now="$(born "$(cat "$dir/pid" 2>/dev/null)")"
    [[ -n "$now" && "$now" == "$(cat "$dir/born")" ]]
}

# True when the open capture fd 6 had bytes the last percept did not see; it
# reads them, so the next call sees only what comes after.
wrote() {
    local LC_ALL=C chunk got=1
    while IFS= read -r -d '' -N 65536 -u 6 chunk || [[ -n "$chunk" ]]; do
        [[ -n "$chunk" ]] || break
        got=0
        (( ${#chunk} == 65536 )) || break
    done
    return "$got"
}

# ponytail: quiet bytes stand in for "the game waits on a key". Every busy
# loop repaints well inside 250 ms; an engine-side ready signal is the upgrade
# if a stall ever outlasts it. Waits on fd 7, a pipe nobody writes to, so a
# 10 ms tick costs no process; gives up after 30 s.
settle() {
    local start last
    start="${EPOCHREALTIME/[.,]/}" last="$start"
    while (( ${EPOCHREALTIME/[.,]/} - last < 250000 && ${EPOCHREALTIME/[.,]/} - start < 30000000 )); do
        read -r -t 0.01 -u 7 || true
        ! wrote || last="${EPOCHREALTIME/[.,]/}"
    done
}

# ponytail: screen.lua replays the whole capture each time; 35 KB after 300
# keys takes 10 ms, so a run is fine, a many-MB capture is not. `shown` is
# the capture's size at this percept, so the next session knows the game has
# not moved since.
percept() {
    stat -c %s "$dir/raw" > "$dir/shown"
    lua "$screen" "$dir/raw" "" 25 80 --percept
    if alive; then echo end; else echo over; return 1; fi
}

# Kills the run once no session has touched it for $NIHILURK_IDLE seconds
# (default 1800; 0 never), so a forgotten run does not hold its pty forever.
reap_when_idle() {
    local idle="${NIHILURK_IDLE:-1800}"
    echo "$idle" > "$dir/idle"
    (( idle > 0 )) || return 0
    touch "$dir/seen"
    while alive; do
        sleep 1
        (( $(date +%s) - $(stat -c %Y "$dir/seen") < idle )) || ! alive || kill "$(cat "$dir/pid")" 2> /dev/null || true
    done < /dev/null > /dev/null 2>&1 &
}

start() {
    dir="$1"
    shift
    if [[ ! -d "$dir" || ! -O "$dir" || "$(stat -c %A "$dir")" =~ ^.{8}w ]]; then
        echo "serve.sh: $dir must be a directory you own that others cannot write to" >&2
        exit 2
    fi
    local cmd pid
    if [[ "${1:-}" == -c ]]; then
        cmd="$2"
    else
        cmd="$(printf '%q' "${NIHILURK_BIN:-nihilurk}") -ns -nobones -nshake -nb -anim-rate 0.1 $(printf '%q ' "$@")"
    fi
    printf -v pid '%q' "$dir/pid"
    mkfifo "$dir/in"
    (cd "$dir" && NIHILURK_DATA="$dir" SHELL=/bin/sh exec script -qefc "echo \$\$ > $pid; stty rows 25 cols 80; exec $cmd" /dev/null \
        0<> "$dir/in" > "$dir/raw" 2>&1) &
    local tries=0
    until [[ -s "$dir/pid" ]] || (( tries++ > 100 )); do sleep 0.05; done
    born "$(cat "$dir/pid" 2>/dev/null)" > "$dir/born"
    reap_when_idle
}

session() {
    dir="$1"
    exec 5> "$dir/lock"
    flock 5
    exec 4<> "$dir/in" 6< "$dir/raw" 7<> <(:)
    touch "$dir/seen"
    wrote || true
    [[ "$(stat -c %s "$dir/raw")" == "$(cat "$dir/shown" 2>/dev/null)" ]] || settle
    percept || return 0
    local line secs idle=()
    secs="$(< "$dir/idle")"
    (( secs > 0 )) && idle=(-t "$secs")
    while IFS= read -r -n 4096 "${idle[@]}" line; do
        touch "$dir/seen"
        printf '%b' "${line%$'\r'}" >&4
        settle
        percept || return 0
    done
}

listen() {
    dir="$(mktemp -d)"
    start "$dir" "$@"
    exec socat TCP-LISTEN:7777,bind="${NIHILURK_BIND:-127.0.0.1}",reuseaddr,fork,max-children=1 EXEC:"$here/serve.sh session $dir"
}

case "${1:-}" in
    start | session | listen) mode="$1"; shift; "$mode" "$@" ;;
    *) echo "usage: serve.sh listen [FLAGS...] | start DIR [-c CMD | FLAGS...] | session DIR" >&2; exit 2 ;;
esac
