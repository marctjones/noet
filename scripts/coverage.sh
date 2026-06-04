#!/usr/bin/env bash
# Coverage ratchet for Knot.
#
# Runs the test suite under llvm-cov and FAILS if line coverage drops below the
# floors below. This is a one-way ratchet: when you raise coverage, raise the
# floors too so it can never silently regress. Run locally before pushing and in
# CI (.github/workflows/ci.yml).
#
#   ./scripts/coverage.sh            # enforce floors
#   ./scripts/coverage.sh --html     # also write an HTML report to target/llvm-cov/html
#
# Floors are intentionally per-layer: backend.rs is pure logic + SQL and SHOULD
# trend to 80%+. main.rs is Slint glue with ~0% unit coverage today (it needs the
# GUI automation harness, not unit tests), so it's tracked only via the TOTAL.
set -euo pipefail
cd "$(dirname "$0")/.."

# --- ratchet floors (raise these as coverage improves; target backend ≥ 80) ---
BACKEND_MIN="${BACKEND_MIN:-80}"
TOTAL_MIN="${TOTAL_MIN:-44}"

EXTRA=()
[[ "${1:-}" == "--html" ]] && EXTRA+=(--html)

echo "Running tests under llvm-cov…"
SUMMARY="$(cargo llvm-cov --summary-only "${EXTRA[@]}")"
echo "$SUMMARY"

# Column 10 of each row is the Lines "Cover" percentage (Regions%, Functions%,
# Lines% are the 4th/7th/10th fields; filename is field 1).
backend_cov="$(awk '/backend\/mod\.rs/ {gsub("%","",$10); print $10}' <<<"$SUMMARY")"
total_cov="$(awk '/^TOTAL/      {gsub("%","",$10); print $10}' <<<"$SUMMARY")"

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
