#!/usr/bin/env bash
#
# Phase 2 of the compat pipeline: run roog on every machine it claims to run
# on, with that machine's limits actually applied, and record what it cost.
#
# One container per row of matrix.tsv. The row's `--cpus` and `--memory` are
# handed straight to Docker, swap is disabled so the memory cap is a real
# ceiling rather than a suggestion, and the binary inside is the static musl
# one `cross_build.sh` produced for that row's architecture.
#
# A `qemu` row is not actually executed by default. qemu-user-static
# translates syscalls, not ioctls with architecture-specific encodings, and
# roog draws through crossterm, whose terminal-size and raw-mode ioctls are
# exactly the ones it gets wrong -- reliably enough that a "does this row run"
# question asked of an emulated crossterm program is really "does qemu's
# ioctl translation work today", which is not what this pipeline is for. See
# "Why emulated rows are build-only" below. `--exec-emulated` overrides this
# for anyone who does have real hardware, or a qemu build good enough to
# trust, to actually check against -- see "Foreign architectures, without
# binfmt_misc" below for how that path still works, unchanged, when asked for.
#
# WHAT IS BEING MEASURED
#
# Whether roog runs on the machine, and how well. A row that is actually
# executed (a native row always; a `qemu` row only with `--exec-emulated` --
# see below) is run three times:
#
#   --load game   a real dungeon floor, animated by the batches the game
#                 actually queues. This is roog running, and it is the only
#                 run anything is graded on.
#   --load reel   Bad Apple: ~800 motes a frame, two to three orders of
#                 magnitude past anything the game produces. This is the
#                 ceiling -- how much harder you could push the machine
#                 before the particle layer gives out. Nothing is gated on
#                 it, and on the slowest rows it is expected to lose.
#   the screen    roog-perf's redraw viewer, not headless, with a pseudo-TTY
#                 sized from inside the container so crossterm believes it
#                 has a real terminal to draw on. Bounded by frames rather
#                 than a keypress -- see "The screen" below. Not timed, but
#                 it must come up clean.
#
# A machine that cannot keep up with Bad Apple may still play roog perfectly
# well. Grading on the reel would fail the rows that matter and would answer
# a question nobody asked.
#
# THE SCREEN
#
# The game and reel runs above are deliberately headless: `--headless` sends
# the frame's diff-and-flush into a counting sink instead of a terminal, which
# is what makes the frame-time numbers reproducible rather than a measurement
# of that day's pty. But defaulting an entire compat pipeline to headless,
# for a game, would mean never actually proving the thing draws -- so an
# executed row also gets one pty-attached, non-headless run of roog-perf's
# redraw viewer (`docker run -t`, no `--headless`), bounded by `--frames`
# like the gate is. It is not graded on speed and is not the reel's ceiling;
# it only has to come up and run to the frame bound without crossterm or the
# viewer falling over. `--no-screen` skips it, for the same reason
# `--no-reel` exists: sometimes you only want the gate. See "The screen"
# below for why this needs a shell, not just a pty.
#
# WHY EMULATED ROWS ARE BUILD-ONLY
#
# `cross_build.sh` already proves more than the game asks of the machine it
# names: it cross-compiles roog with a full Rust toolchain, which is heavier,
# by every measure that matters here, than roog's own frame loop ever is. A
# static, correctly-linked ARM binary coming out of that (checked by
# cross_build.sh's own `file` inspection) is a real claim about the hardware
# roog will run on, and it is a claim this pipeline can actually stand behind
# without also standing behind qemu-user's ioctl translation. A `does not run`
# because the emulator mishandled `TIOCGWINSZ`, not because roog broke, is a
# false alarm dressed as a compat failure -- worse than no answer, because it
# reads exactly like the real thing until someone spends an afternoon on the
# log.
#
# So an emulated row's contribution to "does roog run on these machines" is
# the build, same as a `bare` row's is compiling `particle-core` -- neither is
# executed by default, and both say plainly, on that row, why not.
# `--exec-emulated` is there for whoever eventually points this at real
# hardware, or a qemu build worth trusting for this.
#
# Usage:
#   ./compat/stress_test_matrix.sh                  every Linux row
#   ./compat/stress_test_matrix.sh --targets pi-zero,potato
#   ./compat/stress_test_matrix.sh --no-reel        skip the ceiling
#   ./compat/stress_test_matrix.sh --no-screen      skip the pty/screen check
#   ./compat/stress_test_matrix.sh --exec-emulated  actually run the qemu rows too
#   ./compat/stress_test_matrix.sh --frames 900     longer gate run
#   ./compat/stress_test_matrix.sh --reel-frames 300  shorter ceiling run
#   ./compat/stress_test_matrix.sh --screen-frames 300  longer screen check
#   ./compat/stress_test_matrix.sh --timeout 900    per-run seconds
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

