#!/usr/bin/env bash
#
# Phase 2 of the compat pipeline: run roog on every machine it claims to run
# on, with that machine's limits actually applied, and record what it cost.
#
# One container per row of matrix.tsv. The row's `--cpus` and `--memory` are
# handed straight to Docker, swap is disabled so the memory cap is a real
# ceiling rather than a suggestion, and the binary inside is the static musl
# one `cross_build.sh` produced for that row's architecture. Foreign
# architectures run under qemu-user via binfmt_misc.
#
# WHAT IS BEING MEASURED
#
# Whether roog runs on the machine, and how well. Each row is run twice:
#
#   --load game   a real dungeon floor, animated by the batches the game
#                 actually queues. This is roog running, and it is the only
#                 run anything is graded on.
#   --load reel   Bad Apple: ~800 motes a frame, two to three orders of
#                 magnitude past anything the game produces. This is the
#                 ceiling -- how much harder you could push the machine
#                 before the particle layer gives out. Nothing is gated on
#                 it, and on the slowest rows it is expected to lose.
#
# A machine that cannot keep up with Bad Apple may still play roog perfectly
# well. Grading on the reel would fail the rows that matter and would answer
# a question nobody asked.
#
# Usage:
#   ./compat/stress_test_matrix.sh                  every Linux row
#   ./compat/stress_test_matrix.sh --targets pi-zero,potato
#   ./compat/stress_test_matrix.sh --no-reel        the gate only, much faster
#   ./compat/stress_test_matrix.sh --frames 900     longer gate run
#   ./compat/stress_test_matrix.sh --reel-frames 300  shorter ceiling run
#   ./compat/stress_test_matrix.sh --timeout 900    per-run seconds
#   ./compat/stress_test_matrix.sh --install-qemu   register the binfmt hooks
#   ./compat/stress_test_matrix.sh --gui            finish in the dashboard
#   ./compat/stress_test_matrix.sh --help

set -uo pipefail

. "$(dirname "$0")/lib.sh"

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------

# The gate run. 450 frames is 15 seconds of animation at 30 fps, and it is the
# same default `roog-perf` and `perf_test.sh` use -- so a row's numbers here are
# directly comparable to the host numbers in `target/perf/`.
FRAMES=450

# The ceiling run. Same 450 frames as the gate, and it has to be: Bad Apple
# opens on a nearly black screen and the motes ramp up over the first few
# hundred frames. A short ceiling run measures the titles, not the video --
# 60 frames reports 0.096 ms mean against a p95 of 0.314, which is the ramp
# caught mid-climb and reads as a machine with far more headroom than it has.
# 450 frames is where `roog-perf`'s own numbers stop moving.
#
# --reel-frames is still there for when you are waiting on an emulated row and
# only need the shape of the answer, and REEL_FRAMES_HONEST is the line below
# which the run gets a warning rather than a quiet wrong number.
REEL_FRAMES=450
REEL_FRAMES_HONEST=300

FPS=30
TIMEOUT=600
ONLY=""
WITH_REEL=1
INSTALL_QEMU=0
RUN_GUI=0

while [ $# -gt 0 ]; do
  case "$1" in
    --targets)      ONLY="${2:?--targets needs a list}"; shift ;;
    --frames)       FRAMES="${2:?--frames needs a number}"; shift ;;
    --reel-frames)  REEL_FRAMES="${2:?--reel-frames needs a number}"; shift ;;
    --fps)          FPS="${2:?--fps needs a number}"; shift ;;
    --timeout)      TIMEOUT="${2:?--timeout needs seconds}"; shift ;;
    --no-reel)      WITH_REEL=0 ;;
    --install-qemu) INSTALL_QEMU=1 ;;
    --gui)          RUN_GUI=1 ;;
    --help|-h)      sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "stress_test_matrix.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

mkdir -p "$OUT"
cd "$ROOT" || exit 1

RESULTS="$OUT/results.tsv"
REEL="$ROOT/perf/bad-apple"

printf '%s\n' "$B  roog compat matrix -- does roog run on these machines?$R"
note "gate: --load game, $FRAMES frames at $FPS fps. Graded."
case "$WITH_REEL" in
  1) note "ceiling: --load reel, $REEL_FRAMES frames of Bad Apple. Not graded."
     if [ "$REEL_FRAMES" -lt "$REEL_FRAMES_HONEST" ]; then
       warn "$REEL_FRAMES reel frames is Bad Apple's near-black opening, not its"
       warn "load: the ceiling will read far kinder than the machine deserves."
       note "  $REEL_FRAMES_HONEST or more for a ceiling worth quoting"
     fi ;;
  *) note "ceiling: skipped (--no-reel)" ;;
