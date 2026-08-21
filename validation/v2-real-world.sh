#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS_RELATIVE_PATH="validation/arid-corpora"
ARID_BIN_INPUT=""

usage() {
    cat <<EOF
Usage: $0 <global-root> [--arid-bin <path>]

Exercise v2 workflow features against the real-world Rich validation corpus.
This complements validation/v2.sh: synthetic fixtures define feature contracts;
this harness proves representative feature composition on real source.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

pass() {
    printf 'PASS: %s\n' "$1"
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

for command in awk cmp git grep head jq mktemp realpath sha256sum; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -d "$GLOBAL_ROOT_INPUT" ]] ||
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"
RICH_ROOT="$CORPUS_ROOT/rich"
RESULTS_DIR="$ROOT_DIR/validation/results/v2-real-world/rich"
TMP_ROOT="$(mktemp -d)"
KEEP_GOING_ROOT="$TMP_ROOT/rich-keep-going"

[[ -d "$RICH_ROOT" ]] ||
    die "Rich validation corpus does not exist: $RICH_ROOT"

git -C "$RICH_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
    die "Rich corpus is not a Git repository: $RICH_ROOT"
[[ "$(realpath "$(git -C "$RICH_ROOT" rev-parse --show-toplevel)")" == "$RICH_ROOT" ]] ||
    die "Rich corpus must be the Git repository root: $RICH_ROOT"
[[ -z "$(git -C "$RICH_ROOT" status --porcelain --untracked-files=all)" ]] ||
    die "Rich corpus working tree is not clean: $RICH_ROOT"

cleanup() {
    if git -C "$RICH_ROOT" worktree list --porcelain 2>/dev/null |
        grep -Fqx "worktree $KEEP_GOING_ROOT"; then
        git -C "$RICH_ROOT" worktree remove --force "$KEEP_GOING_ROOT" >/dev/null 2>&1 || true
        git -C "$RICH_ROOT" worktree prune >/dev/null 2>&1 || true
    fi

    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

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
mkdir -p "$RESULTS_DIR"

echo "Arid:    $ARID_VERSION"
echo "Rich:    $(git -C "$RICH_ROOT" rev-parse HEAD)"
echo "Results: $RESULTS_DIR"

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
        echo "error: expected exit $expected, got $status: $*" >&2
        [[ ! -s "$stdout_file" ]] || cat "$stdout_file" >&2
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        exit 2
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
        echo "error: expected exit $expected, got $status: $*" >&2
        [[ ! -s "$stdout_file" ]] || cat "$stdout_file" >&2
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        exit 2
    }

    [[ ! -s "$stderr_file" ]] || {
        cat "$stderr_file" >&2
        die "unexpected stderr: $*"
    }
}

COMMON_ARGS=(
    "$RICH_ROOT"
    --no-config
    --project-root "$RICH_ROOT"
    --hidden
    --workers 1
    --json
)

FULL_JSON="$RESULTS_DIR/full.json"
FULL_STDERR="$RESULTS_DIR/full.stderr"

progress "Scanning the full Rich corpus"
run_expect_status 1 \
    "$FULL_JSON" \
    "$FULL_STDERR" \
    "$ARID_BIN" "${COMMON_ARGS[@]}"

jq -e '
    .schema_version == 4
    and .complete == true
    and .errors == []
    and .duplicate_groups > 0
    and (.findings | length) == .duplicate_groups
' "$FULL_JSON" >/dev/null ||
    die "Rich full scan did not produce the expected complete report-v4 findings"

FOCUS_PATH="$(
    jq -r '
        .findings[]
        | select(.distribution == "cross-file" or .distribution == "hybrid")
        | .locations[0].path
    ' "$FULL_JSON" |
        head -n 1
)"

[[ -n "$FOCUS_PATH" && "$FOCUS_PATH" != "null" ]] ||
    die "Rich full scan has no cross-file/hybrid finding suitable for focus validation"
[[ -f "$RICH_ROOT/$FOCUS_PATH" ]] ||
    die "selected Rich focus path does not exist: $FOCUS_PATH"

FOCUS_JSON="$RESULTS_DIR/focus.json"
FOCUS_STDERR="$RESULTS_DIR/focus.stderr"

progress "Validating focus against $FOCUS_PATH"
run_expect_status 1 \
    "$FOCUS_JSON" \
    "$FOCUS_STDERR" \
    "$ARID_BIN" "${COMMON_ARGS[@]}" \
    --focus "$FOCUS_PATH"

jq -e --arg focus "$FOCUS_PATH" '
    .schema_version == 4
    and .complete == true
    and .errors == []
    and .analysis.focus == [$focus]
    and .duplicate_groups > 0
    and all(.findings[]; any(.locations[]; .path == $focus))
    and any(.findings[]; any(.locations[]; .path != $focus))
' "$FOCUS_JSON" >/dev/null ||
    die "real-world focus report does not preserve whole-corpus occurrence context"

jq -S --arg focus "$FOCUS_PATH" '
    [.findings[] | select(any(.locations[]; .path == $focus))]
' "$FULL_JSON" >"$TMP_ROOT/expected-focus.json"
jq -S '.findings' "$FOCUS_JSON" >"$TMP_ROOT/actual-focus.json"
cmp -s "$TMP_ROOT/expected-focus.json" "$TMP_ROOT/actual-focus.json" ||
    die "real-world focus findings differ from the matching subset of the full Rich scan"

pass "Rich focus filters reporting while preserving whole-corpus findings"

BASELINE="$RESULTS_DIR/rich-baseline.json"