# The screen check only has to prove the redraw viewer comes up and keeps
# drawing -- five seconds at 30 fps is plenty, and it is not measured on
# speed so there is no honesty floor to warn about the way there is for the
# reel.
SCREEN_FRAMES=150

FPS=30
TIMEOUT=600
ONLY=""
WITH_REEL=1
WITH_SCREEN=1
EXEC_EMULATED=0
RUN_GUI=0
# Rows build-checked rather than executed by design (see "why emulated rows
# are build-only" above) -- distinct from $SKIPPED, which is a row this run
# genuinely could not say anything about.
BUILD_ONLY=""

while [ $# -gt 0 ]; do
  case "$1" in
    --targets)       ONLY="${2:?--targets needs a list}"; shift ;;
    --frames)        FRAMES="${2:?--frames needs a number}"; shift ;;
    --reel-frames)   REEL_FRAMES="${2:?--reel-frames needs a number}"; shift ;;
    --screen-frames) SCREEN_FRAMES="${2:?--screen-frames needs a number}"; shift ;;
    --fps)           FPS="${2:?--fps needs a number}"; shift ;;
    --timeout)       TIMEOUT="${2:?--timeout needs seconds}"; shift ;;
    --no-reel)       WITH_REEL=0 ;;
    --no-screen)     WITH_SCREEN=0 ;;
    --exec-emulated) EXEC_EMULATED=1 ;;
    --gui)           RUN_GUI=1 ;;
    --help|-h)       sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
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
case "$WITH_SCREEN" in
  1) note "screen: redraw viewer under a sized pty, $SCREEN_FRAMES frames. Not timed, must come up clean." ;;
  *) note "screen: skipped (--no-screen)" ;;
esac
case "$EXEC_EMULATED" in
  1) note "emulated (qemu) rows: executed like any other, per --exec-emulated" ;;
  *) note "emulated (qemu) rows: build-verified only, not run -- see the header" ;;
esac

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

if ! docker info >/dev/null 2>&1; then
  bad "docker is not answering"
  note "  sudo systemctl start docker    (and: usermod -aG docker \$USER)"
  exit 1
fi

# ---------------------------------------------------------------------------
# Foreign architectures, without binfmt_misc
# ---------------------------------------------------------------------------
#
# A foreign-arch binary needs *something* to translate its instructions for
# the host CPU. The usual way -- registering qemu-user with the kernel's
# binfmt_misc, so `execve` on a foreign ELF is transparently handed to it --
# needs `--privileged` once per machine (it writes to
# /proc/sys/fs/binfmt_misc), does not survive every reboot, and fails exactly
# the same way ("exec format error") whether the hooks were never installed
# or just did not survive the last one. None of that is necessary here.
#
# Every binary this pipeline runs is static (cross_build.sh checks it), so a
# qemu-user interpreter never needs a foreign sysroot -- it only has to
# translate the guest's syscalls to the host kernel. That means it can be
# invoked directly, as the container's own command, on a container built for
# the *host's* architecture: no `--platform`, no binfmt_misc, no
# `--privileged`, nothing written outside this pipeline's own output
# directory. `docker run --platform linux/arm64 alpine ...` is what needs the
# kernel's help; `docker run alpine /qemu-aarch64 /roog-perf ...` does
# not, because nothing is asking the kernel to exec a foreign ELF -- only the
# native `qemu-aarch64` binary is, and it does that in user space.
QEMU_IMAGE=tonistiigi/binfmt
QEMU_DIR="$OUT/qemu"

