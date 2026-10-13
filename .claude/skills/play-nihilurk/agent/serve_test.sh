#!/usr/bin/env bash
# bash .claude/skills/play-nihilurk/agent/serve_test.sh
#
# serve.sh against stand-ins for the game, so it needs no build, no Docker and
# no socat: `start` runs the stand-in, `session` talks over stdio the way it
# talks over the socket.

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
serve="$here/serve.sh"

fail() { echo "FAIL: $1" >&2; exit 1; }

dirs=()
cleanup() {
    for d in "${dirs[@]}"; do
        [[ -s "$d/pid" ]] && kill "$(cat "$d/pid")" 2>/dev/null || true
        rm -rf "$d"
    done
}
trap cleanup EXIT

# Sets `dir`; no subshell, so `dirs` reaches the cleanup.
game() {
    dir="$(mktemp -d)"
    dirs+=("$dir")
    "$serve" start "$dir" -c "$1"
}

# 1. A session opens on a 25-row percept that ends in `end`.
game cat
out="$("$serve" session "$dir" < /dev/null)"
[[ "$(sed -n '26p' <<< "$out")" == end ]] || fail "first percept is not 25 rows and end: $out"

# 2. A line is a key, `\r` is a real CR, and the next percept shows it.
out="$(printf '%s\n' 'a\rb' | "$serve" session "$dir")"
grep -qx 'a' <<< "$out" && grep -q '^b' <<< "$out" || fail "key not typed or \\r not a CR: $out"

# 3. One percept per line, plus the one on connect.
out="$(printf 'x\n\ny\n' | "$serve" session "$dir")"
[[ "$(grep -cx end <<< "$out")" == 4 ]] || fail "want 4 percepts for 3 lines: $out"

# 4. The game outlives a session: the next one sees what the last one typed.
out="$("$serve" session "$dir" < /dev/null)"
grep -q 'xy' <<< "$out" || fail "a new session lost the game: $out"

# 5. A coloured background is a tint line.
game "sh -c 'printf \"\\033[41mX\\033[0m\"; sleep 30'"
out="$("$serve" session "$dir" < /dev/null)"
grep -qx '1 1 X DarkRed' <<< "$out" || fail "no tint line: $out"

# 6. A game that exits ends its last percept with `over`, and takes no more keys.
game "sh -c 'read x; echo bye'"
out="$(printf '%s\n' 'q\r' q | "$serve" session "$dir")"
grep -q '^bye' <<< "$out" || fail "exit screen missing: $out"
[[ "$(tail -n 1 <<< "$out")" == over ]] || fail "last line is not over: $out"
[[ "$(grep -cx 'end\|over' <<< "$out")" == 2 ]] || fail "kept serving after over: $out"

# 7. act.sh -d plays a host game: the last percept prints, `?` adds one mid-list.
game cat
out="$("$here/act.sh" -d "$dir" a '?' b)"
[[ "$(grep -cx end <<< "$out")" == 2 ]] || fail "act.sh -d: want 2 percepts: $out"
grep -q 'ab' <<< "$out" || fail "act.sh -d: keys not typed: $out"

# 8. Without -c, start runs NIHILURK_BIN with the feel layer off.
dir="$(mktemp -d)"
dirs+=("$dir")
NIHILURK_BIN=echo "$serve" start "$dir" -s 9
out="$("$serve" session "$dir" < /dev/null)"
grep -q -- '^-ns -nobones -nshake -nb -anim-rate 0.1 -s 9' <<< "$out" || fail "NIHILURK_BIN not run with the flags: $out"

# 9. Sessions take turns: one that arrives mid-session waits, so its key
#    never lands between another session's keys.
game cat
{ printf 'a\n'; sleep 1; printf 'b\n'; } | "$serve" session "$dir" > "$dir/first" &
sleep 0.3
"$serve" session "$dir" <<< 'c' > "$dir/second"
wait
grep -q 'ab' "$dir/first" && ! grep -q 'acb' "$dir/first" || fail "sessions interleaved: $(cat "$dir/first")"
grep -q 'abc' "$dir/second" || fail "second session did not wait its turn: $(cat "$dir/second")"

# 10. A run nobody talks to for NIHILURK_IDLE seconds is killed.
NIHILURK_IDLE=1 game cat
sleep 2.5
out="$("$serve" session "$dir" < /dev/null)"
[[ "$(tail -n 1 <<< "$out")" == over ]] || fail "idle run still alive: $out"

echo "serve_test: ok"