esac

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

if ! docker info >/dev/null 2>&1; then
  bad "docker is not answering"
  note "  sudo systemctl start docker    (and: usermod -aG docker \$USER)"
  exit 1
fi

# A foreign-architecture binary only runs here if the kernel knows to hand it
# to qemu. Docker Desktop ships these hooks; a plain Arch install does not, and
# without them an aarch64 container dies with `exec format error`, which looks
# like a broken build rather than a missing emulator.
QEMU_IMAGE=tonistiigi/binfmt
if [ "$INSTALL_QEMU" -eq 1 ]; then
  stage "Registering qemu-user with binfmt_misc"
  note "this needs --privileged; it writes to /proc/sys/fs/binfmt_misc"
  if docker run --privileged --rm "$QEMU_IMAGE" --install arm64,arm,386 >/dev/null 2>&1; then
    ok "registered"
  else
    bad "could not register the binfmt hooks"
    note "  docker run --privileged --rm $QEMU_IMAGE --install all"
  fi
fi

# Which interpreter a row needs, by docker platform. Empty for a row the host
# CPU runs itself: x86_64 obviously, and i686 too -- a 64-bit kernel runs
# 32-bit user space natively, so the potato row is starved rather than emulated.
qemu_handler_for() {
  case "$1" in
    linux/arm64)  echo qemu-aarch64 ;;
    linux/arm/v7) echo qemu-arm ;;
    *)            echo "" ;;
  esac
}

have_qemu_for() {
  local handler
  handler=$(qemu_handler_for "$1")
  [ -z "$handler" ] && return 0
  [ -r "/proc/sys/fs/binfmt_misc/$handler" ]
}

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------
#
# What Docker sees from outside the container, which is not what `roog-perf`
# sees from inside it. The in-process figures cover the game; these cover the
# whole cgroup, qemu included. Both are reported, and the gap between them is
# the emulation tax.
#
# `docker stats --no-stream` takes about a second to return, which is also a
# perfectly good sampling period -- so the loop has no sleep in it and the
# cadence comes from the tool.

# "12.34MiB / 512MiB" and "45.32%" into plain numbers. Docker's units are
# decimal-ish and inconsistent across versions (MiB here, MB there), so both
# spellings are handled rather than assumed.
to_bytes() {
  awk -v s="$1" 'BEGIN {
    if (match(s, /^[0-9.]+/) == 0) { print 0; exit }
    n = substr(s, RSTART, RLENGTH) + 0
    unit = substr(s, RSTART + RLENGTH)
    gsub(/[^A-Za-z]/, "", unit)
    u = toupper(unit)
    if (u == "KIB" || u == "KB" || u == "K") n *= 1024
    else if (u == "MIB" || u == "MB" || u == "M") n *= 1024 * 1024
    else if (u == "GIB" || u == "GB" || u == "G") n *= 1024 * 1024 * 1024
    else if (u == "TIB" || u == "TB" || u == "T") n *= 1024 * 1024 * 1024 * 1024
    printf "%d\n", n
  }'
}

