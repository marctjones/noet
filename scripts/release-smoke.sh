#!/usr/bin/env bash
# Deterministic release-gate smoke for Noet.
#
# By default this does not load local AI models and does not build installers.
# Opt in to heavier checks with:
#   NOET_RUN_LOCAL_MODEL_SMOKES=1 scripts/release-smoke.sh
#   NOET_RUN_MACOS_PACKAGE=1 scripts/release-smoke.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

run() {
  echo
  echo "==> $*"
  "$@"
}

memory_free_percent() {
  if ! command -v memory_pressure >/dev/null 2>&1; then
    echo ""
    return 0
  fi
  memory_pressure 2>/dev/null \
    | awk '/System-wide memory free percentage:/ {gsub("%", "", $5); print $5; exit}'
}

run cargo fmt --check
run cargo test --workspace
run cargo check -p noet-gui --features mistralrs-inline
run git diff --check

if [[ "${NOET_RUN_LOCAL_MODEL_SMOKES:-0}" == "1" ]]; then
  min_free="${NOET_AI_MIN_FREE_MEMORY_PERCENT:-25}"
  free="$(memory_free_percent)"
  if [[ -z "$free" ]]; then
    echo "memory_pressure is unavailable; refusing to run local model smokes." >&2
    exit 1
  fi
  if (( free < min_free )); then
    echo "Only ${free}% memory is free; refusing to load local models below ${min_free}%." >&2
    exit 1
  fi
  echo "Memory preflight passed: ${free}% free."
  run cargo test -p noet-gui --features mistralrs-inline \
    headless_ui_local_model_ai_smoke -- --ignored --nocapture
  run cargo test -p noet-gui --features mistralrs-inline \
    headless_ui_local_model_cancel_smoke -- --ignored --nocapture
  run cargo test -p noet-gui --features mistralrs-inline \
    headless_ui_local_embedding_refresh_smoke -- --ignored --nocapture
else
  echo
  echo "Skipping local model smokes; set NOET_RUN_LOCAL_MODEL_SMOKES=1 to run them."
fi

if [[ "${NOET_RUN_MACOS_PACKAGE:-0}" == "1" ]]; then
  run scripts/package-macos.sh
else
  echo
  echo "Skipping macOS package build; set NOET_RUN_MACOS_PACKAGE=1 to run it."
fi

echo
echo "Release smoke passed."
