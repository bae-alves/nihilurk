#!/usr/bin/env bash
#
# The bare-metal half of phase 1: prove that roog's particle arithmetic
# compiles for a microcontroller.
#
# WHAT THIS DOES AND DOES NOT CLAIM
#
# It does not claim roog runs on an ESP32. It does not. roog draws with
# crossterm, crossterm drives a terminal, and a microcontroller has no terminal
# and no operating system to provide one. A bare-metal row that fails to
# produce a game is not a bug, because no game was ever being built.
#
# What is being proved is narrower and worth having: `particle-core` -- when a
# mote is visible, which keyframe is showing, which glyph a beam segment draws
# -- is `no_std`, allocates nothing, and depends on no crate at all, so it
# compiles for a target with no OS under it. That is the crate the game itself
# calls, not a copy of it, so the proof is about the shipping code. If the
# arithmetic is ever quietly given a `Vec`, a `String` or a libm call, this is
# the stage that goes red.
#
# The two targets:
#
#   riscv32imc-unknown-none-elf   ESP32-C3, and any RISC-V microcontroller.
#                                 rustup carries it, so the check is a
#                                 one-second `cargo check` on the host.
#   xtensa-esp32-none-elf         The original ESP32. Xtensa is not an LLVM
#                                 upstream target; it needs Espressif's rustc
#                                 fork, which rustup does not carry. That one
#                                 runs in a container.
#
# Both rows also name an SDK image with a shell in it, so you can go and poke
# at the toolchain by hand:
#
#   ./compat/nostd_check.sh --shell esp32
#
# Usage:
#   ./compat/nostd_check.sh                 check both bare rows
#   ./compat/nostd_check.sh --targets esp32c3
#   ./compat/nostd_check.sh --shell esp32   interactive shell in the SDK image
#   ./compat/nostd_check.sh --pull          fetch the SDK image ahead of time
#   ./compat/nostd_check.sh --help

set -uo pipefail

. "$(dirname "$0")/lib.sh"

ONLY=""
SHELL_ROW=""
PULL=0

while [ $# -gt 0 ]; do
  case "$1" in
    --targets) ONLY="${2:?--targets needs a list}"; shift ;;
    --shell)   SHELL_ROW="${2:?--shell needs a row id}"; shift ;;
    --pull)    PULL=1 ;;
    --help|-h) sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "nostd_check.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

cd "$ROOT" || exit 1
mkdir -p "$OUT"

# The crate under test. Deliberately the only one: `models` and `engine` are
# hosted crates and there is no version of this stage that involves them.
CORE=particle-core

# Where a container's cargo writes. Never the repo's own target/ -- the SDK
# images run as their own user, and a root-owned artifact left in target/ makes
# the next host build fail in a way that takes a while to understand.
CONTAINER_TARGET=/tmp/nostd-target

row_field() {
  matrix_rows bare | awk -F'\t' -v i="$1" -v f="$2" '$1 == i { print $f }'
}

# ---------------------------------------------------------------------------
# --shell: go and look at the toolchain
# ---------------------------------------------------------------------------

if [ -n "$SHELL_ROW" ]; then
  image=$(row_field "$SHELL_ROW" 5)
  target=$(row_field "$SHELL_ROW" 2)
  if [ -z "$image" ]; then
    bad "no bare-metal row called '$SHELL_ROW'"
    note "rows: $(matrix_rows bare | cut -f1 | tr '\n' ' ')"
    exit 1
  fi
  printf '%s\n' "$B  $SHELL_ROW -- $target$R"
  note "image: $image"
  note "the workspace is mounted at /work; cargo writes to $CONTAINER_TARGET"
  note "try: cargo check -p $CORE --target $target --no-default-features"
  # -it and a login shell: the ESP toolchain is put on PATH by the image's
  # profile scripts, and a non-login shell would not have it.
  exec docker run --rm -it \
    -v "$ROOT:/work" -w /work \
    -e CARGO_TARGET_DIR="$CONTAINER_TARGET" \
    "$image" /bin/bash -l
fi

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------

printf '%s\n' "$B  roog bare-metal check -- does the particle core compile with no OS?$R"
note "$(rustc -V 2>/dev/null || echo 'rustc: not found')"

# ---------------------------------------------------------------------------
# 1. The parity test
# ---------------------------------------------------------------------------
#
# This runs first because it is what makes the rest of the stage mean anything.
# `particle-core` has exactly one `#[cfg(feature = "std")]` in it -- `ceil`,
# which takes the intrinsic on a hosted build and a hand-rolled branch on a
# bare-metal one. If those two ever disagree, the bare-metal build is compiling
# different arithmetic than the game runs, and a green "it compiles" would be
# proving something about code nobody plays.