sample_container() {
  local name=$1 out=$2
  local start line cpu mem_pair used limit now
  start=$(date +%s)
  : > "$out"
  while docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null | grep -q true; do
    line=$(docker stats --no-stream --format '{{.CPUPerc}}|{{.MemUsage}}' "$name" 2>/dev/null) || break
    [ -z "$line" ] && break
    cpu=${line%%|*}
    mem_pair=${line#*|}
    used=$(to_bytes "$(echo "${mem_pair%%/*}" | tr -d ' ')")
    limit=$(to_bytes "$(echo "${mem_pair##*/}" | tr -d ' ')")
    now=$(date +%s)
    # The last read of a container that is on its way out comes back as `--`,
    # which parses to zeroes. Recording it would put a 0 B sample at the end of
    # every series -- and since the cap is read off the samples, a dashboard
    # showing "0 B of 0 B" for a run that went perfectly well.
    if [ "$limit" -gt 0 ] && [ "$used" -gt 0 ]; then
      printf '%s\t%s\t%s\t%s\n' \
        "$((now - start))" "$(echo "$cpu" | tr -d '% ')" "$used" "$limit" >> "$out"
    fi
  done
}

# ---------------------------------------------------------------------------
# Reading the report back
# ---------------------------------------------------------------------------
#
# `roog-perf --headless` prints a plain-text report and this pulls the six
# numbers the dashboard grades on out of it. Parsing our own tool's output is
# not ideal, and the alternative -- a --json flag on roog-perf -- would put a
# serialiser in the crate whose whole point is that it has no dependencies.
# The report format is covered by perf's own tests, so it does not drift
# silently.

parse_report() {
  awk '
    # "  frames        450 in 15.1s wall (29.8 fps achieved)"
    #    $1      $2  $3     $4   $5    $6    $7  $8
    # No three-argument match(): that is a gawk extension, and on mawk the
    # whole program fails to parse rather than just that line. Same rule
    # lib.sh follows.
    /^  frames / { frames = $2; fps = substr($6, 2) }
    /^  FRAME TIME \(budget/ { gsub(/[^0-9.]/, "", $4); budget = $4 }
    /^    mean / { mean = $2 }
    /^    p99 / { p99 = $2 }
    /^    dropped / { dropped = $2 }
    /^    peak RSS / { rss = $3 " " $4 }
    /^    cpu / { cpu = $2 }
    END {
      printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n",
        (frames == "" ? 0 : frames), (mean == "" ? 0 : mean),
        (p99 == "" ? 0 : p99), (dropped == "" ? 0 : dropped),
        (fps == "" ? 0 : fps), (rss == "" ? "0 B" : rss),
        (cpu == "" ? 0 : cpu)
    }
  ' "$1"
}

# ---------------------------------------------------------------------------
# One run
# ---------------------------------------------------------------------------
#
# Detached rather than in the foreground, because the container has to be alive
# and named for `docker stats` to have something to watch. The exit code comes
# back from `docker wait`, bounded by `timeout` -- a row too slow to finish is
# a result, not a reason to hang the pipeline.

run_row() {
  local id=$1 target=$2 platform=$3 image=$4 cpus=$5 memory=$6 load=$7 frames=$8
  local name="roog-compat-$id-$load"
  local log="$OUT/run-$id-$load.log"
  local stats="$OUT/stats-$id-$load.tsv"
  local bin
  bin=$(target_bin "$target" "$RIG" release)

  if [ ! -x "$bin" ]; then
    warn "$id/$load: no binary at $bin"
    note "  ./compat/cross_build.sh --targets $id"
    record "$id" "$target" "$load" skipped 0 0 0 0 0 0 0 0 0
    return 1
  fi

  docker rm -f "$name" >/dev/null 2>&1

  # --memory-swap equal to --memory is what makes the cap real: without it the
  # container gets the same amount again as swap, and a row that should have
  # been killed for running out of RAM quietly swaps instead and reports a
  # frame time from a machine that does not exist.
  #
  # The reel is mounted read-only and only where it is used. The game load
  # needs no file at all -- the floor comes from a seed -- which is what lets
  # the gate run on a row too small to hold 10 MiB of Bad Apple.
  local mounts=(-v "$bin:/roog-perf:ro")
  #
  # `--workload both` and not the default: the question is whether the machine
  # can draw a frame of roog, and a frame of roog is the floor repainted, the
  # live motes composited over it, and one diff-and-flush over the result --
  # exactly what engine/src/view.rs does. Measuring the particle layer alone
  # would leave out the redraw, which on a slow machine is most of the cost.
  # Headless, the flush goes into a counting sink rather than a terminal, so
  # there is nothing attached to stdout being benchmarked by accident.
  local args=(--headless --workload both --load "$load" --frames "$frames" --fps "$FPS")
  if [ "$load" = "reel" ]; then
    mounts+=(-v "$REEL:/bad-apple:ro")
    args+=(--reel /bad-apple)
  fi

  local started ended wall exit_code status
  started=$(date +%s)
  if ! docker run -d --name "$name" \
        --platform "$platform" \
        --cpus "$cpus" --memory "$memory" --memory-swap "$memory" \
        "${mounts[@]}" \
        "$image" /roog-perf "${args[@]}" >/dev/null 2>"$log"; then
    bad "$id/$load: container would not start"
    tail -3 "$log" | sed 's/^/      /'
    record "$id" "$target" "$load" failed 0 0 0 0 0 0 0 0 0
    return 1
  fi

  sample_container "$name" "$stats" &
  local sampler=$!

  exit_code=$(timeout "$TIMEOUT" docker wait "$name" 2>/dev/null)
  status=ok
  if [ -z "$exit_code" ]; then
    status=timeout
    exit_code=-1
    docker kill "$name" >/dev/null 2>&1
  fi
  wait "$sampler" 2>/dev/null

  ended=$(date +%s)
  wall=$((ended - started))
  docker logs "$name" > "$log" 2>&1
  docker rm -f "$name" >/dev/null 2>&1

  [ "$exit_code" != "0" ] && [ "$status" = "ok" ] && status=failed

  local parsed frames_run mean p99 dropped fps_got rss_text cpu rss budget
  parsed=$(parse_report "$log")
  IFS=$'\t' read -r frames_run mean p99 dropped fps_got rss_text cpu <<< "$parsed"
  rss=$(to_bytes "$(echo "$rss_text" | tr -d ' ')")
  budget=$(awk -v f="$FPS" 'BEGIN { printf "%.2f", 1000.0 / f }')

  # A container that exited 0 but printed nothing parseable did not run the
  # game, whatever its exit code says.
  [ "$frames_run" = "0" ] && [ "$status" = "ok" ] && status=failed

  record "$id" "$target" "$load" "$status" \
    "$frames_run" "$mean" "$p99" "$dropped" "$fps_got" "$rss" "$cpu" "$wall" "$budget"

  case "$status" in
    ok) ok "$load: ${mean}ms mean, ${p99}ms p99, $dropped dropped, $(human_bytes "$rss") peak" ;;
    timeout)
      bad "$load: still going after ${TIMEOUT}s -- this machine is too slow to finish"
      FAILED="${FAILED}$id/$load " ;;
    *)
      bad "$load: exit $exit_code; see $log"
      tail -6 "$log" | sed 's/^/      /'
      FAILED="${FAILED}$id/$load " ;;
  esac
}

