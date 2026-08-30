#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULT_DIR="$ROOT_DIR/benchmarks/results/v2.2-prepublication"
CORPUS_ROOT_RELATIVE="benchmarks/arid-corpora"

REQUESTS_COMMIT="6e83187b8feb273ed4c6cdab5efd8d54901dfab3"
PYDANTIC_COMMIT="cf67d4b3193c3fe43ede18612ed62785eee11382"
POLARIS_COMMIT="00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031"
DUPLICATE_HEAVY_COMMIT="815510e4ba755496ce3d2d4c4eea3d89afbc2ffc"

DEFAULT_WARMUP=3
DEFAULT_RUNS=10

usage() {
    cat <<EOF
Usage: $0 <global-root> --stable-bin <path> [options]

Compare stable Arid 2.1.0 with the v2.2 candidate across the canonical small,
medium, large, and duplicate-heavy benchmark corpora.

Options:
  --stable-bin <path>   Exact Arid 2.1.0 executable. Required.
  --candidate-bin <path>
                        Existing v2.2 candidate executable. Default: build
                        target/release/arid from the current clean repository.
  --warmup <N>          Hyperfine warmup runs per pass. Default: $DEFAULT_WARMUP
  --runs <N>            Hyperfine measured runs per pass. Default: $DEFAULT_RUNS
  --help                Show this help.

Candidate modes measured for every corpus:
  implicit              no --workers argument
  serial                --workers 1
  auto                  --workers auto

The stable baseline uses its 2.1 implicit default. Each corpus is measured twice
with reversed command order. Reported means average both passes to reduce
ordering/load bias. The harness records evidence; it does not impose an
arbitrary millisecond or percentage SLA.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

GLOBAL_ROOT_INPUT=""
STABLE_BIN_INPUT=""
CANDIDATE_BIN_INPUT=""
WARMUP="$DEFAULT_WARMUP"
RUNS="$DEFAULT_RUNS"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --stable-bin)
            [[ $# -ge 2 ]] || die "--stable-bin requires a value"
            [[ -z "$STABLE_BIN_INPUT" ]] || die "duplicate option: --stable-bin"
            STABLE_BIN_INPUT="$2"
            shift 2
            ;;
        --candidate-bin)
            [[ $# -ge 2 ]] || die "--candidate-bin requires a value"
            [[ -z "$CANDIDATE_BIN_INPUT" ]] || die "duplicate option: --candidate-bin"
            CANDIDATE_BIN_INPUT="$2"
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
[[ -n "$STABLE_BIN_INPUT" ]] || die "--stable-bin is required"
[[ "$WARMUP" =~ ^[0-9]+$ ]] || die "--warmup must be a non-negative integer"
[[ "$RUNS" =~ ^[1-9][0-9]*$ ]] || die "--runs must be a positive integer"

for command in awk git hyperfine jq realpath sha256sum; do
    command -v "$command" >/dev/null 2>&1 || die "required command not found: $command"
done

[[ "$(hyperfine --version)" == "hyperfine 1.20.0" ]] ||
    die "expected hyperfine 1.20.0, found $(hyperfine --version)"
[[ -d "$GLOBAL_ROOT_INPUT" ]] || die "global root is not a directory: $GLOBAL_ROOT_INPUT"
[[ -f "$STABLE_BIN_INPUT" && -x "$STABLE_BIN_INPUT" ]] ||
    die "stable executable is not executable: $STABLE_BIN_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_ROOT_RELATIVE"
STABLE_BIN="$(realpath "$STABLE_BIN_INPUT")"

STABLE_VERSION="$("$STABLE_BIN" --version)"
[[ "$STABLE_VERSION" == "arid 2.1.0" ]] ||
    die "--stable-bin must report exactly 'arid 2.1.0'; found: $STABLE_VERSION"

if [[ -n "$CANDIDATE_BIN_INPUT" ]]; then
    [[ -f "$CANDIDATE_BIN_INPUT" && -x "$CANDIDATE_BIN_INPUT" ]] ||
        die "candidate executable is not executable: $CANDIDATE_BIN_INPUT"
    CANDIDATE_BIN="$(realpath "$CANDIDATE_BIN_INPUT")"
else
    command -v cargo >/dev/null 2>&1 || die "required command not found: cargo"

    if [[ -n "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ]]; then
        echo "error: Arid working tree must be clean before building the candidate benchmark binary" >&2
        git -C "$ROOT_DIR" status --short >&2
        exit 2
    fi

    echo "Building the v2.2 candidate release binary..."
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"
    CANDIDATE_BIN="$ROOT_DIR/target/release/arid"
fi

CANDIDATE_VERSION="$("$CANDIDATE_BIN" --version)"
[[ "$CANDIDATE_VERSION" == arid\ * ]] ||
    die "candidate executable does not identify itself as Arid: $CANDIDATE_VERSION"

if [[ "$STABLE_BIN" == "$CANDIDATE_BIN" ]]; then
    die "stable and candidate executables resolve to the same path"
fi

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
        echo "error: benchmark corpus is not at the canonical revision" >&2
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

STABLE_SHA256="$(sha256sum "$STABLE_BIN" | awk '{print $1}')"
CANDIDATE_SHA256="$(sha256sum "$CANDIDATE_BIN" | awk '{print $1}')"
HARNESS_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"

printf -v stable_q '%q' "$STABLE_BIN"
printf -v candidate_q '%q' "$CANDIDATE_BIN"

run_corpus() {
    local repository="$1"
    local corpus="$CORPUS_ROOT/$repository"
    local corpus_q
    local stable_cmd
    local implicit_cmd
    local serial_cmd
    local auto_cmd
    local pass_a="$RESULT_DIR/$repository-a.json"
    local pass_b="$RESULT_DIR/$repository-b.json"

    printf -v corpus_q '%q' "$corpus"

    stable_cmd="$stable_q $corpus_q --hidden --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"
    implicit_cmd="$candidate_q $corpus_q --hidden --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"
    serial_cmd="$candidate_q $corpus_q --hidden --workers 1 --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"
    auto_cmd="$candidate_q $corpus_q --hidden --workers auto --json > /dev/null; status=\$?; [[ \$status -eq 0 || \$status -eq 1 ]]"

    echo
    echo "==> $repository / pass A: stable, implicit, serial, auto"
    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$pass_a" \
        "$stable_cmd" \
        "$implicit_cmd" \
        "$serial_cmd" \
        "$auto_cmd"

    echo
    echo "==> $repository / pass B: auto, serial, implicit, stable"
    hyperfine \
        --shell=bash \
        --warmup "$WARMUP" \
        --runs "$RUNS" \
        --export-json "$pass_b" \
        "$auto_cmd" \
        "$serial_cmd" \
        "$implicit_cmd" \
        "$stable_cmd"
}

cat <<EOF
========================================
 Arid v2.2 performance qualification
========================================

Stable:    $STABLE_VERSION
Candidate: $CANDIDATE_VERSION
Warmups:   $WARMUP per pass
Runs:      $RUNS per pass
EOF

for repository in requests pydantic polaris duplicate-heavy; do
    run_corpus "$repository"
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

speedup() {
    awk -v baseline="$1" -v candidate="$2" \
        'BEGIN { printf "%.2f", baseline / candidate }'
}

SUMMARY="$RESULT_DIR/summary.txt"
METADATA="$RESULT_DIR/metadata.txt"

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "harness_commit=$HARNESS_COMMIT"
    echo "warmup_per_pass=$WARMUP"
    echo "runs_per_pass=$RUNS"
    echo "stable_version=$STABLE_VERSION"
    echo "stable_binary=$STABLE_BIN"
    echo "stable_sha256=$STABLE_SHA256"
    echo "candidate_version=$CANDIDATE_VERSION"
    echo "candidate_binary=$CANDIDATE_BIN"
    echo "candidate_sha256=$CANDIDATE_SHA256"
    echo "method=two Hyperfine passes per corpus with reversed command order; reported means average both passes"
} >"$METADATA"

{
    echo "Arid v2.2 performance qualification summary"
    echo "Stable:    $STABLE_VERSION"
    echo "Candidate: $CANDIDATE_VERSION"
    echo "Warmups:   $WARMUP per pass"
    echo "Runs:      $RUNS per pass"
    echo
    printf '%-18s %10s %10s %10s %10s %10s %10s\n' \
        "Corpus" "2.1 impl" "2.2 impl" "2.2 w1" "2.2 auto" "impl Δ" "impl/w1"
    printf '%-18s %10s %10s %10s %10s %10s %10s\n' \
        "------------------" "----------" "----------" "----------" "----------" "----------" "----------"

    for repository in requests pydantic polaris duplicate-heavy; do
        pass_a="$RESULT_DIR/$repository-a.json"
        pass_b="$RESULT_DIR/$repository-b.json"

        stable_a="$(jq -er '.results[0].mean' "$pass_a")"
        implicit_a="$(jq -er '.results[1].mean' "$pass_a")"
        serial_a="$(jq -er '.results[2].mean' "$pass_a")"
        auto_a="$(jq -er '.results[3].mean' "$pass_a")"

        auto_b="$(jq -er '.results[0].mean' "$pass_b")"
        serial_b="$(jq -er '.results[1].mean' "$pass_b")"
        implicit_b="$(jq -er '.results[2].mean' "$pass_b")"
        stable_b="$(jq -er '.results[3].mean' "$pass_b")"

        stable_mean="$(average_means "$stable_a" "$stable_b")"
        implicit_mean="$(average_means "$implicit_a" "$implicit_b")"
        serial_mean="$(average_means "$serial_a" "$serial_b")"
        auto_mean="$(average_means "$auto_a" "$auto_b")"

        printf '%-18s %10s %10s %10s %10s %9s%% %9sx\n' \
            "$repository" \
            "$(milliseconds "$stable_mean")" \
            "$(milliseconds "$implicit_mean")" \
            "$(milliseconds "$serial_mean")" \
            "$(milliseconds "$auto_mean")" \
            "$(delta_percent "$stable_mean" "$implicit_mean")" \
            "$(speedup "$serial_mean" "$implicit_mean")"
    done
} >"$SUMMARY"

echo
cat "$SUMMARY"
echo
echo "Results: $RESULT_DIR"
