#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JSONSCHEMA_VERSION="4.23.0"
REPORT_V3_BLOB="f3f466e2a16c476bc6832b8cc12c11c33cef1c63"
BASELINE_V1_BLOB="ff2e7dcfea3eb9d0a982d7294ce65943de882e53"

usage() {
    cat <<'EOF'
Usage: validation/v2.sh <arid-bin>

Run focused Arid v2 integration validation against an existing Arid executable.
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

for command in cmp cp git mktemp python3 realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

for schema in \
    baseline-v1.schema.json \
    capabilities-v1.schema.json \
    error-v1.schema.json \
    report-v3.schema.json \
    report-v4.schema.json
do
    [[ -f "$ROOT_DIR/schemas/$schema" ]] ||
        die "required schema not found: schemas/$schema"
done

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"

[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

[[ "$(git -C "$ROOT_DIR" hash-object schemas/report-v3.schema.json)" == "$REPORT_V3_BLOB" ]] ||
    die "schemas/report-v3.schema.json differs from the published v1.2 contract"
pass "report-v3 historical immutability"

[[ "$(git -C "$ROOT_DIR" hash-object schemas/baseline-v1.schema.json)" == "$BASELINE_V1_BLOB" ]] ||
    die "schemas/baseline-v1.schema.json differs from the published v1.2 contract"
pass "baseline-v1 schema immutability"

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

cat >file-1.py <<'PY'
def duplicated_calculation():
    alpha = 1
    beta = 2
    gamma = alpha + beta
    delta = gamma * 2
    return delta
PY

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

COMMON_SCAN_ARGS=(
    .
    --no-config
    --project-root .
    --min-lines 4
    --json
)

run_expect_status 1 serial.json serial.stderr \
    "$ARID_BIN" "${COMMON_SCAN_ARGS[@]}" --workers 1
run_expect_status 1 numeric.json numeric.stderr \
    "$ARID_BIN" "${COMMON_SCAN_ARGS[@]}" --workers 4
run_expect_status 1 auto.json auto.stderr \
    "$ARID_BIN" "${COMMON_SCAN_ARGS[@]}" --workers auto

cmp -s serial.json numeric.json ||
    die "--workers 4 JSON differs from --workers 1"
cmp -s serial.json auto.json ||
    die "--workers auto JSON differs from --workers 1"
pass "serial/numeric/auto deterministic report-v4 equivalence"

run_expect_status 1 source.json source.stderr \
    "$ARID_BIN" "${COMMON_SCAN_ARGS[@]}" --workers auto --show-source

run_expect_status 0 baseline.stdout baseline.stderr \
    "$ARID_BIN" . \
    --no-config \
    --project-root . \
    --min-lines 4 \
    --write-baseline baseline.json

run_expect_status 0 capabilities-1.json capabilities-1.stderr \
    "$ARID_BIN" --capabilities
run_expect_status 0 capabilities-2.json capabilities-2.stderr \
    "$ARID_BIN" --capabilities

cmp -s capabilities-1.json capabilities-2.json ||
    die "--capabilities output is not deterministic"
pass "deterministic capabilities output"

python3 -m venv "$SCHEMA_VENV"

"$SCHEMA_VENV/bin/python" \
    -m pip install \
    --disable-pip-version-check \
    --no-cache-dir \
    --quiet \
    "jsonschema==$JSONSCHEMA_VERSION"

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/report-v4.schema.json" \
    "$ROOT_DIR/schemas/error-v1.schema.json" \
    "$ROOT_DIR/schemas/capabilities-v1.schema.json" \
    "$ROOT_DIR/schemas/baseline-v1.schema.json" \
    serial.json \
    source.json \
    capabilities-1.json \
    baseline.json \
    <<'PY'
import copy
import json
import re
import sys
from pathlib import Path

from jsonschema import ValidationError
from jsonschema.validators import validator_for

report_schema_path = Path(sys.argv[1])
error_schema_path = Path(sys.argv[2])
capabilities_schema_path = Path(sys.argv[3])
baseline_schema_path = Path(sys.argv[4])
report_paths = [Path(sys.argv[5]), Path(sys.argv[6])]
capabilities_path = Path(sys.argv[7])
baseline_path = Path(sys.argv[8])


def load(path: Path):
    with path.open(encoding="utf-8") as file:
        return json.load(file)


def validator(schema_path: Path):
    schema = load(schema_path)
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema)


def require_rejected(instance, schema_validator, message: str):
    try:
        schema_validator.validate(instance)
    except ValidationError:
        return
    raise SystemExit(message)


report_validator = validator(report_schema_path)
error_validator = validator(error_schema_path)
capabilities_validator = validator(capabilities_schema_path)
baseline_validator = validator(baseline_schema_path)

reports = [load(path) for path in report_paths]
for report in reports:
    report_validator.validate(report)
    for error in report["errors"]:
        error_validator.validate(error)

serial = reports[0]
if serial["schema_version"] != 4:
    raise SystemExit("report does not declare schema version 4")
if not serial["complete"] or serial["errors"]:
    raise SystemExit("complete fixture scan is not complete and error-free")
if serial["duplicate_groups"] != 1 or len(serial["findings"]) != 1:
    raise SystemExit("fixture does not produce exactly one duplicate group")
if serial["files"] != 8:
    raise SystemExit(f"fixture reports {serial['files']} files instead of 8")
if serial["analysis"]["min_lines"] != 4:
    raise SystemExit("report analysis does not preserve --min-lines 4")
if serial["analysis"]["focus"]:
    raise SystemExit("unfocused fixture unexpectedly reports focus selectors")
if serial["analysis"]["virtual_source"] is not None:
    raise SystemExit("disk-only fixture unexpectedly reports a virtual source")
if serial["analysis"]["keep_going"]:
    raise SystemExit("normal fixture unexpectedly reports keep-going")

fingerprint = serial["findings"][0]["fingerprint"]
if re.fullmatch(r"arid-finding-v1:sha256:[0-9a-f]{64}", fingerprint) is None:
    raise SystemExit("finding fingerprint does not match the v1 contract")

invalid_report = copy.deepcopy(serial)
invalid_report["schema_version"] = 3
require_rejected(
    invalid_report,
    report_validator,
    "report-v4 schema accepted schema_version 3",
)

capabilities = load(capabilities_path)
capabilities_validator.validate(capabilities)
if capabilities["schema_version"] != 1:
    raise SystemExit("capabilities document does not declare schema version 1")
for required in (
    "focus",
    "keep-going",
    "multi-report",
    "no-fail-on-findings",
    "stdin-path",
    "workers-auto",
):
    if required not in capabilities["features"]:
        raise SystemExit(f"capabilities document is missing feature: {required}")

invalid_capabilities = copy.deepcopy(capabilities)
invalid_capabilities["schema_version"] = 2
require_rejected(
    invalid_capabilities,
    capabilities_validator,
    "capabilities-v1 schema accepted schema_version 2",
)

baseline = load(baseline_path)
baseline_validator.validate(baseline)
if baseline["version"] != 1:
    raise SystemExit("baseline does not preserve schema version 1")

invalid_baseline = copy.deepcopy(baseline)
invalid_baseline["version"] = 2
require_rejected(
    invalid_baseline,
    baseline_validator,
    "baseline-v1 schema accepted baseline version 2",
)
PY

pass "report-v4 JSON Schema and core contract"
pass "error-v1 JSON Schema"
pass "capabilities-v1 JSON Schema and contract"
pass "baseline-v1 JSON Schema compatibility"

echo
echo "V2 targeted integration validation foundation PASS"
echo "Arid: $ARID_VERSION"
echo "jsonschema: $JSONSCHEMA_VERSION"
