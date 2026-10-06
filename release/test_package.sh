#!/usr/bin/env bash
#
# release/test_package.sh [TARGET] -- does release/package.sh make an archive a
# stranger can unpack and play?
#
# It builds for real (four languages, fat LTO), so it takes a few minutes. The
# checks are the ones a release breaks on:
#
#   1. the archive holds exactly the files the installer expects
#   2. `nihilurk` finds a sibling for every language, so the dispatcher works
#   3. the four language binaries differ, so the feature loop really switched
#   4. a musl build has no dynamic loader, which is the whole point of musl
#
# TARGET defaults to this machine's own triple. Pass the musl one to check 4.
# A Windows target makes a .zip of .exe files; every other target, a .tar.gz.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"

fail() { printf 'FAIL  %s\n' "$*" >&2; exit 1; }
ok()   { printf 'ok    %s\n' "$*"; }

case "$TARGET" in
  *-windows-*) EXT=.exe; ARCHIVE=zip ;;
  *)           EXT="";   ARCHIVE=tar.gz ;;
esac
# macOS has no sha256sum.
sha256check() { if command -v sha256sum >/dev/null; then sha256sum -c "$@"; else shasum -a 256 -c "$@"; fi; }

release/package.sh "$TARGET" >/dev/null

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1)"
NAME="nihilurk-$VERSION-$TARGET"
PACKED="dist/$NAME.$ARCHIVE"
[ -f "$PACKED" ] || fail "no $PACKED"

( cd dist && sha256check "$NAME.$ARCHIVE.sha256" >/dev/null ) \
  || fail "the .sha256 file does not match the archive"
ok "checksum file matches"

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
if [ "$ARCHIVE" = zip ]; then
  ( cd "$tmp" && 7z x -bso0 "$ROOT/$PACKED" )
else
  tar -xzf "$PACKED" -C "$tmp"
fi
dir="$tmp/$NAME"

# 1. contents
want="$(printf '%s\n' \
  "$NAME" "$NAME/LICENSE" "$NAME/MANUAL.md" "$NAME/nihilurk$EXT" \
  "$NAME/nihilurk-en$EXT" "$NAME/nihilurk-es$EXT" "$NAME/nihilurk-ht$EXT" \
  "$NAME/nihilurk-pt$EXT" "$NAME/nihilurk.6" | sort)"
got="$(cd "$tmp" && find "$NAME" | sort)"
[ "$want" = "$got" ] || fail "archive contents differ
--- want
$want
--- got
$got"
ok "archive holds exactly the expected files"

# 2. dispatcher reaches every language
for lang in en pt es ht; do
  LC_ALL="${lang}_XX.UTF-8" "$dir/nihilurk" -content >/dev/null 2>&1 \
    || fail "nihilurk with LC_ALL=${lang}_XX could not run its sibling"
done
ok "the dispatcher runs all four languages"

# 3. the loop switched features between builds
for lang in pt es ht; do
  cmp -s "$dir/nihilurk-en$EXT" "$dir/nihilurk-$lang$EXT" \
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
