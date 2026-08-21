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

write_duplicate_source() {
    local path="$1"

    cat >"$path" <<'PY'
def duplicated_calculation():
    alpha = 1
    beta = 2
    gamma = alpha + beta
    delta = gamma * 2
    return delta
PY
}

write_duplicate_source file-1.py

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

run_with_stdin_expect_status() {
    local expected="$1"
    local stdin_file="$2"
    local stdout_file="$3"
    local stderr_file="$4"
    shift 4

    local status

    set +e
    "$@" <"$stdin_file" >"$stdout_file" 2>"$stderr_file"
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

FOCUS_PROJECT="$TMP_ROOT/focus-project"
mkdir -p "$FOCUS_PROJECT"
for path in a.py b.py c.py; do
    write_duplicate_source "$FOCUS_PROJECT/$path"
done

run_expect_status 1 \
    "$TMP_ROOT/focus.json" \
    "$TMP_ROOT/focus.stderr" \
    "$ARID_BIN" "$FOCUS_PROJECT" \
    --no-config \
    --project-root "$FOCUS_PROJECT" \
    --min-lines 4 \
    --focus b.py \
    --json

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/report-v4.schema.json" \
    "$TMP_ROOT/focus.json" \
    <<'PY'
import json
import sys
from pathlib import Path

from jsonschema.validators import validator_for

schema = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
report = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
validator_class = validator_for(schema)
validator_class.check_schema(schema)
validator_class(schema).validate(report)

if report["analysis"]["focus"] != ["b.py"]:
    raise SystemExit("focus metadata is not canonical")
if report["files"] != 3 or report["duplicate_groups"] != 1:
    raise SystemExit("focused scan did not analyze the whole three-file corpus")
locations = report["findings"][0]["locations"]
if [location["path"] for location in locations] != ["a.py", "b.py", "c.py"]:
    raise SystemExit("focused finding did not retain complete group context")
PY
pass "focus preserves whole-project detection and complete group context"

BASELINE_PROJECT="$TMP_ROOT/focus-baseline-project"
mkdir -p "$BASELINE_PROJECT"
write_duplicate_source "$BASELINE_PROJECT/a.py"
write_duplicate_source "$BASELINE_PROJECT/b.py"

run_expect_status 0 \
    "$TMP_ROOT/focus-baseline-write.stdout" \
    "$TMP_ROOT/focus-baseline-write.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --write-baseline "$BASELINE_PROJECT/baseline.json"

write_duplicate_source "$BASELINE_PROJECT/c.py"

run_expect_status 1 \
    "$TMP_ROOT/focus-baseline.json" \
    "$TMP_ROOT/focus-baseline.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --baseline "$BASELINE_PROJECT/baseline.json" \
    --focus a.py \
    --json

"$SCHEMA_VENV/bin/python" \
    - \
    "$TMP_ROOT/focus-baseline.json" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if not report["analysis"]["baseline_enabled"]:
    raise SystemExit("focused baseline scan does not record baseline use")
if report["analysis"]["focus"] != ["a.py"]:
    raise SystemExit("focused baseline scan lost focus metadata")
if report["duplicate_groups"] != 1:
    raise SystemExit("new debt was incorrectly removed before focus filtering")
locations = report["findings"][0]["locations"]
if [location["path"] for location in locations] != ["a.py", "b.py", "c.py"]:
    raise SystemExit("focus+baseline did not preserve the complete active group")
PY
pass "baseline enforcement precedes focus filtering"

VIRTUAL_PROJECT="$TMP_ROOT/focus-virtual-project"
mkdir -p "$VIRTUAL_PROJECT"
write_duplicate_source "$VIRTUAL_PROJECT/a.py"
write_duplicate_source "$TMP_ROOT/virtual-input.py"

run_with_stdin_expect_status 1 \
    "$TMP_ROOT/virtual-input.py" \
    "$TMP_ROOT/focus-virtual.json" \
    "$TMP_ROOT/focus-virtual.stderr" \
    "$ARID_BIN" "$VIRTUAL_PROJECT" \
    --no-config \
    --project-root "$VIRTUAL_PROJECT" \
    --min-lines 4 \
    --stdin-path proposed.py \
    --focus proposed.py \
    --json

[[ ! -e "$VIRTUAL_PROJECT/proposed.py" ]] ||
    die "virtual source was written to disk"

"$SCHEMA_VENV/bin/python" \
    - \
    "$TMP_ROOT/focus-virtual.json" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report["analysis"]["focus"] != ["proposed.py"]:
    raise SystemExit("virtual focus selector was not preserved")
if report["analysis"]["virtual_source"] != "proposed.py":
    raise SystemExit("virtual source metadata is incorrect")
if report["files"] != 2 or report["duplicate_groups"] != 1:
    raise SystemExit("virtual focused scan did not analyze both sources")
locations = report["findings"][0]["locations"]
if [location["path"] for location in locations] != ["a.py", "proposed.py"]:
    raise SystemExit("virtual focused finding has incorrect locations")
PY
pass "focus composes with virtual source without disk mutation"

KEEP_GOING_PROJECT="$TMP_ROOT/keep-going-project"
mkdir -p "$KEEP_GOING_PROJECT"
write_duplicate_source "$KEEP_GOING_PROJECT/a.py"
write_duplicate_source "$KEEP_GOING_PROJECT/b.py"
cat >"$KEEP_GOING_PROJECT/bad-a.py" <<'PY'
def broken(:
    pass
PY
cat >"$KEEP_GOING_PROJECT/bad-b.py" <<'PY'
if True print("broken")
PY

run_expect_status 2 \
    "$TMP_ROOT/keep-going-serial.json" \
    "$TMP_ROOT/keep-going-serial.stderr" \
    "$ARID_BIN" "$KEEP_GOING_PROJECT" \
    --no-config \
    --project-root "$KEEP_GOING_PROJECT" \
    --min-lines 4 \
    --keep-going \
    --workers 1 \
    --json

run_expect_status 2 \
    "$TMP_ROOT/keep-going-auto.json" \
    "$TMP_ROOT/keep-going-auto.stderr" \
    "$ARID_BIN" "$KEEP_GOING_PROJECT" \
    --no-config \
    --project-root "$KEEP_GOING_PROJECT" \
    --min-lines 4 \
    --keep-going \
    --workers auto \
    --json

cmp -s "$TMP_ROOT/keep-going-serial.json" "$TMP_ROOT/keep-going-auto.json" ||
    die "keep-going JSON errors differ between serial and auto workers"

run_expect_status 2 \
    "$TMP_ROOT/keep-going-no-fail.json" \
    "$TMP_ROOT/keep-going-no-fail.stderr" \
    "$ARID_BIN" "$KEEP_GOING_PROJECT" \
    --no-config \
    --project-root "$KEEP_GOING_PROJECT" \
    --min-lines 4 \
    --keep-going \
    --no-fail-on-findings \
    --workers 1 \
    --json

cmp -s "$TMP_ROOT/keep-going-serial.json" "$TMP_ROOT/keep-going-no-fail.json" ||
    die "--no-fail-on-findings changed an incomplete report"

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/report-v4.schema.json" \
    "$ROOT_DIR/schemas/error-v1.schema.json" \
    "$TMP_ROOT/keep-going-serial.json" \
    <<'PY'
import json
import sys
from pathlib import Path

from jsonschema.validators import validator_for

report_schema = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
error_schema = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
report = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))

