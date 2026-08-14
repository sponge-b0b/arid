#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CORPUS_RELATIVE_PATH="benchmarks/arid-corpora"
REPOSITORIES="polaris,pydantic,requests"

HYPERFINE_VERSION="hyperfine 1.20.0"
PYLINT_VERSION="pylint 4.0.6"
JSCPD_VERSION="cpd 5.0.12"

WARMUP="${WARMUP:-1}"
RUNS="${RUNS:-5}"
REPOS="$REPOSITORIES"
TOOLS="arid,pylint,jscpd"
WORKERS="1,2,4,8"
WORKER_SCALING=true
LABEL=""

usage() {
    cat <<EOF
Usage: $0 <global-root> [options]

Benchmark Arid against corpora beneath:
  <global-root>/$CORPUS_RELATIVE_PATH

Options:
  --repos <list>        Repositories to benchmark: $REPOSITORIES
                        Default: $REPOSITORIES
  --label <label>       Suffix applied to each result directory
  --tools <list>        Tools to benchmark: arid,pylint,jscpd
                        Default: arid,pylint,jscpd
  --workers <list>      Arid worker counts for the scaling benchmark
                        Default: 1,2,4,8
  --no-worker-scaling   Skip the Arid worker-scaling benchmark
  --warmup <N>          Hyperfine warmup runs
                        Default: WARMUP or 1
  --runs <N>            Hyperfine measured runs
                        Default: RUNS or 5
  --help                Show this help

Environment:
  WARMUP=N              Hyperfine warmup runs (default: 1)
  RUNS=N                Hyperfine measured runs (default: 5)

CLI options take precedence over environment variables.

Examples:
  # Full benchmark suite
  $0 /home/bobt

  # Full benchmark suite with labeled result directories
  $0 /home/bobt --label beta-entry

  # Benchmark selected repositories
  $0 /home/bobt --repos requests,pydantic

  # Fast Arid-only regression benchmark
  $0 /home/bobt --repos pydantic --tools arid --no-worker-scaling

  # Arid benchmark plus worker scaling
  $0 /home/bobt --repos pydantic --tools arid

  # Cross-tool comparison without worker scaling
  $0 /home/bobt --repos requests,polaris \
    --tools arid,pylint,jscpd \
    --no-worker-scaling

  # Short development benchmark
  $0 /home/bobt \
    --repos requests \
    --tools arid \
    --workers 1,4 \
    --warmup 0 \
    --runs 3
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

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

GLOBAL_ROOT_INPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repos)
            [[ $# -ge 2 ]] || die "--repos requires a value"
            REPOS="$2"
            shift 2
            ;;
        --label)
            [[ $# -ge 2 ]] || die "--label requires a value"
            LABEL="$2"
            shift 2
            ;;
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
            if [[ -n "$GLOBAL_ROOT_INPUT" ]]; then
                die "unexpected argument: $1"
            fi

            GLOBAL_ROOT_INPUT="$1"
            shift
            ;;
    esac
done

if [[ -z "$GLOBAL_ROOT_INPUT" ]]; then
    usage
    exit 2
fi

if [[ ! "$WARMUP" =~ ^[0-9]+$ ]]; then
    die "--warmup must be a non-negative integer"
fi

if [[ ! "$RUNS" =~ ^[1-9][0-9]*$ ]]; then
    die "--runs must be a positive integer"
fi

if [[ ! "$REPOS" =~ ^(polaris|pydantic|requests)(,(polaris|pydantic|requests))*$ ]]; then
    die "--repos must be a comma-separated list containing polaris, pydantic, and/or requests"
fi

if [[ ! "$TOOLS" =~ ^(arid|pylint|jscpd)(,(arid|pylint|jscpd))*$ ]]; then
    die "--tools must be a comma-separated list containing arid, pylint, and/or jscpd"
fi

if [[ ! "$WORKERS" =~ ^[1-9][0-9]*(,[1-9][0-9]*)*$ ]]; then
    die "--workers must be a comma-separated list of positive integers"
fi

if [[ -n "$LABEL" && ! "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
    die "label may contain only letters, numbers, '.', '_', and '-'"
fi

IFS=',' read -r -a SELECTED_REPOS <<< "$REPOS"
IFS=',' read -r -a SELECTED_TOOLS <<< "$TOOLS"

declare -A seen_repos=()
declare -A seen_tools=()

for repository in "${SELECTED_REPOS[@]}"; do
    if [[ -n "${seen_repos[$repository]:-}" ]]; then
        die "duplicate repository in --repos: $repository"
    fi

    seen_repos["$repository"]=1
done

for tool in "${SELECTED_TOOLS[@]}"; do
    if [[ -n "${seen_tools[$tool]:-}" ]]; then
        die "duplicate tool in --tools: $tool"
    fi

    seen_tools["$tool"]=1
done

required_commands=(cargo git hyperfine jq lscpu nproc realpath rustc)

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

if [[ ! -d "$GLOBAL_ROOT_INPUT" ]]; then
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"
fi

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"
ARID_BIN="$ROOT_DIR/target/release/arid"

if [[ ! -d "$CORPUS_ROOT" ]]; then
    die "benchmark corpus root does not exist: $CORPUS_ROOT; run benchmarks/build.sh $GLOBAL_ROOT first"
fi

for repository in "${SELECTED_REPOS[@]}"; do
    corpus="$CORPUS_ROOT/$repository"

    if [[ ! -d "$corpus" ]]; then
        die "benchmark corpus does not exist: $corpus; run benchmarks/build.sh $GLOBAL_ROOT first"
    fi
done

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

echo "Building Arid release binary..."

cargo build \
    --release \
    --locked \
    --manifest-path "$ROOT_DIR/Cargo.toml"

benchmark_repository() {
    local repository="$1"
    local corpus="$CORPUS_ROOT/$repository"
    local result_label="$repository"
    local result_dir

    local repository_root
    local tracked_python_files
    local tracked_py_files
    local tracked_pyi_files
    local arid_status
    local arid_files
    local source_lines
    local analyzed_lines
    local corpus_commit
    local corpus_remote
    local worker_metadata

    local corpus_q
    local arid_q
    local arid_cmd
    local arid_pylint_cmd
    local pylint_cmd
    local arid_jscpd_cmd
    local arid_workers_cmd
    local jscpd_serial_cmd
    local jscpd_auto_cmd

    if [[ -n "$LABEL" ]]; then
        result_label="$repository-$LABEL"
    fi

    result_dir="$ROOT_DIR/benchmarks/results/$result_label"

    echo
    echo "========================================"
    echo " Benchmark: $repository"
    echo "========================================"

    if ! git -C "$corpus" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        die "corpus is not a Git repository: $corpus"
    fi

    repository_root="$(git -C "$corpus" rev-parse --show-toplevel)"
    repository_root="$(realpath "$repository_root")"

    if [[ "$repository_root" != "$corpus" ]]; then
        echo "error: corpus must be the Git repository root" >&2
        echo "corpus:          $corpus" >&2
        echo "repository root: $repository_root" >&2
        exit 2
    fi

    if [[ -n "$(git -C "$corpus" status --porcelain --untracked-files=all)" ]]; then
        echo "error: corpus working tree is not clean: $corpus" >&2
        git -C "$corpus" status --short >&2
        exit 2
    fi

    tracked_python_files="$(
        git -C "$corpus" ls-files -z -- '*.py' '*.pyi' |
            tr -cd '\0' |
            wc -c
    )"

    tracked_py_files="$(
        git -C "$corpus" ls-files -z -- '*.py' |
            tr -cd '\0' |
            wc -c
    )"

    tracked_pyi_files="$(
        git -C "$corpus" ls-files -z -- '*.pyi' |
            tr -cd '\0' |
            wc -c
    )"

    if [[ "$tracked_python_files" -eq 0 ]]; then
        die "corpus contains no tracked Python files: $corpus"
    fi

    rm -rf "$result_dir"
    mkdir -p "$result_dir"

    echo "Validating corpus with Arid..."

    set +e
    "$ARID_BIN" \
        "$corpus" \
        --hidden \
        --json \
        > "$result_dir/arid-baseline.json"
    arid_status=$?
    set -e

    if [[ "$arid_status" -ne 0 && "$arid_status" -ne 1 ]]; then
        echo "error: Arid corpus validation failed with exit code $arid_status" >&2
        exit "$arid_status"
    fi

    arid_files="$(jq -r '.files' "$result_dir/arid-baseline.json")"
    source_lines="$(jq -r '.source_lines' "$result_dir/arid-baseline.json")"
    analyzed_lines="$(jq -r '.analyzed_lines' "$result_dir/arid-baseline.json")"

    if [[ "$arid_files" -ne "$tracked_python_files" ]]; then
        echo "error: Git tracks $tracked_python_files Python files but Arid analyzed $arid_files" >&2
        echo "The corpus cannot be used until the discovery difference is understood." >&2
        exit 2
    fi

    corpus_commit="$(git -C "$corpus" rev-parse HEAD)"
    corpus_remote="$(git -C "$corpus" remote get-url origin 2>/dev/null || true)"

    if [[ "$WORKER_SCALING" == true ]]; then
        worker_metadata="$WORKERS"
    else
        worker_metadata="disabled"
    fi

    {
        echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "repository=$repository"
        echo "label=$result_label"
        echo "corpus=$corpus"
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
    } > "$result_dir/metadata.txt"

    printf -v corpus_q '%q' "$corpus"
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
    echo "Corpus: $repository"

    if [[ -n "$LABEL" ]]; then
        echo "Result label: $result_label"
    fi

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
            --export-json "$result_dir/arid.json" \
            --export-markdown "$result_dir/arid.md" \
            "$arid_cmd"

        echo
    fi

    if contains_tool pylint; then
        echo "Benchmarking Arid vs Pylint R0801..."

        hyperfine \
            --shell=bash \
            --warmup "$WARMUP" \
            --runs "$RUNS" \
            --export-json "$result_dir/pylint.json" \
            --export-markdown "$result_dir/pylint.md" \
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
            --export-json "$result_dir/jscpd-serial.json" \
            --export-markdown "$result_dir/jscpd-serial.md" \
            "$arid_jscpd_cmd" \
            "$jscpd_serial_cmd"

        echo
        echo "Benchmarking serial Arid vs auto-parallel jscpd..."

        hyperfine \
            --shell=bash \
            --warmup "$WARMUP" \
            --runs "$RUNS" \
            --export-json "$result_dir/jscpd-auto.json" \
            --export-markdown "$result_dir/jscpd-auto.md" \
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
            --export-json "$result_dir/arid-workers.json" \
            --export-markdown "$result_dir/arid-workers.md" \
            "$arid_workers_cmd"

        echo
    fi

    echo "Benchmark results written to:"
    echo "  $result_dir"
}

for repository in "${SELECTED_REPOS[@]}"; do
    benchmark_repository "$repository"
done

echo
echo "All selected benchmarks completed successfully."