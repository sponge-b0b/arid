#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORPUS_RELATIVE_PATH="validation/arid-corpora"
REPOSITORIES="black,django,mypy,rich"

REPOS="$REPOSITORIES"
V1_BIN_INPUT=""
V2_BIN_INPUT=""

usage() {
    cat <<EOF
Usage: $0 <global-root> --v1-bin <path> [options]

Compare qualified Arid v1.2 detector behavior with v2 on the same real-world
validation corpora beneath:
  <global-root>/$CORPUS_RELATIVE_PATH

Options:
  --v1-bin <path>    Required Arid 1.2.0 executable
  --v2-bin <path>    Existing v2 executable instead of building
                     target/release/arid from the current repository
  --repos <list>     Repositories to compare: $REPOSITORIES
                     Default: $REPOSITORIES
  --help             Show this help

The comparison intentionally ignores only published v2 machine-contract changes:
  - report version/metadata fields
  - finding fingerprint
  - occurrence distribution mixed -> hybrid

Detector-derived metrics, finding order, group structure, structural metadata,
and physical source locations must otherwise match exactly.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

GLOBAL_ROOT_INPUT=""

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
        --repos)
            [[ $# -ge 2 ]] || die "--repos requires a value"
            REPOS="$2"
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

if [[ ! "$REPOS" =~ ^(black|django|mypy|rich)(,(black|django|mypy|rich))*$ ]]; then
    die "--repos must be a comma-separated list containing black, django, mypy, and/or rich"
fi

IFS=',' read -r -a SELECTED_REPOS <<< "$REPOS"

declare -A seen_repos=()
for repository in "${SELECTED_REPOS[@]}"; do
    [[ -z "${seen_repos[$repository]:-}" ]] ||
        die "duplicate repository in --repos: $repository"
    seen_repos["$repository"]=1
done

for command in cmp git jq realpath sha256sum; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -d "$GLOBAL_ROOT_INPUT" ]] ||
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"
RESULTS_DIR="$ROOT_DIR/validation/results/v2-regression"

[[ -d "$CORPUS_ROOT" ]] ||
    die "validation corpus root does not exist: $CORPUS_ROOT"

resolve_binary() {
    local input="$1"
    local label="$2"

    [[ -f "$input" ]] || die "$label executable does not exist: $input"
    [[ -x "$input" ]] || die "$label executable is not executable: $input"
    realpath "$input"
}

V1_BIN="$(resolve_binary "$V1_BIN_INPUT" "v1.2")"

if [[ -n "$V2_BIN_INPUT" ]]; then
    V2_BIN="$(resolve_binary "$V2_BIN_INPUT" "v2")"
    V2_SOURCE="external"
else
    command -v cargo >/dev/null 2>&1 ||
        die "required command not found: cargo"

    echo "Building v2 release binary..."
    cargo build \
        --release \
        --locked \
        --manifest-path "$ROOT_DIR/Cargo.toml"

    V2_BIN="$ROOT_DIR/target/release/arid"
    V2_SOURCE="repository"
fi

V1_VERSION="$("$V1_BIN" --version)"
V2_VERSION="$("$V2_BIN" --version)"

[[ "$V1_VERSION" == "arid 1.2.0" ]] ||
    die "v1 executable must identify itself as 'arid 1.2.0', got: $V1_VERSION"
[[ "$V2_VERSION" == arid\ 2.0.0-* ]] ||
    die "v2 executable must identify itself as a 2.0.0 prerelease, got: $V2_VERSION"

echo "v1 baseline: $V1_VERSION"
echo "v2 candidate: $V2_VERSION"

validate_repository() {
    local repository="$1"
    local corpus="$CORPUS_ROOT/$repository"
    local repository_root

    [[ -d "$corpus" ]] || die "validation corpus does not exist: $corpus"

    git -C "$corpus" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
        die "corpus is not a Git repository: $corpus"

    repository_root="$(realpath "$(git -C "$corpus" rev-parse --show-toplevel)")"
    [[ "$repository_root" == "$corpus" ]] ||
        die "corpus must be the Git repository root: $corpus"

    [[ -z "$(git -C "$corpus" status --porcelain --untracked-files=all)" ]] ||
        die "corpus working tree is not clean: $corpus"
}

for repository in "${SELECTED_REPOS[@]}"; do
    validate_repository "$repository"
done

rm -rf "$RESULTS_DIR"
mkdir -p "$RESULTS_DIR"

canonicalize_v1() {
    local source="$1"
    local destination="$2"

    jq -S '
        {
            files,
            source_lines,
            analyzed_lines,
            duplicate_groups,
            duplicate_lines,
            duplication_percent,
            findings: [
                .findings[] | {
                    code,
                    lines,
                    context,
                    scope,
                    occurrences,
                    files,
                    distribution: (
                        if .distribution == "mixed" then "hybrid"
                        else .distribution
                        end
                    ),
                    locations
                }
            ]
        }
    ' "$source" >"$destination"
}

canonicalize_v2() {
    local source="$1"
    local destination="$2"

    jq -S '
        {
            files,
            source_lines,
            analyzed_lines,
            duplicate_groups,
            duplicate_lines,
            duplication_percent,
            findings: [
                .findings[] | {
                    code,
                    lines,
                    context,
                    scope,
                    occurrences,
                    files,
                    distribution,
                    locations
                }
            ]
        }
    ' "$source" >"$destination"
}

