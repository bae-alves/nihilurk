#!/usr/bin/env bash
#
# Shared plumbing for the compat pipeline: the presentation vocabulary, the
# matrix reader, and the "where did that tool go" lookups.
#
# Sourced, never executed. Every script in compat/ starts with:
#
#     . "$(dirname "$0")/lib.sh"
#

# ---------------------------------------------------------------------------
# Presentation
# ---------------------------------------------------------------------------

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  B=$'\033[1m'; DIM=$'\033[2m'; R=$'\033[0m'
  GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'; BLUE=$'\033[34m'
else
  B=""; DIM=""; R=""; GREEN=""; YELLOW=""; RED=""; BLUE=""
fi

STAGE=${STAGE:-0}
SKIPPED=${SKIPPED:-}
FAILED=${FAILED:-}

stage() { STAGE=$((STAGE+1)); printf '\n%s== %d. %s ==%s\n' "$B$BLUE" "$STAGE" "$1" "$R"; }

# An unnumbered heading, for a script that is not a stage in its own sequence
# -- compat_test.sh's closing verdict, which follows three sub-scripts that
# each numbered their own stages from 1.
heading() { printf '\n%s-- %s --%s\n' "$B$BLUE" "$1" "$R"; }

# Set by compat_test.sh, and by nothing else. Each phase script signs off with
# what to run next, which is the right thing to print when someone ran that
# script by hand and pure noise when the driver is about to do it for them.
driven() { [ -n "${COMPAT_DRIVEN:-}" ]; }
ok()    { printf '%s   ok%s  %s\n' "$GREEN" "$R" "$1"; }
warn()  { printf '%s   ..%s  %s\n' "$YELLOW" "$R" "$1"; }
bad()   { printf '%s   !!%s  %s\n' "$RED" "$R" "$1"; }
note()  { printf '%s      %s%s\n' "$DIM" "$1" "$R"; }
skip()  { SKIPPED="$SKIPPED$1 "; warn "skipped: $2"; }
have()  { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------------------
# Where things are
# ---------------------------------------------------------------------------

# Every path in the pipeline is relative to the workspace root, so a script can
# be started from anywhere.
COMPAT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(dirname "$COMPAT_DIR")
MATRIX="$COMPAT_DIR/matrix.tsv"
OUT="$ROOT/target/compat"
export CROSS_CONFIG="$COMPAT_DIR/Cross.toml"

# What the containers run: the game itself, built to have its size measured,
# to prove the target links, and to prove it starts. `GAME_PKG` is the cargo
# package to build (`-p`); `GAME` is the binary that package produces, which
# is not the same name since `engine`'s `[[bin]]` renamed its output.
GAME_PKG=engine
GAME=nihilurk

# ---------------------------------------------------------------------------
# Tools that hide
# ---------------------------------------------------------------------------

# Cargo-installed binaries live in ~/.cargo/bin, which cargo itself searches and
# which is frequently not on PATH in a non-login shell -- so `command -v cross`
# reports "missing" for a tool that is sitting right there. `cross` is a plain
# binary, so look for it directly instead.
cross_bin() {
  if have cross; then echo cross; return 0; fi
  for candidate in "${CARGO_HOME:-$HOME/.cargo}/bin/cross" "$HOME/.cargo/bin/cross"; do
    [ -x "$candidate" ] && { echo "$candidate"; return 0; }
  done
  return 1
}

# llvm-nm reads every architecture rustc can emit, which is the whole reason the
# blame table works on a foreign binary. rustup ships it as a component; the
# system LLVM will do; GNU nm is the last resort and often refuses a foreign
# ELF, which the caller checks for rather than assuming.
nm_bin() {
  local sysroot
  sysroot=$(rustc --print sysroot 2>/dev/null)
  for candidate in \
      "$sysroot/lib/rustlib/$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')/bin/llvm-nm" \
      llvm-nm nm; do
    [ -n "$candidate" ] && command -v "$candidate" >/dev/null 2>&1 && { echo "$candidate"; return 0; }
    [ -x "$candidate" ] && { echo "$candidate"; return 0; }
  done
  return 1
}

# ---------------------------------------------------------------------------
# The matrix
# ---------------------------------------------------------------------------

# Print the matrix rows, tab-separated, honouring a `--targets a,b` filter in
# $ONLY. Comments and blank lines are dropped here so no caller has to think
# about them.
#
# $1, when given, filters by class (`linux` or `bare`).
matrix_rows() {
  local want_class="${1:-}"
  awk -F'\t' -v want="$want_class" -v only="${ONLY:-}" '
    /^[[:space:]]*#/ || NF < 7 { next }
    want != "" && $3 != want   { next }
    only != "" {
      found = 0
      n = split(only, wanted, ",")
      for (i = 1; i <= n; i++) if (wanted[i] == $1) found = 1
      if (!found) next
    }
    { print }
  ' "$MATRIX"
}

# Fail loudly rather than silently measuring nothing: a typo in --targets that
# matched no row would otherwise print an empty report and exit 0.
matrix_rows_or_die() {
  local rows
  rows=$(matrix_rows "$@")
  if [ -z "$rows" ]; then
    # Every caller feeds this straight into `while read` via process
    # substitution, so anything printed to stdout here becomes a fake row
    # instead of a diagnostic. Both lines go to stderr for that reason.
    bad "no matrix rows matched${ONLY:+ --targets $ONLY}" >&2
    note "rows available: $(matrix_rows | cut -f1 | tr '\n' ' ')" >&2
    exit 1
  fi
  printf '%s\n' "$rows"
}

# ---------------------------------------------------------------------------
# Where the cross builds live
# ---------------------------------------------------------------------------
#
# Each target triple gets its own `CARGO_TARGET_DIR`, and none of them is the
# workspace's own `target/`. That is not tidiness -- it is the fix for a real
# failure.
#
# A cross build compiles two kinds of thing: the crate, for the target, and
# every dependency's build script, for the *host*. Target artifacts are
# already namespaced by triple (`target/<triple>/`), but host build scripts
# all land in one shared `target/release/build/`. So they get reused across
# builds that must not share them:
#
#   cargo build --release       # host: Arch, glibc 2.42
#   ./compat_test.sh            # i686 cross image: Ubuntu 16.04, glibc 2.23
#
# The second run finds the first run's build-script binaries already built,
# runs them inside the old container, and they die on
# `weak version GLIBC_2.29 not found` -- an error about a dependency's build
# script that says nothing about nihilurk and takes a while to trace. Two cross
# images of different vintages collide the same way.
#
# One directory per triple means no two toolchains ever share a host artifact.
# It costs disk and a first-build recompile per target, and buys a pipeline
# whose result does not depend on what was built before it.
cross_target_dir() {
  echo "target/cross/$1"   # $1 target triple; relative, so `cross` can mount it
}

# Where a built binary for a target lands. `cross` and `cargo` agree on this
# layout, which is what lets `run_check.sh` find binaries without caring which
# of the two built them. The triple appears twice because cargo puts
# cross-compiled output in `$CARGO_TARGET_DIR/<triple>/`, and the outer one is
# ours.
target_bin() {
  echo "$ROOT/$(cross_target_dir "$1")/$1/$3/$2"   # $1 triple, $2 binary, $3 profile
}

# A row's claim to have a shell, checked rather than assumed. Used two ways:
# nostd_check.sh's bare rows use it to say whether `--shell <id>` will work;
# run_check.sh's linux rows use it as a hard gate, because a shell is not a
# nicety there -- nihilurk needs a real terminal under it, an image with no
# shell can't give it one, and a row that can't run the game has no business
# being in "does nihilurk run on these machines".
check_has_shell() {
  local image=$1
  docker run --rm "$image" /bin/sh -c 'echo shell-ok' 2>/dev/null | grep -q shell-ok
}

human_bytes() {
  awk -v b="${1:-0}" 'BEGIN {
    split("B KiB MiB GiB", u, " ")
    i = 1
    while (b >= 1024 && i < 4) { b /= 1024; i++ }
    printf (i == 1 ? "%d %s\n" : "%.1f %s\n"), b, u[i]
  }'
}

# ---------------------------------------------------------------------------
# Who is to blame for the binary
# ---------------------------------------------------------------------------
#
# Turn `llvm-nm --print-size` output into bytes per crate.
#
# `cargo bloat` is the better tool and cannot be used here: pointing it at a
# foreign target means having that target's linker on the host, which is the
# thing cross-compiling in a container exists to avoid. Symbol sizes are the
# portable substitute -- llvm-nm reads every architecture rustc emits -- and
# they answer the question the stage is actually asking, which is "what got
# bigger".
#
# What it reports is *symbol* bytes, not file bytes. The two differ by section
# headers, alignment padding, relocation and the constant pool, which is why the
# table's total comes in under the size printed beside it. Read the shares, not
# the absolute totals.
#
# Symbol names are decoded rather than demangled: rustc's legacy mangling is
# `_ZN<len><crate><len><module>...`, so the crate is the first length-prefixed
# component, and that is one `substr` rather than a dependency on rustfilt. The
# v0 scheme puts it after `Cs<disambiguator>_` instead; both are handled.
#
#   $1  the nm output file
#   $2  how many crates to list (default 10)
blame_table() {
  awk -v top="${2:-10}" '
    function crate_of(sym,   rest, len, name, p, q) {
      # Legacy mangling: _ZN + length-prefixed path components.
      if (sym ~ /^_ZN/) {
        rest = substr(sym, 4)
        if (match(rest, /^[0-9]+/) == 0) return "other"
        len = substr(rest, RSTART, RLENGTH) + 0
        rest = substr(rest, RLENGTH + 1)
        name = substr(rest, 1, len)
        # An inherent or trait impl mangles as `44_$LT$alloc..vec..Vec$LT$...`,
        # where the first component is the whole type. The crate is the first
        # path segment inside it.
        p = index(name, "$LT$")
        if (p > 0) {
          name = substr(name, p + 4)
          q = index(name, "..")
          if (q > 0) name = substr(name, 1, q - 1)
        }
        sub(/^_+/, "", name)
        return name == "" ? "other" : name
      }
      # v0 mangling: the crate follows the `Cs<disambiguator>_` marker.
      if (sym ~ /^_R/) {
        if (match(sym, /Cs[0-9A-Za-z]+_/) == 0) return "rust (v0)"
        rest = substr(sym, RSTART + RLENGTH)
        if (match(rest, /^[0-9]+/) == 0) return "rust (v0)"
        len = substr(rest, RSTART, RLENGTH) + 0
        return substr(rest, RLENGTH + 1, len)
      }
      # Everything else is C: musl, and the compiler builtins rustc emits calls
      # to. Worth its own bucket rather than a name apiece -- on a static musl
      # build it is a real share of the binary and there is nothing to do about
      # it.
      return "libc + compiler builtins"
    }
    # nm prints `<addr> <size> <type> <name>`, and omits the size for undefined
    # or absolute symbols -- those lines are shorter and are skipped.
    NF >= 4 && $2 ~ /^[0-9]+$/ {
      size = $2 + 0
      by[crate_of($4)] += size
      total += size
      n++
    }
    END {
      if (total == 0) {
        print "  no sized symbols -- the binary is stripped, or nm read it wrong"
        exit
      }
      printf "  %-34s %10s %8s\n", "crate", "bytes", "share"
      # Selection sort: a binary has a few dozen crates in it, and this keeps
      # the function working on any awk rather than only gawk.
      for (c in by) names[++m] = c
      for (i = 1; i <= m && i <= top; i++) {
        best = i
        for (j = i + 1; j <= m; j++) if (by[names[j]] > by[names[best]]) best = j
        tmp = names[i]; names[i] = names[best]; names[best] = tmp
        printf "  %-34s %10d %7.1f%%\n", names[i], by[names[i]], by[names[i]] / total * 100
        listed += by[names[i]]
      }
      if (m > top) printf "  %-34s %10d %7.1f%%\n", "(" m - top " more)", total - listed, (total - listed) / total * 100
      printf "  %-34s %10d\n", "total in symbols", total
      printf "  %d symbols; file is larger by its headers, padding and constant pool\n", n
    }
  ' "$1"
}
