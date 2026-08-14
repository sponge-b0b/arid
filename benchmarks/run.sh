#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

HYPERFINE_VERSION="hyperfine 1.20.0"
PYLINT_VERSION="pylint 4.0.6"
JSCPD_VERSION="cpd 5.0.12"

WARMUP="${WARMUP:-1}"
RUNS="${RUNS:-5}"

usage() {
    echo "Usage: $0 <corpus-path> [label]"
    echo
    echo "The corpus must be a clean Git repository root."
    echo
    echo "Environment:"
    echo "  WARMUP=N   Hyperfine warmup runs (default: 1)"
    echo "  RUNS=N     Hyperfine measured runs (default: 5)"
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
    usage
    exit 2
fi

CORPUS="$(realpath "$1")"
LABEL="${2:-$(basename "$CORPUS")}"

if [[ ! "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
    echo "error: label may contain only letters, numbers, '.', '_', and '-'" >&2
    exit 2
fi

RESULT_DIR="$ROOT_DIR/benchmarks/results/$LABEL"
ARID_BIN="$ROOT_DIR/target/release/arid"

if [[ ! -d "$CORPUS" ]]; then
    echo "error: corpus is not a directory: $CORPUS" >&2
    exit 2
fi

for command in cargo git hyperfine jq jscpd lscpu nproc pylint rustc; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "error: required command not found: $command" >&2
        exit 2
    fi
done

if ! git -C "$CORPUS" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "error: corpus is not a Git repository: $CORPUS" >&2
    exit 2
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

actual_pylint="$(pylint --version)"
actual_pylint="${actual_pylint%%$'\n'*}"

actual_jscpd="$(jscpd --version)"

if [[ "$actual_hyperfine" != "$HYPERFINE_VERSION" ]]; then
    echo "error: expected $HYPERFINE_VERSION, found $actual_hyperfine" >&2
    exit 2
fi

if [[ "$actual_pylint" != "$PYLINT_VERSION" ]]; then
    echo "error: expected $PYLINT_VERSION, found $actual_pylint" >&2
    exit 2
fi

if [[ "$actual_jscpd" != "$JSCPD_VERSION" ]]; then
    echo "error: expected $JSCPD_VERSION, found $actual_jscpd" >&2
    exit 2
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
    echo "error: corpus contains no tracked Python files" >&2
    exit 2
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
    echo "warmup=$WARMUP"
    echo "runs=$RUNS"
    echo "arid_worker_counts=1,2,4,8"
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
echo

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
echo "Benchmarking Arid worker scaling..."

hyperfine \
    --shell=bash \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --parameter-list workers 1,2,4,8 \
    --export-json "$RESULT_DIR/arid-workers.json" \
    --export-markdown "$RESULT_DIR/arid-workers.md" \
    "$arid_workers_cmd"

echo
echo "Benchmark results written to:"
echo "  $RESULT_DIR"