run_binary() {
    local binary="$1"
    local repository="$2"
    local output="$3"
    local stderr_file="$4"
    shift 4

    local corpus="$CORPUS_ROOT/$repository"
    local status
    local expected_status
    local findings
    local -a args=(
        "$binary"
        "$corpus"
        --hidden
        --workers 1
        --json
    )

    local exclude
    for exclude in "$@"; do
        args+=(--exclude "$exclude")
    done

    set +e
    "${args[@]}" >"$output" 2>"$stderr_file"
    status=$?
    set -e

    [[ "$status" -eq 0 || "$status" -eq 1 ]] || {
        echo "error: scan failed for $repository with $binary (exit $status)" >&2
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        exit 2
    }

    [[ ! -s "$stderr_file" ]] || {
        echo "error: scan produced stderr for $repository with $binary" >&2
        cat "$stderr_file" >&2
        exit 2
    }

    jq -e . "$output" >/dev/null ||
        die "scan did not produce valid JSON: $repository"

    findings="$(jq -r '.findings | length' "$output")"
    if [[ "$findings" -eq 0 ]]; then
        expected_status=0
    else
        expected_status=1
    fi

    [[ "$status" -eq "$expected_status" ]] ||
        die "scan exit status does not match finding state: $repository"
}

compare_repository() {
    local repository="$1"
    shift

    local repository_results="$RESULTS_DIR/$repository"
    local v1_json="$repository_results/v1.2.json"
    local v1_stderr="$repository_results/v1.2.stderr"
    local v1_canonical="$repository_results/v1.2-canonical.json"
    local v2_json="$repository_results/v2.json"
    local v2_stderr="$repository_results/v2.stderr"
    local v2_canonical="$repository_results/v2-canonical.json"
    local -a exclusions=("$@")

    mkdir -p "$repository_results"

    echo
    echo "========================================"
    echo " Detector regression: $repository"
    echo "========================================"
    echo "Commit: $(git -C "$CORPUS_ROOT/$repository" rev-parse HEAD)"

    run_binary \
        "$V1_BIN" \
        "$repository" \
        "$v1_json" \
        "$v1_stderr" \
        "${exclusions[@]}"

    run_binary \
        "$V2_BIN" \
        "$repository" \
        "$v2_json" \
        "$v2_stderr" \
        "${exclusions[@]}"

    jq -e '.version == 3' "$v1_json" >/dev/null ||
        die "v1.2 scan did not produce report-v3: $repository"

    jq -e '
        .schema_version == 4
        and .complete == true
        and .errors == []
    ' "$v2_json" >/dev/null ||
        die "v2 scan did not produce a complete report-v4: $repository"

    canonicalize_v1 "$v1_json" "$v1_canonical"
    canonicalize_v2 "$v2_json" "$v2_canonical"

    if ! cmp -s "$v1_canonical" "$v2_canonical"; then
        echo "error: detector-level regression detected: $repository" >&2
        echo >&2
        echo "v1.2 canonical SHA-256:" >&2
        sha256sum "$v1_canonical" >&2
        echo "v2 canonical SHA-256:" >&2
        sha256sum "$v2_canonical" >&2
        echo >&2
        echo "Compare:" >&2
        echo "  diff -u '$v1_canonical' '$v2_canonical'" >&2
        exit 2
    fi

    echo "Canonical detector behavior: IDENTICAL"
    echo "Canonical SHA-256: $(sha256sum "$v2_canonical" | awk '{print $1}')"
    echo "Findings: $(jq -r '.findings | length' "$v2_json")"
    echo "Duplicate lines: $(jq -r '.duplicate_lines' "$v2_json")"
}

{
    echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "harness_commit=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    echo "global_root=$GLOBAL_ROOT"
    echo "corpus_root=$CORPUS_ROOT"
    echo "selected_repositories=$REPOS"
    echo "v1_binary=$V1_BIN"
    echo "v1_version=$V1_VERSION"
    echo "v1_sha256=$(sha256sum "$V1_BIN" | awk '{print $1}')"
    echo "v2_binary=$V2_BIN"
    echo "v2_version=$V2_VERSION"
    echo "v2_sha256=$(sha256sum "$V2_BIN" | awk '{print $1}')"
    echo "v2_source=$V2_SOURCE"

    if [[ "$V2_SOURCE" == "repository" ]]; then
        echo "v2_commit=$(git -C "$ROOT_DIR" rev-parse HEAD)"
    fi
} >"$RESULTS_DIR/metadata.txt"

for repository in "${SELECTED_REPOS[@]}"; do
    echo "${repository}_commit=$(git -C "$CORPUS_ROOT/$repository" rev-parse HEAD)" \
        >>"$RESULTS_DIR/metadata.txt"

    case "$repository" in
        black)
            compare_repository \
                black \
                'tests/data/**' \
                'profiling/**'
            ;;
        django)
            compare_repository \
                django \
                'tests/test_runner_apps/tagged/tests_syntax_error.py'
            ;;
        mypy)
            compare_repository \
                mypy \
                'test-data/**'
            ;;
        rich)
            compare_repository rich
            ;;
    esac
done

echo
echo "========================================"
echo " V2 detector regression validation PASS"
echo "========================================"
echo
echo "Compared against: $V1_VERSION"
echo "Candidate:        $V2_VERSION"
echo "Repositories:     $REPOS"
echo "Results:          $RESULTS_DIR"
