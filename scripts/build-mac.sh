#!/bin/bash
# Builds dist/Mindful.app – double-click it or pin it to the Dock.
#
#   ./scripts/build-mac.sh            build only
#   ./scripts/build-mac.sh --install  build and replace /Applications/Mindful.app
#   ./scripts/build-mac.sh --zip      build and pack dist/Mindful-macos-arm64.zip for a release
#
# Pin the app from /Applications: the Dock icon then always opens the latest build.
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO="${CARGO:-cargo}"
command -v "$CARGO" >/dev/null || CARGO="$HOME/.cargo/bin/cargo"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP=dist/Mindful.app
ICONSET=target/AppIcon.iconset
INSTALL_DIR=/Applications

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

if [[ "${1:-}" == "--zip" ]]; then
    # Fixed file name, so the README's "latest download" link always works
    ZIP="dist/Mindful-macos-$(uname -m).zip"
    rm -f "$ZIP"
    ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"
    echo "==> Packed: $ZIP"
    shasum -a 256 "$ZIP"
fi

if [[ "${1:-}" == "--install" ]]; then
    echo "==> Installing to $INSTALL_DIR"
    # A running copy would keep the old version open
    osascript -e 'tell application "Mindful" to quit' >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$INSTALL_DIR/Mindful.app"
    cp -R "$APP" "$INSTALL_DIR/"
    echo "==> Installed: $INSTALL_DIR/Mindful.app"
fi
