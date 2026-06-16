#!/usr/bin/env bash

set -euo pipefail

MISTRALRS_BIN="${MISTRALRS_BIN:-/Users/marc/.cargo/bin/mistralrs}"
HF_CACHE_ROOT="${HF_CACHE_ROOT:-$HOME/.cache/huggingface/hub}"
OUT_DIR="${OUT_DIR:-$PWD/tmp/noet-supported-candidates}"
# Keep defaults conservative so candidate checks do not load full checkpoints
# or reserve large KV/prefix caches while the desktop is in normal use.
QUANT="${QUANT:-4}"
MAX_SEQ_LEN="${MAX_SEQ_LEN:-1024}"
MAX_SEQS="${MAX_SEQS:-1}"
PREFIX_CACHE_N="${PREFIX_CACHE_N:-0}"
MIN_FREE_PERCENT="${MIN_FREE_PERCENT:-25}"
RUN_ALL=0

usage() {
  cat <<'EOF'
Usage:
  scripts/benchmark-noet-supported-candidates.sh <model> <prompt>
  scripts/benchmark-noet-supported-candidates.sh --all

Models:  qwen17, smollm3, qwen34, granite4micro
Prompts: labels, tasks, patch, long_context

Defaults are memory-conservative:
  QUANT=4 MAX_SEQ_LEN=1024 MAX_SEQS=1 PREFIX_CACHE_N=0 MIN_FREE_PERCENT=25
EOF
}

check_memory_pressure() {
  if ! command -v memory_pressure >/dev/null 2>&1; then
    return
  fi

  local free_percent
  free_percent="$(memory_pressure 2>/dev/null | awk -F': ' '/System-wide memory free percentage/ {gsub(/%/, "", $2); print $2}' | tail -n 1)"
  if [[ -z "$free_percent" ]]; then
    return
  fi

  if (( free_percent < MIN_FREE_PERCENT )); then
    echo "memory pressure too high for local model load: ${free_percent}% free, require ${MIN_FREE_PERCENT}% (override MIN_FREE_PERCENT to change)" >&2
    exit 75
  fi
}

case "${1:-}" in
  --help|-h)
    usage
    exit 0
    ;;
  --all)
    RUN_ALL=1
    shift
    ;;
esac

SINGLE_MODEL="${1:-}"
SINGLE_PROMPT="${2:-}"

if [[ "$RUN_ALL" == "0" && ( -z "$SINGLE_MODEL" || -z "$SINGLE_PROMPT" ) ]]; then
  usage >&2
  exit 2
fi

mkdir -p "$OUT_DIR"

if [[ ! -x "$MISTRALRS_BIN" ]]; then
  echo "mistralrs binary not found at $MISTRALRS_BIN" >&2
  exit 1
fi

check_memory_pressure

snapshot_dir() {
  local model_dir="$1"
  find "$model_dir/snapshots" -mindepth 1 -maxdepth 1 -type d | head -n 1
}

write_prompt() {
  local kind="$1"
  local path="$2"

  case "$kind" in
    labels)
      cat >"$path" <<'EOF'
You are helping Noet organize a local note. Respond with exactly three short labels, one per line, no bullets, no explanation.

Meeting notes with Maya Chen about the Q3 launch:
- finalize the launch checklist before Friday
- legal still needs to confirm the revised privacy copy
- support team wants a short escalation runbook
- marketing asked for a one page summary of approved claims
- we should schedule a 30 minute follow-up next Tuesday
EOF
      ;;
    tasks)
      cat >"$path" <<'EOF'
You are helping Noet extract actionable tasks from a note. Return exactly three tasks, one per line, concise and imperative, no bullets, no explanation.

1:1 with Jordan:
- Jordan will send the latest customer churn analysis
- I need to review the retention experiment notes before our staff meeting
- we should update the board to reflect that the migration is blocked on SSO
- remember to ask finance whether the vendor renewal can slip to August
EOF
      ;;
    patch)
      cat >"$path" <<'EOF'
You are helping Noet rewrite a note fragment. Rewrite the text into a tighter, professional project note in under 90 words. Output only the rewritten note.

draft:
talked with ops and it sounds like the rollout is probably fine but we have a few loose ends. docs are kind of behind, on-call does not know the new alert names yet, and there is still confusion about whether the staging checklist is the same as prod. should circle back after the dry run and make somebody own the remaining checklist work.
EOF
      ;;
    long_context)
      {
        printf '%s\n' "You are helping Noet review a local note archive. Respond with exactly five short bullet points and nothing else."
        printf '%s\n\n' "Archive excerpt:"
        for i in $(seq 1 8); do
          cat <<EOF
### Note $i
Project: local-first work tracker
Owner: Alex Rivera
Status: active
Open questions:
- search ranking still feels noisy when labels and person links collide
- import review needs a better way to surface stale tasks without mutating notes automatically
- one-on-one agenda drafts are helpful but follow-up ownership is inconsistent
- the release checklist still depends on manual packaging notes
- the local AI prototype should stay offline and avoid hosted providers

Recent decisions:
- keep Markdown as the durable source of truth
- make task extraction proposal-first
- prefer predictable read models over UI-specific state
- keep the runtime local and inspectable

EOF
        done
      } >"$path"
      ;;
    *)
      echo "unknown prompt kind: $kind" >&2
      exit 1
      ;;
  esac
}

