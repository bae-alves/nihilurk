#!/usr/bin/env bash
#
# Plays a served nihilurk run (serve.sh) one key at a time.
#
#   act.sh [-H HOST] [-p PORT] KEY...
#   act.sh -d DIR KEY...      a run started with `serve.sh start DIR`, no socket
#
# Each KEY goes over as one line and waits for its percept; `?` prints the
# percept at that point. The last percept always prints. With no KEY it prints
# the percept on connect. Keys use play.sh's notation: `'\r'`, `'\e'`, `'\eq'`.
# Bash's /dev/tcp does the connecting, so the host needs nothing else. Control
# bytes from the server are dropped before anything prints.

set -euo pipefail

host=127.0.0.1 port=7777 dir=""
while getopts "H:p:d:" opt; do
    case "$opt" in
        H) host="$OPTARG" ;;
        p) port="$OPTARG" ;;
        d) dir="$OPTARG" ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))

if [[ -n "$dir" ]]; then
    coproc SESSION { "$(dirname "${BASH_SOURCE[0]}")/serve.sh" session "$dir"; }
    exec 3<&"${SESSION[0]}" 4>&"${SESSION[1]}"
else
    exec 3<> "/dev/tcp/$host/$port" 4>&3
fi

# Drops C0 and C1 control bytes (C1 as UTF-8) from `line`, so a server cannot
# drive the terminal a percept is printed to.
clean() {
    local LC_ALL=C c1=$'\xc2[\x80-\x9f]'
    line="${line//[[:cntrl:]]/}"
    line="${line//$c1/}"
}

last=""
take() {
    local line i
    last=""
    for ((i = 0; i < 25; i++)); do
        IFS= read -r line <&3 || { echo "act.sh: the server hung up" >&2; exit 1; }
        clean
        last+="$line"$'\n'
    done
    while IFS= read -r line <&3; do
        clean
        last+="$line"$'\n'
        [[ "$line" == end || "$line" == over ]] && return
    done
}

take
for key in "$@"; do
    case "$key" in
        '?') printf '%s' "$last" ;;
        *) [[ "$last" == *$'\nover\n' ]] && break
           printf '%s\n' "$key" >&4
           take ;;
    esac
done
printf '%s' "$last"
