#!/usr/bin/env bash
#
# release/test_package.sh [TARGET] -- does release/package.sh make an archive a
# stranger can unpack and play?
#
# It builds for real (fat LTO), so it takes a few minutes. The
# checks are the ones a release breaks on:
#
#   1. the archive holds exactly the files the installer expects
#   2. a musl build has no dynamic loader, which is the whole point of musl
#
# TARGET defaults to this machine's own triple. Pass the musl one to check 2.
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
  "$NAME" "$NAME/LICENSE" "$NAME/MANUAL.md" "$NAME/nihilurk$EXT" | sort)"
got="$(cd "$tmp" && find "$NAME" | sort)"
[ "$want" = "$got" ] || fail "archive contents differ
--- want
$want
--- got
$got"
ok "archive holds exactly the expected files"

# 2. musl means static
case "$TARGET" in
  *-linux-musl*)
    for f in nihilurk; do
      readelf -lW "$dir/$f" | grep -q INTERP && fail "$f has a dynamic loader"
    done
    ok "no dynamic loader in a musl build" ;;
  *) printf 'skip  static check (not a musl target)\n' ;;
esac
