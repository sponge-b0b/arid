#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

usage() {
    cat <<'EOF'
Usage: validation/v1.1.sh <arid-bin>

Run focused Arid v1.1 integration validation against an existing Arid executable.
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

for command in cmp grep jq mktemp realpath; do
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

cat >pyproject.toml <<'EOF'
[tool.arid]
min-lines = 4
EOF

cat >a.py <<'EOF'
alpha = 1
beta = 2
gamma = 3
delta = 4
EOF

cp a.py b.py

run_expect_status() {
    local expected="$1"
    local output="$2"
    shift 2

    local status

    set +e
    "$@" >"$output"
    status=$?
    set -e

    [[ "$status" -eq "$expected" ]] ||
        die "expected exit $expected, got $status: $*"
}

assert_no_ansi() {
    local file="$1"

    if grep -Fq $'\033[' "$file"; then
        die "unexpected ANSI escape sequence in $file"
    fi
}

run_expect_status 1 text.txt "$ARID_BIN" .
grep -Fq 'DUP001 4 duplicated lines' text.txt ||
    die "default text output is missing DUP001 finding"
grep -Fq 'a.py:1-4' text.txt ||
    die "default text output is missing a.py location"
grep -Fq 'b.py:1-4' text.txt ||
    die "default text output is missing b.py location"
assert_no_ansi text.txt
pass "plain text"

run_expect_status 1 auto.txt "$ARID_BIN" . --format text --color auto
cmp -s text.txt auto.txt ||
    die "redirected auto-color text differs from default redirected text"
assert_no_ansi auto.txt
pass "redirected auto color"

run_expect_status 1 forced.txt "$ARID_BIN" . --format text --color always
grep -Fq $'\033[' forced.txt ||
    die "forced-color text contains no ANSI escape sequence"
pass "forced color"

run_expect_status 1 json-short.json "$ARID_BIN" . --json
run_expect_status 1 json-format.json "$ARID_BIN" . --format json
cmp -s json-short.json json-format.json ||
    die "--json and --format json differ"
jq -e '.version == 3 and .duplicate_groups == 1 and (.findings | length) == 1' \
    json-format.json >/dev/null ||
    die "JSON output does not match the expected schema-v3 finding shape"
assert_no_ansi json-format.json
pass "JSON compatibility"

run_expect_status 1 report.md "$ARID_BIN" . --format markdown
grep -Fq '# Arid duplicate-code report' report.md ||
    die "Markdown output is missing report heading"
grep -Fq '## `DUP001` — 4 duplicated lines' report.md ||
    die "Markdown output is missing DUP001 finding"
grep -Fq '### `a.py:1-4`' report.md ||
    die "Markdown output is missing a.py location"
assert_no_ansi report.md
pass "Markdown"

run_expect_status 1 report.sarif "$ARID_BIN" . --format sarif
jq -e '
    .version == "2.1.0"
    and .runs[0].tool.driver.name == "Arid"
    and (.runs[0].results | length) == 1
    and .runs[0].results[0].ruleId == "DUP001"
' report.sarif >/dev/null ||
    die "SARIF output does not match the expected Arid result shape"
assert_no_ansi report.sarif
pass "SARIF"

run_expect_status 0 baseline-write.txt \
    "$ARID_BIN" . --write-baseline arid-baseline.json
jq -e '.version == 1 and (.groups | length) == 1' \
    arid-baseline.json >/dev/null ||
    die "baseline creation did not produce the expected schema-v1 group"
pass "baseline creation"

cat >pyproject.toml <<'EOF'
[tool.arid]
min-lines = 4
baseline = "arid-baseline.json"
EOF

run_expect_status 0 baseline-clean.txt "$ARID_BIN" .
if grep -Fq 'DUP001' baseline-clean.txt; then
    die "unchanged baseline still reports an active DUP001 finding"
fi
pass "unchanged baseline enforcement"

cp a.py c.py

run_expect_status 1 baseline-new.txt "$ARID_BIN" .
grep -Fq 'DUP001 4 duplicated lines' baseline-new.txt ||
    die "new duplicate debt is missing DUP001 finding"
grep -Fq 'Occurrences: 3 across 3 files (cross-file)' baseline-new.txt ||
    die "new duplicate debt does not preserve the complete current group"
for path in a.py b.py c.py; do
    grep -Fq "$path:1-4" baseline-new.txt ||
        die "new duplicate debt is missing $path location"
done
pass "new duplicate against baseline"

echo
echo "V1.1 targeted integration validation PASS"
echo "Arid: $ARID_VERSION"
