#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS_RELATIVE_PATH="validation/arid-corpora"
DJANGO_INVALID_SOURCE="tests/test_runner_apps/tagged/tests_syntax_error.py"
ARID_BIN_INPUT=""

usage() {
    cat <<EOF
Usage: $0 <global-root> [--arid-bin <path>]

Exercise Arid v2 multi-output behavior against the real-world Django corpus.
One scan must produce primary report-v4 JSON plus supplemental JSON, Markdown,
SARIF, and text reports describing the same complete finding set.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

progress() {
    printf '\n==> %s\n' "$1"
}

GLOBAL_ROOT_INPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arid-bin)
            [[ $# -ge 2 ]] || die "--arid-bin requires a value"
            [[ -z "$ARID_BIN_INPUT" ]] || die "duplicate option: --arid-bin"
            ARID_BIN_INPUT="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        -*)
            die "unknown option: $1"
            ;;
        *)
            [[ -z "$GLOBAL_ROOT_INPUT" ]] || die "unexpected argument: $1"
            GLOBAL_ROOT_INPUT="$1"
            shift
            ;;
    esac
done

[[ -n "$GLOBAL_ROOT_INPUT" ]] || {
    usage
    exit 2
}

for command in awk cmp date git jq mktemp python3 realpath sha256sum sleep; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -d "$GLOBAL_ROOT_INPUT" ]] ||
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"
DJANGO_ROOT="$CORPUS_ROOT/django"
RESULTS_DIR="$ROOT_DIR/validation/results/v2-real-world/django-multi-output"
ARTIFACTS_DIR="$RESULTS_DIR/artifacts"
TMP_ROOT="$(mktemp -d)"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

[[ -d "$DJANGO_ROOT" ]] ||
    die "Django validation corpus does not exist: $DJANGO_ROOT"

git -C "$DJANGO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    die "Django corpus is not a Git repository: $DJANGO_ROOT"
[[ "$(realpath "$(git -C "$DJANGO_ROOT" rev-parse --show-toplevel)")" == "$DJANGO_ROOT" ]] ||
    die "Django corpus must be the Git repository root: $DJANGO_ROOT"
[[ -z "$(git -C "$DJANGO_ROOT" status --porcelain --untracked-files=all)" ]] ||
    die "Django corpus working tree is not clean: $DJANGO_ROOT"
[[ -f "$DJANGO_ROOT/$DJANGO_INVALID_SOURCE" ]] ||
    die "expected Django syntax-error fixture does not exist: $DJANGO_INVALID_SOURCE"

if [[ -n "$ARID_BIN_INPUT" ]]; then
    [[ -f "$ARID_BIN_INPUT" ]] ||
        die "Arid executable does not exist: $ARID_BIN_INPUT"
    [[ -x "$ARID_BIN_INPUT" ]] ||
        die "Arid executable is not executable: $ARID_BIN_INPUT"

    ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
    ARID_SOURCE="external"
else
    command -v cargo >/dev/null 2>&1 ||
        die "required command not found: cargo"

    echo "Building Arid release binary..."
    cargo build \
        --release \
        --locked \
        --manifest-path "$ROOT_DIR/Cargo.toml"

    ARID_BIN="$ROOT_DIR/target/release/arid"
    ARID_SOURCE="repository"
fi

ARID_VERSION="$("$ARID_BIN" --version)"
[[ "$ARID_VERSION" == arid\ 2.0.0-* ]] ||
    die "executable must identify itself as an Arid 2.0.0 prerelease, got: $ARID_VERSION"
ARID_SHA256="$(sha256sum "$ARID_BIN" | awk '{print $1}')"

rm -rf "$RESULTS_DIR"
mkdir -p "$ARTIFACTS_DIR"

PRIMARY_JSON="$RESULTS_DIR/primary.json"
STDERR_FILE="$RESULTS_DIR/scan.stderr"
REPORT_JSON="$ARTIFACTS_DIR/arid.json"
REPORT_MARKDOWN="$ARTIFACTS_DIR/arid.md"
REPORT_SARIF="$ARTIFACTS_DIR/arid.sarif"
REPORT_TEXT="$ARTIFACTS_DIR/arid.txt"

echo "Arid:    $ARID_VERSION"
echo "Django:  $(git -C "$DJANGO_ROOT" rev-parse HEAD)"
echo "Workers: auto"
echo "Results: $RESULTS_DIR"

wait_with_activity() {
    local pid="$1"
    local started="$SECONDS"
    local next_report=5
    local elapsed

    while kill -0 "$pid" 2>/dev/null; do
        sleep 1
        elapsed="$((SECONDS - started))"

        if [[ "$elapsed" -ge "$next_report" ]] && kill -0 "$pid" 2>/dev/null; then
            printf '    still running (%ss elapsed)\n' "$elapsed"
            next_report="$((next_report + 5))"
        fi
    done
}

progress "Running one Django scan with JSON/Markdown/SARIF/text outputs"

