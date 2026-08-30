#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
JSONSCHEMA_VERSION="4.23.0"

usage() {
    cat <<'EOF'
Usage: validation/v2.2.sh <arid-bin>

Run the complete targeted Arid v2.2 integration validation suite against an
existing Arid executable. The inherited v2.1 suite runs first.
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

for command in cmp cp grep mktemp python3 realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -f "$ROOT_DIR/schemas/summary-v1.schema.json" ]] ||
    die "required schema not found: schemas/summary-v1.schema.json"
[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"
[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

"$SCRIPT_DIR/v2.1.sh" "$ARID_BIN"
echo

TMP_ROOT="$(mktemp -d)"
DUPES="$TMP_ROOT/dupes"
CLEAN="$TMP_ROOT/clean"
SCHEMA_VENV="$TMP_ROOT/schema-venv"

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
        [[ ! -s "$stdout_file" ]] || cat "$stdout_file" >&2
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        die "expected exit $expected, got $status: $*"
    }

    [[ ! -s "$stderr_file" ]] || {
        cat "$stderr_file" >&2
        die "unexpected stderr: $*"
    }
}

run_expect_error() {
    local stdout_file="$1"
    local stderr_file="$2"
    shift 2

    local status
    set +e
    "$@" >"$stdout_file" 2>"$stderr_file"
    status=$?
    set -e

    [[ "$status" -eq 2 ]] || {
        [[ ! -s "$stdout_file" ]] || cat "$stdout_file" >&2
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        die "expected exit 2, got $status: $*"
    }
}

assert_one_timing() {
    local path="$1"
    local count

    count="$(grep -c '^Total time: ' "$path" || true)"
    [[ "$count" -eq 1 ]] ||
        die "expected exactly one Total time footer in $path"
}

assert_no_timing() {
    local path="$1"
    ! grep -Fq 'Total time:' "$path" ||
        die "unexpected timing output in $path"
}

mkdir -p "$DUPES" "$CLEAN"

cat >"$DUPES/a.py" <<'PY'
alpha = 1
beta = 2
gamma = 3
delta = 4
PY
cp "$DUPES/a.py" "$DUPES/b.py"
cp "$DUPES/a.py" "$DUPES/c.py"

cat >"$CLEAN/a.py" <<'PY'
alpha = 1
beta = 2
PY
cat >"$CLEAN/b.py" <<'PY'
gamma = 3
delta = 4
PY

run_expect_status 1 "$TMP_ROOT/detailed.txt" "$TMP_ROOT/detailed.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 --color never
grep -Fq 'DUP001' "$TMP_ROOT/detailed.txt" || die "detailed text omitted findings"
grep -Fxq 'Summary' "$TMP_ROOT/detailed.txt" || die "detailed text omitted Summary"
grep -Fxq 'Breakdown' "$TMP_ROOT/detailed.txt" || die "detailed text omitted Breakdown"
grep -Fxq 'Hotspots' "$TMP_ROOT/detailed.txt" || die "detailed text omitted Hotspots"
assert_one_timing "$TMP_ROOT/detailed.txt"
pass "rich normal text summary"

run_expect_status 1 "$TMP_ROOT/summary-only.txt" "$TMP_ROOT/summary-only.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --color never
! grep -Fq 'DUP001' "$TMP_ROOT/summary-only.txt" || die "summary-only text emitted finding blocks"
grep -Fxq 'Summary' "$TMP_ROOT/summary-only.txt" || die "summary-only text omitted Summary"
grep -Fxq 'Breakdown' "$TMP_ROOT/summary-only.txt" || die "summary-only text omitted Breakdown"
grep -Fxq 'Hotspots' "$TMP_ROOT/summary-only.txt" || die "summary-only text omitted Hotspots"
assert_one_timing "$TMP_ROOT/summary-only.txt"
pass "summary-only text presentation"

run_expect_status 0 "$TMP_ROOT/clean-summary.txt" "$TMP_ROOT/clean-summary.stderr" \
    "$ARID_BIN" "$CLEAN" --no-config --project-root "$CLEAN" --min-lines 2 \
    --summary-only --color never