report_validator_class = validator_for(report_schema)
report_validator_class.check_schema(report_schema)
report_validator_class(report_schema).validate(report)
error_validator_class = validator_for(error_schema)
error_validator_class.check_schema(error_schema)
error_validator = error_validator_class(error_schema)

if report["complete"]:
    raise SystemExit("keep-going partial scan incorrectly reports complete=true")
if not report["analysis"]["keep_going"]:
    raise SystemExit("keep-going analysis metadata is false")
if report["files"] != 2 or report["duplicate_groups"] != 1:
    raise SystemExit("keep-going did not retain the two valid duplicate sources")
if len(report["errors"]) != 2:
    raise SystemExit("keep-going did not report both malformed sources")
for error in report["errors"]:
    error_validator.validate(error)
if [error.get("path") for error in report["errors"]] != ["bad-a.py", "bad-b.py"]:
    raise SystemExit("keep-going errors are not deterministically path ordered")
if any(error["kind"] != "parse" for error in report["errors"]):
    raise SystemExit("malformed-source keep-going errors are not parse errors")
PY
pass "keep-going preserves findings with deterministic incomplete errors"
pass "no-fail cannot mask incomplete scan exit 2"

run_expect_status 2 \
    "$TMP_ROOT/fatal-error.json" \
    "$TMP_ROOT/fatal-error.stderr" \
    "$ARID_BIN" "$FOCUS_PROJECT" \
    --no-config \
    --project-root "$FOCUS_PROJECT" \
    --min-lines 4 \
    --focus missing.py \
    --json

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/error-v1.schema.json" \
    "$TMP_ROOT/fatal-error.json" \
    <<'PY'
import json
import sys
from pathlib import Path

from jsonschema.validators import validator_for

schema = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
document = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
validator_class = validator_for(schema)
validator_class.check_schema(schema)
validator_class(schema).validate(document)

if document["schema_version"] != 1:
    raise SystemExit("fatal JSON error does not declare error schema v1")
if document["error"]["kind"] != "configuration":
    raise SystemExit("unmatched focus is not a configuration error")
if "missing.py" not in document["error"]["message"]:
    raise SystemExit("fatal JSON error does not identify the unmatched focus")
PY
pass "fatal JSON operational-error document"

run_expect_status 1 \
    "$TMP_ROOT/exit-policy-normal.json" \
    "$TMP_ROOT/exit-policy-normal.stderr" \
    "$ARID_BIN" "$FOCUS_PROJECT" \
    --no-config \
    --project-root "$FOCUS_PROJECT" \
    --min-lines 4 \
    --json

run_expect_status 0 \
    "$TMP_ROOT/exit-policy-no-fail.json" \
    "$TMP_ROOT/exit-policy-no-fail.stderr" \
    "$ARID_BIN" "$FOCUS_PROJECT" \
    --no-config \
    --project-root "$FOCUS_PROJECT" \
    --min-lines 4 \
    --no-fail-on-findings \
    --json

cmp -s "$TMP_ROOT/exit-policy-normal.json" "$TMP_ROOT/exit-policy-no-fail.json" ||
    die "--no-fail-on-findings changed report content"
pass "no-fail maps findings-only exit 1 to 0 without changing report"

echo
echo "V2 targeted integration validation composition PASS"
echo "Arid: $ARID_VERSION"
echo "jsonschema: $JSONSCHEMA_VERSION"
