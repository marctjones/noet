#!/usr/bin/env bash

set -euo pipefail

if ! command -v hf >/dev/null 2>&1; then
  echo "hf CLI is required but not found on PATH" >&2
  exit 1
fi

download_repo() {
  local repo="$1"
  echo "==> $(date '+%Y-%m-%d %H:%M:%S') downloading ${repo}"
  hf download "$repo"
  echo "==> $(date '+%Y-%m-%d %H:%M:%S') completed ${repo}"
}

download_repo "Qwen/Qwen3-1.7B"
download_repo "HuggingFaceTB/SmolLM3-3B"
download_repo "Qwen/Qwen3-4B"
download_repo "ibm-granite/granite-4.0-micro"
