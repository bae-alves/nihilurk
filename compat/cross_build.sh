#!/usr/bin/env bash
#
# Phase 1 of the compat pipeline: build nihilurk for every machine it claims to run
# on, then say how big it came out and who is to blame for that.
#
# One container per target, driven by cross-rs, reading the matrix in
# matrix.tsv. Two binaries per row:
#
#   engine      the game. Built `--release`, which is the profile that ships:
#               fat LTO, one codegen unit, panic=abort, symbols stripped. This
#               is the artifact whose size is reported.
#   nihilurk-perf   the stress rig, for `stress_test_matrix.sh` to run inside the
#               emulated container in phase 2.
#
# and, unless `--no-blame`, a third build of `engine` on the `profiling`
# profile -- byte-for-byte the same machine code with the symbols left in, which
# is the only way to ask a stripped binary what is inside it. Same trick
# perf_test.sh uses in its footprint stage, for the same reason.
#
# Usage:
#   ./compat/cross_build.sh                    every Linux row of the matrix
#   ./compat/cross_build.sh --targets pi-zero  just that row (comma-separated)
#   ./compat/cross_build.sh --no-blame         skip the symbol attribution
#   ./compat/cross_build.sh --help

set -uo pipefail

. "$(dirname "$0")/lib.sh"

ONLY=""
BLAME=1
TOP=10

while [ $# -gt 0 ]; do
  case "$1" in
    --targets)  ONLY="${2:?--targets needs a list}"; shift ;;
    --no-blame) BLAME=0 ;;
    --top)      TOP="${2:?--top needs a number}"; shift ;;
    --help|-h)  sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "cross_build.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

mkdir -p "$OUT"
cd "$ROOT" || exit 1

printf '%s\n' "$B  nihilurk cross-compilation matrix$R"
note "$(rustc -V 2>/dev/null || echo 'rustc: not found')"

CROSS=$(cross_bin) || CROSS=""
if [ -z "$CROSS" ]; then
  bad "cross not found (cargo install cross), and no fallback can produce a"
  bad "foreign binary on its own"
  note "the host's own musl triple may still build with plain cargo:"
  note "  cargo build --release --target x86_64-unknown-linux-musl -p engine"
  exit 1
fi
note "$("$CROSS" --version 2>/dev/null | head -1)"
if ! docker info >/dev/null 2>&1; then
  bad "docker is not answering; cross needs it to hold the toolchains"
  note "  sudo systemctl start docker    (and: usermod -aG docker \$USER)"
  exit 1
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

BUILT=""
FOOTPRINT="$OUT/footprint.txt"
: > "$FOOTPRINT"

while IFS=$'\t' read -r id target class platform image exec cpus memory note_text; do
  stage "$id  ($target)"
  note "$note_text"

  log="$OUT/build-$id.log"
  failed=0
  # Per-triple, and never the workspace's own target/ -- see cross_target_dir
  # in lib.sh for the glibc collision this avoids. Relative on purpose: cargo
  # resolves it against the working directory, which inside the container is
  # the mounted workspace, so the path means the same thing on both sides.
  export CARGO_TARGET_DIR
  CARGO_TARGET_DIR=$(cross_target_dir "$target")
  for pkg in "$GAME" "$RIG"; do
    printf '%s      building %-10s %s' "$DIM" "$pkg" "$R"
    if "$CROSS" build --release --target "$target" -p "$pkg" >>"$log" 2>&1; then
      printf '%sok%s\n' "$GREEN" "$R"
      continue
    fi
    printf '%sfailed%s\n' "$RED" "$R"
    failed=1
  done

  if [ "$failed" -eq 1 ]; then
    bad "$id did not build; see $log"
    tail -12 "$log" | sed 's/^/      /'
    FAILED="${FAILED}$id "
    continue
  fi

  bin=$(target_bin "$target" "$GAME" release)
  rig=$(target_bin "$target" "$RIG" release)
  size=$(stat -c %s "$bin" 2>/dev/null || echo 0)
  rig_size=$(stat -c %s "$rig" 2>/dev/null || echo 0)
  ok "$(human_bytes "$size") $GAME, $(human_bytes "$rig_size") $RIG"

  # A static binary is the whole point of the musl rows: one file, no
  # `INTERP` segment, no runtime dependency on a libc the target may not have.
  # Anything else is a portability bug that would only show up as a failure to
  # start on the actual hardware, which is the worst place to find it.
  kind=$(file -b "$bin" 2>/dev/null | cut -c1-72)
  case "$kind" in
    *"statically linked"*) note "static: $kind" ;;
    *"dynamically linked"*)
      bad "$id is dynamically linked -- it will not start on a machine with a"
      bad "different libc. Check the triple is a -musl one."
      FAILED="${FAILED}$id "
      ;;
    *) note "${kind:-unrecognised ELF}" ;;
  esac

  printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$target" "$size" "$rig_size" "$note_text" >> "$FOOTPRINT"
  BUILT="$BUILT$id "
