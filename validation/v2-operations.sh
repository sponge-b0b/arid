#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

usage() {
    cat <<'EOF'
Usage: validation/v2-operations.sh <arid-bin>

Validate Arid v2 baseline administration and multi-output behavior.
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

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"

[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

TMP_ROOT="$(mktemp -d)"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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

assert_json_equal() {
    local left="$1"
    local right="$2"
    local message="$3"

    python3 - "$left" "$right" "$message" <<'PY'
import json
import sys
from pathlib import Path

left = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
right = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
if left != right:
    raise SystemExit(sys.argv[3])
PY
}

BASELINE_PROJECT="$TMP_ROOT/baseline-project"
mkdir -p "$BASELINE_PROJECT"
write_duplicate_source "$BASELINE_PROJECT/a.py"
write_duplicate_source "$BASELINE_PROJECT/b.py"

run_expect_status 0 \
    "$TMP_ROOT/baseline-write.stdout" \
    "$TMP_ROOT/baseline-write.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --write-baseline "$BASELINE_PROJECT/baseline.json"

[[ ! -s "$TMP_ROOT/baseline-write.stdout" ]] ||
    die "--write-baseline unexpectedly wrote stdout"

run_expect_status 0 \
    "$TMP_ROOT/baseline-status-initial.json" \
    "$TMP_ROOT/baseline-status-initial.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --baseline-status "$BASELINE_PROJECT/baseline.json" \
    --json

rm "$BASELINE_PROJECT/b.py"
write_duplicate_source "$BASELINE_PROJECT/c.py"

run_expect_status 1 \
    "$TMP_ROOT/baseline-status-active-1.json" \
    "$TMP_ROOT/baseline-status-active-1.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --baseline-status "$BASELINE_PROJECT/baseline.json" \
    --json

run_expect_status 1 \
    "$TMP_ROOT/baseline-status-active-2.json" \
    "$TMP_ROOT/baseline-status-active-2.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --baseline-status "$BASELINE_PROJECT/baseline.json" \
    --json

cmp -s \
    "$TMP_ROOT/baseline-status-active-1.json" \
    "$TMP_ROOT/baseline-status-active-2.json" ||
    die "baseline status JSON is not deterministic"

cp "$BASELINE_PROJECT/baseline.json" "$TMP_ROOT/baseline-before-prune.json"

run_expect_status 1 \
    "$TMP_ROOT/baseline-prune.json" \
    "$TMP_ROOT/baseline-prune.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --prune-baseline "$BASELINE_PROJECT/baseline.json" \
    --json

cmp -s "$TMP_ROOT/baseline-before-prune.json" "$BASELINE_PROJECT/baseline.json" &&
    die "--prune-baseline did not remove stale baseline debt"

run_expect_status 1 \
    "$TMP_ROOT/baseline-status-pruned.json" \
    "$TMP_ROOT/baseline-status-pruned.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --baseline-status "$BASELINE_PROJECT/baseline.json" \
    --json

cp "$BASELINE_PROJECT/baseline.json" "$TMP_ROOT/baseline-before-noop-prune.json"

run_expect_status 1 \
    "$TMP_ROOT/baseline-noop-prune.json" \
    "$TMP_ROOT/baseline-noop-prune.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" \
    --no-config \
    --project-root "$BASELINE_PROJECT" \
    --min-lines 4 \
    --prune-baseline "$BASELINE_PROJECT/baseline.json" \
    --json

cmp -s "$TMP_ROOT/baseline-before-noop-prune.json" "$BASELINE_PROJECT/baseline.json" ||
    die "--prune-baseline rewrote an already-pruned baseline"

python3 - \
    "$TMP_ROOT/baseline-status-initial.json" \
    "$TMP_ROOT/baseline-status-active-1.json" \
    "$TMP_ROOT/baseline-status-pruned.json" \
    "$BASELINE_PROJECT/baseline.json" \
    <<'PY'
import json
import sys
from pathlib import Path

initial = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
active = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
pruned = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
baseline = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))

if initial["schema_version"] != 1:
    raise SystemExit("baseline status does not declare schema version 1")
if initial["summary"] != {"accepted": 2, "active": 0, "stale": 0}:
    raise SystemExit(f"unexpected initial baseline status: {initial['summary']}")
if active["summary"] != {"accepted": 1, "active": 1, "stale": 1}:
    raise SystemExit(f"unexpected active/stale baseline status: {active['summary']}")
if pruned["summary"] != {"accepted": 1, "active": 1, "stale": 0}:
    raise SystemExit(f"unexpected post-prune baseline status: {pruned['summary']}")
if baseline["version"] != 1:
    raise SystemExit("pruning changed the baseline schema version")
paths = [
    occurrence["path"]
    for group in baseline["groups"]
    for occurrence in group["occurrences"]
]
if paths != ["a.py"]:
    raise SystemExit(f"pruned baseline retained unexpected paths: {paths}")
PY

pass "baseline status reports accepted/active/stale debt deterministically"
pass "baseline prune removes only stale debt and is idempotent"

OUTPUT_PROJECT="$TMP_ROOT/output-project"
ARTIFACTS="$OUTPUT_PROJECT/artifacts"
mkdir -p "$ARTIFACTS"
write_duplicate_source "$OUTPUT_PROJECT/a.py"
write_duplicate_source "$OUTPUT_PROJECT/b.py"