run_case() {
  local model_key="$1"
  local model_dir="$2"
  local prompt_kind="$3"
  local prompt_file="$OUT_DIR/${model_key}-${prompt_kind}.txt"
  local log_file="$OUT_DIR/${model_key}-${prompt_kind}.log"
  local time_file="$OUT_DIR/${model_key}-${prompt_kind}.time"
  local status=0

  write_prompt "$prompt_kind" "$prompt_file"

  set +e
  /usr/bin/time -lp \
    "$MISTRALRS_BIN" run \
    -m "$model_dir" \
    --quant "$QUANT" \
    --max-seq-len "$MAX_SEQ_LEN" \
    --max-seqs "$MAX_SEQS" \
    --prefix-cache-n "$PREFIX_CACHE_N" \
    -i "$(cat "$prompt_file")" \
    >"$log_file" 2>"$time_file"
  status=$?
  set -e

  local ttft prompt_tps decode_tps wall_real max_rss failure
  ttft="$(awk -F': ' '/CLI time to first token/ {print $2}' "$log_file" | tail -n 1)"
  prompt_tps="$(awk -F'[, ]+' '/^Prompt:/ {print $(NF-1)}' "$log_file" | tail -n 1)"
  decode_tps="$(awk -F'[, ]+' '/^Decode:/ {print $(NF-1)}' "$log_file" | tail -n 1)"
  wall_real="$(awk '$1=="real" {print $2}' "$time_file" | tail -n 1)"
  max_rss="$(awk '$2=="maximum" && $3=="resident" && $4=="set" && $5=="size" {print $1}' "$time_file" | tail -n 1)"
  failure="$(sed -n '1p' "$time_file" | tr '\t' ' ' | tr '\n' ' ' | sed 's/  */ /g; s/[[:space:]]*$//')"

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$model_key" "$prompt_kind" "$status" "${ttft:-n/a}" "${prompt_tps:-n/a}" "${decode_tps:-n/a}" "${wall_real:-n/a}" "${max_rss:-n/a}" "${failure:-}"
}

model_qwen17_dir="$(snapshot_dir "$HF_CACHE_ROOT/models--Qwen--Qwen3-1.7B")"
model_smollm3_dir="$(snapshot_dir "$HF_CACHE_ROOT/models--HuggingFaceTB--SmolLM3-3B")"
model_qwen34_dir="$(snapshot_dir "$HF_CACHE_ROOT/models--Qwen--Qwen3-4B")"
model_granite4micro_dir="$(snapshot_dir "$HF_CACHE_ROOT/models--ibm-granite--granite-4.0-micro")"

for dir_var in model_qwen17_dir model_smollm3_dir model_qwen34_dir model_granite4micro_dir; do
  if [[ -z "${!dir_var}" ]]; then
    echo "missing snapshot directory for $dir_var" >&2
    exit 1
  fi
done

report_file="$OUT_DIR/report.tsv"
printf 'model\tprompt\texit_status\tttft\tprompt_tps\tdecode_tps\twall_seconds\tmax_rss_bytes\tfailure\n' >"$report_file"

if [[ "$RUN_ALL" == "0" ]]; then
  case "$SINGLE_MODEL" in
    qwen17)
      run_case "qwen17" "$model_qwen17_dir" "$SINGLE_PROMPT" >>"$report_file"
      ;;
    smollm3)
      run_case "smollm3" "$model_smollm3_dir" "$SINGLE_PROMPT" >>"$report_file"
      ;;
    qwen34)
      run_case "qwen34" "$model_qwen34_dir" "$SINGLE_PROMPT" >>"$report_file"
      ;;
    granite4micro)
      run_case "granite4micro" "$model_granite4micro_dir" "$SINGLE_PROMPT" >>"$report_file"
      ;;
    *)
      echo "unknown model key: $SINGLE_MODEL" >&2
      exit 1
      ;;
  esac
else
  for prompt in labels tasks patch long_context; do
    run_case "qwen17" "$model_qwen17_dir" "$prompt" >>"$report_file"
    run_case "smollm3" "$model_smollm3_dir" "$prompt" >>"$report_file"
    run_case "qwen34" "$model_qwen34_dir" "$prompt" >>"$report_file"
    run_case "granite4micro" "$model_granite4micro_dir" "$prompt" >>"$report_file"
  done
fi

cat "$report_file"
