#!/usr/bin/env bash
# Coverage ratchet for Noet.
#
# Runs the test suite under llvm-cov and FAILS if line coverage drops below the
# floors below. This is a one-way ratchet: when you raise coverage, raise the
# floors too so it can never silently regress. Run locally before pushing and in
# CI (.github/workflows/ci.yml).
#
#   ./scripts/coverage.sh            # enforce floors
#   ./scripts/coverage.sh --html     # also write an HTML report to target/llvm-cov/html
#
# Floors are intentionally per-layer: the backend (crates/core/src/backend/*.rs,
# excluding the test module) is pure logic + SQL and SHOULD trend to 80%+. main.rs
# is Slint glue with ~0% unit coverage today (it needs the GUI automation harness,
# not unit tests), so it's tracked only via the TOTAL.
set -euo pipefail
cd "$(dirname "$0")/.."

# --- ratchet floors (raise these as coverage improves; target backend ≥ 80) ---
# NOTE: the backend was decomposed from a single backend/mod.rs into focused
# submodules; this floor aggregates line coverage across all of them. The honest
# current number is ~75% — the previous 80 default was never actually enforced
# (CI runs build/test/clippy, not this script). Raise toward 80 as tests land
# (render.rs / the export PDF + mutate error paths are the big gaps).
BACKEND_MIN="${BACKEND_MIN:-79}"
# The TOTAL spans the whole workspace. It jumped once the headless Slint GUI test
# (crates/gui/src/ui_tests.rs) started exercising setup_app + the main.rs callback
# wiring; bench.rs (the perf harness) stays ~0% by design. Floor tracks reality.
TOTAL_MIN="${TOTAL_MIN:-62}"

EXTRA=()
[[ "${1:-}" == "--html" ]] && EXTRA+=(--html)

echo "Running tests under llvm-cov…"
SUMMARY="$(cargo llvm-cov --summary-only "${EXTRA[@]}")"
echo "$SUMMARY"

# Per-row fields: filename=1, Lines(total)=8, Missed Lines=9, Lines Cover%=10.
# The backend now spans several files, so aggregate true line coverage across the
# whole backend/ tree (summing lines/missed) rather than reading one file's %.
# The test module (tests.rs) carries no instrumented production lines, so it's
# excluded for clarity.
backend_cov="$(awk '
  /core\/src\/backend\// && $1 !~ /tests\.rs/ { lines += $8; missed += $9 }
  END { if (lines > 0) printf "%.2f", (lines - missed) / lines * 100; else print "0" }
' <<<"$SUMMARY")"
total_cov="$(awk '/^TOTAL/ {gsub("%","",$10); print $10}' <<<"$SUMMARY")"

fail=0
check() { # name actual floor
  awk -v a="$2" -v f="$3" 'BEGIN{exit !(a+0 >= f+0)}' \
    && printf "  ✓ %-10s %6.2f%% (floor %s%%)\n" "$1" "$2" "$3" \
    || { printf "  ✗ %-10s %6.2f%% < floor %s%% — RATCHET FAILED\n" "$1" "$2" "$3"; fail=1; }
}
echo "Ratchet:"
check "backend" "${backend_cov:-0}" "$BACKEND_MIN"
check "TOTAL"   "${total_cov:-0}"   "$TOTAL_MIN"

if [[ "$fail" == 1 ]]; then
  echo "Coverage regressed below the ratchet floor. Add tests or (if intentional) lower the floor with reason."
  exit 1
fi
echo "Coverage ratchet passed."