grep -Fq 'No duplicate code found.' "$TMP_ROOT/clean-summary.txt" ||
    die "clean summary-only text omitted success message"
grep -Fxq 'Summary' "$TMP_ROOT/clean-summary.txt" || die "clean summary-only text omitted Summary"
! grep -Fxq 'Breakdown' "$TMP_ROOT/clean-summary.txt" || die "clean summary unexpectedly rendered Breakdown"
! grep -Fxq 'Hotspots' "$TMP_ROOT/clean-summary.txt" || die "clean summary unexpectedly rendered Hotspots"
assert_one_timing "$TMP_ROOT/clean-summary.txt"
pass "zero-finding summary-only text"

run_expect_status 1 "$TMP_ROOT/summary.json" "$TMP_ROOT/summary.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --json
run_expect_status 1 "$TMP_ROOT/summary-format.json" "$TMP_ROOT/summary-format.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --format json
cmp -s "$TMP_ROOT/summary.json" "$TMP_ROOT/summary-format.json" ||
    die "--summary-only --json differs from --format json"
assert_no_timing "$TMP_ROOT/summary.json"

python3 - "$TMP_ROOT/summary.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["schema_version"] != 1:
    raise SystemExit("summary schema version changed")
if value["complete"] is not True:
    raise SystemExit("complete summary marked incomplete")
if value["files"] != 3 or value["files_with_duplicates"] != 3:
    raise SystemExit("summary file counts are incorrect")
if value["duplicate_groups"] != 1 or value["occurrences"] != 3:
    raise SystemExit("summary duplicate counts are incorrect")
if len(value["hotspots"]) != 3:
    raise SystemExit("summary hotspots omitted duplicate files")
PY
pass "summary-v1 primary JSON contract"

run_expect_status 1 "$TMP_ROOT/report-v4.json" "$TMP_ROOT/report-v4.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 --json
python3 - "$TMP_ROOT/report-v4.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["schema_version"] != 4:
    raise SystemExit("ordinary JSON no longer emits report-v4")
if len(value["findings"]) != value["duplicate_groups"]:
    raise SystemExit("report-v4 findings no longer match duplicate_groups")
PY
pass "ordinary JSON remains report-v4"

run_expect_status 1 "$TMP_ROOT/summary-with-report.txt" "$TMP_ROOT/summary-with-report.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --color never --report "json=$TMP_ROOT/full-report.json"
python3 - "$TMP_ROOT/full-report.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["schema_version"] != 4 or not value["findings"]:
    raise SystemExit("summary-only supplemental JSON is not a full report-v4")
PY
pass "summary-only preserves supplemental full report"

run_expect_status 1 "$TMP_ROOT/focus.json" "$TMP_ROOT/focus.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --focus "$DUPES/a.py" --summary-only --json
python3 - "$TMP_ROOT/focus.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["duplicate_groups"] != 1 or value["occurrences"] != 3:
    raise SystemExit("focus narrowed occurrence membership")
if value["files_with_duplicates"] != 3:
    raise SystemExit("focus removed outside-focus duplicate paths")
if {row["path"] for row in value["hotspots"]} != {"a.py", "b.py", "c.py"}:
    raise SystemExit("focus hotspots do not preserve full reported group")
PY
pass "focus summary preserves outside-focus occurrences"

run_expect_status 0 "$TMP_ROOT/baseline-write.txt" "$TMP_ROOT/baseline-write.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --write-baseline "$TMP_ROOT/baseline.json"
run_expect_status 0 "$TMP_ROOT/baseline-summary.json" "$TMP_ROOT/baseline-summary.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --baseline "$TMP_ROOT/baseline.json" --summary-only --json
python3 - "$TMP_ROOT/baseline-summary.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["duplicate_groups"] != 0 or value["occurrences"] != 0:
    raise SystemExit("accepted baseline debt leaked into summary")
if value["analysis"]["baseline_enabled"] is not True:
    raise SystemExit("summary analysis omitted effective baseline policy")
PY
pass "baseline filtering precedes summary derivation"

