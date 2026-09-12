#!/usr/bin/env bash
#
# perf_test.sh -- roog's particle-layer performance pipeline.
#
# Static analysis, then micro-benchmarks, then a live stress test, then a
# profile. Every stage prints what it found and moves on; a stage whose tool is
# not installed is skipped with a note saying how to get it, never fataled.
# That is deliberate. roog is meant to build and run on anything with a
# terminal, and a pipeline that only works on a workstation with `perf`,
# cargo-bloat and a browser installed would be a pipeline nobody runs.
#
# The core stages -- fmt, clippy, test, bench, stress -- need nothing but a
# Rust toolchain, and they include a phase-level profile of their own. The
# optional stages add detail on top where the tooling happens to exist.
#
# Usage:
#   ./perf_test.sh                  everything available
#   ./perf_test.sh --quick          skip the benchmarks (they are the slow part)
#   ./perf_test.sh --frames 900     longer stress run (default 450)
#   ./perf_test.sh --duration 30    seconds to record the profile for
#   ./perf_test.sh --density 8      eight motes per lit cell
#   ./perf_test.sh --fps 60         drive the reel at 60 fps
#   ./perf_test.sh --gui            finish in the live dashboard
#   ./perf_test.sh --baseline main  record a criterion baseline under that name
#   ./perf_test.sh --help

set -uo pipefail

cd "$(dirname "$0")" || exit 1

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------

# Stage 7 is bounded by frames and stage 8 by seconds, and they want different
# things. A frame count makes the stress run reproducible -- the same 450 frames
# of reel render on any machine, so the totals underneath are comparable between
# runs. The profile wants wall-clock instead, because what it collects is
# samples.
FRAMES=450
DURATION=15
DENSITY=1
FPS=30
RUN_BENCH=1
RUN_GUI=0
BASELINE=""
PKG=roog-perf
OUT=target/perf

while [ $# -gt 0 ]; do
  case "$1" in
    --quick|--no-bench) RUN_BENCH=0 ;;
    --gui)              RUN_GUI=1 ;;
    --frames)           FRAMES="${2:?--frames needs a value}"; shift ;;
    --duration)         DURATION="${2:?--duration needs a value}"; shift ;;
    --density)          DENSITY="${2:?--density needs a value}"; shift ;;
    --fps)              FPS="${2:?--fps needs a value}"; shift ;;
    --baseline)         BASELINE="${2:?--baseline needs a name}"; shift ;;
    --help|-h)          sed -n '3,/^[^#]/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "perf_test.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

