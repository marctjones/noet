#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VAULT="${NOET_VISUAL_REVIEW_VAULT:-"$ROOT/target/noet-demo-vault"}"
TRACE="${NOET_VISUAL_REVIEW_TRACE:-/tmp/noet-ui-review.jsonl}"
CONFIG_DIR="${NOET_VISUAL_REVIEW_CONFIG_DIR:-/tmp/noet-review-config}"
CACHE_DIR="${NOET_VISUAL_REVIEW_CACHE_DIR:-/tmp/noet-review-cache}"
OUT="$ROOT/target/noet-visual-review"
APP="$OUT/Noet Visual Review.app"
EXE="$APP/Contents/MacOS/noet"

xml_escape() {
  local s="$1"
  s="${s//&/&amp;}"
  s="${s//</&lt;}"
  s="${s//>/&gt;}"
  s="${s//\"/&quot;}"
  printf '%s' "$s"
}

if [[ ! -d "$VAULT/notes" ]]; then
  "$ROOT/scripts/reset-demo-vault.sh" "$VAULT"
fi

mkdir -p "$CONFIG_DIR" "$CACHE_DIR"
cargo build -p noet-gui

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/target/debug/noet" "$EXE"
chmod +x "$EXE"

if [[ -f "$ROOT/assets/app-icon/Noet.icns" ]]; then
  cp "$ROOT/assets/app-icon/Noet.icns" "$APP/Contents/Resources/Noet.icns"
fi

VAULT_XML="$(xml_escape "$VAULT")"
TRACE_XML="$(xml_escape "$TRACE")"
CONFIG_DIR_XML="$(xml_escape "$CONFIG_DIR")"
CACHE_DIR_XML="$(xml_escape "$CACHE_DIR")"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Noet Visual Review</string>
<key>CFBundleDisplayName</key><string>Noet Visual Review</string>
<key>CFBundleExecutable</key><string>noet</string>
<key>CFBundleIdentifier</key><string>cl.skpt.noet.visualreview</string>
<key>CFBundleIconFile</key><string>Noet</string>
<key>CFBundleVersion</key><string>0.0.0-review</string>
<key>CFBundleShortVersionString</key><string>0.0.0-review</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSEnvironment</key><dict>
<key>NOET_DISABLE_IPC</key><string>1</string>
<key>NOET_DISABLE_TRAY</key><string>1</string>
<key>NOET_AI_RUNTIME</key><string>preview</string>
<key>NOET_CONFIG_DIR</key><string>$CONFIG_DIR_XML</string>
<key>NOET_CACHE_DIR</key><string>$CACHE_DIR_XML</string>
<key>NOET_UI_TRACE</key><string>$TRACE_XML</string>
<key>NOET_UI_TRACE_CONTENT</key><string>1</string>
<key>NOET_VAULT</key><string>$VAULT_XML</string>
</dict>
</dict></plist>
EOF

echo "Created visual review app:"
echo "  $APP"
echo
echo "Launch with:"
echo "  open '$APP'"