stage "Parity between the hosted and bare-metal branches"
if cargo test -p "$CORE" --all-features >"$OUT/nostd-parity.log" 2>&1; then
  ok "the std and no_std branches agree over the domain roog uses"
else
  bad "parity test failed -- the bare-metal build is not the game's arithmetic"
  tail -12 "$OUT/nostd-parity.log" | sed 's/^/      /'
  FAILED="${FAILED}parity "
fi

# ---------------------------------------------------------------------------
# 2. Each bare row
# ---------------------------------------------------------------------------

check_on_host() {
  local id=$1 target=$2
  if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
    note "adding $target..."
    if ! rustup target add "$target" >>"$OUT/nostd-$id.log" 2>&1; then
      skip "$id" "rustup could not add $target"
      return 1
    fi
  fi
  # --no-default-features is belt and braces: the crate is `no_std`
  # unconditionally and `std` is off by default, so this only guarantees that
  # a future change to the default feature set cannot quietly hosted-ify the
  # bare-metal check.
  cargo check -p "$CORE" --target "$target" --no-default-features \
    >>"$OUT/nostd-$id.log" 2>&1
}

check_in_container() {
  local id=$1 target=$2 image=$3
  if ! docker info >/dev/null 2>&1; then
    skip "$id" "docker is not answering, and rustup has no $target"
    return 1
  fi
  if [ "$PULL" -eq 1 ]; then
    note "pulling $image (this is a few GB)..."
    docker pull "$image" >>"$OUT/nostd-$id.log" 2>&1
  fi
  # A login shell, so the image's profile scripts put the esp toolchain on
  # PATH. `+esp` names Espressif's rustc fork, which is what knows Xtensa.
  #
  # `-Zbuild-std=core` is not optional here. Xtensa is a tier-3 target: the
  # toolchain knows how to emit code for it but ships no precompiled `core`,
  # so without this the check fails on `can't find crate for core` -- which
  # reads like `particle-core` needing something it should not, and is really
  # the standard library not being there to link against. The toolchain
  # carries `rust-src`, so cargo builds `core` from source instead. Nothing
  # about the crate under test changes; it is still `no_std`.
  #
  # The RISC-V row needs none of this: rustup ships a prebuilt `core` for it,
  # which is why that row is a one-second check on the host.
  docker run --rm \
    -v "$ROOT:/work" -w /work \
    -e CARGO_TARGET_DIR="$CONTAINER_TARGET" \
    "$image" /bin/bash -lc \
    "cargo +esp check -p $CORE --target $target --no-default-features -Zbuild-std=core" \
    >>"$OUT/nostd-$id.log" 2>&1
}

CHECKED=""
while IFS=$'\t' read -r id target class platform image exec cpus memory note_text; do
  stage "$id  ($target)"
  note "$note_text"
  : > "$OUT/nostd-$id.log"

  ran=1
  if rustup target list 2>/dev/null | grep -q "^$target"; then
    note "rustup carries this target; checking on the host"
    check_on_host "$id" "$target"
    ran=$?
  else
    note "not a rustup target -- Xtensa needs Espressif's fork; using $image"
    check_in_container "$id" "$target" "$image"
    ran=$?
  fi

  case "$ran" in
    0) ok "$CORE compiles for $target with no operating system under it"
       CHECKED="$CHECKED$id " ;;
    *) if [ -s "$OUT/nostd-$id.log" ]; then
         bad "$id did not compile; see $OUT/nostd-$id.log"
         tail -12 "$OUT/nostd-$id.log" | sed 's/^/      /'
         FAILED="${FAILED}$id "
       fi ;;
  esac

  # The shell image is a separate claim from the compile, and is reported
  # separately: a row can compile on the host and still have an unusable
  # image, and you want to know that before you need it.
  if docker info >/dev/null 2>&1; then
    if check_has_shell "$image"; then
      note "shell image ok: docker run --rm -it $image /bin/bash -l"
      note "  or: ./compat/nostd_check.sh --shell $id"
    else
      warn "$image has no usable shell (or is not pulled yet: --pull)"
    fi
  fi
done < <(matrix_rows_or_die bare)

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

stage "Summary"
printf '      checked: %s\n' "${CHECKED:-nothing}"
note "what this proves: the particle arithmetic needs no OS."
note "what it does not: roog itself does. crossterm wants a terminal, and"
note "a microcontroller has none. See docs/explanation/cross-platform-testing.md."
[ -n "$SKIPPED" ] && note "skipped: $SKIPPED"

if [ -n "$FAILED" ]; then
  printf '\n%s  FAILED:%s %s\n\n' "$RED$B" "$R" "$FAILED"
  exit 1
fi
printf '\n%s  ok.%s  the particle core is still bare-metal clean\n\n' "$GREEN$B" "$R"
