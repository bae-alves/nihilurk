#!/usr/bin/env bash
#
# The headless half of phase 1: prove that the game's rules run with no
# terminal anywhere in their build.
#
# WHAT THIS DOES AND DOES NOT CLAIM
#
# It does not claim the game runs headless. It draws and reads keys through
# crossterm, and that lives in one file, `engine/src/term.rs`. What is proved
# is that nothing else needs a terminal: `models` -- every rule, component and
# system that decides what a turn does -- has no crossterm in its dependency
# graph, builds without it, and plays its whole test suite against worlds that
# never had a screen. A second front end (a web page, a bot, a test rig) can
# take `models` as it stands.
#
# Three checks, in order:
#
#   1. the graph    `cargo tree` for nihilurk-models lists no crossterm,
#                   dev- and build-dependencies included. This sees a
#                   dependency that came in through another crate, which
#                   a scan of Cargo.toml would not.
#   2. the boundary engine/tests/term_boundary.rs: only term.rs names
#                   crossterm, and models/Cargo.toml does not list it.
#   3. the rules    `cargo test -p nihilurk-models`, headless by
#                   construction: the models crate cannot open a terminal.
#
# Host-only. There is no matrix row for it, because a terminal is the same
# absence on every machine; the per-machine claim is run_check.sh's.
#
# Usage:
#   ./compat/headless_check.sh           all three
#   ./compat/headless_check.sh --quick   the first two (a few seconds)
#   ./compat/headless_check.sh --help

set -uo pipefail

. "$(dirname "$0")/lib.sh"

QUICK=0

while [ $# -gt 0 ]; do
  case "$1" in
    --quick)   QUICK=1 ;;
    --help|-h) sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "headless_check.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

cd "$ROOT" || exit 1
mkdir -p "$OUT"

MODELS=nihilurk-models

printf '%s\n' "$B  nihilurk headless check -- do the rules need a terminal?$R"
note "$(rustc -V 2>/dev/null || echo 'rustc: not found')"

stage "The dependency graph of $MODELS"
if cargo tree -p "$MODELS" -e normal,build,dev --prefix none >"$OUT/headless-tree.log" 2>&1; then
  if grep -qw crossterm "$OUT/headless-tree.log"; then
    bad "crossterm is in the graph of $MODELS"
    grep -w crossterm "$OUT/headless-tree.log" | sort -u | sed 's/^/      /'
    FAILED="${FAILED}graph "
  else
    ok "$MODELS depends on no terminal library"
  fi
else
  bad "cargo tree failed; see $OUT/headless-tree.log"
  FAILED="${FAILED}graph "
fi

stage "The boundary"
if cargo test -p nihilurk --test term_boundary >"$OUT/headless-boundary.log" 2>&1; then
  ok "only engine/src/term.rs names crossterm"
else
  bad "the boundary test failed; see $OUT/headless-boundary.log"
  tail -12 "$OUT/headless-boundary.log" | sed 's/^/      /'
  FAILED="${FAILED}boundary "
fi

if [ "$QUICK" -eq 0 ]; then
  stage "The rules, with no terminal"
  if cargo test -p "$MODELS" >"$OUT/headless-models.log" 2>&1; then
    ok "$MODELS builds and passes its tests with no terminal"
  else
    bad "$MODELS tests failed; see $OUT/headless-models.log"
    tail -12 "$OUT/headless-models.log" | sed 's/^/      /'
    FAILED="${FAILED}rules "
  fi
else
  note "skipped the models test run (--quick)"
fi

stage "Summary"
note "what this proves: the game's rules need no terminal."
note "what it does not: that the game draws without one. term.rs is that seam."
note "See docs/explanation/cross-platform-testing.md."

if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi
printf '\n%s  ok.%s  the rules are still terminal-free\n\n' "$GREEN$B" "$R"