done < <(matrix_rows_or_die linux)

# ---------------------------------------------------------------------------
# Footprint
# ---------------------------------------------------------------------------

stage "Footprint"
if [ ! -s "$FOOTPRINT" ]; then
  bad "nothing built"
  exit 1
fi
# Exact bytes beside the rounded figure, because this table exists to be
# compared -- across targets and against the last run -- and rounding to one
# decimal place hides the comparison. The 32-bit rows come out about 4% smaller
# than the 64-bit ones, which is 84 KiB and reads as "1.8 MiB" either way.
printf '      %-10s %-32s %10s %10s %10s\n' "target" "triple" "game" "bytes" "rig"
while IFS=$'\t' read -r id target size rig_size _; do
  printf '      %-10s %-32s %10s %10d %10s\n' \
    "$id" "$target" "$(human_bytes "$size")" "$size" "$(human_bytes "$rig_size")"
done < "$FOOTPRINT"
note "game = engine, --release: fat LTO, one codegen unit, symbols stripped"
note "the arm/i686 rows are smaller because a 32-bit pointer is smaller"

# ---------------------------------------------------------------------------
# Blame
# ---------------------------------------------------------------------------
# A stripped binary cannot be asked what is inside it, so the attribution is
# read off the `profiling` profile: the same optimisation settings, the same
# machine code, symbols left in. `cargo bloat` does this for the host and is
# excellent; it cannot be pointed at a foreign target without the linker for
# it, which is exactly the situation here. llvm-nm can read every architecture
# rustc emits, so the blame table is built from symbol sizes instead.

stage "Blame"
if [ "$BLAME" -eq 0 ]; then
  skip blame "--no-blame"
else
  NM=$(nm_bin) || NM=""
  if [ -z "$NM" ]; then
    skip blame "no llvm-nm or nm (rustup component add llvm-tools)"
  else
    note "using $NM"
    for id in $BUILT; do
      target=$(matrix_rows linux | awk -F'\t' -v i="$id" '$1 == i {print $2}')
      log="$OUT/build-$id.log"
      CARGO_TARGET_DIR=$(cross_target_dir "$target")
      printf '%s      %-10s symbols... %s' "$DIM" "$id" "$R"
      if ! "$CROSS" build --profile profiling --target "$target" -p "$GAME" >>"$log" 2>&1; then
        printf '%sfailed%s\n' "$RED" "$R"
        warn "$id: profiling build failed; see $log"
        continue
      fi
      unstripped=$(target_bin "$target" "$GAME" profiling)
      if ! "$NM" --print-size --size-sort --radix=d "$unstripped" > "$OUT/syms-$id.txt" 2>/dev/null; then
        printf '%sunreadable%s\n' "$YELLOW" "$R"
        warn "$id: $NM cannot read a $target binary; llvm-nm can, GNU nm often cannot"
        continue
      fi
      printf '%sok%s\n' "$GREEN" "$R"
      blame_table "$OUT/syms-$id.txt" "$TOP" > "$OUT/blame-$id.txt"
      printf '\n%s      %s -- what is in the binary%s\n' "$B" "$id" "$R"
      sed 's/^/      /' "$OUT/blame-$id.txt"
    done
  fi
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

stage "Summary"
printf '      built: %s\n' "${BUILT:-nothing}"
printf '      artifacts in %s/\n' "$OUT"
[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"
if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi
driven || printf '\n%s  next:%s ./compat/stress_test_matrix.sh\n\n' "$GREEN$B" "$R"
