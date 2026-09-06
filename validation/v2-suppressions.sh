#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

usage() {
    cat <<'EOF'
Usage: validation/v2-suppressions.sh <arid-bin>

Validate suppression health summary and stale-suppression exit policy during
normal Arid scans.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

pass() {
    printf 'PASS: %s\n' "$1"
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

[[ $# -eq 1 ]] || {
    usage
    exit 2
}

ARID_BIN_INPUT="$1"

for command in grep mktemp python3 realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
TMP_ROOT="$(mktemp -d)"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

run_expect_status() {
    local expected="$1"
    local stdout_file="$2"
    local stderr_file="$3"
    shift 3

    local status

    set +e
    "$@" >"$stdout_file" 2>"$stderr_file"
    status=$?
    set -e

    [[ "$status" -eq "$expected" ]] || {
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        die "expected exit $expected, got $status: $*"
    }

    [[ ! -s "$stderr_file" ]] || {
        cat "$stderr_file" >&2
        die "unexpected stderr: $*"
    }
}

PROJECT="$TMP_ROOT/project"
mkdir -p "$PROJECT"

cat >"$PROJECT/a.py" <<'PY'
alpha = 1
beta = 2
PY

cat >"$PROJECT/b.py" <<'PY'
# arid: disable
gamma = 3
delta = 4
# arid: enable
PY

COMMON=(
    "$ARID_BIN"
    "$PROJECT"
    --no-config
    --project-root "$PROJECT"
    --min-lines 2
    --workers 1
)

run_expect_status \
    0 \
    "$TMP_ROOT/normal.txt" \
    "$TMP_ROOT/normal.err" \
    "${COMMON[@]}"

! grep -q '^Suppressions$' "$TMP_ROOT/normal.txt" ||
    die "ordinary scan unexpectedly audited suppressions"
pass "ordinary scan keeps fast suppression-free summary path"

run_expect_status \
    0 \
    "$TMP_ROOT/summary.txt" \
    "$TMP_ROOT/summary.err" \
    "${COMMON[@]}" --suppression-summary

grep -q '^Suppressions$' "$TMP_ROOT/summary.txt" ||
    die "suppression summary heading missing"
grep -q '│ Total  │ 1 │' "$TMP_ROOT/summary.txt" ||
    die "suppression summary total count is wrong"
grep -q '│ Active │ 0 │' "$TMP_ROOT/summary.txt" ||
    die "suppression summary active count is wrong"
grep -q '│ Stale  │ 1 │' "$TMP_ROOT/summary.txt" ||
    die "suppression summary stale count is wrong"
pass "suppression summary enriches normal text without changing exit policy"

run_expect_status \
    1 \
    "$TMP_ROOT/fail.txt" \
    "$TMP_ROOT/fail.err" \
    "${COMMON[@]}" --fail-on-stale

grep -q '^Suppressions$' "$TMP_ROOT/fail.txt" ||
    die "--fail-on-stale did not show suppression health in text output"
grep -q '│ Stale  │ 1 │' "$TMP_ROOT/fail.txt" ||
    die "--fail-on-stale stale count is wrong"
pass "normal scan fail-on-stale fails stale-only suppression health"

run_expect_status \
    1 \
    "$TMP_ROOT/fail.json" \
    "$TMP_ROOT/fail-json.err" \
    "${COMMON[@]}" --fail-on-stale --json

python3 - "$TMP_ROOT/fail.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["schema_version"] != 4:
    raise SystemExit("normal --fail-on-stale changed report-v4 schema")
if value["duplicate_groups"] != 0:
    raise SystemExit("stale-only fixture unexpectedly reported duplicates")
if "suppressions" in value:
    raise SystemExit("normal --fail-on-stale changed report-v4 shape")
PY
pass "normal scan fail-on-stale preserves machine report contract"

cat >"$PROJECT/b.py" <<'PY'
# arid: disable
alpha = 1
beta = 2
# arid: enable
PY

run_expect_status \
    0 \
    "$TMP_ROOT/active.txt" \
    "$TMP_ROOT/active.err" \
    "${COMMON[@]}" --fail-on-stale

grep -q '│ Active │ 1 │' "$TMP_ROOT/active.txt" ||
    die "active suppression count is wrong"
grep -q '│ Stale  │ 0 │' "$TMP_ROOT/active.txt" ||
    die "active suppression was incorrectly classified stale"
pass "normal scan fail-on-stale accepts active suppressions"

echo
echo "V2 suppression health validation PASS"