run_expect_status 1 \
    "$TMP_ROOT/multi-output.json" \
    "$TMP_ROOT/multi-output.stderr" \
    "$ARID_BIN" "$OUTPUT_PROJECT" \
    --no-config \
    --project-root "$OUTPUT_PROJECT" \
    --min-lines 4 \
    --json \
    --report "json=$ARTIFACTS/arid.json" \
    --report "markdown=$ARTIFACTS/arid.md" \
    --report "sarif=$ARTIFACTS/arid.sarif" \
    --report "text=$ARTIFACTS/arid.txt"

assert_json_equal \
    "$TMP_ROOT/multi-output.json" \
    "$ARTIFACTS/arid.json" \
    "supplemental JSON does not match primary JSON output"

python3 - \
    "$ARTIFACTS/arid.json" \
    "$ARTIFACTS/arid.md" \
    "$ARTIFACTS/arid.sarif" \
    "$ARTIFACTS/arid.txt" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
markdown = Path(sys.argv[2]).read_text(encoding="utf-8")
sarif = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
text = Path(sys.argv[4]).read_text(encoding="utf-8")

if report["schema_version"] != 4 or report["duplicate_groups"] != 1:
    raise SystemExit("supplemental JSON does not describe the expected report-v4 finding")
if not markdown.startswith("# Arid duplicate-code report\n\n") or "DUP001" not in markdown:
    raise SystemExit("supplemental Markdown does not represent the finding")
if sarif["version"] != "2.1.0":
    raise SystemExit("supplemental SARIF does not declare SARIF 2.1.0")
if sarif["runs"][0]["results"][0]["ruleId"] != "DUP001":
    raise SystemExit("supplemental SARIF does not represent DUP001")
if "DUP001" not in text or "\x1b" in text:
    raise SystemExit("supplemental text is missing DUP001 or contains ANSI escapes")
PY

pass "multi-output renders JSON/Markdown/SARIF/text from one report"

cat >"$OUTPUT_PROJECT/broken.py" <<'PY'
def broken(:
    pass
PY
rm -f "$ARTIFACTS/partial.json" "$ARTIFACTS/partial.sarif"

run_expect_status 2 \
    "$TMP_ROOT/partial.json" \
    "$TMP_ROOT/partial.stderr" \
    "$ARID_BIN" "$OUTPUT_PROJECT" \
    --no-config \
    --project-root "$OUTPUT_PROJECT" \
    --min-lines 4 \
    --keep-going \
    --json \
    --report "json=$ARTIFACTS/partial.json" \
    --report "sarif=$ARTIFACTS/partial.sarif"

assert_json_equal \
    "$TMP_ROOT/partial.json" \
    "$ARTIFACTS/partial.json" \
    "partial supplemental JSON does not match primary JSON output"
[[ ! -e "$ARTIFACTS/partial.sarif" ]] ||
    die "SARIF was written for an incomplete scan"

python3 - "$ARTIFACTS/partial.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report["complete"]:
    raise SystemExit("partial multi-output report incorrectly declares complete=true")
if not report["errors"]:
    raise SystemExit("partial multi-output report does not contain preparation errors")
PY

pass "incomplete multi-output writes partial JSON and suppresses SARIF"

cp "$OUTPUT_PROJECT/a.py" "$TMP_ROOT/a-original.py"
run_expect_status 2 \
    "$TMP_ROOT/source-overlap-error.json" \
    "$TMP_ROOT/source-overlap-error.stderr" \
    "$ARID_BIN" "$OUTPUT_PROJECT" \
    --no-config \
    --project-root "$OUTPUT_PROJECT" \
    --min-lines 4 \
    --json \
    --report "json=$OUTPUT_PROJECT/a.py"

cmp -s "$TMP_ROOT/a-original.py" "$OUTPUT_PROJECT/a.py" ||
    die "report destination overlap modified a source file"

run_expect_status 2 \
    "$TMP_ROOT/write-failure-error.json" \
    "$TMP_ROOT/write-failure-error.stderr" \
    "$ARID_BIN" "$OUTPUT_PROJECT" \
    --no-config \
    --project-root "$OUTPUT_PROJECT" \
    --min-lines 4 \
    --json \
    --report "json=$OUTPUT_PROJECT/missing/report.json"

[[ ! -d "$OUTPUT_PROJECT/missing" ]] ||
    die "report write failure unexpectedly created the missing parent directory"

python3 - \
    "$TMP_ROOT/source-overlap-error.json" \
    "$TMP_ROOT/write-failure-error.json" \
    <<'PY'
import json
import sys
from pathlib import Path

overlap = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
write_failure = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))

if overlap["schema_version"] != 1 or overlap["error"]["kind"] != "configuration":
    raise SystemExit("source-overlap failure is not a fatal configuration error document")
if "overlaps a source file" not in overlap["error"]["message"]:
    raise SystemExit("source-overlap failure does not explain the conflict")
if write_failure["schema_version"] != 1 or write_failure["error"]["kind"] != "output":
    raise SystemExit("report write failure is not a fatal output error document")
if "failed to open report destination" not in write_failure["error"]["message"]:
    raise SystemExit("report write failure does not explain the destination error")
PY

pass "multi-output rejects source overlap without mutation"
pass "multi-output surfaces deterministic report write failures"

echo
echo "V2 baseline/output operational validation PASS"
echo "Arid: $ARID_VERSION"
