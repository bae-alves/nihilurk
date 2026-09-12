#!/usr/bin/env bash
#
# compat_test.sh -- roog's cross-platform compatibility pipeline.
#
# Build roog for every machine it claims to run on, check the particle core
# still compiles with no OS under it, then actually run the game on each of
# those machines under emulation with their real CPU and memory limits, and
# report whether it is playable there.
#
# The question this pipeline answers is "does roog run on that hardware, and
# how well". It is not a benchmark of the stress test. The Bad Apple reel is
# the *load* used to push the machine past what the game asks of it, and it is
# never what anything is graded on -- a machine that cannot keep up with 800
# motes a frame may still play roog perfectly well. See
# docs/explanation/cross-platform-testing.md.
#
# Three scripts, in order, each of which is worth running on its own:
#
#   compat/cross_build.sh          phase 1: one container per architecture,
#                                  builds the game, prints its size and who
#                                  is to blame for it
#   compat/nostd_check.sh          the bare-metal rows: particle-core for
#                                  ESP32 and RISC-V microcontrollers
#   compat/stress_test_matrix.sh   phase 2: runs the game on each machine
#                                  with that machine's limits applied, and
#                                  records CPU and memory the whole time
#
# and then `roog-compat`, which is the dashboard over what they recorded.
#
# Like perf_test.sh, a stage whose tooling is missing is skipped with a note
# saying how to get it, not fataled -- except cross and docker, without which
# there is no pipeline at all.
#
# Usage:
#   ./compat_test.sh                  everything (~20 min cold, mostly pulls)
#   ./compat_test.sh --targets cloud  one machine (comma-separated)
#   ./compat_test.sh --quick          the gate only; skip the Bad Apple ceiling
#   ./compat_test.sh --no-build       reuse the binaries already built
#   ./compat_test.sh --no-bare        skip the microcontroller check
#   ./compat_test.sh --frames 900     longer gate run       [default 450]
#   ./compat_test.sh --timeout 900    per-run seconds       [default 600]
#   ./compat_test.sh --gui            finish in the dashboard
#   ./compat_test.sh --help

set -uo pipefail

cd "$(dirname "$0")" || exit 1
. compat/lib.sh

ONLY=""
DO_BUILD=1
DO_BARE=1
QUICK=0
RUN_GUI=0
FRAMES=450
TIMEOUT=600

while [ $# -gt 0 ]; do
  case "$1" in
    --targets)   ONLY="${2:?--targets needs a list}"; shift ;;
    --frames)    FRAMES="${2:?--frames needs a number}"; shift ;;
    --timeout)   TIMEOUT="${2:?--timeout needs seconds}"; shift ;;
    --no-build)  DO_BUILD=0 ;;
    --no-bare)   DO_BARE=0 ;;
    --quick)     QUICK=1 ;;
    --gui)       RUN_GUI=1 ;;
    --help|-h)   sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "compat_test.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

TARGET_ARGS=()
[ -n "$ONLY" ] && TARGET_ARGS=(--targets "$ONLY")

# Tells the three phase scripts that something else is driving them, so they
# leave off their own "run this next" sign-offs.
export COMPAT_DRIVEN=1

printf '\n%s  roog compatibility pipeline%s\n' "$B$BLUE" "$R"
note "does roog run on the machines it claims to, and how well"

# ---------------------------------------------------------------------------
# 1. Build
# ---------------------------------------------------------------------------

if [ "$DO_BUILD" -eq 1 ]; then
  if ! ./compat/cross_build.sh "${TARGET_ARGS[@]}"; then
    bad "phase 1 failed; nothing to run"
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
  # from "roog runs on this hardware", and a missing Xtensa toolchain should
  # not stop the matrix from being measured.
  ./compat/nostd_check.sh || SKIPPED="${SKIPPED}bare "
else
  warn "skipping the bare-metal check (--no-bare)"
fi

# ---------------------------------------------------------------------------
# 3. Run it
# ---------------------------------------------------------------------------

STRESS_ARGS=("${TARGET_ARGS[@]}" --frames "$FRAMES" --timeout "$TIMEOUT")
[ "$QUICK" -eq 1 ] && STRESS_ARGS+=(--no-reel)

./compat/stress_test_matrix.sh "${STRESS_ARGS[@]}"
STRESS_STATUS=$?

# ---------------------------------------------------------------------------
# 4. Verdict
# ---------------------------------------------------------------------------

heading "Verdict"
BIN="$ROOT/target/release/roog-compat"
[ -x "$BIN" ] || cargo build --release -p roog-compat >/dev/null 2>&1

GATE=0
if [ -x "$BIN" ]; then
  "$BIN" gate --dir "$OUT" || GATE=1
fi

[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"
printf '      artifacts in %s/\n' "$OUT"

if [ "$RUN_GUI" -eq 1 ] && [ -t 1 ] && [ -x "$BIN" ]; then
  printf '\n  starting the dashboard (q to quit)...\n'
  sleep 1
  exec "$BIN" --dir "$OUT"
fi

if [ "$GATE" -ne 0 ] || [ "$STRESS_STATUS" -ne 0 ]; then
  printf '\n%s  a machine stopped running roog.%s see the report above\n\n' "$RED$B" "$R"
  exit 1
fi
printf '\n%s  done.%s  Watch it: target/release/roog-compat\n\n' "$GREEN$B" "$R"