cat >"$DUPES/broken.py" <<'PY'
def broken(:
PY
run_expect_status 2 "$TMP_ROOT/incomplete.json" "$TMP_ROOT/incomplete.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --keep-going --summary-only --json
assert_no_timing "$TMP_ROOT/incomplete.json"
python3 - "$TMP_ROOT/incomplete.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["complete"] is not False or not value["errors"]:
    raise SystemExit("keep-going summary did not preserve incomplete state")
if value["analysis"]["keep_going"] is not True:
    raise SystemExit("keep-going summary omitted effective analysis policy")
PY
rm "$DUPES/broken.py"
pass "incomplete keep-going summary-v1"

run_expect_error "$TMP_ROOT/markdown.stdout" "$TMP_ROOT/markdown.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --format markdown
grep -Fq 'supports only text or JSON primary output' "$TMP_ROOT/markdown.stderr" ||
    die "summary-only Markdown rejection is unclear"
run_expect_error "$TMP_ROOT/sarif.stdout" "$TMP_ROOT/sarif.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --format sarif
grep -Fq 'supports only text or JSON primary output' "$TMP_ROOT/sarif.stderr" ||
    die "summary-only SARIF rejection is unclear"
pass "summary-only rejects Markdown and SARIF primary output"

run_expect_status 1 "$TMP_ROOT/plain.txt" "$TMP_ROOT/plain.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 --color never
run_expect_status 1 "$TMP_ROOT/color.txt" "$TMP_ROOT/color.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 --color always
python3 - "$TMP_ROOT/plain.txt" "$TMP_ROOT/color.txt" <<'PY'
import re
import sys
from pathlib import Path

ansi = re.compile(r"\x1b\[[0-9;]*m")

def stable_text(path: str) -> str:
    lines = Path(path).read_text(encoding="utf-8").splitlines()
    return "\n".join(line for line in lines if not line.startswith("Total time: "))

plain = stable_text(sys.argv[1])
colored = ansi.sub("", stable_text(sys.argv[2]))
if plain != colored:
    raise SystemExit("colored and plain visible text differ")
PY
pass "plain and colored visible summary parity"

run_expect_status 1 "$TMP_ROOT/implicit.json" "$TMP_ROOT/implicit.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --summary-only --json
run_expect_status 1 "$TMP_ROOT/auto.json" "$TMP_ROOT/auto.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --workers auto --summary-only --json
run_expect_status 1 "$TMP_ROOT/serial.json" "$TMP_ROOT/serial.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 4 \
    --workers 1 --summary-only --json
cmp -s "$TMP_ROOT/implicit.json" "$TMP_ROOT/auto.json" ||
    die "implicit worker mode differs from explicit auto summary output"
cmp -s "$TMP_ROOT/implicit.json" "$TMP_ROOT/serial.json" ||
    die "worker mode changed deterministic summary output"
"$ARID_BIN" --help >"$TMP_ROOT/help.txt"
grep -Fq '[default: auto]' "$TMP_ROOT/help.txt" ||
    die "published help does not document automatic worker default"
pass "adaptive worker default and deterministic summary output"

python3 -m venv "$SCHEMA_VENV"
"$SCHEMA_VENV/bin/python" \
    -m pip install \
    --disable-pip-version-check \
    --no-cache-dir \
    --quiet \
    "jsonschema==$JSONSCHEMA_VERSION"

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/summary-v1.schema.json" \
    "$TMP_ROOT/summary.json" \
    "$TMP_ROOT/incomplete.json" \
    "$TMP_ROOT/baseline-summary.json" \
    <<'PY'
import json
import sys
from pathlib import Path

from jsonschema.validators import validator_for

schema = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
validator_class = validator_for(schema)
validator_class.check_schema(schema)
validator = validator_class(schema)
for document_path in map(Path, sys.argv[2:]):
    validator.validate(json.loads(document_path.read_text(encoding="utf-8")))
PY
pass "summary-v1 complete/incomplete/zero schema validation"

echo
echo "V2.2 targeted integration validation PASS"
echo "Arid: $ARID_VERSION"
