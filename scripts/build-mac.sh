#!/bin/bash
# Baut dist/Mindful.app – zum Doppelklicken und Anpinnen im Dock.
set -euo pipefail
cd "$(dirname "$0")/.."

CARGO="${CARGO:-cargo}"
command -v "$CARGO" >/dev/null || CARGO="$HOME/.cargo/bin/cargo"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP=dist/Mindful.app
ICONSET=target/AppIcon.iconset

echo "==> Programm bauen (Release)"
"$CARGO" build --release

echo "==> Icon erzeugen"
rm -rf "$ICONSET"
"$CARGO" run --quiet --release --example make_icon -- "$ICONSET"

echo "==> App-Paket zusammenstellen"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/mindful "$APP/Contents/MacOS/mindful"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# Lokale Signatur ohne Apple-Konto, damit macOS das Paket als Ganzes akzeptiert
codesign --force --deep --sign - "$APP"

echo "==> Fertig: $APP"
