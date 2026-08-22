#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="$ROOT_DIR/benchmarks/run.sh"
RESULT_ROOT="$ROOT_DIR/benchmarks/results"
SUMMARY_DIR="$RESULT_ROOT/v2-phase14"

REQUESTS_COMMIT="6e83187b8feb273ed4c6cdab5efd8d54901dfab3"
PYDANTIC_COMMIT="cf67d4b3193c3fe43ede18612ed62785eee11382"
POLARIS_COMMIT="00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031"
DUPLICATE_HEAVY_COMMIT="815510e4ba755496ce3d2d4c4eea3d89afbc2ffc"

DEFAULT_WARMUP=3
DEFAULT_RUNS=10
WORKERS="1,2,4,8,auto"

usage() {
    cat <<EOF_USAGE
Usage: $0 <global-root> --v1-bin <path> [options]

Run Arid v2 Phase 14 performance qualification against the pinned v1.2
benchmark corpora and an exact Arid 1.2.0 executable.

Options:
  --v1-bin <path>     Exact Arid 1.2.0 executable. Required.
  --v2-bin <path>     Existing v2 executable. Default: build target/release/arid
                      from the current repository.
  --warmup <N>        Hyperfine warmup runs. Default: $DEFAULT_WARMUP
  --runs <N>          Hyperfine measured runs. Default: $DEFAULT_RUNS
  --help              Show this help.

The qualification campaign:
  - verifies the canonical Requests, Pydantic, Polaris, and duplicate-heavy pins
  - benchmarks v1.2 and v2 serial/worker modes on identical hardware
  - reruns the established v2 Pylint and jscpd comparisons
  - enforces the >=10x Pylint floor on Pydantic and Polaris
  - records v1.2 -> v2 timing deltas without inventing a regression threshold

Provision the exact corpora first with:

  benchmarks/build.sh <global-root> \
    --requests $REQUESTS_COMMIT \
    --pydantic $PYDANTIC_COMMIT \
    --polaris $POLARIS_COMMIT
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

for command in git jq awk realpath sha256sum; do
    command -v "$command" >/dev/null 2>&1 || die "required command not found: $command"
done

[[ -x "$RUNNER" ]] || die "benchmark runner is not executable: $RUNNER"
[[ -d "$GLOBAL_ROOT_INPUT" ]] || die "global root is not a directory: $GLOBAL_ROOT_INPUT"
[[ -f "$V1_BIN_INPUT" && -x "$V1_BIN_INPUT" ]] || die "v1 executable is not executable: $V1_BIN_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/benchmarks/arid-corpora"
V1_BIN="$(realpath "$V1_BIN_INPUT")"

validate_git_corpus() {
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
        echo "rebuild the pinned corpora with benchmarks/build.sh before qualification" >&2
        exit 2
    }
}

validate_git_corpus requests "$REQUESTS_COMMIT"
validate_git_corpus pydantic "$PYDANTIC_COMMIT"
validate_git_corpus polaris "$POLARIS_COMMIT"
validate_git_corpus duplicate-heavy "$DUPLICATE_HEAVY_COMMIT"

V1_VERSION="$("$V1_BIN" --version)"
[[ "$V1_VERSION" == "arid 1.2.0" ]] || die "--v1-bin must report exactly 'arid 1.2.0'; found: $V1_VERSION"

if [[ -n "$V2_BIN_INPUT" ]]; then
    [[ -f "$V2_BIN_INPUT" && -x "$V2_BIN_INPUT" ]] || die "v2 executable is not executable: $V2_BIN_INPUT"
    V2_BIN="$(realpath "$V2_BIN_INPUT")"
    V2_SOURCE="external"
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
    V2_SOURCE="repository"
fi

V2_VERSION="$("$V2_BIN" --version)"
[[ "$V2_VERSION" == arid\ 2.0.0-* ]] || die "v2 executable must report an Arid 2.0.0 prerelease; found: $V2_VERSION"

V1_SHA256="$(sha256sum "$V1_BIN" | awk '{print $1}')"
V2_SHA256="$(sha256sum "$V2_BIN" | awk '{print $1}')"
HARNESS_COMMIT="$(git -C "$ROOT_DIR" rev-parse HEAD)"

cat <<EOF_HEADER
========================================
 Arid v2 Phase 14 performance qualification
========================================

Baseline:  $V1_VERSION
Candidate: $V2_VERSION
Warmups:   $WARMUP
Runs:      $RUNS
Workers:   $WORKERS
EOF_HEADER