# One line per finished run. The columns are read by `roog-compat`; see
# compat/src/results.rs, which is the other half of this contract.
record() {
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t-\n' "$@" >> "$RESULTS"
}

# ---------------------------------------------------------------------------
# The matrix
# ---------------------------------------------------------------------------

# Truncated, not appended to, and that is on purpose even with `--targets`.
# A results file describes one run of the matrix. Keeping rows from an earlier
# run would mean a report mixing numbers from two different builds of the game,
# with nothing on the page to say which row came from which -- and the row that
# went stale is exactly the row someone is about to quote.
{
  printf '# roog compat results -- written by stress_test_matrix.sh, read by roog-compat\n'
  printf '#id\ttarget\tload\tstatus\tframes\tmean_ms\tp99_ms\tdropped\tfps\tpeak_rss\tcpu_pct\twall_s\tbudget_ms\tnote\n'
} > "$RESULTS"

# Mirror the footprint table into the results directory in the layout the
# dashboard reads. `cross_build.sh` writes it with the sizes in columns 3 and 4
# already; copying rather than re-statting keeps one definition of "how big is
# it" in the pipeline.
if [ -s "$OUT/footprint.txt" ]; then
  cp "$OUT/footprint.txt" "$OUT/footprint.tsv"
fi

while IFS=$'\t' read -r id target class platform image exec cpus memory note_text; do
  stage "$id  ($cpus cpu, $memory)"
  note "$note_text"

  if ! have_qemu_for "$platform"; then
    bad "$id needs $(qemu_handler_for "$platform"), and binfmt_misc does not have it"
    note "  ./compat/stress_test_matrix.sh --install-qemu"
    note "  or: docker run --privileged --rm $QEMU_IMAGE --install all"
    record "$id" "$target" game skipped 0 0 0 0 0 0 0 0 0
    SKIPPED="$SKIPPED$id "
    continue
  fi

  run_row "$id" "$target" "$platform" "$image" "$cpus" "$memory" game "$FRAMES"
  [ "$WITH_REEL" -eq 1 ] && \
    run_row "$id" "$target" "$platform" "$image" "$cpus" "$memory" reel "$REEL_FRAMES"
done < <(matrix_rows_or_die linux)

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
#
# The grading lives in `roog-compat`, not here. One implementation of "is this
# machine fast enough", in a language with tests, reached by the script, the
# dashboard and CI alike -- rather than a band in awk that drifts away from the
# band in Rust.

stage "Report"
REPORT_BIN="$ROOT/target/release/roog-compat"
if [ ! -x "$REPORT_BIN" ]; then
  note "building roog-compat..."
  cargo build --release -p roog-compat >/dev/null 2>&1
fi
if [ -x "$REPORT_BIN" ]; then
  "$REPORT_BIN" report --dir "$OUT"
else
  warn "could not build roog-compat; the raw numbers are in $RESULTS"
fi

printf '      artifacts in %s/\n' "$OUT"
[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"

if [ "$RUN_GUI" -eq 1 ] && [ -t 1 ] && [ -x "$REPORT_BIN" ]; then
  printf '\n  starting the dashboard (q to quit)...\n'
  sleep 1
  exec "$REPORT_BIN" --dir "$OUT"
fi

if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi
driven || printf '\n%s  done.%s  Watch it: %s\n\n' "$GREEN$B" "$R" "target/release/roog-compat"
