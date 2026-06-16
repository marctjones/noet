#!/usr/bin/env bash

set -euo pipefail

if ! command -v hf >/dev/null 2>&1; then
  echo "hf CLI is required but not found on PATH" >&2
  exit 1
fi

download() {
  local repo="$1"
  local file="$2"

  echo "==> $(date '+%Y-%m-%d %H:%M:%S') downloading ${repo}:${file}"
  hf download "$repo" --include "$file"
  echo "==> $(date '+%Y-%m-%d %H:%M:%S') completed ${repo}:${file}"
}

download "bartowski/Mistral-Nemo-Instruct-2407-GGUF" \
  "Mistral-Nemo-Instruct-2407-Q4_K_M.gguf"

download "bartowski/Mistral-7B-Instruct-v0.3-GGUF" \
  "Mistral-7B-Instruct-v0.3-Q4_K_M.gguf"

download "bartowski/Ministral-8B-Instruct-2410-GGUF" \
  "Ministral-8B-Instruct-2410-Q4_K_M.gguf"

# English-focused inline embedding default. This is intentionally not a GGUF
# chat model; Noet loads it through the mistral.rs Rust embedding API.
download "google/embeddinggemma-300m" \
  "*"

# Small English alternative. Keep downloaded for evaluation and future loader
# support, but it is not the inline mistral.rs default today.
download "Snowflake/snowflake-arctic-embed-s" \
  "*"
