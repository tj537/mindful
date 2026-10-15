#!/bin/bash
# Builds dist/Mindful.app – double-click it or pin it to the Dock.
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO="${CARGO:-cargo}"
command -v "$CARGO" >/dev/null || CARGO="$HOME/.cargo/bin/cargo"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP=dist/Mindful.app
ICONSET=target/AppIcon.iconset

echo "==> Building release binary"
"$CARGO" build --release

echo "==> Rendering icon"
rm -rf "$ICONSET"
"$CARGO" run --quiet --release --example make_icon -- "$ICONSET"

echo "==> Assembling app bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/mindful "$APP/Contents/MacOS/mindful"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# Local ad-hoc signature (no Apple account needed) so macOS accepts the bundle as a whole
codesign --force --deep --sign - "$APP"

echo "==> Done: $APP"
