#!/usr/bin/env bash
#
# release/package.sh TARGET -- build one release tarball.
#
# Mirrors aur/PKGBUILD: one binary per language, plus the `nihilurk` dispatcher
# that picks between them from $LANG. Same target dir for every build, so the
# shared dependencies are not compiled four times over.
#
# Writes dist/nihilurk-<version>-<TARGET>.tar.gz and a .sha256 next to it. The
# version is `engine/Cargo.toml`'s; the release workflow refuses a tag that
# disagrees with it.
#
# A Windows target gets `.exe` on every binary. See
# docs/how-to/publish-a-github-release.md.
set -euo pipefail

cd "$(dirname "$0")/.."
TARGET="${1:?usage: release/package.sh <target-triple>}"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1)"
NAME="nihilurk-$VERSION-$TARGET"
OUT="dist/$NAME"
BIN="target/$TARGET/release"

EXE=""
case "$TARGET" in *-windows-*) EXE=".exe" ;; esac

# macOS has `shasum`, not `sha256sum`; Git Bash on Windows may have either.
sha() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }

rm -rf "$OUT" "dist/$NAME.tar.gz" "dist/$NAME.tar.gz.sha256"
mkdir -p "$OUT"

for lang in en pt es ht; do
    cargo build --locked --release --target "$TARGET" -p nihilurk --bin nihilurk \
        --no-default-features --features "lang-$lang"
    cp "$BIN/nihilurk$EXE" "$OUT/nihilurk-$lang$EXE"
done
cargo build --locked --release --target "$TARGET" -p nihilurk --bin nihilurk-dispatch \
    --features dispatch
cp "$BIN/nihilurk-dispatch$EXE" "$OUT/nihilurk$EXE"

cp LICENSE MANUAL.md doc/nihilurk.6 "$OUT/"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
(cd dist && sha "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
rm -rf "$OUT"
echo "dist/$NAME.tar.gz"