# Which static interpreter a row needs, by docker platform. Empty for a row
# the host CPU runs itself: x86_64 obviously, and i686 too -- a 64-bit kernel
# runs 32-bit user space natively, so the potato row is starved rather than
# emulated.
qemu_handler_for() {
  case "$1" in
    linux/arm64)  echo qemu-aarch64 ;;
    linux/arm/v7) echo qemu-arm ;;
    *)            echo "" ;;
  esac
}

# Fetches one static interpreter out of $QEMU_IMAGE's filesystem and caches it
# in $QEMU_DIR, without ever running that image -- `docker create` plus
# `docker cp` touches nothing but this pipeline's own output directory, so it
# needs no more privilege than pulling any other image. Prints the cached
# path on success.
ensure_qemu_interpreter() {
  local handler="$1" dest cid
  dest="$QEMU_DIR/$handler"
  if [ -s "$dest" ]; then
    echo "$dest"
    return 0
  fi
  mkdir -p "$QEMU_DIR"
  cid=$(docker create "$QEMU_IMAGE" true 2>/dev/null) || return 1
  docker cp "$cid:/usr/bin/$handler" "$dest" >/dev/null 2>&1
  docker rm -f "$cid" >/dev/null 2>&1
  [ -s "$dest" ] || return 1
  chmod +x "$dest"
  echo "$dest"
}

