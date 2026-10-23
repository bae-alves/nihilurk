#!/usr/bin/env bash
#
# release/package.sh TARGET -- build one release tarball.
#
# Writes dist/nihilurk-<version>-<TARGET>.tar.gz (a .zip for a Windows target,
# whose binaries end in .exe) and a .sha256 next to it. The version is
# `engine/Cargo.toml`'s; the release workflow refuses a tag that disagrees with
# it.
#
# Runs in bash on Linux, macOS and Windows (Git Bash). See
# docs/how-to/publish-a-github-release.md.
set -euo pipefail

cd "$(dirname "$0")/.."
TARGET="${1:?usage: release/package.sh <target-triple>}"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1)"
NAME="nihilurk-$VERSION-$TARGET"
OUT="dist/$NAME"
BIN="target/$TARGET/release"

case "$TARGET" in
    *-windows-*) EXT=.exe; ARCHIVE=zip ;;
    *)           EXT="";   ARCHIVE=tar.gz ;;
esac


rm -rf "$OUT" "dist/$NAME.$ARCHIVE" "dist/$NAME.$ARCHIVE.sha256"
mkdir -p "$OUT"

cargo build --locked --release --target "$TARGET" -p nihilurk --bin nihilurk
cp "$BIN/nihilurk$EXT" "$OUT/nihilurk$EXT"

cp LICENSE MANUAL.md doc/nihilurk.6 "$OUT/"

# macOS has no sha256sum, and Git Bash on Windows has no zip, but has 7z.
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
if [ "$ARCHIVE" = zip ]; then
    (cd dist && 7z a -tzip -bso0 "$NAME.zip" "$NAME")
else
    tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
fi
(cd dist && sha256 "$NAME.$ARCHIVE" > "$NAME.$ARCHIVE.sha256")
rm -rf "$OUT"
echo "dist/$NAME.$ARCHIVE"
