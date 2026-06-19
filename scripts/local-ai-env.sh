#!/usr/bin/env bash
# Source this file before running local-model Noet tests from a terminal:
#   source scripts/local-ai-env.sh
#
# Do not source it for the default fast test suite unless you intend tests to
# load real local models.

export NOET_AI_RUNTIME="${NOET_AI_RUNTIME:-local}"
export NOET_RUN_LOCAL_MODEL_SMOKES="${NOET_RUN_LOCAL_MODEL_SMOKES:-1}"
export NOET_AI_MIN_FREE_MEMORY_PERCENT="${NOET_AI_MIN_FREE_MEMORY_PERCENT:-25}"
export HF_HOME="${HF_HOME:-$HOME/.cache/huggingface}"
export HF_CACHE_ROOT="${HF_CACHE_ROOT:-$HF_HOME/hub}"
