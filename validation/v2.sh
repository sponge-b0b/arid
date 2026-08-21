#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<'EOF'
Usage: validation/v2.sh <arid-bin>

Run the complete targeted Arid v2 integration validation suite against an
existing Arid executable.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

[[ $# -eq 1 ]] || {
    usage
    exit 2
}

"$SCRIPT_DIR/v2-contract.sh" "$1"
echo
"$SCRIPT_DIR/v2-operations.sh" "$1"
echo
"$SCRIPT_DIR/v2-project.sh" "$1"
echo
"$SCRIPT_DIR/v2-identity.sh" "$1"
echo
"$SCRIPT_DIR/v2-compatibility.sh" "$1"

echo
echo "V2 targeted integration validation PASS"
