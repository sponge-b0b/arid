#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
JSONSCHEMA_VERSION="4.23.0"

usage() {
    cat <<'EOF'
Usage: validation/v2.1.sh <arid-bin>

Run the complete targeted Arid v2.1 integration validation suite against an
existing Arid executable. The inherited v2 suite runs first.
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

for schema in suppression-status-v1.schema.json path-explanation-v1.schema.json; do
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

"$SCRIPT_DIR/v2.sh" "$ARID_BIN"
echo

TMP_ROOT="$(mktemp -d)"
CLEAN="$TMP_ROOT/clean"
DUPES="$TMP_ROOT/dupes"
ADMIN="$TMP_ROOT/admin"
DISCOVERY="$TMP_ROOT/discovery"
BASELINE_PROJECT="$TMP_ROOT/baseline"
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

assert_no_timing() {
    local path="$1"
    ! grep -Fq 'Total time:' "$path" ||
        die "unexpected timing output in $path"
}

assert_one_timing() {
    local path="$1"
    local count

    count="$(grep -c '^Total time: ' "$path" || true)"
    [[ "$count" -eq 1 ]] ||
        die "expected exactly one Total time footer in $path"

    grep -Eq '^Total time: [0-9]+([.][0-9]+)? (µs|ms|s)$' "$path" ||
        die "invalid Total time footer in $path"
}

mkdir -p "$CLEAN" "$DUPES" "$ADMIN" "$DISCOVERY/.git" "$BASELINE_PROJECT"

cat >"$CLEAN/a.py" <<'PY'
alpha = 1
beta = 2
PY
cat >"$CLEAN/b.py" <<'PY'
gamma = 3
delta = 4
PY

cat >"$DUPES/a.py" <<'PY'
alpha = 1
beta = 2
PY
cp "$DUPES/a.py" "$DUPES/b.py"

cat >"$ADMIN/a.py" <<'PY'
alpha = 1
beta = 2
PY
cat >"$ADMIN/b.py" <<'PY'
# arid: disable
alpha = 1
beta = 2
# arid: enable
PY
cat >"$ADMIN/c.py" <<'PY'
# arid: disable
gamma = 3
delta = 4
# arid: enable
PY
cat >"$ADMIN/eof.py" <<'PY'
# arid: enable
# arid: disable
# arid: disable
omega = 9
psi = 10
PY

mkdir -p "$DISCOVERY/src" "$DISCOVERY/generated" "$DISCOVERY/.hidden"
cat >"$DISCOVERY/src/visible.py" <<'PY'
def broken(:
PY
cat >"$DISCOVERY/generated/ignored.py" <<'PY'
pass
PY
cat >"$DISCOVERY/.hidden/hidden.py" <<'PY'
pass
PY
cat >"$DISCOVERY/.gitignore" <<'EOF'
generated/
EOF

cat >"$BASELINE_PROJECT/a.py" <<'PY'
alpha = 1
beta = 2
PY
cp "$BASELINE_PROJECT/a.py" "$BASELINE_PROJECT/b.py"

run_expect_status 0 "$TMP_ROOT/clean.txt" "$TMP_ROOT/clean.stderr" \
    "$ARID_BIN" "$CLEAN" --no-config --project-root "$CLEAN" --min-lines 2
assert_one_timing "$TMP_ROOT/clean.txt"
pass "normal text exit 0 timing footer"

run_expect_status 1 "$TMP_ROOT/dupes.txt" "$TMP_ROOT/dupes.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 2
assert_one_timing "$TMP_ROOT/dupes.txt"
pass "normal text exit 1 timing footer"

cat >"$CLEAN/broken.py" <<'PY'
def broken(:
PY
run_expect_error "$TMP_ROOT/error.stdout" "$TMP_ROOT/error.stderr" \
    "$ARID_BIN" "$CLEAN" --no-config --project-root "$CLEAN" --min-lines 2
assert_no_timing "$TMP_ROOT/error.stdout"
assert_no_timing "$TMP_ROOT/error.stderr"
rm "$CLEAN/broken.py"
pass "exit 2 remains timing-free"

run_expect_status 1 "$TMP_ROOT/report.json" "$TMP_ROOT/report.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 2 --json
assert_no_timing "$TMP_ROOT/report.json"
run_expect_status 1 "$TMP_ROOT/report.md" "$TMP_ROOT/markdown.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 2 --format markdown
assert_no_timing "$TMP_ROOT/report.md"
run_expect_status 1 "$TMP_ROOT/report.sarif" "$TMP_ROOT/sarif.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 2 --format sarif
assert_no_timing "$TMP_ROOT/report.sarif"
pass "machine-oriented primary formats remain timing-free"

run_expect_status 1 "$TMP_ROOT/multi.txt" "$TMP_ROOT/multi.stderr" \
    "$ARID_BIN" "$DUPES" --no-config --project-root "$DUPES" --min-lines 2 \
    --report "text=$TMP_ROOT/supplemental.txt" \
    --report "json=$TMP_ROOT/supplemental.json"
assert_one_timing "$TMP_ROOT/multi.txt"
assert_no_timing "$TMP_ROOT/supplemental.txt"
assert_no_timing "$TMP_ROOT/supplemental.json"
pass "supplemental normal reports remain timing-free"

run_expect_status 0 "$TMP_ROOT/suppression.json" "$TMP_ROOT/suppression.stderr" \
    "$ARID_BIN" "$ADMIN" --no-config --project-root "$ADMIN" --min-lines 2 \
    --suppression-status --json \
    --report "json=$TMP_ROOT/suppression-file.json"
cmp -s "$TMP_ROOT/suppression.json" "$TMP_ROOT/suppression-file.json" ||
    die "suppression-status JSON stdout differs from direct JSON file"
assert_no_timing "$TMP_ROOT/suppression.json"

python3 - "$TMP_ROOT/suppression.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["schema_version"] != 1:
    raise SystemExit("suppression status schema version changed")
if value["summary"]["active"] != 1:
    raise SystemExit("expected one active suppression")
if value["summary"]["stale"] != 2:
    raise SystemExit("expected two stale suppressions")
if not any(region["termination"] == "eof" for region in value["regions"]):
    raise SystemExit("disable-through-EOF suppression was not reported")
PY
pass "suppression active/stale and EOF lifecycle"

run_expect_status 1 "$TMP_ROOT/suppression-fail.txt" "$TMP_ROOT/suppression-fail.stderr" \
    "$ARID_BIN" "$ADMIN" --no-config --project-root "$ADMIN" --min-lines 2 \
    --suppression-status --fail-on-stale
assert_no_timing "$TMP_ROOT/suppression-fail.txt"
pass "suppression stale policy"

run_expect_status 0 "$TMP_ROOT/baseline-write.txt" "$TMP_ROOT/baseline-write.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" --no-config --project-root "$BASELINE_PROJECT" \
    --min-lines 2 --write-baseline "$TMP_ROOT/baseline.json"
rm "$BASELINE_PROJECT/b.py"
run_expect_status 1 "$TMP_ROOT/baseline-stale.txt" "$TMP_ROOT/baseline-stale.stderr" \
    "$ARID_BIN" "$BASELINE_PROJECT" --no-config --project-root "$BASELINE_PROJECT" \
    --min-lines 2 --baseline-status "$TMP_ROOT/baseline.json" --fail-on-stale
assert_no_timing "$TMP_ROOT/baseline-stale.txt"
pass "baseline stale policy"

run_expect_status 0 "$TMP_ROOT/ignored.json" "$TMP_ROOT/ignored.stderr" \
    "$ARID_BIN" "$DISCOVERY" --no-config --project-root "$DISCOVERY" \
    --explain-path "$DISCOVERY/generated/ignored.py" --json
python3 - "$TMP_ROOT/ignored.json" <<'PY'
import json
import sys
from pathlib import Path

value = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if value["decision"] != "exclude" or value["reasons"] != ["ignore-file"]:
    raise SystemExit("ignored path explanation does not report ignore-file exclusion")
PY

run_expect_status 0 "$TMP_ROOT/no-ignore.json" "$TMP_ROOT/no-ignore.stderr" \
    "$ARID_BIN" "$DISCOVERY" --no-config --project-root "$DISCOVERY" \
    --explain-path "$DISCOVERY/generated/ignored.py" --no-ignore-files --json \
    --report "json=$TMP_ROOT/path-file.json"
cmp -s "$TMP_ROOT/no-ignore.json" "$TMP_ROOT/path-file.json" ||
    die "path-explanation JSON stdout differs from direct JSON file"

run_expect_status 0 "$TMP_ROOT/list.txt" "$TMP_ROOT/list.stderr" \
    "$ARID_BIN" "$DISCOVERY" --no-config --project-root "$DISCOVERY" \
    --list-files --no-ignore-files
grep -Fxq 'generated/ignored.py' "$TMP_ROOT/list.txt" ||
    die "--no-ignore-files path explanation disagrees with --list-files"
if grep -Fxq '.hidden/hidden.py' "$TMP_ROOT/list.txt"; then
    die "--no-ignore-files unexpectedly bypassed hidden-path policy"
fi
pass "path explanation and no-ignore-files discovery parity"

run_expect_status 0 "$TMP_ROOT/broken-explain.json" "$TMP_ROOT/broken-explain.stderr" \
    "$ARID_BIN" "$DISCOVERY" --no-config --project-root "$DISCOVERY" \
    --explain-path "$DISCOVERY/src/visible.py" --json
assert_no_timing "$TMP_ROOT/broken-explain.json"
pass "path explanation stops before Python parsing"

run_expect_error "$TMP_ROOT/admin-report.stdout" "$TMP_ROOT/admin-report.stderr" \
    "$ARID_BIN" "$ADMIN" --no-config --project-root "$ADMIN" --min-lines 2 \
    --suppression-status --report "text=$TMP_ROOT/not-allowed.txt"
grep -Fq 'supports only json=PATH supplemental reports' "$TMP_ROOT/admin-report.stderr" ||
    die "non-JSON administrative report rejection is unclear"
pass "administrative supplemental reports are JSON-only"

python3 -m venv "$SCHEMA_VENV"
"$SCHEMA_VENV/bin/python" \
    -m pip install \
    --disable-pip-version-check \
    --no-cache-dir \
    --quiet \
    "jsonschema==$JSONSCHEMA_VERSION"

"$SCHEMA_VENV/bin/python" \
    - \
    "$ROOT_DIR/schemas/suppression-status-v1.schema.json" \
    "$ROOT_DIR/schemas/path-explanation-v1.schema.json" \
    "$TMP_ROOT/suppression.json" \
    "$TMP_ROOT/no-ignore.json" \
    <<'PY'
import json
import sys
from pathlib import Path

from jsonschema.validators import validator_for


def validate(schema_path: Path, document_path: Path) -> None:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    document = json.loads(document_path.read_text(encoding="utf-8"))
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    validator_class(schema).validate(document)


validate(Path(sys.argv[1]), Path(sys.argv[3]))
validate(Path(sys.argv[2]), Path(sys.argv[4]))
PY
pass "v2.1 administrative schema validation"

echo
echo "V2.1 targeted integration validation PASS"
echo "Arid: $ARID_VERSION"
