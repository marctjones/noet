#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VAULT="${1:-"$ROOT/target/noet-demo-vault"}"
FORCE="${NOET_DEMO_FORCE_RESET:-}"

case "$(basename "$VAULT")" in
  *noet-demo-vault*) ;;
  *)
    if [[ "$FORCE" != "1" ]]; then
      echo "Refusing to reset non-demo path: $VAULT" >&2
      echo "Set NOET_DEMO_FORCE_RESET=1 to override." >&2
      exit 2
    fi
    ;;
esac

ARGS=("$VAULT")
if [[ "$FORCE" == "1" ]]; then
  ARGS+=("--force")
fi

cargo run -p noet-core --bin noet-demo-vault -- "${ARGS[@]}"

cat <<EOF

Demo vault reset:
  $VAULT

Run Noet against this vault with:
  NOET_VAULT="$VAULT" cargo run -p noet-gui
EOF
