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
# Linux targets only. See docs/how-to/publish-a-github-release.md.
set -euo pipefail

cd "$(dirname "$0")/.."
TARGET="${1:?usage: release/package.sh <target-triple>}"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' engine/Cargo.toml | head -1)"
NAME="nihilurk-$VERSION-$TARGET"
OUT="dist/$NAME"
BIN="target/$TARGET/release"


rm -rf "$OUT" "dist/$NAME.tar.gz" "dist/$NAME.tar.gz.sha256"
mkdir -p "$OUT"

for lang in en pt es ht; do
    cargo build --locked --release --target "$TARGET" -p nihilurk --bin nihilurk \
        --no-default-features --features "lang-$lang"
    cp "$BIN/nihilurk" "$OUT/nihilurk-$lang"
done
cargo build --locked --release --target "$TARGET" -p nihilurk --bin nihilurk-dispatch \
    --features dispatch
cp "$BIN/nihilurk-dispatch" "$OUT/nihilurk"

cp LICENSE MANUAL.md doc/nihilurk.6 "$OUT/"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
rm -rf "$OUT"
echo "dist/$NAME.tar.gz"
