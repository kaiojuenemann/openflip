#!/usr/bin/env bash
# Renders docs/images/social-preview.png, the repository's social media
# preview (Settings → General → Social preview), 1280×640 px: the clock in
# mid-flip with the project name and tagline below, in the bundled font.
#
# Needs a built openflip and ImageMagick:
#   cargo build --release -p openflip-app
#   docs/images/social-preview.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$ROOT/target/release/openflip"
FONT="$ROOT/crates/openflip-render/assets/InterDisplay-Bold.otf"
OUT="$ROOT/docs/images/social-preview.png"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
CLOCK="$TMP/clock.png"

"$BIN" --screenshot "$CLOCK" --size 1280x440 --time 10:42 --progress 0.32
convert -size 1280x640 xc:black "$CLOCK" -geometry +0+20 -composite \
    -font "$FONT" -gravity north \
    -fill '#e6e6e6' -pointsize 64 -annotate +0+452 'openflip' \
    -fill '#8c8c8c' -pointsize 30 \
    -annotate +0+540 'A free flip clock and screensaver for macOS, Windows and Linux' \
    "$OUT"
echo "wrote $OUT"
