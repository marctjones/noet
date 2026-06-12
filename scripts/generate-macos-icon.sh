#!/usr/bin/env bash
# Generate the macOS .icns app icon from the canonical SVG source.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/assets/app-icon/noet-icon.svg"
OUT="$ROOT/assets/app-icon/Noet.icns"
TMP="${TMPDIR:-/tmp}/noet-icon-$$"
PNG="$TMP/noet-icon.svg.png"
ICONSET="$TMP/Noet.iconset"

if [[ "$(uname)" != "Darwin" ]]; then
  echo "macOS icon generation requires iconutil and qlmanage on Darwin." >&2
  exit 1
fi

rm -rf "$TMP"
mkdir -p "$TMP" "$ICONSET"

qlmanage -t -s 1024 -o "$TMP" "$SRC" >/dev/null 2>&1
if [[ ! -f "$PNG" ]]; then
  echo "qlmanage did not create $PNG" >&2
  exit 1
fi

sips -z 16 16 "$PNG" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$PNG" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$PNG" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$PNG" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$PNG" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$PNG" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$PNG" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$PNG" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$PNG" --out "$ICONSET/icon_512x512.png" >/dev/null
cp "$PNG" "$ICONSET/icon_512x512@2x.png"

iconutil -c icns "$ICONSET" -o "$OUT"
rm -rf "$TMP"

echo "Created $OUT"
