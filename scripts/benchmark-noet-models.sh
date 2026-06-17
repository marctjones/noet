#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  scripts/benchmark-noet-models.sh <model> <prompt>
  scripts/benchmark-noet-models.sh --all

Models:  mistral7b, ministral8b, mistralnemo
Prompts: labels, tasks, agenda, review

This wrapper runs Noet's embedded `mistral.rs` benchmark binary, not the
standalone CLI.
EOF
}

case "${1:-}" in
  --help|-h)
    usage
    exit 0
    ;;
esac

cargo run --release -p noet-ai --features mistralrs-inline-metal --bin noet-model-bench -- "$@"
