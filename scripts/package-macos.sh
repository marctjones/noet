#!/usr/bin/env bash
# Build an Apple Silicon Noet.app, DMG, and tarball.
#
# The macOS tray/menu dependency stack currently fails under release LTO on some
# local Apple Silicon builds. Keep this script explicit so the local packaging
# path is reproducible while the release pipeline is hardened.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/gui/Cargo.toml | head -1)"
ARCH="$(uname -m)"
if [[ "$ARCH" != "arm64" ]]; then
  echo "This script currently builds the Apple Silicon artifact only (found $ARCH)." >&2
  exit 1
fi

: "${CARGO_PROFILE_RELEASE_LTO:=false}"
export CARGO_PROFILE_RELEASE_LTO

OUT="${NOET_MACOS_DIST:-$ROOT/dist/macos}"
APP="$OUT/Noet.app"
DMG_ROOT="$OUT/dmgroot"
DMG="$ROOT/noet-v${VERSION}-local-macos-arm64.dmg"
TARBALL="$ROOT/noet-v${VERSION}-local-macos-arm64.tar.gz"

rm -rf "$OUT"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$DMG_ROOT"

cargo build --release -p noet-gui
cp target/release/noet "$APP/Contents/MacOS/noet"
chmod +x "$APP/Contents/MacOS/noet"

PL="$APP/Contents/Info.plist"
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
  echo '<plist version="1.0"><dict>'
  echo '<key>CFBundleName</key><string>Noet</string>'
  echo '<key>CFBundleDisplayName</key><string>Noet</string>'
  echo '<key>CFBundleExecutable</key><string>noet</string>'
  echo '<key>CFBundleIdentifier</key><string>cl.skpt.noet</string>'
  echo "<key>CFBundleVersion</key><string>${VERSION}-local</string>"
  echo "<key>CFBundleShortVersionString</key><string>${VERSION}-local</string>"
  echo '<key>CFBundlePackageType</key><string>APPL</string>'
  echo '<key>LSMinimumSystemVersion</key><string>11.0</string>'
  echo '<key>NSHighResolutionCapable</key><true/>'
  echo '</dict></plist>'
} > "$PL"

# Ad-hoc signing is enough for a local artifact. Set SIGN_IDENTITY to a Developer
# ID Application identity when one is available; notarization is intentionally
# outside this local script so the no-Developer-ID path stays straightforward.
SIGN_IDENTITY="${SIGN_IDENTITY:--}"
if [[ "$SIGN_IDENTITY" == "-" ]]; then
  echo "Ad-hoc signing Noet.app (set SIGN_IDENTITY for Developer ID signing)."
  codesign --force --deep --sign - "$APP"
else
  echo "Signing Noet.app with identity: $SIGN_IDENTITY"
  codesign --force --deep --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
fi
codesign --verify --deep --strict --verbose=2 "$APP"

ditto "$APP" "$DMG_ROOT/Noet.app"
ln -s /Applications "$DMG_ROOT/Applications"

tar -C "$OUT" -czf "$TARBALL" Noet.app
hdiutil create -volname "Noet" -srcfolder "$DMG_ROOT" -ov -format UDZO "$DMG"
hdiutil verify "$DMG"

echo "Created:"
echo "  $APP"
echo "  $DMG"
echo "  $TARBALL"
