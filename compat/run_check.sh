#!/usr/bin/env bash
#
# Phase 2 of the compat pipeline: does the machine have a shell, and does the
# binary `cross_build.sh` built for it actually start there.
#
# nihilurk's compat strategy is exactly this claim: if it builds, the image has
# a real shell, and the target is std (every `linux` row is), nihilurk can run
# there. There is no stress test here, and no CPU or memory cap standing in
# for slow hardware -- nihilurk has no workload that asks a machine to be
# fast. Every feel-layer effect is bounded, and cross-compiling a full Rust
# toolchain (which `cross_build.sh` just proved this target can do) asks more
# of a machine than nihilurk's own frame loop ever will. See
# docs/explanation/cross-platform-testing.md.
#
# The check itself is `engine -content`: it prints the content index and
# exits, without ever touching the terminal's alternate screen (see the
# comment above `list_content` in engine/src/main.rs). That is what lets a
# `qemu` row run for real, unlike a full game session -- there is no pty to
# size and no terminal ioctl for qemu-user to mistranslate, only a shell to
# launch the binary from and stdout to read back. If it prints something and
# exits 0, the row can run nihilurk.
#
# Usage:
#   ./compat/run_check.sh                 every Linux row
#   ./compat/run_check.sh --targets pi-zero,potato
#   ./compat/run_check.sh --timeout 60    per-row seconds
#   ./compat/run_check.sh --help

set -uo pipefail

. "$(dirname "$0")/lib.sh"

ONLY=""
TIMEOUT=60

while [ $# -gt 0 ]; do
  case "$1" in
    --targets) ONLY="${2:?--targets needs a list}"; shift ;;
    --timeout) TIMEOUT="${2:?--timeout needs seconds}"; shift ;;
    --help|-h) sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "run_check.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

mkdir -p "$OUT"
cd "$ROOT" || exit 1

RESULTS="$OUT/results.tsv"

printf '%s\n' "$B  nihilurk compat check -- does nihilurk run on these machines?$R"
note "if it builds, has a shell, and the target is std, it can run nihilurk"

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
# the host CPU. Every binary this pipeline runs is static (cross_build.sh
# checks it), so a qemu-user interpreter never needs a foreign sysroot -- it
# only has to translate the guest's syscalls, which means it can be invoked
# directly as the container's own command, on a container built for the
# *host's* architecture: no `--platform`, no binfmt_misc, no `--privileged`.

QEMU_IMAGE=tonistiigi/binfmt
QEMU_DIR="$OUT/qemu"

# Which static interpreter a row needs, by docker platform. Empty for a row
# the host CPU runs itself.
qemu_handler_for() {
  case "$1" in
    linux/arm64)  echo qemu-aarch64 ;;
    linux/arm/v7) echo qemu-arm ;;
    *)            echo "" ;;
  esac
}

# Fetches one static interpreter out of $QEMU_IMAGE's filesystem and caches it
# in $QEMU_DIR, without ever running that image.
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

# ---------------------------------------------------------------------------
# One row
# ---------------------------------------------------------------------------

# One line per checked row. Columns are read by `nihilurk-compat`; see
# compat/src/results.rs, which is the other half of this contract.
record() {
  printf '%s\t%s\t%s\n' "$@" >> "$RESULTS"
}

{
  printf '# nihilurk compat results -- written by run_check.sh, read by nihilurk-compat\n'
  printf '#id\ttarget\tstatus\n'
} > "$RESULTS"

# Mirror the footprint table into the results directory in the layout the
# report reads. `cross_build.sh` writes it with the size already in column 3;
# copying rather than re-statting keeps one definition of "how big is it".
if [ -s "$OUT/footprint.txt" ]; then
  cp "$OUT/footprint.txt" "$OUT/footprint.tsv"
fi

while IFS=$'\t' read -r id target class platform image exec note_text; do
  stage "$id"
  note "$note_text"

  bin=$(target_bin "$target" "$GAME" release)
  if [ ! -x "$bin" ]; then
    warn "$id: no binary at $bin"
    note "  ./compat/cross_build.sh --targets $id"
    record "$id" "$target" skipped
    SKIPPED="$SKIPPED$id "
    continue
  fi

  # A `linux` row is a claim that nihilurk runs there, and it cannot even
  # start without a real terminal under it -- checked per run rather than
  # trusted from the `class` column, because "alpine has a shell" is true
  # until someone points a row at a distroless or scratch image and finds out
  # the hard way mid-run.
  if ! check_has_shell "$image"; then
    bad "$id: $image has no usable shell -- nihilurk cannot run without one"
    record "$id" "$target" no-shell
    FAILED="${FAILED}$id "
    continue
  fi

  mounts=(-v "$bin:/engine:ro")
  cmd=(/engine -content)
  docker_platform=(--platform "$platform")
  if [ "$exec" = "qemu" ]; then
    handler=$(qemu_handler_for "$platform")
    qemu_bin=$(ensure_qemu_interpreter "$handler") || {
      warn "$id: could not fetch $handler from $QEMU_IMAGE"
      record "$id" "$target" skipped
      SKIPPED="$SKIPPED$id "
      continue
    }
    docker_platform=()
    mounts+=(-v "$qemu_bin:/qemu-static:ro")
    cmd=(/qemu-static "${cmd[@]}")
  fi

  log="$OUT/run-$id.log"
  if timeout "$TIMEOUT" docker run --rm "${docker_platform[@]}" "${mounts[@]}" "$image" "${cmd[@]}" \
        >"$log" 2>&1; then
    if [ -s "$log" ]; then
      ok "runs: engine -content printed $(wc -l < "$log" | tr -d ' ') lines and exited 0"
      record "$id" "$target" ok
    else
      bad "$id: exited 0 but printed nothing -- see $log"
      record "$id" "$target" failed
      FAILED="${FAILED}$id "
    fi
  else
    bad "$id: did not run cleanly; see $log"
    tail -6 "$log" | sed 's/^/      /'
    record "$id" "$target" failed
    FAILED="${FAILED}$id "
  fi
done < <(matrix_rows_or_die linux)

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
#
# The grading lives in `nihilurk-compat`, not here.

stage "Report"
REPORT_BIN="$ROOT/target/release/nihilurk-compat"
note "building nihilurk-compat..."
cargo build --release -p nihilurk-compat >/dev/null 2>&1
if [ -x "$REPORT_BIN" ]; then
  "$REPORT_BIN" report --dir "$OUT"
else
  warn "could not build nihilurk-compat; the raw results are in $RESULTS"
fi

printf '      artifacts in %s/\n' "$OUT"
[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"

if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi
driven || printf '\n%s  done.%s  Read it: target/release/nihilurk-compat\n\n' "$GREEN$B" "$R"
