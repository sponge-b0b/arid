#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

usage() {
    cat <<'EOF'
Usage: validation/v2-compatibility.sh <arid-bin>

Validate the inherited v1.1/v1.2 CLI behavior that remains part of the Arid v2
contract, excluding intentionally superseded report-v3 assertions.
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

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"
[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

TMP_ROOT="$(mktemp -d)"
PROJECT="$TMP_ROOT/project"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$PROJECT"
cd "$PROJECT"

cat >pyproject.toml <<'TOML'
[tool.arid]
min-lines = 4
TOML

cat >a.py <<'PY'
alpha = 1
beta = 2
gamma = 3
delta = 4
PY
cp a.py b.py

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

assert_no_ansi() {
    local path="$1"
    if grep -Fq $'\033[' "$path"; then
        die "unexpected ANSI escape sequence in $path"
    fi
}

run_expect_status 1 text.txt text.stderr "$ARID_BIN" .
grep -Fq 'DUP001 4 duplicated lines' text.txt ||
    die "default text output is missing DUP001 finding"
grep -Fq 'a.py:1-4' text.txt ||
    die "default text output is missing a.py location"
grep -Fq 'b.py:1-4' text.txt ||
    die "default text output is missing b.py location"
assert_no_ansi text.txt
pass "inherited plain-text diagnostics"

run_expect_status 1 auto.txt auto.stderr \
    "$ARID_BIN" . --format text --color auto
cmp -s text.txt auto.txt ||
    die "redirected auto-color text differs from default redirected text"
assert_no_ansi auto.txt

run_expect_status 1 forced.txt forced.stderr \
    "$ARID_BIN" . --format text --color always
grep -Fq $'\033[' forced.txt ||
    die "forced-color text contains no ANSI escape sequence"
pass "inherited redirected and forced color behavior"

run_expect_status 1 json-short.json json-short.stderr \
    "$ARID_BIN" . --json
run_expect_status 1 json-format.json json-format.stderr \
    "$ARID_BIN" . --format json
cmp -s json-short.json json-format.json ||
    die "--json and --format json differ"

python3 - json-format.json <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report["schema_version"] != 4:
    raise SystemExit("v2 JSON compatibility output is not report-v4")
if "version" in report:
    raise SystemExit("v2 JSON compatibility output retained obsolete report-v3 version field")
if report["duplicate_groups"] != 1 or len(report["findings"]) != 1:
    raise SystemExit("v2 JSON compatibility output does not contain the expected finding")
if report["findings"][0]["code"] != "DUP001":
    raise SystemExit("v2 JSON compatibility output changed the duplicate finding code")
PY
assert_no_ansi json-format.json
pass "inherited JSON flag equivalence with intentional report-v4 migration"

run_expect_status 1 report.md markdown.stderr \
    "$ARID_BIN" . --format markdown
grep -Fq '# Arid duplicate-code report' report.md ||
    die "Markdown output is missing report heading"
grep -Fq '## `DUP001` — 4 duplicated lines' report.md ||
    die "Markdown output is missing DUP001 finding"
grep -Fq '### `a.py:1-4`' report.md ||
    die "Markdown output is missing a.py location"
assert_no_ansi report.md
pass "inherited Markdown diagnostics"

run_expect_status 1 report.sarif sarif.stderr \
    "$ARID_BIN" . --format sarif
python3 - report.sarif <<'PY'
import json
import sys
from pathlib import Path

sarif = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if sarif["version"] != "2.1.0":
    raise SystemExit("SARIF version changed from 2.1.0")
if sarif["runs"][0]["tool"]["driver"]["name"] != "Arid":
    raise SystemExit("SARIF tool identity changed")
results = sarif["runs"][0]["results"]
if len(results) != 1 or results[0]["ruleId"] != "DUP001":
    raise SystemExit("SARIF no longer contains the expected DUP001 result")
PY
assert_no_ansi report.sarif
pass "inherited SARIF result contract"

run_expect_status 0 baseline-write.txt baseline-write.stderr \
    "$ARID_BIN" . --write-baseline arid-baseline.json
python3 - arid-baseline.json <<'PY'
import json
import sys
from pathlib import Path

baseline = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if baseline["version"] != 1 or len(baseline["groups"]) != 1:
    raise SystemExit("baseline creation no longer produces the expected baseline-v1 group")
PY
pass "inherited baseline creation"

cat >pyproject.toml <<'TOML'
[tool.arid]
min-lines = 4
baseline = "arid-baseline.json"
TOML

run_expect_status 0 baseline-clean.txt baseline-clean.stderr "$ARID_BIN" .
if grep -Fq 'DUP001' baseline-clean.txt; then
    die "unchanged baseline still reports an active DUP001 finding"
fi
pass "inherited unchanged-baseline enforcement"

cp a.py c.py
run_expect_status 1 baseline-new.txt baseline-new.stderr "$ARID_BIN" .
grep -Fq 'DUP001 4 duplicated lines' baseline-new.txt ||
    die "new duplicate debt is missing DUP001 finding"
grep -Fq 'Occurrences: 3 across 3 files (cross-file)' baseline-new.txt ||
    die "new duplicate debt does not preserve the complete current group"
for path in a.py b.py c.py; do
    grep -Fq "$path:1-4" baseline-new.txt ||
        die "new duplicate debt is missing $path location"
done
pass "inherited new-debt baseline behavior"

echo
echo "V2 inherited compatibility validation PASS"
echo "Arid: $ARID_VERSION"
