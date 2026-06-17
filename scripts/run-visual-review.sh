#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VAULT="${NOET_VISUAL_REVIEW_VAULT:-"$ROOT/target/noet-demo-vault"}"
TRACE="${NOET_VISUAL_REVIEW_TRACE:-/tmp/noet-ui-review.jsonl}"
CONFIG_DIR="${NOET_VISUAL_REVIEW_CONFIG_DIR:-/tmp/noet-review-config}"
CACHE_DIR="${NOET_VISUAL_REVIEW_CACHE_DIR:-/tmp/noet-review-cache}"

if [[ ! -d "$VAULT/notes" ]]; then
  "$ROOT/scripts/reset-demo-vault.sh" "$VAULT"
fi

mkdir -p "$CONFIG_DIR" "$CACHE_DIR"

cat <<EOF
Launching Noet visual review:
  vault: $VAULT
  trace: $TRACE
  config: $CONFIG_DIR
  cache: $CACHE_DIR

EOF

cd "$ROOT"
exec env \
  NOET_DISABLE_IPC=1 \
  NOET_DISABLE_TRAY=1 \
  NOET_AI_RUNTIME=preview \
  NOET_CONFIG_DIR="$CONFIG_DIR" \
  NOET_CACHE_DIR="$CACHE_DIR" \
  NOET_UI_TRACE="$TRACE" \
  NOET_UI_TRACE_CONTENT="${NOET_UI_TRACE_CONTENT:-1}" \
  NOET_VAULT="$VAULT" \
  cargo run -p noet-gui