set +e
"$ARID_BIN" "$DJANGO_ROOT" \
    --no-config \
    --project-root "$DJANGO_ROOT" \
    --hidden \
    --workers auto \
    --exclude "$DJANGO_INVALID_SOURCE" \
    --json \
    --report "json=$REPORT_JSON" \
    --report "markdown=$REPORT_MARKDOWN" \
    --report "sarif=$REPORT_SARIF" \
    --report "text=$REPORT_TEXT" \
    >"$PRIMARY_JSON" 2>"$STDERR_FILE" &
SCAN_PID=$!
wait_with_activity "$SCAN_PID"
wait "$SCAN_PID"
STATUS=$?
set -e

[[ "$STATUS" -eq 1 ]] || {
    echo "error: expected Django findings exit 1, got $STATUS" >&2
    [[ ! -s "$PRIMARY_JSON" ]] || cat "$PRIMARY_JSON" >&2
    [[ ! -s "$STDERR_FILE" ]] || cat "$STDERR_FILE" >&2
    exit 2
}

[[ ! -s "$STDERR_FILE" ]] || {
    cat "$STDERR_FILE" >&2
    die "Django multi-output scan produced unexpected stderr"
}

for artifact in \
    "$PRIMARY_JSON" \
    "$REPORT_JSON" \
    "$REPORT_MARKDOWN" \
    "$REPORT_SARIF" \
    "$REPORT_TEXT"
do
    [[ -s "$artifact" ]] || die "expected non-empty report artifact: $artifact"
done

jq -e --arg excluded "$DJANGO_INVALID_SOURCE" '
    .schema_version == 4
    and .complete == true
    and .errors == []
    and .duplicate_groups > 0
    and (.findings | length) == .duplicate_groups
    and (.analysis.exclude | index($excluded)) != null
' "$PRIMARY_JSON" >/dev/null ||
    die "primary Django report is not the expected complete report-v4"

jq -S . "$PRIMARY_JSON" >"$TMP_ROOT/primary-canonical.json"
jq -S . "$REPORT_JSON" >"$TMP_ROOT/supplemental-canonical.json"
cmp -s "$TMP_ROOT/primary-canonical.json" "$TMP_ROOT/supplemental-canonical.json" ||
    die "supplemental JSON differs logically from primary report-v4 JSON"

python3 - \
    "$PRIMARY_JSON" \
    "$REPORT_MARKDOWN" \
    "$REPORT_SARIF" \
    "$REPORT_TEXT" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
markdown = Path(sys.argv[2]).read_text(encoding="utf-8")
sarif = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
text = Path(sys.argv[4]).read_text(encoding="utf-8")

if not markdown.startswith("# Arid duplicate-code report\n\n"):
    raise SystemExit("Markdown report has the wrong document header")
if "DUP001" not in markdown:
    raise SystemExit("Markdown report does not represent Django findings")

if sarif.get("version") != "2.1.0":
    raise SystemExit("SARIF report does not declare SARIF 2.1.0")
runs = sarif.get("runs")
if not isinstance(runs, list) or len(runs) != 1:
    raise SystemExit("SARIF report does not contain exactly one run")
results = runs[0].get("results")
if not isinstance(results, list) or len(results) != report["duplicate_groups"]:
    raise SystemExit(
        "SARIF result count does not match report-v4 duplicate_groups: "
        f"{len(results) if isinstance(results, list) else 'invalid'} != "
        f"{report['duplicate_groups']}"
    )
if any(result.get("ruleId") != "DUP001" for result in results):
    raise SystemExit("SARIF contains a result with an unexpected ruleId")

if "DUP001" not in text:
    raise SystemExit("text report does not represent Django findings")
if "\x1b" in text:
    raise SystemExit("text report contains ANSI escapes")

first_path = report["findings"][0]["locations"][0]["path"]
if first_path not in markdown or first_path not in text:
    raise SystemExit("human-readable reports do not preserve a representative finding path")
PY

echo "PASS: primary and supplemental JSON are logically identical"
echo "PASS: Markdown represents the complete Django finding report"
echo "PASS: SARIF contains one DUP001 result per Django duplicate group"
echo "PASS: text output represents Django findings without ANSI escapes"

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "harness_commit=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    echo "arid_source=$ARID_SOURCE"
    echo "arid_binary=$ARID_BIN"
    echo "arid_sha256=$ARID_SHA256"
    echo "arid_version=$ARID_VERSION"
    if [[ "$ARID_SOURCE" == "repository" ]]; then
        echo "arid_commit=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    fi
    echo "django_commit=$(git -C "$DJANGO_ROOT" rev-parse HEAD)"
    echo "django_remote=$(git -C "$DJANGO_ROOT" remote get-url origin 2>/dev/null || true)"
    echo "workers=auto"
    echo "excluded_source=$DJANGO_INVALID_SOURCE"
    echo "duplicate_groups=$(jq -r '.duplicate_groups' "$PRIMARY_JSON")"
} >"$RESULTS_DIR/metadata.txt"

echo
echo "========================================"
echo " V2 Django multi-output validation PASS"
echo "========================================"
echo
echo "Arid:             $ARID_VERSION"
echo "Django:           $(git -C "$DJANGO_ROOT" rev-parse HEAD)"
echo "Workers:          auto"
echo "Duplicate groups: $(jq -r '.duplicate_groups' "$PRIMARY_JSON")"
echo "Results:           $RESULTS_DIR"
