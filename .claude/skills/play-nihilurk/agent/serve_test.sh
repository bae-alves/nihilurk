#!/usr/bin/env bash
# bash .claude/skills/play-nihilurk/agent/serve_test.sh
#
# serve.sh against stand-ins for the game, so it needs no build, no Docker and
# no socat: `start` runs the stand-in, `session` talks over stdio the way it
# talks over the socket.

set -euo pipefail
unset NIHILURK_BIN NIHILURK_BIND NIHILURK_DATA NIHILURK_IDLE
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

# 11. The game keeps its data in the run directory, never the player's own
#     leaderboard, morgue or keymap.
NIHILURK_DATA=/elsewhere game "printenv NIHILURK_DATA"
out="$("$serve" session "$dir" < /dev/null)"
grep -qxF "$dir" <<< "$out" || fail "NIHILURK_DATA is not the run dir: $out"

# 12. A session that sends nothing for NIHILURK_IDLE seconds hangs up, so a
#     silent client cannot hold the run's turn forever.
NIHILURK_IDLE=1 game cat
sleep 10 | "$serve" session "$dir" > /dev/null &
quiet=$!
sleep 0.3
timeout 5 "$serve" session "$dir" < /dev/null > /dev/null || fail "a silent session kept its turn"
kill -0 "$quiet" 2>/dev/null && fail "the silent session is still open"
wait "$quiet" || true

# 13. A line is cut at 4096 bytes, so a client cannot fill memory with one.
game cat
out="$({ head -c 9000 /dev/zero | tr '\0' a; echo; } | "$serve" session "$dir")"
[[ "$(grep -cx end <<< "$out")" == 4 ]] || fail "want 4 percepts for a 9000-byte line: $(grep -cx end <<< "$out")"

# 14. start refuses a directory other users can write into, and keeps what it
#     makes private.
dir="$(mktemp -d)"
dirs+=("$dir")
chmod 777 "$dir"
"$serve" start "$dir" -c cat 2> /dev/null && fail "started in a world-writable dir"
[[ ! -e "$dir/in" ]] || fail "touched a world-writable dir"
game cat
[[ "$(stat -c %a "$dir/raw")" == 600 ]] || fail "raw is not private: $(stat -c %a "$dir/raw")"

# 15. A run directory with a quote in its name still works.
dir="$(mktemp -d "${TMPDIR:-/tmp}/it's.XXXXXX")"
dirs+=("$dir")
"$serve" start "$dir" -c cat
[[ -s "$dir/pid" ]] || fail "a quote in the dir broke start"

# 16. listen binds 127.0.0.1 unless NIHILURK_BIND says otherwise. A stand-in
#     socat prints how it was called.
stub="$(mktemp -d)"
dirs+=("$stub")
printf '#!/bin/sh\necho "$@"\n' > "$stub/socat"
chmod +x "$stub/socat"
# Sets `args`; no subshell, so the run it starts reaches the cleanup.
listened() {
    args="$(PATH="$stub:$PATH" "$serve" listen -c cat)"
    dirs+=("${args##* }")
}
listened
grep -q 'TCP-LISTEN:7777,bind=127.0.0.1,' <<< "$args" || fail "listen is not on 127.0.0.1: $args"
NIHILURK_BIND=0.0.0.0 listened
grep -q 'TCP-LISTEN:7777,bind=0.0.0.0,' <<< "$args" || fail "NIHILURK_BIND ignored: $args"

# 17. Once the game is gone, a process that takes its pid is not the game: the
#     run answers `over` and the reaper leaves that process alone.
NIHILURK_IDLE=1 game cat
sleep 30 &
other=$!
real="$(cat "$dir/pid")"
echo "$other" > "$dir/pid"
kill "$real"
sleep 2.5
out="$("$serve" session "$dir" < /dev/null)"
kill -0 "$other" 2>/dev/null || fail "the reaper killed a process that took the game's pid"
kill "$other"
[[ "$(tail -n 1 <<< "$out")" == over ]] || fail "a reused pid counts as the game: $out"

# 18. The run starts whatever your shell is: `script` runs its command through
#     $SHELL, and a fish or nologin shell cannot read that command.
SHELL=/bin/false game cat
[[ -s "$dir/pid" ]] || fail "\$SHELL ran the game: $(cat "$dir/raw")"

# 19. A game that never starts leaves a run that is over, and `start` still
#     returns: with no pid, `/proc//stat` is the kernel's stat file, not a game.
#     A stand-in `script` that fails plays the game that never starts.
broken="$(mktemp -d)"
dirs+=("$broken")
printf '#!/bin/sh\nexit 1\n' > "$broken/script"
chmod +x "$broken/script"
dir="$(mktemp -d)"
dirs+=("$dir")
PATH="$broken:$PATH" "$serve" start "$dir" -c cat 2> "$dir/err" || fail "start died with no pid: $(cat "$dir/err")"
out="$("$serve" session "$dir" < /dev/null)"
[[ "$(tail -n 1 <<< "$out")" == over ]] || fail "a run with no game is not over: $out"

# 20. act.sh drops control bytes from what a server sends, C1 ones included,
#     so a server it does not trust cannot drive the terminal it prints to.
#     A stand-in serve.sh beside a copy of act.sh plays that server.
fake="$(mktemp -d)"
dirs+=("$fake")
cp "$here/act.sh" "$fake/"
cat > "$fake/serve.sh" <<'EOF'
#!/usr/bin/env bash
printf '\033[2Jboom\a \302\23331mred\n'
for ((i = 1; i < 25; i++)); do echo; done
echo end
EOF
chmod +x "$fake/serve.sh"
out="$("$fake/act.sh" -d "$fake")"
grep -q 'boom' <<< "$out" || fail "act.sh lost the text: $out"
LC_ALL=C grep -q $'[\x01-\x09\x0b-\x1f\x7f]\|\xc2[\x80-\x9f]' <<< "$out" && fail "act.sh passed control bytes: $(head -1 <<< "$out" | od -c | head -2)"

# 21. A percept waits for 250 ms without output: output that starts late, or
#     comes in bursts less than 250 ms apart, is all on it.
game "sh -c 'read x; sleep 0.15; echo late; read y'"
out="$("$serve" session "$dir" <<< 'x\r')"
grep -q '^late' <<< "$out" || fail "a percept missed output 150 ms after the key: $out"
game "sh -c 'read x; for i in 1 2 3 4; do printf \$i; sleep 0.2; done; read y'"
out="$("$serve" session "$dir" <<< 'x\r')"
grep -q '1234' <<< "$out" || fail "a percept cut a run of bursts 200 ms apart: $out"

# 22. A session on a run that has written nothing since its last percept
#    answers at once: the game is still waiting on a key.
game cat
"$serve" session "$dir" <<< 'a' > /dev/null
start="${EPOCHREALTIME/[.,]/}"
out="$("$serve" session "$dir" < /dev/null)"
took=$(( ${EPOCHREALTIME/[.,]/} - start ))
grep -q '^a' <<< "$out" || fail "the quick percept lost the screen: $out"
(( took < 200000 )) || fail "a settled run took $((took / 1000)) ms to answer on connect"

echo "serve_test: ok"