progress "Writing a baseline from the full Rich corpus"
run_expect_status 0 \
    "$RESULTS_DIR/baseline-write.stdout" \
    "$RESULTS_DIR/baseline-write.stderr" \
    "$ARID_BIN" "$RICH_ROOT" \
    --no-config \
    --project-root "$RICH_ROOT" \
    --hidden \
    --workers 1 \
    --write-baseline "$BASELINE"

BASELINE_FOCUS_JSON="$RESULTS_DIR/baseline-focus.json"

progress "Validating baseline + focus ordering"
run_expect_status 0 \
    "$BASELINE_FOCUS_JSON" \
    "$RESULTS_DIR/baseline-focus.stderr" \
    "$ARID_BIN" "${COMMON_ARGS[@]}" \
    --baseline "$BASELINE" \
    --focus "$FOCUS_PATH"

jq -e --arg focus "$FOCUS_PATH" '
    .schema_version == 4
    and .complete == true
    and .errors == []
    and .analysis.baseline_enabled == true
    and .analysis.focus == [$focus]
    and .duplicate_groups == 0
    and .findings == []
' "$BASELINE_FOCUS_JSON" >/dev/null ||
    die "real-world baseline enforcement did not suppress accepted Rich debt before focus reporting"

pass "Rich baseline enforcement composes with focus"

FOCUS_SOURCE="$RICH_ROOT/$FOCUS_PATH"
FOCUS_SHA_BEFORE="$(sha256sum "$FOCUS_SOURCE" | awk '{print $1}')"
VIRTUAL_JSON="$RESULTS_DIR/virtual-replace.json"

progress "Validating virtual-source replacement for $FOCUS_PATH"
run_with_stdin_expect_status 1 \
    "$FOCUS_SOURCE" \
    "$VIRTUAL_JSON" \
    "$RESULTS_DIR/virtual-replace.stderr" \
    "$ARID_BIN" "${COMMON_ARGS[@]}" \
    --stdin-path "$FOCUS_PATH"

FOCUS_SHA_AFTER="$(sha256sum "$FOCUS_SOURCE" | awk '{print $1}')"
[[ "$FOCUS_SHA_BEFORE" == "$FOCUS_SHA_AFTER" ]] ||
    die "virtual-source replacement modified the real Rich source file"

jq -e --arg path "$FOCUS_PATH" '
    .schema_version == 4
    and .complete == true
    and .errors == []
    and .analysis.virtual_source == $path
' "$VIRTUAL_JSON" >/dev/null ||
    die "real-world virtual-source replacement did not record the selected Rich path"

jq -S '{
    files,
    source_lines,
    analyzed_lines,
    duplicate_groups,
    duplicate_lines,
    duplication_percent,
    findings
}' "$FULL_JSON" >"$TMP_ROOT/full-detector.json"
jq -S '{
    files,
    source_lines,
    analyzed_lines,
    duplicate_groups,
    duplicate_lines,
    duplication_percent,
    findings
}' "$VIRTUAL_JSON" >"$TMP_ROOT/virtual-detector.json"
cmp -s "$TMP_ROOT/full-detector.json" "$TMP_ROOT/virtual-detector.json" ||
    die "identical virtual replacement changed Rich detector results"

pass "Rich virtual-source replacement is detector-equivalent and non-mutating"

# Use a detached worktree so the canonical corpus remains clean while the
# keep-going probe adds one controlled malformed Python source.
progress "Validating keep-going with one controlled Rich parse failure"
git -C "$RICH_ROOT" worktree add --quiet --detach "$KEEP_GOING_ROOT" HEAD

cat >"$KEEP_GOING_ROOT/zz_arid_invalid_fixture.py" <<'PY'
def intentionally_invalid(:
    pass
PY

KEEP_JSON="$RESULTS_DIR/keep-going.json"

run_expect_status 2 \
    "$KEEP_JSON" \
    "$RESULTS_DIR/keep-going.stderr" \
    "$ARID_BIN" "$KEEP_GOING_ROOT" \
    --no-config \
    --project-root "$KEEP_GOING_ROOT" \
    --hidden \
    --workers 1 \
    --keep-going \
    --json

jq -e '
    .schema_version == 4
    and .complete == false
    and .analysis.keep_going == true
    and (.errors | length) == 1
    and .errors[0].kind == "parse"
    and .errors[0].path == "zz_arid_invalid_fixture.py"
' "$KEEP_JSON" >/dev/null ||
    die "real-world keep-going report did not expose the controlled parse failure correctly"

jq -S '.findings' "$FULL_JSON" >"$TMP_ROOT/full-findings.json"
jq -S '.findings' "$KEEP_JSON" >"$TMP_ROOT/keep-findings.json"
cmp -s "$TMP_ROOT/full-findings.json" "$TMP_ROOT/keep-findings.json" ||
    die "real-world keep-going changed findings from valid Rich sources"

pass "Rich keep-going preserves valid findings and reports incomplete analysis"

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
    echo "rich_commit=$(git -C "$RICH_ROOT" rev-parse HEAD)"
    echo "rich_remote=$(git -C "$RICH_ROOT" remote get-url origin 2>/dev/null || true)"
    echo "focus_path=$FOCUS_PATH"
} >"$RESULTS_DIR/metadata.txt"

echo
echo "========================================"
echo " V2 Rich workflow validation PASS"
echo "========================================"
echo
echo "Arid:      $ARID_VERSION"
echo "Rich:      $(git -C "$RICH_ROOT" rev-parse HEAD)"
echo "Focus:     $FOCUS_PATH"
echo "Results:   $RESULTS_DIR"
