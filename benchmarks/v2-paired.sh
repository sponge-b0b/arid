#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULT_DIR="$ROOT_DIR/benchmarks/results/v2-phase14-paired"
CORPUS_ROOT_RELATIVE="benchmarks/arid-corpora"

REQUESTS_COMMIT="6e83187b8feb273ed4c6cdab5efd8d54901dfab3"
PYDANTIC_COMMIT="cf67d4b3193c3fe43ede18612ed62785eee11382"
POLARIS_COMMIT="00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031"
DUPLICATE_HEAVY_COMMIT="815510e4ba755496ce3d2d4c4eea3d89afbc2ffc"

DEFAULT_WARMUP=3
DEFAULT_RUNS=10

usage() {
    cat <<EOF_USAGE
Usage: $0 <global-root> --v1-bin <path> [options]

Run a paired Arid 1.2.0 vs v2 serial regression benchmark isolated from
Pylint/jscpd. Each corpus/mode is measured twice with reversed command order,
then the two means for each version are averaged to reduce ordering/load bias.

Options:
  --v1-bin <path>     Exact Arid 1.2.0 executable. Required.
  --v2-bin <path>     Existing v2 executable. Default: build target/release/arid.
  --warmup <N>        Hyperfine warmup runs per pass. Default: $DEFAULT_WARMUP
  --runs <N>          Hyperfine measured runs per pass. Default: $DEFAULT_RUNS
  --help              Show this help.

Modes measured for every canonical corpus:
  default             --hidden --workers 1 --json
  pylint-compatible   --hidden --no-same-file --workers 1 --json
EOF_USAGE
}

die() {
    echo "error: $*" >&2
    exit 2
}

