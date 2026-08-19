#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JSONSCHEMA_VERSION="4.23.0"

usage() {
    cat <<'EOF'
Usage: validation/v1.2.sh <arid-bin>

Run focused Arid v1.2 integration validation against an existing Arid executable.
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

for command in cmp cp mktemp python3 realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -x "$ROOT_DIR/validation/v1.1.sh" ]] ||
    die "required executable not found: validation/v1.1.sh"
[[ -f "$ROOT_DIR/schemas/report-v3.schema.json" ]] ||
    die "required schema not found: schemas/report-v3.schema.json"
[[ -f "$ROOT_DIR/schemas/baseline-v1.schema.json" ]] ||
    die "required schema not found: schemas/baseline-v1.schema.json"

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"

[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

echo "Running inherited v1.1 integration validation..."
"$ROOT_DIR/validation/v1.1.sh" "$ARID_BIN"
pass "inherited v1.1 integration surface"

TMP_ROOT="$(mktemp -d)"
PROJECT="$TMP_ROOT/project"
SCHEMA_VENV="$TMP_ROOT/schema-venv"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$PROJECT"
cd "$PROJECT"

cat >pyproject.toml <<'EOF'
[tool.arid]
min-lines = 4
EOF

cat >file-1.py <<'EOF'
alpha = calculate()
beta = transform(alpha)
gamma = persist(beta)
report(gamma)
EOF

for index in 2 3 4 5 6 7 8; do
    cp file-1.py "file-$index.py"
done

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
        if [[ -s "$stderr_file" ]]; then
            cat "$stderr_file" >&2
        fi
        die "expected exit $expected, got $status: $*"
    }

    [[ ! -s "$stderr_file" ]] || {
        cat "$stderr_file" >&2
        die "unexpected stderr: $*"
    }
}

run_expect_status 1 serial.json serial.stderr \
    "$ARID_BIN" . --workers 1 --json
run_expect_status 1 numeric.json numeric.stderr \
    "$ARID_BIN" . --workers 4 --json
run_expect_status 1 auto.json auto.stderr \
    "$ARID_BIN" . --workers auto --json

cmp -s serial.json numeric.json ||
    die "--workers 4 JSON differs from --workers 1"
cmp -s serial.json auto.json ||
    die "--workers auto JSON differs from --workers 1"
pass "serial/numeric/auto deterministic JSON equivalence"

run_expect_status 1 source.json source.stderr \
    "$ARID_BIN" . --workers auto --json --show-source

run_expect_status 0 baseline.stdout baseline.stderr \
    "$ARID_BIN" . --write-baseline baseline.json

python3 -m venv "$SCHEMA_VENV"

"$SCHEMA_VENV/bin/python" \
    -m pip install \
    --disable-pip-version-check \
    --no-cache-dir \
    --quiet \
    "jsonschema==$JSONSCHEMA_VERSION"

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/report-v3.schema.json" \
    "$ROOT_DIR/schemas/baseline-v1.schema.json" \
    serial.json \
    source.json \
    baseline.json \
    <<'PY'
import copy
import json
import sys
from pathlib import Path

from jsonschema import ValidationError
from jsonschema.validators import validator_for

report_schema_path = Path(sys.argv[1])
baseline_schema_path = Path(sys.argv[2])
report_paths = [Path(sys.argv[3]), Path(sys.argv[4])]
baseline_path = Path(sys.argv[5])


def load(path: Path):
    with path.open(encoding="utf-8") as file:
        return json.load(file)


def validator(schema_path: Path):
    schema = load(schema_path)
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema)


report_validator = validator(report_schema_path)
for report_path in report_paths:
    report_validator.validate(load(report_path))

invalid_report = copy.deepcopy(load(report_paths[0]))
invalid_report["version"] = 4
try:
    report_validator.validate(invalid_report)
except ValidationError:
    pass
else:
    raise SystemExit("report-v3 schema accepted report version 4")

baseline_validator = validator(baseline_schema_path)
baseline = load(baseline_path)
baseline_validator.validate(baseline)

invalid_baseline = copy.deepcopy(baseline)
invalid_baseline["version"] = 2
try:
    baseline_validator.validate(invalid_baseline)
except ValidationError:
    pass
else:
    raise SystemExit("baseline-v1 schema accepted baseline version 2")
PY

pass "report-v3 JSON Schema"
pass "baseline-v1 JSON Schema"

echo
echo "V1.2 targeted integration validation PASS"
echo "Arid: $ARID_VERSION"
echo "jsonschema: $JSONSCHEMA_VERSION"