have_qemu_for() {
  local handler
  handler=$(qemu_handler_for "$1")
  [ -z "$handler" ] && return 0
  ensure_qemu_interpreter "$handler" >/dev/null
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
  local id=$1 target=$2 platform=$3 image=$4 exec_kind=$5 cpus=$6 memory=$7 load=$8 frames=$9
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
  local cmd=(/roog-perf "${args[@]}")

  # A qemu row runs as a plain host-architecture container -- no `--platform`
  # -- with a native qemu-user-static interpreter mounted in and put in front
  # of the command. See "Foreign architectures, without binfmt_misc" above:
  # the guest binary is static, so the interpreter needs nothing else from
  # this row's own architecture, and the container's platform stops mattering.
  local docker_platform=(--platform "$platform")
  if [ "$exec_kind" = "qemu" ]; then
    local handler qemu_bin
    handler=$(qemu_handler_for "$platform")
    qemu_bin=$(ensure_qemu_interpreter "$handler") || {
      warn "$id/$load: could not fetch $handler from $QEMU_IMAGE"
      record "$id" "$target" "$load" skipped 0 0 0 0 0 0 0 0 0
      return 1
    }
    docker_platform=()
    mounts+=(-v "$qemu_bin:/qemu-static:ro")
    cmd=(/qemu-static "${cmd[@]}")
  fi

  local started ended wall exit_code status
  started=$(date +%s)
  # `-t` allocates a pseudo-TTY even though nothing is attached to it (`-d`,
  # no `-i`): it costs nothing on a headless run, and it is what lets the
  # screen check below get a real terminal for crossterm to find, on the same
  # code path every other row uses.
  if ! docker run -d -t --name "$name" \
        "${docker_platform[@]}" \
        --cpus "$cpus" --memory "$memory" --memory-swap "$memory" \
        "${mounts[@]}" \
        "$image" "${cmd[@]}" >/dev/null 2>"$log"; then
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

# ---------------------------------------------------------------------------
# The screen
# ---------------------------------------------------------------------------
#
# Same binary, same row, no `--headless`: `--workload both` (and `screen`)
# redraw, so `run_stress` in perf/src/main.rs sends this to `viewer::watch` in
# perf/src/viewer.rs, not the dashboard -- see the parity note in
# perf/src/scene.rs's `Workload::redraws`. `viewer::watch` takes the terminal
# directly and refuses to run under its 80x26 minimum, which a bare
# `docker run -t` does not clear on its own: nothing is attached to the pty's
# other end to send it a window-change ioctl, so it reads back as 0x0. The
# `stty rows 26 cols 80` below sets that size on the pty from inside the
# container before roog-perf starts, which is the concrete reason a shell is
# not optional here (see check_has_shell above) -- this is what it is for.
#
# `--frames` bounds the loop the same way it bounds the headless gate (see the
# frame-count check `viewer::run` gained in perf/src/main.rs), which is what
# lets this run unattended in a detached, keyboard-less container instead of
# sitting there until someone presses `q`.
#
# There is nothing to parse out of this run -- the viewer has no textual
# report -- so it feeds nothing into results.tsv/roog-compat. All that matters
# is the exit code: 0 means the row drew its own screen without crossterm or
# the viewer falling over, which a headless-only pipeline would never prove.
# By default this only ever runs for a native row -- see "why emulated rows
# are build-only" at the top of this file.
run_screen_check() {
  local id=$1 target=$2 platform=$3 image=$4 exec_kind=$5 cpus=$6 memory=$7
  local name="roog-compat-$id-screen"
  local log="$OUT/run-$id-screen.log"
  local bin
  bin=$(target_bin "$target" "$RIG" release)

  if [ ! -x "$bin" ]; then
    warn "$id/screen: no binary at $bin"
    return 1
  fi

  docker rm -f "$name" >/dev/null 2>&1

  local mounts=(-v "$bin:/roog-perf:ro")
  local cmd=(/roog-perf --workload both --load game --frames "$SCREEN_FRAMES" --fps "$FPS")

  local docker_platform=(--platform "$platform")
  if [ "$exec_kind" = "qemu" ]; then
    local handler qemu_bin
    handler=$(qemu_handler_for "$platform")
    qemu_bin=$(ensure_qemu_interpreter "$handler") || {
      warn "$id/screen: could not fetch $handler from $QEMU_IMAGE"
      return 1
    }
    docker_platform=()
    mounts+=(-v "$qemu_bin:/qemu-static:ro")
    cmd=(/qemu-static "${cmd[@]}")
  fi

  # `-t` allocates the pty but does not size it: with nothing attached to the
  # other end to send a window-change ioctl, an unresized pty reads back as
  # 0x0, and `--workload both` draws through perf/src/viewer.rs, which refuses
  # to run below its 80x26 minimum -- correctly, since a real Pi's actual
  # terminal has a real size and a silent 0x0 run would prove nothing. `stty`
  # sets that size on the pty from inside the container, which is exactly why
  # a shell is not optional here either (see check_has_shell above): this is
  # what it is for. 80x26 mirrors `SCREEN_W`/`SCREEN_H + 1` in
  # perf/src/screen.rs and perf/src/viewer.rs; if those move, this must too.
  cmd=(/bin/sh -c 'stty rows 26 cols 80 2>/dev/null; exec "$@"' sh "${cmd[@]}")

  if ! docker run -d -t --name "$name" \
        "${docker_platform[@]}" \
        --cpus "$cpus" --memory "$memory" --memory-swap "$memory" \
        "${mounts[@]}" \
        "$image" "${cmd[@]}" >/dev/null 2>"$log"; then
    bad "screen: container would not start"
    tail -3 "$log" | sed 's/^/      /'
    FAILED="${FAILED}$id/screen "
    return 1
  fi

  local exit_code
  exit_code=$(timeout "$TIMEOUT" docker wait "$name" 2>/dev/null)
  if [ -z "$exit_code" ]; then
    docker kill "$name" >/dev/null 2>&1
  fi
  docker logs "$name" > "$log" 2>&1
  docker rm -f "$name" >/dev/null 2>&1

  if [ -z "$exit_code" ]; then
    bad "screen: still going after ${TIMEOUT}s -- the viewer never reached $SCREEN_FRAMES frames"
    FAILED="${FAILED}$id/screen "
    return 1
  fi
  if [ "$exit_code" != "0" ]; then
    bad "screen: exit $exit_code; see $log"
    tail -6 "$log" | sed 's/^/      /'
    FAILED="${FAILED}$id/screen "
    return 1
  fi
  ok "screen: rendered $SCREEN_FRAMES frames under a real, sized pty"
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

  # A shell is not optional here (see matrix.tsv's house rule): a `linux` row
  # is a claim that roog runs there, and it cannot even start without a real
  # terminal under it. Checked per run rather than trusted from the `class`
  # column, because "alpine has a shell" is true until someone points a row
  # at a distroless or scratch image and finds out the hard way mid-run. This
  # needs no interpreter and no qemu, so it runs even for a row execution is
  # about to skip -- an unexecuted row is still a claim, and this is the part
  # of that claim that costs nothing to check.
  if ! check_has_shell "$image"; then
    bad "$id: $image has no usable shell -- roog cannot run without one"
    record "$id" "$target" game skipped 0 0 0 0 0 0 0 0 0
    SKIPPED="$SKIPPED$id "
    continue
  fi

  if [ "$exec" = "qemu" ] && [ "$EXEC_EMULATED" -eq 0 ]; then
    ok "builds: cross-compiled and statically linked; not run here by default"
    note "  roog's own frame loop asks far less of the machine than rustc just did"
    note "  --exec-emulated runs it for real, if you have hardware or a qemu build to trust"
    record "$id" "$target" game build-only 0 0 0 0 0 0 0 0 0
    BUILD_ONLY="$BUILD_ONLY$id "
    continue
  fi

  if ! have_qemu_for "$platform"; then
    bad "$id needs $(qemu_handler_for "$platform") and could not fetch it from $QEMU_IMAGE"
    note "  is docker able to pull images right now?"
    record "$id" "$target" game skipped 0 0 0 0 0 0 0 0 0
    SKIPPED="$SKIPPED$id "
    continue
  fi

  run_row "$id" "$target" "$platform" "$image" "$exec" "$cpus" "$memory" game "$FRAMES"
  [ "$WITH_REEL" -eq 1 ] && \
    run_row "$id" "$target" "$platform" "$image" "$exec" "$cpus" "$memory" reel "$REEL_FRAMES"
  [ "$WITH_SCREEN" -eq 1 ] && \
    run_screen_check "$id" "$target" "$platform" "$image" "$exec" "$cpus" "$memory"
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
# Always asked to build, never gated on whether a binary is already sitting
# there: `-x` only proves *a* roog-compat exists, not that it was built after
# the last change to matrix.tsv or the verdict/results/report source. A stale
# reporter reading a fresh results.tsv is worse than a slow one -- it silently
# mis-grades rows the current code would have graded right, and every symptom
# points at the row rather than at the binary. `cargo build` is the one thing
# that actually knows whether a rebuild is needed, so let it decide; it is
# fast and does nothing when nothing changed.
note "building roog-compat..."
cargo build --release -p roog-compat >/dev/null 2>&1
if [ -x "$REPORT_BIN" ]; then
  "$REPORT_BIN" report --dir "$OUT"
else
  warn "could not build roog-compat; the raw numbers are in $RESULTS"
fi

printf '      artifacts in %s/\n' "$OUT"
[ -n "$BUILD_ONLY" ] && note "builds, not run by default (--exec-emulated runs them): $BUILD_ONLY"
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
