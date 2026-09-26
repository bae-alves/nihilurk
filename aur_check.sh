#!/usr/bin/env bash
#
# aur_check.sh -- does aur/PKGBUILD actually build what it claims to.
#
# Sources aur/PKGBUILD itself, rather than reimplementing its build(),
# check() and package() -- one source of truth for what the packaging step
# does. Those functions get run against two trees:
#
#   tag     the tree a real `makepkg` downloads today: the git tag that
#           PKGBUILD's own pkgver names, checked out via `git worktree`
#           (skips the GitHub round-trip; sha256sums is checked separately,
#           below, and only if the network is up).
#   head    the tree a release cut right now would tag: this working copy,
#           uncommitted changes included.
#
# `tag` is what catches the real failure this script exists for: PKGBUILD
# changes ahead of the release it still points at, so `build()` reaches for
# a feature or a file the pinned tag doesn't have. `head` catches the
# opposite drift: a packaging-affecting change with no matching PKGBUILD
# update yet.
#
# Usage:
#   ./aur_check.sh          both trees
#   ./aur_check.sh --tag    only the pinned tag (what AUR ships right now)
#   ./aur_check.sh --head   only this working copy (pre-release gate)
#   ./aur_check.sh --help

set -uo pipefail
cd "$(dirname "$0")" || exit 1
ROOT=$(pwd)

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  B=$'\033[1m'; R=$'\033[0m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'
else
  B=""; R=""; GREEN=""; YELLOW=""; RED=""
fi
ok()   { printf '%s   ok%s  %s\n' "$GREEN" "$R" "$1"; }
warn() { printf '%s   ..%s  %s\n' "$YELLOW" "$R" "$1"; }
bad()  { printf '%s   !!%s  %s\n' "$RED" "$R" "$1"; }

DO_TAG=1
DO_HEAD=1
case "${1:-}" in
  --tag)  DO_HEAD=0 ;;
  --head) DO_TAG=0 ;;
  --help|-h) sed -n '3,/^set -/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
  "") ;;
  *) echo "aur_check.sh: unknown option $1 (try --help)" >&2; exit 1 ;;
esac

# Only variable and function definitions run here -- `source=()` is a plain
# array literal, so this never touches the network.
. aur/PKGBUILD

STATUS=0

# $1 label, $2 directory standing in for the extracted tarball. prepare(),
# build(), check() and package() each `cd "$pkgname-$pkgver"` on their own
# first line -- real makepkg runs every function in its own subshell so that
# `cd` never leaks into the next one, so each call here gets the same
# treatment rather than reusing whatever directory the previous function left
# the shell in.
run_stage() {
  local label=$1 dir=$2
  echo
  echo "${B}-- $label --$R"
  (
    cd "$(dirname "$dir")" || exit 1
    export CARGO_TARGET_DIR="$ROOT/target"
    pkgdir=$(mktemp -d)
    (prepare) && (build) && (check) && (package)
  )
  if [ $? -eq 0 ]; then
    ok "$label: build(), check() and package() pass"
  else
    bad "$label: failed -- see output above"
    STATUS=1
  fi
}

if [ "$DO_TAG" -eq 1 ]; then
  TAG="v$pkgver"
  if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    WORKTREE=$(mktemp -d)/"$pkgname-$pkgver"
    if git worktree add --detach "$WORKTREE" "$TAG" >/dev/null 2>&1; then
      run_stage "pinned tag $TAG (what AUR ships now)" "$WORKTREE"
      git worktree remove --force "$WORKTREE" 2>/dev/null
    else
      bad "couldn't check out $TAG into a worktree"
      STATUS=1
    fi
  else
    bad "PKGBUILD pins pkgver=$pkgver but tag $TAG doesn't exist"
    STATUS=1
  fi

  url=${source[0]#*::}
  want=${sha256sums[0]}
  got=$(curl -fsSL --max-time 20 "$url" 2>/dev/null | sha256sum | awk '{print $1}')
  if [ -z "$got" ]; then
    warn "no network (or fetch failed): skipped sha256sums check against $url"
  elif [ "$got" = "$want" ]; then
    ok "sha256sums matches the tarball at $url"
  else
    bad "sha256sums stale: PKGBUILD says $want, tarball is $got"
    STATUS=1
  fi
fi

if [ "$DO_HEAD" -eq 1 ]; then
  SCRATCH=$(mktemp -d)
  ln -s "$ROOT" "$SCRATCH/$pkgname-$pkgver"
  run_stage "working copy (pre-release gate)" "$SCRATCH/$pkgname-$pkgver"
fi

if [ "$STATUS" -ne 0 ]; then
  echo
  bad "aur/PKGBUILD is not ready to ship"
  exit 1
fi
echo
ok "aur/PKGBUILD is ready to ship"