# ---------------------------------------------------------------------------
# Presentation
# ---------------------------------------------------------------------------

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  B=$'\033[1m'; DIM=$'\033[2m'; R=$'\033[0m'
  GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'; BLUE=$'\033[34m'
else
  B=""; DIM=""; R=""; GREEN=""; YELLOW=""; RED=""; BLUE=""
fi

COLS=$(tput cols 2>/dev/null || echo 100)
[ "$COLS" -lt 60 ] && COLS=100

STAGE=0
SKIPPED=""
FAILED=""

stage()   { STAGE=$((STAGE+1)); printf '\n%s== %d. %s ==%s\n' "$B$BLUE" "$STAGE" "$1" "$R"; }
ok()      { printf '%s   ok%s  %s\n' "$GREEN" "$R" "$1"; }
warn()    { printf '%s   ..%s  %s\n' "$YELLOW" "$R" "$1"; }
bad()     { printf '%s   !!%s  %s\n' "$RED" "$R" "$1"; }
note()    { printf '%s      %s%s\n' "$DIM" "$1" "$R"; }
skip()    { SKIPPED="$SKIPPED$1"$'\n'; warn "skipped: $2"; }
have()    { command -v "$1" >/dev/null 2>&1; }
# Cargo subcommands live in ~/.cargo/bin, which cargo searches itself and which
# is often not on PATH in a non-login shell. `command -v cargo-bloat` therefore
# says "missing" for a tool that `cargo bloat` runs perfectly well; ask cargo.
have_cargo() { cargo "$1" --version >/dev/null 2>&1; }

mkdir -p "$OUT"

printf '%s\n' "$B  roog particle-layer performance pipeline$R"
note "$(rustc -V 2>/dev/null || echo 'rustc: not found')"
note "target ${FPS}fps, density x${DENSITY}, ${FRAMES}-frame stress run"

if ! have cargo; then
  bad "cargo not found; nothing can run"
  exit 1
fi

# ---------------------------------------------------------------------------
# 1. Formatting
# ---------------------------------------------------------------------------

stage "Formatting"
if ! have rustfmt && ! cargo fmt --version >/dev/null 2>&1; then
  skip fmt "rustfmt not installed (rustup component add rustfmt)"
else
  if cargo fmt --all -- --check >/dev/null 2>&1; then
    ok "all crates formatted"
  else
    bad "formatting drift; run: cargo fmt --all"
    FAILED="${FAILED}fmt "
  fi
fi

# ---------------------------------------------------------------------------
# 2. Lints
# ---------------------------------------------------------------------------

stage "Lints"
if ! cargo clippy --version >/dev/null 2>&1; then
  skip clippy "clippy not installed (rustup component add clippy)"
else
  # The workspace denies `correctness` and warns on `perf`, which is the group
  # that matters here: needless clones and allocations in a hot loop.
  if cargo clippy --workspace --all-targets 2>&1 | tee "$OUT/clippy.txt" | grep -q '^error'; then
    bad "clippy errors; see $OUT/clippy.txt"
    FAILED="${FAILED}clippy "
  else
    # `grep -c` prints 0 and exits 1 when it matches nothing, so a `|| echo 0`
    # fallback would append a second zero. `|| true` keeps grep's own count.
    WARNS=$(grep -c '^warning' "$OUT/clippy.txt" 2>/dev/null || true)
    ok "clean (${WARNS} warnings)"
  fi
fi

# ---------------------------------------------------------------------------
# 3. Correctness
# ---------------------------------------------------------------------------
# A faster particle layer that draws the wrong thing is not an improvement, so
# the game's own suite gates the rest of the run.

stage "Correctness"
if cargo test --workspace --quiet >"$OUT/test.txt" 2>&1; then
  ok "test suite green ($(grep -c 'test result: ok' "$OUT/test.txt" 2>/dev/null || true) binaries)"
else
  bad "tests failing; see $OUT/test.txt"
  FAILED="${FAILED}test "
  tail -20 "$OUT/test.txt"
fi

# ---------------------------------------------------------------------------
# 4. Build
# ---------------------------------------------------------------------------
# `profiling` is `release` with the symbols left in -- same optimisation
# settings, so what is measured is what ships. Frame pointers are forced on
# because `perf` cannot walk an LTO'd, codegen-units=1 stack without them.

stage "Build"
# Stages 1-3 run on the toolchain's ordinary flags so they share the cache with
# a plain `cargo test`; from here on the profiling build needs frame pointers.
# The shipping-size build in stage 5 is put back on the original flags for the
# same reason -- and because that number is only worth printing if it is the
# binary that actually ships.
SHIP_RUSTFLAGS="${RUSTFLAGS:-}"
export RUSTFLAGS="${RUSTFLAGS:-} -C force-frame-pointers=yes"
if cargo build --profile profiling -p "$PKG" -p engine >"$OUT/build.txt" 2>&1; then
  BIN="target/profiling/roog-perf"
  GAME="target/profiling/engine"
  ok "built $BIN"
  [ -f "$GAME" ] && note "game binary: $(du -h "$GAME" | cut -f1) (with symbols)"
else
  bad "build failed; see $OUT/build.txt"
  tail -25 "$OUT/build.txt"
  exit 1
fi

# ---------------------------------------------------------------------------
# 5. Binary footprint
# ---------------------------------------------------------------------------

stage "Binary footprint"
if have_cargo bloat; then
  cargo bloat --profile profiling -p engine --crates -n 12 2>/dev/null \
    | tee "$OUT/bloat-crates.txt" | sed 's/^/      /'
  cargo bloat --profile profiling -p engine -n 12 >"$OUT/bloat-fns.txt" 2>/dev/null
  ok "per-crate above; per-function in $OUT/bloat-fns.txt"
else
  skip bloat "cargo-bloat not installed (cargo install cargo-bloat)"
  # The portable fallback. Not a breakdown, but it does answer "did this change
  # make the binary bigger", which is what the stage is for.
  note "falling back to section sizes:"
  if have size; then
    size "$GAME" 2>/dev/null | sed 's/^/      /'
  fi
  ls -lh "$GAME" 2>/dev/null | awk '{printf "      %s  %s\n", $5, $9}'
fi
# The stripped size is the number that actually ships.
if RUSTFLAGS="$SHIP_RUSTFLAGS" cargo build --release -p engine >/dev/null 2>&1; then
  note "shipped (release, stripped): $(du -h target/release/engine 2>/dev/null | cut -f1)"
fi

# ---------------------------------------------------------------------------
# 6. Micro-benchmarks
# ---------------------------------------------------------------------------

stage "Micro-benchmarks (criterion)"
if [ "$RUN_BENCH" -eq 0 ]; then
  skip bench "--quick"
else
  BENCH_ARGS=""
  [ -n "$BASELINE" ] && BENCH_ARGS="-- --save-baseline $BASELINE"
  note "this takes a few minutes; --quick skips it"
  # One `awk` rather than `grep | sed`: grep's stdout here is a pipe, so it
  # block-buffers in 4 KB chunks and a stage that takes minutes prints nothing
  # for most of them. `fflush()` after every line is what makes it live.
  # Exiting 1 on no matches keeps the `else` branch below meaningful, which is
  # what grep's own exit status used to provide.
  # shellcheck disable=SC2086
  if cargo bench -p "$PKG" $BENCH_ARGS 2>&1 | tee "$OUT/bench.txt" \
      | awk '/^(spawn|advance|cull|current|constructors|redraw|paint)\/|time:|change:|Performance has/ \
             { print "      " $0; fflush(); n++ } END { exit(n == 0) }'; then
    ok "results in $OUT/bench.txt"
    if [ -d target/criterion ]; then
      note "criterion keeps the comparison in target/criterion/"
      note "next run reports the delta; --baseline <name> pins one to compare against"
    fi
  else
    warn "benchmarks reported no comparable output; see $OUT/bench.txt"
  fi
fi

# ---------------------------------------------------------------------------
# 7. Stress run
# ---------------------------------------------------------------------------
# Headless, so it needs no terminal and can run in CI. This is also the exact
# workload the profile in stage 8 is taken from.

stage "Stress run (headless)"
# All three workloads off one parse of the reel: the particle layer alone, the
# redraw alone, and the two composed the way the game composes them. Sharing the
# parse is what makes the three reports comparable -- same frames, same machine,
# same moment.
if "$BIN" --headless --workload all --frames "$FRAMES" \
    --density "$DENSITY" --fps "$FPS" \
    2>&1 | tee "$OUT/stress.txt" | sed 's/^/  /'; then
  ok "saved to $OUT/stress.txt"
else
  bad "stress run failed; see $OUT/stress.txt"
  FAILED="${FAILED}stress "
fi

# ---------------------------------------------------------------------------
# 8. CPU profile
# ---------------------------------------------------------------------------
# Rendered in the terminal. `perf` is Linux-only and needs permission to open a
# performance counter, so when it is unavailable the stage says so and points
# at the phase breakdown from stage 7, which measures the same split with no
# tooling at all.

stage "CPU profile"
PARANOID=$(cat /proc/sys/kernel/perf_event_paranoid 2>/dev/null || echo 99)
if ! have perf; then
  skip perf "perf not installed (Arch: sudo pacman -S perf)"
  note "stage 7's PHASES block is the portable substitute: it splits the"
  note "frame budget across advance/spawn/raster without any profiler."
elif [ "$PARANOID" -gt 2 ]; then
  # 2 is the kernel default and is fine: it forbids kernel-symbol sampling, not
  # user-space sampling of a process you own, which is all this stage wants.
  # 3 is the Debian/Ubuntu hardened setting, and that one does block it.
  skip perf "kernel.perf_event_paranoid=$PARANOID blocks sampling"
  note "allow it for this boot with:"
  note "  sudo sysctl kernel.perf_event_paranoid=2"
else
  # `--flat-out` is what makes this stage worth running. A paced stress run
  # spends ~99% of its wall clock asleep in the frame budget, and `perf` only
  # samples a running process -- so a paced 15s recording lands a few dozen
  # samples in the particle layer and several hundred in `Reel::load`, and the
  # flamegraph comes out a picture of the parser. Unpaced, the same frames run
  # back to back and the profile is the thing being profiled.
  note "recording ${DURATION}s at 999 Hz (unpaced, screen+particles)..."
  if perf record -F 999 -g --call-graph fp -o "$OUT/perf.data" -- \
       "$BIN" --headless --flat-out --workload both --duration "$DURATION" \
       --density "$DENSITY" --fps "$FPS" \
       >"$OUT/perf-record.txt" 2>&1; then
    # Raw `perf script`, not folded stacks: `roog-perf flame` sniffs either, and
    # the raw form keeps the per-sample detail, so the recording can be re-read
    # later or fed to inferno/stackcollapse without re-running anything.
    perf script -i "$OUT/perf.data" > "$OUT/perf.script" 2>/dev/null
    # Drawn twice on purpose: once plain into a file that can be diffed against
    # the last run, once to the terminal in colour.
    if "$BIN" flame "$OUT/perf.script" --width "$COLS" --no-color > "$OUT/flame.txt"; then
      "$BIN" flame "$OUT/perf.script" --width "$COLS"
      ok "profile in $OUT/perf.data, plain-text copy in $OUT/flame.txt"
      if have_cargo flamegraph; then
        note "an SVG, if you want one:"
        note "  cargo flamegraph --profile profiling -p $PKG -- --headless --flat-out"
      fi
    else
      warn "no readable samples in $OUT/perf.script; stage 7's PHASES stands in"
    fi
  else
    warn "perf record failed; see $OUT/perf-record.txt"
    tail -5 "$OUT/perf-record.txt" | sed 's/^/      /'
  fi
fi

# ---------------------------------------------------------------------------
# 9. Summary
# ---------------------------------------------------------------------------

stage "Summary"
# The comparison table the stress run ends with already is the summary: one row
# per workload, and the three ran off one parse of the reel, so they are directly
# comparable. Everything above it in the file is the three reports it draws from.
sed -n '/^=== comparison/,$p' "$OUT/stress.txt" 2>/dev/null | sed 's/^/  /'

if [ -n "$SKIPPED" ]; then
  printf '\n%s  skipped stages:%s %s\n' "$YELLOW" "$R" "$(echo "$SKIPPED" | tr '\n' ' ')"
  note "everything above ran without them; they add detail, not correctness"
fi

printf '\n  artifacts in %s/\n' "$OUT"

if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi

if [ "$RUN_GUI" -eq 1 ]; then
  if [ -t 1 ]; then
    printf '\n  starting the dashboard (q to quit)...\n'
    sleep 1
    exec "$BIN" --density "$DENSITY" --fps "$FPS"
  fi
  warn "--gui needs a terminal; skipping"
fi

printf '\n%s  done.%s  Watch it: %s            (particle dashboard)\n' "$GREEN$B" "$R" "$BIN"
printf '            %s --workload both  (the redraw, for real)\n\n' "$BIN"
