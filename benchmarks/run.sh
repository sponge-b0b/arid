#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

HYPERFINE_VERSION="hyperfine 1.20.0"
PYLINT_VERSION="pylint 4.0.6"
JSCPD_VERSION="cpd 5.0.12"

WARMUP="${WARMUP:-1}"
RUNS="${RUNS:-5}"
TOOLS="arid,pylint,jscpd"
WORKERS="1,2,4,8"
WORKER_SCALING=true

usage() {
    cat <<EOF
Usage: $0 <corpus-path> [label] [options]

The corpus must be a clean Git repository root.

Options:
  --tools <list>       Tools to benchmark: arid,pylint,jscpd
                       Default: arid,pylint,jscpd
  --workers <list>     Arid worker counts for the scaling benchmark
                       Default: 1,2,4,8
  --no-worker-scaling  Skip the Arid worker-scaling benchmark
  --warmup <N>         Hyperfine warmup runs
                       Default: WARMUP or 1
  --runs <N>           Hyperfine measured runs
                       Default: RUNS or 5
  --help               Show this help

Environment:
  WARMUP=N             Hyperfine warmup runs (default: 1)
  RUNS=N               Hyperfine measured runs (default: 5)

CLI options take precedence over environment variables.

Examples:
  # Full benchmark suite
  $0 /path/to/corpus corpus

  # Fast Arid-only regression benchmark
  $0 /path/to/corpus corpus --tools arid --no-worker-scaling

  # Arid benchmark plus worker scaling
  $0 /path/to/corpus corpus --tools arid

  # Cross-tool comparison without worker scaling
  $0 /path/to/corpus corpus --tools arid,pylint,jscpd --no-worker-scaling

  # Short development benchmark
  $0 /path/to/corpus corpus --tools arid --workers 1,4 --warmup 0 --runs 3
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

CORPUS_INPUT=""
LABEL=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tools)
            [[ $# -ge 2 ]] || die "--tools requires a value"
            TOOLS="$2"
            shift 2
            ;;
        --workers)
            [[ $# -ge 2 ]] || die "--workers requires a value"
            WORKERS="$2"
            shift 2
            ;;
        --no-worker-scaling)
            WORKER_SCALING=false
            shift
            ;;
        --warmup)
            [[ $# -ge 2 ]] || die "--warmup requires a value"
            WARMUP="$2"
            shift 2
            ;;
        --runs)
            [[ $# -ge 2 ]] || die "--runs requires a value"
            RUNS="$2"
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
            if [[ -z "$CORPUS_INPUT" ]]; then
                CORPUS_INPUT="$1"
            elif [[ -z "$LABEL" ]]; then
                LABEL="$1"
            else
                die "unexpected argument: $1"
            fi
            shift
            ;;
    esac
done

if [[ -z "$CORPUS_INPUT" ]]; then
    usage
    exit 2
fi

if [[ ! "$WARMUP" =~ ^[0-9]+$ ]]; then
    die "--warmup must be a non-negative integer"
fi

if [[ ! "$RUNS" =~ ^[1-9][0-9]*$ ]]; then
    die "--runs must be a positive integer"
fi

if [[ ! "$TOOLS" =~ ^(arid|pylint|jscpd)(,(arid|pylint|jscpd))*$ ]]; then
    die "--tools must be a comma-separated list containing arid, pylint, and/or jscpd"
fi

if [[ ! "$WORKERS" =~ ^[1-9][0-9]*(,[1-9][0-9]*)*$ ]]; then
    die "--workers must be a comma-separated list of positive integers"
fi

IFS=',' read -r -a SELECTED_TOOLS <<< "$TOOLS"

declare -A seen_tools=()

for tool in "${SELECTED_TOOLS[@]}"; do
    if [[ -n "${seen_tools[$tool]:-}" ]]; then
        die "duplicate tool in --tools: $tool"
    fi
    seen_tools["$tool"]=1
done

contains_tool() {
    local expected="$1"
    local tool

    for tool in "${SELECTED_TOOLS[@]}"; do
        if [[ "$tool" == "$expected" ]]; then
            return 0
        fi
    done

    return 1
}

if [[ ! -d "$CORPUS_INPUT" ]]; then
    die "corpus is not a directory: $CORPUS_INPUT"
fi

CORPUS="$(realpath "$CORPUS_INPUT")"
LABEL="${LABEL:-$(basename "$CORPUS")}"

if [[ ! "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
    die "label may contain only letters, numbers, '.', '_', and '-'"
fi

RESULT_DIR="$ROOT_DIR/benchmarks/results/$LABEL"
ARID_BIN="$ROOT_DIR/target/release/arid"

required_commands=(cargo git hyperfine jq lscpu nproc rustc)

if contains_tool pylint; then
    required_commands+=(pylint)
fi

if contains_tool jscpd; then
    required_commands+=(jscpd)
fi

for command in "${required_commands[@]}"; do
    if ! command -v "$command" >/dev/null 2>&1; then
        die "required command not found: $command"
    fi
done

if ! git -C "$CORPUS" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    die "corpus is not a Git repository: $CORPUS"
fi

CORPUS_ROOT="$(git -C "$CORPUS" rev-parse --show-toplevel)"

if [[ "$CORPUS_ROOT" != "$CORPUS" ]]; then
    echo "error: corpus must be the Git repository root" >&2
    echo "repository root: $CORPUS_ROOT" >&2
    exit 2
fi

if [[ -n "$(git -C "$CORPUS" status --porcelain --untracked-files=all)" ]]; then
    echo "error: corpus working tree is not clean: $CORPUS" >&2
    git -C "$CORPUS" status --short >&2
    exit 2
fi

actual_hyperfine="$(hyperfine --version)"

if [[ "$actual_hyperfine" != "$HYPERFINE_VERSION" ]]; then
    die "expected $HYPERFINE_VERSION, found $actual_hyperfine"
fi

actual_pylint="not-run"

if contains_tool pylint; then
    actual_pylint="$(pylint --version)"
    actual_pylint="${actual_pylint%%$'\n'*}"

    if [[ "$actual_pylint" != "$PYLINT_VERSION" ]]; then
        die "expected $PYLINT_VERSION, found $actual_pylint"
    fi
fi

actual_jscpd="not-run"

if contains_tool jscpd; then
    actual_jscpd="$(jscpd --version)"

    if [[ "$actual_jscpd" != "$JSCPD_VERSION" ]]; then
        die "expected $JSCPD_VERSION, found $actual_jscpd"
    fi
fi

tracked_python_files="$(
    git -C "$CORPUS" ls-files -z -- '*.py' '*.pyi' |
        tr -cd '\0' |
        wc -c
)"

tracked_py_files="$(
    git -C "$CORPUS" ls-files -z -- '*.py' |
        tr -cd '\0' |
        wc -c
)"

tracked_pyi_files="$(
    git -C "$CORPUS" ls-files -z -- '*.pyi' |
        tr -cd '\0' |
        wc -c
)"

if [[ "$tracked_python_files" -eq 0 ]]; then
    die "corpus contains no tracked Python files"
fi

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

echo "Building Arid release binary..."

cargo build \
    --release \
    --locked \
    --manifest-path "$ROOT_DIR/Cargo.toml"

echo "Validating corpus with Arid..."

set +e
"$ARID_BIN" \
    "$CORPUS" \
    --hidden \
    --json \
    > "$RESULT_DIR/arid-baseline.json"
arid_status=$?
set -e

if [[ "$arid_status" -ne 0 && "$arid_status" -ne 1 ]]; then
    echo "error: Arid corpus validation failed with exit code $arid_status" >&2
    exit "$arid_status"
fi

arid_files="$(jq -r '.files' "$RESULT_DIR/arid-baseline.json")"
source_lines="$(jq -r '.source_lines' "$RESULT_DIR/arid-baseline.json")"
analyzed_lines="$(jq -r '.analyzed_lines' "$RESULT_DIR/arid-baseline.json")"

if [[ "$arid_files" -ne "$tracked_python_files" ]]; then
    echo "error: Git tracks $tracked_python_files Python files but Arid analyzed $arid_files" >&2
    echo "The corpus cannot be used until the discovery difference is understood." >&2
    exit 2
fi

corpus_commit="$(git -C "$CORPUS" rev-parse HEAD)"
corpus_remote="$(git -C "$CORPUS" remote get-url origin 2>/dev/null || true)"

if [[ "$WORKER_SCALING" == true ]]; then
    worker_metadata="$WORKERS"
else
    worker_metadata="disabled"
fi

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "label=$LABEL"
    echo "corpus=$CORPUS"
    echo "corpus_remote=$corpus_remote"
    echo "corpus_commit=$corpus_commit"
    echo "python_files=$arid_files"
    echo "py_files=$tracked_py_files"
    echo "pyi_files=$tracked_pyi_files"
    echo "physical_source_lines=$source_lines"
    echo "analyzed_effective_lines=$analyzed_lines"
    echo "tools=$TOOLS"
    echo "warmup=$WARMUP"
    echo "runs=$RUNS"
    echo "worker_scaling=$WORKER_SCALING"
    echo "arid_worker_counts=$worker_metadata"
    echo "hyperfine=$actual_hyperfine"
    echo "pylint=$actual_pylint"
    echo "jscpd=$actual_jscpd"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "os=$(uname -srmo)"
    echo "cpu=$(lscpu | awk -F: '/Model name/ {gsub(/^[ \t]+/, "", $2); print $2; exit}')"
    echo "logical_cpus=$(nproc)"
    echo "arid_commit=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    echo "corpus_definition=clean pinned Git repository"
    echo "invocation=repository root with native recursive discovery"
} > "$RESULT_DIR/metadata.txt"

printf -v corpus_q '%q' "$CORPUS"
printf -v arid_q '%q' "$ARID_BIN"

arid_cmd="$arid_q \
$corpus_q \
--hidden \
--workers 1 \
--json > /dev/null; \
status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

arid_pylint_cmd="$arid_q \
$corpus_q \
--hidden \
--no-same-file \
--workers 1 \
--json > /dev/null; \
status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

pylint_cmd="pylint \
--rcfile=/dev/null \
--recursive=y \
--disable=all \
--enable=similarities \
--min-similarity-lines=4 \
--ignore-comments=y \
--ignore-docstrings=y \
--ignore-imports=y \
--ignore-signatures=y \
--jobs=1 \
--persistent=n \
--reports=n \
--score=n \
$corpus_q > /dev/null; \
status=\$?; [[ \$status -eq 0 || \$status -eq 8 ]]"

arid_jscpd_cmd="$arid_q \
$corpus_q \
--hidden \
--workers 1 \
--json > /dev/null; \
status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

arid_workers_cmd="$arid_q \
$corpus_q \
--hidden \
--workers {workers} \
--json > /dev/null; \
status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

jscpd_serial_cmd="jscpd \
--format python \
--formats-exts python:py,pyi \
--min-lines 4 \
--mode weak \
--workers 1 \
--max-size 1073741824 \
--silent \
--no-tips \
$corpus_q > /dev/null"

jscpd_auto_cmd="jscpd \
--format python \
--formats-exts python:py,pyi \
--min-lines 4 \
--mode weak \
--max-size 1073741824 \
--silent \
--no-tips \
$corpus_q > /dev/null"

echo
echo "Corpus: $LABEL"
echo "Commit: $corpus_commit"
echo "Python files: $arid_files"
echo "  .py:  $tracked_py_files"
echo "  .pyi: $tracked_pyi_files"
echo "Physical Python source lines: $source_lines"
echo "Tools: $TOOLS"
echo "Worker scaling: $WORKER_SCALING"
echo

if contains_tool arid; then
    echo "Benchmarking Arid..."

    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$RESULT_DIR/arid.json" \
        --export-markdown "$RESULT_DIR/arid.md" \
        "$arid_cmd"

    echo
fi

if contains_tool pylint; then
    echo "Benchmarking Arid vs Pylint R0801..."

    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$RESULT_DIR/pylint.json" \
        --export-markdown "$RESULT_DIR/pylint.md" \
        "$arid_pylint_cmd" \
        "$pylint_cmd"

    echo
fi

if contains_tool jscpd; then
    echo "Benchmarking serial Arid vs serial jscpd..."

    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$RESULT_DIR/jscpd-serial.json" \
        --export-markdown "$RESULT_DIR/jscpd-serial.md" \
        "$arid_jscpd_cmd" \
        "$jscpd_serial_cmd"

    echo
    echo "Benchmarking serial Arid vs auto-parallel jscpd..."

    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$RESULT_DIR/jscpd-auto.json" \
        --export-markdown "$RESULT_DIR/jscpd-auto.md" \
        "$arid_jscpd_cmd" \
        "$jscpd_auto_cmd"

    echo
fi

if [[ "$WORKER_SCALING" == true ]]; then
    echo "Benchmarking Arid worker scaling..."

    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --parameter-list workers "$WORKERS" \
        --export-json "$RESULT_DIR/arid-workers.json" \
        --export-markdown "$RESULT_DIR/arid-workers.md" \
        "$arid_workers_cmd"

    echo
fi

echo "Benchmark results written to:"
echo "  $RESULT_DIR"