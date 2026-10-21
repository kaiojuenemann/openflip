#!/usr/bin/env bash
# Builds openflip.app and OpenFlip.saver as universal (Apple silicon + Intel)
# bundles into target/macos/. Requires Xcode command line tools and the Rust
# targets aarch64-apple-darwin and x86_64-apple-darwin.
#
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#   platform/macos/build.sh
#
# The bundles are signed ad hoc (no Apple Developer ID); see README.md for
# installing them.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HERE="$ROOT/platform/macos"
OUT="$ROOT/target/macos"
MIN_MACOS="12.0"
export MACOSX_DEPLOYMENT_TARGET="$MIN_MACOS"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCHS=(arm64 x86_64)
rust_target() { [ "$1" = arm64 ] && echo aarch64-apple-darwin || echo x86_64-apple-darwin; }

rm -rf "$OUT"
mkdir -p "$OUT/build"

for arch in "${ARCHS[@]}"; do
    target="$(rust_target "$arch")"
    echo "==> Rust ($target)"
    cargo build --manifest-path "$ROOT/Cargo.toml" --release --locked \
        --target "$target" -p openflip-app -p openflip-ffi
done

# The system libraries the Rust static library needs, as rustc reports them.
touch "$ROOT/crates/openflip-ffi/src/lib.rs"
NATIVE_LIBS="$(cargo rustc --manifest-path "$ROOT/Cargo.toml" --release --locked \
    --target aarch64-apple-darwin -p openflip-ffi --lib --crate-type staticlib \
    -- --print native-static-libs 2>&1 | sed -n 's/.*native-static-libs: //p' | tail -1)"
echo "==> native libraries: $NATIVE_LIBS"

for arch in "${ARCHS[@]}"; do
    target="$(rust_target "$arch")"
    echo "==> Swift screensaver ($arch)"
    # shellcheck disable=SC2086 # NATIVE_LIBS is a list of linker flags.
    swiftc -O -parse-as-library \
        -module-name OpenFlipSaver \
        -target "$arch-apple-macos$MIN_MACOS" \
        -I "$HERE/include" \
        -emit-library -o "$OUT/build/OpenFlip-$arch" \
        "$HERE"/Sources/*.swift \
        "$ROOT/target/$target/release/libopenflip_ffi.a" \
        $NATIVE_LIBS \
        -framework ScreenSaver -framework AppKit -framework UniformTypeIdentifiers
done

fill_plist() {
    sed -e "s/@VERSION@/$VERSION/g" -e "s/@MIN_MACOS@/$MIN_MACOS/g" "$1" > "$2"
}

echo "==> OpenFlip.saver"
SAVER="$OUT/OpenFlip.saver"
mkdir -p "$SAVER/Contents/MacOS"
lipo -create "${ARCHS[@]/#/$OUT/build/OpenFlip-}" -output "$SAVER/Contents/MacOS/OpenFlip"
fill_plist "$HERE/Resources/Saver-Info.plist" "$SAVER/Contents/Info.plist"
codesign --force --sign - "$SAVER"

echo "==> openflip.app"
APP="$OUT/openflip.app"
mkdir -p "$APP/Contents/MacOS"
lipo -create \
    "$ROOT/target/aarch64-apple-darwin/release/openflip" \
    "$ROOT/target/x86_64-apple-darwin/release/openflip" \
    -output "$APP/Contents/MacOS/openflip"
fill_plist "$HERE/Resources/App-Info.plist" "$APP/Contents/Info.plist"
codesign --force --sign - "$APP"

lipo -info "$SAVER/Contents/MacOS/OpenFlip" "$APP/Contents/MacOS/openflip"
echo "==> done: $OUT"
