#!/usr/bin/env bash
#
# compat_test.sh -- nihilurk's cross-platform compatibility pipeline.
#
# Build nihilurk for every machine it claims to run on, check the particle core
# still compiles with no OS under it, and check that the built binary actually
# starts on each of those machines.
#
# The question this pipeline answers is "does nihilurk run on that hardware".
# That is a narrower claim than it used to be, and on purpose: nihilurk has no
# workload heavy enough to need a stress test, so this does not drive the game
# under load or cap a container's CPU or memory to emulate slow hardware. The
# actual strategy is: if it builds, the image has a shell, and the target is
# std (every `linux` row is), nihilurk can run there. See
# docs/explanation/cross-platform-testing.md.
#
# Three scripts, in order, each of which is worth running on its own:
#
#   compat/cross_build.sh   phase 1: one container per architecture, builds
#                           the game, prints its size and who is to blame
#                           for it
#   compat/nostd_check.sh   the bare-metal rows: particle-core for ESP32
#                           and RISC-V microcontrollers
#   compat/run_check.sh     phase 2: checks each machine's image has a
#                           shell and that the binary it built actually
#                           starts and runs there
#
# and then `nihilurk-compat`, which reports what they found.
#
# A stage whose tooling is missing is skipped with a note saying how to get
# it, not fataled -- except cross and docker, without which there is no
# pipeline at all.
#
# Usage:
#   ./compat_test.sh                  everything
#   ./compat_test.sh --targets cloud  one machine (comma-separated)
#   ./compat_test.sh --no-build       reuse the binaries already built
#   ./compat_test.sh --no-bare        skip the microcontroller check
#   ./compat_test.sh --timeout 60     per-row seconds for the run check
#   ./compat_test.sh --help

set -uo pipefail

cd "$(dirname "$0")" || exit 1
. compat/lib.sh

ONLY=""
DO_BUILD=1
DO_BARE=1
TIMEOUT=60

while [ $# -gt 0 ]; do
  case "$1" in
    --targets)  ONLY="${2:?--targets needs a list}"; shift ;;
    --timeout)  TIMEOUT="${2:?--timeout needs seconds}"; shift ;;
    --no-build) DO_BUILD=0 ;;
    --no-bare)  DO_BARE=0 ;;
    --help|-h)  sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "compat_test.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

TARGET_ARGS=()
[ -n "$ONLY" ] && TARGET_ARGS=(--targets "$ONLY")

# Tells the phase scripts that something else is driving them, so they leave
# off their own "run this next" sign-offs.
export COMPAT_DRIVEN=1

printf '\n%s  nihilurk compatibility pipeline%s\n' "$B$BLUE" "$R"
note "does nihilurk run on the machines it claims to"

# ---------------------------------------------------------------------------
# 1. Build
# ---------------------------------------------------------------------------

if [ "$DO_BUILD" -eq 1 ]; then
  if ! ./compat/cross_build.sh "${TARGET_ARGS[@]}"; then
    bad "phase 1 failed; nothing to check"
    exit 1
  fi
else
  warn "skipping the build (--no-build); reusing target/cross/<triple>/"
fi

# ---------------------------------------------------------------------------
# 2. Bare metal
# ---------------------------------------------------------------------------

if [ "$DO_BARE" -eq 1 ]; then
  # Not fatal to the pipeline: the microcontroller rows are a separate claim
  # from "nihilurk runs on this hardware", and a missing Xtensa toolchain should
  # not stop the matrix from being checked.
  ./compat/nostd_check.sh || SKIPPED="${SKIPPED}bare "
else
  warn "skipping the bare-metal check (--no-bare)"
fi

# ---------------------------------------------------------------------------
# 3. Check it
# ---------------------------------------------------------------------------

./compat/run_check.sh "${TARGET_ARGS[@]}" --timeout "$TIMEOUT"
CHECK_STATUS=$?

# ---------------------------------------------------------------------------
# 4. Verdict
# ---------------------------------------------------------------------------

heading "Verdict"
BIN="$ROOT/target/release/nihilurk-compat"
# Cheap here: run_check.sh just built it moments ago, so this is normally a
# no-op cargo already knows is a no-op.
cargo build --release -p nihilurk-compat >/dev/null 2>&1

GATE=0
if [ -x "$BIN" ]; then
  "$BIN" gate --dir "$OUT" || GATE=1
fi

[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"
printf '      artifacts in %s/\n' "$OUT"

if [ "$GATE" -ne 0 ] || [ "$CHECK_STATUS" -ne 0 ]; then
  printf '\n%s  a machine cannot run nihilurk.%s see the report above\n\n' "$RED$B" "$R"
  exit 1
fi
printf '\n%s  done.%s  Read it: target/release/nihilurk-compat\n\n' "$GREEN$B" "$R"