echo
echo "==> Benchmarking Arid 1.2.0 across canonical and duplicate-heavy corpora"
"$RUNNER" "$GLOBAL_ROOT" \
    --repos requests,pydantic,polaris,duplicate-heavy \
    --tools arid \
    --workers "$WORKERS" \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --arid-bin "$V1_BIN" \
    --label phase14-v1.2

echo
echo "==> Benchmarking v2 across canonical real-world corpora and current comparison tools"
"$RUNNER" "$GLOBAL_ROOT" \
    --repos requests,pydantic,polaris \
    --tools arid,pylint,jscpd \
    --workers "$WORKERS" \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --arid-bin "$V2_BIN" \
    --label phase14-v2

echo
echo "==> Benchmarking v2 on the duplicate-heavy stress corpus"
"$RUNNER" "$GLOBAL_ROOT" \
    --repos duplicate-heavy \
    --tools arid \
    --workers "$WORKERS" \
    --warmup "$WARMUP" \
    --runs "$RUNS" \
    --arid-bin "$V2_BIN" \
    --label phase14-v2

rm -rf "$SUMMARY_DIR"
mkdir -p "$SUMMARY_DIR"

mean_at() {
    local path="$1"
    local index="$2"
    jq -er --argjson index "$index" '.results[$index].mean' "$path"
}

worker_mean() {
    local path="$1"
    local worker="$2"
    jq -er --arg worker "$worker" \
        '.results[] | select((.parameters.workers | tostring) == $worker) | .mean' \
        "$path"
}

milliseconds() {
    awk -v seconds="$1" 'BEGIN { printf "%.1f", seconds * 1000.0 }'
}

delta_percent() {
    awk -v baseline="$1" -v candidate="$2" \
        'BEGIN { printf "%+.1f", ((candidate - baseline) / baseline) * 100.0 }'
}

speed_ratio() {
    awk -v slower="$1" -v faster="$2" 'BEGIN { printf "%.2f", slower / faster }'
}

ratio_meets_floor() {
    awk -v ratio="$1" 'BEGIN { exit !(ratio >= 10.0) }'
}

SUMMARY_MD="$SUMMARY_DIR/summary.md"
SUMMARY_TXT="$SUMMARY_DIR/summary.txt"
METADATA="$SUMMARY_DIR/metadata.txt"

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "harness_commit=$HARNESS_COMMIT"
    echo "warmup=$WARMUP"
    echo "runs=$RUNS"
    echo "workers=$WORKERS"
    echo "v1_binary=$V1_BIN"
    echo "v1_version=$V1_VERSION"
    echo "v1_sha256=$V1_SHA256"
    echo "v2_binary=$V2_BIN"
    echo "v2_source=$V2_SOURCE"
    echo "v2_version=$V2_VERSION"
    echo "v2_sha256=$V2_SHA256"
    echo "requests_commit=$REQUESTS_COMMIT"
    echo "pydantic_commit=$PYDANTIC_COMMIT"
    echo "polaris_commit=$POLARIS_COMMIT"
    echo "duplicate_heavy_commit=$DUPLICATE_HEAVY_COMMIT"
    echo "pylint_required=4.0.6"
    echo "jscpd_required=5.0.12"
    echo "hyperfine_required=1.20.0"
} >"$METADATA"

{
    echo "# Arid v2 Phase 14 benchmark summary"
    echo
    echo "Baseline: \`$V1_VERSION\`  "
    echo "Candidate: \`$V2_VERSION\`  "
    echo "Warmups: $WARMUP  "
    echo "Measured runs: $RUNS"
    echo
    echo "## Serial Arid regression comparison"
    echo
    echo "| Corpus | v1.2 | v2 | Change |"
    echo "| --- | ---: | ---: | ---: |"

    for repository in requests pydantic polaris duplicate-heavy; do
        v1_mean="$(mean_at "$RESULT_ROOT/$repository-phase14-v1.2/arid.json" 0)"
        v2_mean="$(mean_at "$RESULT_ROOT/$repository-phase14-v2/arid.json" 0)"
        echo "| $repository | $(milliseconds "$v1_mean") ms | $(milliseconds "$v2_mean") ms | $(delta_percent "$v1_mean" "$v2_mean")% |"
    done

    echo
    echo "Positive change means v2 measured slower; negative means faster. The roadmap does not define an arbitrary percentage regression threshold, so meaningful positive deltas require engineering review rather than automatic rejection."
    echo
    echo "## Worker-mode comparison"
    echo
    echo "| Corpus | Workers | v1.2 | v2 | Change |"
    echo "| --- | ---: | ---: | ---: | ---: |"

    for repository in requests pydantic polaris duplicate-heavy; do
        for worker in 1 2 4 8 auto; do
            v1_mean="$(worker_mean "$RESULT_ROOT/$repository-phase14-v1.2/arid-workers.json" "$worker")"
            v2_mean="$(worker_mean "$RESULT_ROOT/$repository-phase14-v2/arid-workers.json" "$worker")"
            echo "| $repository | $worker | $(milliseconds "$v1_mean") ms | $(milliseconds "$v2_mean") ms | $(delta_percent "$v1_mean" "$v2_mean")% |"
        done
    done

    echo
    echo "## Current stable Pylint comparison"
    echo
    echo "| Corpus | Arid v2 | Pylint 4.0.6 | Advantage | Gate |"
    echo "| --- | ---: | ---: | ---: | --- |"
} >"$SUMMARY_MD"

