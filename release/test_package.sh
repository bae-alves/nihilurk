#!/usr/bin/env bash
#
# release/test_package.sh [TARGET] -- does release/package.sh make a tarball a
# stranger can unpack and play?
#
# It builds for real (four languages, fat LTO), so it takes a few minutes. The
# checks are the ones a release breaks on:
#
#   1. the tarball holds exactly the files the installer expects
#   2. `nihilurk` finds a sibling for every language, so the dispatcher works
#   3. the four language binaries differ, so the feature loop really switched
#   4. a musl build has no dynamic loader, which is the whole point of musl
#
# TARGET defaults to this machine's own triple. Pass the musl one to check 4.
# A Windows target names its files `*.exe`; nothing else here changes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"

EXE=""
case "$TARGET" in *-windows-*) EXE=".exe" ;; esac

# macOS has `shasum`, not `sha256sum`; Git Bash on Windows may have either.
sha_check() { if command -v sha256sum >/dev/null; then sha256sum -c "$1"; else shasum -a 256 -c "$1"; fi; }

fail() { printf 'FAIL  %s\n' "$*" >&2; exit 1; }
ok()   { printf 'ok    %s\n' "$*"; }

release/package.sh "$TARGET" >/dev/null

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1)"
NAME="nihilurk-$VERSION-$TARGET"
TARBALL="dist/$NAME.tar.gz"
[ -f "$TARBALL" ] || fail "no $TARBALL"

# 1. contents
want="$(printf '%s\n' \
  "$NAME/" "$NAME/LICENSE" "$NAME/MANUAL.md" "$NAME/nihilurk$EXE" \
  "$NAME/nihilurk-en$EXE" "$NAME/nihilurk-es$EXE" "$NAME/nihilurk-ht$EXE" \
  "$NAME/nihilurk-pt$EXE" "$NAME/nihilurk.6" | sort)"
got="$(tar -tzf "$TARBALL" | sort)"
[ "$want" = "$got" ] || fail "tarball contents differ
--- want
$want
--- got
$got"
ok "tarball holds exactly the expected files"

( cd dist && sha_check "$NAME.tar.gz.sha256" >/dev/null ) \
  || fail "the .sha256 file does not match the tarball"
ok "checksum file matches"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
tar -xzf "$TARBALL" -C "$tmp"
dir="$tmp/$NAME"

# 2. dispatcher reaches every language
for lang in en pt es ht; do
  LC_ALL="${lang}_XX.UTF-8" "$dir/nihilurk$EXE" -content >/dev/null 2>&1 \
    || fail "nihilurk with LC_ALL=${lang}_XX could not run its sibling"
done
ok "the dispatcher runs all four languages"

# 3. the loop switched features between builds
for lang in pt es ht; do
  cmp -s "$dir/nihilurk-en$EXE" "$dir/nihilurk-$lang$EXE" \
    && fail "nihilurk-$lang is byte-identical to nihilurk-en"
done
ok "the language binaries differ from English"

# 4. musl means static
case "$TARGET" in
  *-linux-musl*)
    for f in nihilurk nihilurk-en nihilurk-pt nihilurk-es nihilurk-ht; do
      readelf -lW "$dir/$f" | grep -q INTERP && fail "$f has a dynamic loader"
    done
    ok "no dynamic loader in a musl build" ;;
  *) printf 'skip  static check (not a musl target)\n' ;;
esac