GLOBAL_ROOT_INPUT=""
V1_BIN_INPUT=""
V2_BIN_INPUT=""
WARMUP="$DEFAULT_WARMUP"
RUNS="$DEFAULT_RUNS"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --v1-bin)
            [[ $# -ge 2 ]] || die "--v1-bin requires a value"
            [[ -z "$V1_BIN_INPUT" ]] || die "duplicate option: --v1-bin"
            V1_BIN_INPUT="$2"
            shift 2
            ;;
        --v2-bin)
            [[ $# -ge 2 ]] || die "--v2-bin requires a value"
            [[ -z "$V2_BIN_INPUT" ]] || die "duplicate option: --v2-bin"
            V2_BIN_INPUT="$2"
            shift 2
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
[[ -n "$V1_BIN_INPUT" ]] || die "--v1-bin is required"
[[ "$WARMUP" =~ ^[0-9]+$ ]] || die "--warmup must be a non-negative integer"
[[ "$RUNS" =~ ^[1-9][0-9]*$ ]] || die "--runs must be a positive integer"

for command in git hyperfine jq awk realpath sha256sum; do
    command -v "$command" >/dev/null 2>&1 || die "required command not found: $command"
done

[[ "$(hyperfine --version)" == "hyperfine 1.20.0" ]] ||
    die "expected hyperfine 1.20.0, found $(hyperfine --version)"
[[ -d "$GLOBAL_ROOT_INPUT" ]] || die "global root is not a directory: $GLOBAL_ROOT_INPUT"
[[ -f "$V1_BIN_INPUT" && -x "$V1_BIN_INPUT" ]] || die "v1 executable is not executable: $V1_BIN_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_ROOT_RELATIVE"
V1_BIN="$(realpath "$V1_BIN_INPUT")"

V1_VERSION="$("$V1_BIN" --version)"
[[ "$V1_VERSION" == "arid 1.2.0" ]] ||
    die "--v1-bin must report exactly 'arid 1.2.0'; found: $V1_VERSION"

if [[ -n "$V2_BIN_INPUT" ]]; then
    [[ -f "$V2_BIN_INPUT" && -x "$V2_BIN_INPUT" ]] || die "v2 executable is not executable: $V2_BIN_INPUT"
    V2_BIN="$(realpath "$V2_BIN_INPUT")"
else
    command -v cargo >/dev/null 2>&1 || die "required command not found: cargo"

    if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
        echo "error: Arid working tree must be clean before building the v2 benchmark binary" >&2
        git -C "$ROOT_DIR" status --short >&2
        exit 2
    fi

    echo "Building the v2 release binary..."
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"
    V2_BIN="$ROOT_DIR/target/release/arid"
fi

V2_VERSION="$("$V2_BIN" --version)"
[[ "$V2_VERSION" == arid\ 2.0.0-* ]] ||
    die "v2 executable must report an Arid 2.0.0 prerelease; found: $V2_VERSION"

validate_corpus() {
    local name="$1"
    local expected_commit="$2"
    local corpus="$CORPUS_ROOT/$name"
    local root
    local actual_commit

    [[ -d "$corpus" ]] || die "benchmark corpus does not exist: $corpus"
    git -C "$corpus" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
        die "corpus is not a Git repository: $corpus"

    root="$(realpath "$(git -C "$corpus" rev-parse --show-toplevel)")"
    [[ "$root" == "$corpus" ]] || die "corpus is not its Git repository root: $corpus"

    if [[ -n "$(git -C "$corpus" status --porcelain --untracked-files=all)" ]]; then
        echo "error: benchmark corpus must be clean: $corpus" >&2
        git -C "$corpus" status --short >&2
        exit 2
    fi

    actual_commit="$(git -C "$corpus" rev-parse HEAD)"
    [[ "$actual_commit" == "$expected_commit" ]] || {
        echo "error: benchmark corpus is not at the Phase 14 canonical revision" >&2
        echo "repository: $name" >&2
        echo "expected:   $expected_commit" >&2
        echo "actual:     $actual_commit" >&2
        exit 2
    }
}

validate_corpus requests "$REQUESTS_COMMIT"
validate_corpus pydantic "$PYDANTIC_COMMIT"
validate_corpus polaris "$POLARIS_COMMIT"
validate_corpus duplicate-heavy "$DUPLICATE_HEAVY_COMMIT"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

V1_SHA256="$(sha256sum "$V1_BIN" | awk '{print $1}')"
V2_SHA256="$(sha256sum "$V2_BIN" | awk '{print $1}')"
HARNESS_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"

printf -v v1_q '%q' "$V1_BIN"
printf -v v2_q '%q' "$V2_BIN"

run_mode() {
    local repository="$1"
    local mode="$2"
    local extra_args="$3"
    local corpus="$CORPUS_ROOT/$repository"
    local corpus_q
    local v1_cmd
    local v2_cmd
    local pass_a="$RESULT_DIR/$repository-$mode-a.json"
    local pass_b="$RESULT_DIR/$repository-$mode-b.json"

    printf -v corpus_q '%q' "$corpus"

    v1_cmd="$v1_q $corpus_q --hidden $extra_args --workers 1 --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"
    v2_cmd="$v2_q $corpus_q --hidden $extra_args --workers 1 --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

    echo
    echo "==> $repository / $mode / pass A: v1.2 then v2"
    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$pass_a" \
        "$v1_cmd" \
        "$v2_cmd"

    echo
    echo "==> $repository / $mode / pass B: v2 then v1.2"
    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$pass_b" \
        "$v2_cmd" \
        "$v1_cmd"
}

cat <<EOF_HEADER
========================================
 Arid v2 paired serial regression check
========================================

Baseline:  $V1_VERSION
Candidate: $V2_VERSION
Warmups:   $WARMUP per pass
Runs:      $RUNS per pass
EOF_HEADER

for repository in requests pydantic polaris duplicate-heavy; do
    run_mode "$repository" default ""
    run_mode "$repository" pylint-compatible "--no-same-file"
done

average_means() {
    local first="$1"
    local second="$2"
    awk -v first="$first" -v second="$second" 'BEGIN { printf "%.12f", (first + second) / 2.0 }'
}

milliseconds() {
    awk -v seconds="$1" 'BEGIN { printf "%.1f", seconds * 1000.0 }'
}

delta_percent() {
    awk -v baseline="$1" -v candidate="$2" \
        'BEGIN { printf "%+.1f", ((candidate - baseline) / baseline) * 100.0 }'
}

SUMMARY="$RESULT_DIR/summary.txt"
METADATA="$RESULT_DIR/metadata.txt"

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "harness_commit=$HARNESS_COMMIT"
    echo "warmup_per_pass=$WARMUP"
    echo "runs_per_pass=$RUNS"
    echo "v1_version=$V1_VERSION"
    echo "v1_binary=$V1_BIN"
    echo "v1_sha256=$V1_SHA256"
    echo "v2_version=$V2_VERSION"
    echo "v2_binary=$V2_BIN"
    echo "v2_sha256=$V2_SHA256"
    echo "method=two Hyperfine passes per corpus/mode with reversed command order; reported means average both passes"
} >"$METADATA"

{
    echo "Arid v2 paired serial regression summary"
    echo "Baseline:  $V1_VERSION"
    echo "Candidate: $V2_VERSION"
    echo "Warmups:   $WARMUP per pass"
    echo "Runs:      $RUNS per pass"
    echo

    for repository in requests pydantic polaris duplicate-heavy; do
        for mode in default pylint-compatible; do
            pass_a="$RESULT_DIR/$repository-$mode-a.json"
            pass_b="$RESULT_DIR/$repository-$mode-b.json"

            v1_a="$(jq -er '.results[0].mean' "$pass_a")"
            v2_a="$(jq -er '.results[1].mean' "$pass_a")"
            v2_b="$(jq -er '.results[0].mean' "$pass_b")"
            v1_b="$(jq -er '.results[1].mean' "$pass_b")"

            v1_mean="$(average_means "$v1_a" "$v1_b")"
            v2_mean="$(average_means "$v2_a" "$v2_b")"

            printf '  %-16s %-18s v1.2 %8s ms   v2 %8s ms   %s%%\n' \
                "$repository" \
                "$mode" \
                "$(milliseconds "$v1_mean")" \
                "$(milliseconds "$v2_mean")" \
                "$(delta_percent "$v1_mean" "$v2_mean")"
        done
    done
} >"$SUMMARY"

echo
cat "$SUMMARY"
echo
echo "Results: $RESULT_DIR"