PYLINT_GATE_PASS=true

for repository in requests pydantic polaris; do
    comparison="$RESULT_ROOT/$repository-phase14-v2/pylint.json"
    arid_mean="$(mean_at "$comparison" 0)"
    pylint_mean="$(mean_at "$comparison" 1)"
    ratio="$(speed_ratio "$pylint_mean" "$arid_mean")"
    gate="informational"

    if [[ "$repository" == "pydantic" || "$repository" == "polaris" ]]; then
        if ratio_meets_floor "$ratio"; then
            gate="PASS"
        else
            gate="FAIL"
            PYLINT_GATE_PASS=false
        fi
    fi

    echo "| $repository | $(milliseconds "$arid_mean") ms | $(milliseconds "$pylint_mean") ms | ${ratio}x | $gate |" >>"$SUMMARY_MD"
done

{
    echo
    echo "The Phase 14 release floor is at least 10x faster than isolated Pylint duplicate detection on the canonical medium (Pydantic) and large (Polaris) corpora."
    echo
    echo "## jscpd comparison"
    echo
    echo "| Corpus | Serial jscpd advantage | Auto jscpd advantage |"
    echo "| --- | ---: | ---: |"
} >>"$SUMMARY_MD"

for repository in requests pydantic polaris; do
    serial="$RESULT_ROOT/$repository-phase14-v2/jscpd-serial.json"
    auto="$RESULT_ROOT/$repository-phase14-v2/jscpd-auto.json"
    arid_serial="$(mean_at "$serial" 0)"
    jscpd_serial="$(mean_at "$serial" 1)"
    arid_auto="$(mean_at "$auto" 0)"
    jscpd_auto="$(mean_at "$auto" 1)"
    serial_ratio="$(speed_ratio "$jscpd_serial" "$arid_serial")"
    auto_ratio="$(speed_ratio "$jscpd_auto" "$arid_auto")"
    echo "| $repository | ${serial_ratio}x | ${auto_ratio}x |" >>"$SUMMARY_MD"
done

{
    echo "Arid v2 Phase 14 benchmark summary"
    echo "Baseline:  $V1_VERSION"
    echo "Candidate: $V2_VERSION"
    echo "Warmups:   $WARMUP"
    echo "Runs:      $RUNS"
    echo
    echo "Serial regression comparison:"

    for repository in requests pydantic polaris duplicate-heavy; do
        v1_mean="$(mean_at "$RESULT_ROOT/$repository-phase14-v1.2/arid.json" 0)"
        v2_mean="$(mean_at "$RESULT_ROOT/$repository-phase14-v2/arid.json" 0)"
        printf '  %-16s v1.2 %8s ms   v2 %8s ms   %s%%\n' \
            "$repository" \
            "$(milliseconds "$v1_mean")" \
            "$(milliseconds "$v2_mean")" \
            "$(delta_percent "$v1_mean" "$v2_mean")"
    done

    echo
    echo "Pylint 4.0.6 comparison:"

    for repository in requests pydantic polaris; do
        comparison="$RESULT_ROOT/$repository-phase14-v2/pylint.json"
        arid_mean="$(mean_at "$comparison" 0)"
        pylint_mean="$(mean_at "$comparison" 1)"
        ratio="$(speed_ratio "$pylint_mean" "$arid_mean")"
        printf '  %-16s %sx faster\n' "$repository" "$ratio"
    done
} >"$SUMMARY_TXT"

echo
echo "========================================"
if [[ "$PYLINT_GATE_PASS" == true ]]; then
    echo " Phase 14 explicit Pylint gate PASS"
else
    echo " Phase 14 explicit Pylint gate FAIL"
fi
echo "========================================"
echo
echo "Summary: $SUMMARY_MD"
echo "Metadata: $METADATA"
echo
cat "$SUMMARY_TXT"

if [[ "$PYLINT_GATE_PASS" != true ]]; then
    exit 1
fi
