#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

CORPUS_RELATIVE_PATH="validation/arid-corpora"
REPOSITORIES="black,django,mypy,rich"

REPOS="$REPOSITORIES"
ARID_BIN_INPUT=""

usage() {
    cat <<EOF
Usage: $0 <global-root> [options]

Validate Arid against corpora beneath:
  <global-root>/$CORPUS_RELATIVE_PATH

Options:
  --repos <list>      Repositories to validate: $REPOSITORIES
                      Default: $REPOSITORIES
  --arid-bin <path>  Validate an existing Arid executable instead of building
                     target/release/arid from the current repository
  --help             Show this help

Examples:
  # Validate all repositories using the current repository release build
  $0 /home/bobt

  # Validate selected repositories
  $0 /home/bobt --repos rich,mypy

  # Validate a previously built or published Arid executable
  $0 /home/bobt --arid-bin /path/to/arid
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

GLOBAL_ROOT_INPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repos)
            [[ $# -ge 2 ]] || die "--repos requires a value"
            REPOS="$2"
            shift 2
            ;;
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

if [[ ! "$REPOS" =~ ^(black|django|mypy|rich)(,(black|django|mypy|rich))*$ ]]; then
    die "--repos must be a comma-separated list containing black, django, mypy, and/or rich"
fi

IFS=',' read -r -a SELECTED_REPOS <<< "$REPOS"

declare -A seen_repos=()

for repository in "${SELECTED_REPOS[@]}"; do
    if [[ -n "${seen_repos[$repository]:-}" ]]; then
        die "duplicate repository in --repos: $repository"
    fi

    seen_repos["$repository"]=1
done

for command in cmp git grep jq realpath rg sha256sum; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

if [[ -z "$ARID_BIN_INPUT" ]]; then
    command -v cargo >/dev/null 2>&1 ||
        die "required command not found: cargo"
fi

if [[ ! -d "$GLOBAL_ROOT_INPUT" ]]; then
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"
fi

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"
RESULTS_DIR="$ROOT_DIR/validation/results"

if [[ -n "$ARID_BIN_INPUT" ]]; then
    [[ -f "$ARID_BIN_INPUT" ]] ||
        die "Arid executable does not exist: $ARID_BIN_INPUT"
    [[ -x "$ARID_BIN_INPUT" ]] ||
        die "Arid executable is not executable: $ARID_BIN_INPUT"

    ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
    ARID_SOURCE="external"
else
    ARID_BIN="$ROOT_DIR/target/release/arid"
    ARID_SOURCE="repository"
fi

if [[ ! -d "$CORPUS_ROOT" ]]; then
    die "validation corpus root does not exist: $CORPUS_ROOT; run validation/build.sh $GLOBAL_ROOT first"
fi

for repository in "${SELECTED_REPOS[@]}"; do
    corpus="$CORPUS_ROOT/$repository"

    if [[ ! -d "$corpus" ]]; then
        die "validation corpus does not exist: $corpus; run validation/build.sh $GLOBAL_ROOT first"
    fi
done

validate_repository() {
    local repository="$1"
    local corpus="$CORPUS_ROOT/$repository"
    local repository_root

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
}

for repository in "${SELECTED_REPOS[@]}"; do
    validate_repository "$repository"
done

if [[ "$ARID_SOURCE" == "repository" ]]; then
    echo "Building Arid release binary..."

    cargo build \
        --release \
        --locked \
        --manifest-path "$ROOT_DIR/Cargo.toml"
else
    echo "Using existing Arid executable:"
    echo "  $ARID_BIN"
fi

ARID_VERSION="$("$ARID_BIN" --version)"
[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"
ARID_SHA256="$(sha256sum "$ARID_BIN" | awk '{print $1}')"

echo "Arid version: $ARID_VERSION"

mkdir -p "$RESULTS_DIR"

# Refresh only the validation targets selected for this run. Results from
# unselected repositories are left untouched.
for repository in "${SELECTED_REPOS[@]}"; do
    rm -rf "$RESULTS_DIR/$repository"
    mkdir -p "$RESULTS_DIR/$repository"
done

# Path handling is always validated, regardless of --repos selection.
rm -rf "$RESULTS_DIR/path-cases"
mkdir -p "$RESULTS_DIR/path-cases"

rm -f "$RESULTS_DIR/metadata.txt"

expect_invalid_source() {
    local repository="$1"
    local relative_path="$2"
    local corpus="$CORPUS_ROOT/$repository"
    local source="$corpus/$relative_path"
    local stdout_file
    local stderr_file
    local status

    [[ -f "$source" ]] ||
        die "expected invalid-source fixture does not exist: $source"

    stdout_file="$(mktemp)"
    stderr_file="$(mktemp)"

    # Run the entire unfiltered repository. This verifies that Arid does not
    # silently skip malformed Python during normal directory discovery.
    set +e
    "$ARID_BIN" \
        "$corpus" \
        --hidden \
        --workers 1 \
        --json \
        >"$stdout_file" \
        2>"$stderr_file"
    status=$?
    set -e

    if [[ "$status" -ne 2 ]]; then
        echo "error: expected raw corpus to fail with exit 2" >&2
        echo "repository: $repository" >&2
        echo "fixture:    $relative_path" >&2
        echo "exit:       $status" >&2

        if [[ -s "$stderr_file" ]]; then
            cat "$stderr_file" >&2
        fi

        rm -f "$stdout_file" "$stderr_file"
        exit 2
    fi

    if ! grep -Fq "$relative_path" "$stderr_file"; then
        echo "error: invalid-source diagnostic did not identify the expected fixture" >&2
        echo "repository: $repository" >&2
        echo "fixture:    $relative_path" >&2
        cat "$stderr_file" >&2
        rm -f "$stdout_file" "$stderr_file"
        exit 2
    fi

    if ! grep -Fq "invalid Python syntax" "$stderr_file"; then
        echo "error: invalid-source diagnostic did not report invalid Python syntax" >&2
        cat "$stderr_file" >&2
        rm -f "$stdout_file" "$stderr_file"
        exit 2
    fi

    rm -f "$stdout_file" "$stderr_file"

    echo "Invalid-source probe: PASS"
    echo "  $relative_path"
}

count_discoverable_files() {
    local corpus="$1"
    shift

    local file_list
    local status
    local count

    local -a rg_args=(
        --files
        --hidden
        -g '*.py'
        -g '*.pyi'
    )

    local exclude

    for exclude in "$@"; do
        rg_args+=(-g "!$exclude")
    done

    file_list="$(mktemp)"

    set +e
    (
        cd "$corpus"
        rg "${rg_args[@]}" >"$file_list"
    )
    status=$?
    set -e

    if [[ "$status" -ne 0 && "$status" -ne 1 ]]; then
        rm -f "$file_list"
        die "unable to enumerate discoverable Python files in $corpus"
    fi

    count="$(wc -l <"$file_list")"
    count="${count//[[:space:]]/}"

    rm -f "$file_list"

    printf '%s\n' "$count"
}

run_scan() {
    local repository="$1"
    shift

    local corpus="$CORPUS_ROOT/$repository"
    local repository_results="$RESULTS_DIR/$repository"
    local json_file="$repository_results/arid.json"
    local stderr_file="$repository_results/arid.stderr"
    local status
    local files
    local expected_files
    local findings
    local duplicate_groups
    local expected_status
    local exclude

    local -a exclusions=("$@")

    local -a arid_args=(
        "$ARID_BIN"
        "$corpus"
        --hidden
        --workers 1
        --json
    )

    for exclude in "${exclusions[@]}"; do
        arid_args+=(--exclude "$exclude")
    done

    set +e
    "${arid_args[@]}" >"$json_file" 2>"$stderr_file"
    status=$?
    set -e

    if [[ "$status" -ne 0 && "$status" -ne 1 ]]; then
        echo "error: validation scan failed with exit code $status: $repository" >&2

        if [[ -s "$stderr_file" ]]; then
            cat "$stderr_file" >&2
        fi

        exit "$status"
    fi

    if [[ -s "$stderr_file" ]]; then
        echo "error: validation scan produced unexpected stderr: $repository" >&2
        cat "$stderr_file" >&2
        exit 2
    fi

    if ! jq -e . "$json_file" >/dev/null; then
        die "validation scan did not produce valid JSON: $repository"
    fi

    files="$(jq -r '.files' "$json_file")"

    expected_files="$(
        count_discoverable_files \
            "$corpus" \
            "${exclusions[@]}"
    )"

    if [[ "$files" -ne "$expected_files" ]]; then
        echo "error: discovery mismatch: $repository" >&2
        echo "Arid files:  $files" >&2
        echo "Discoverable: $expected_files" >&2
        exit 2
    fi

    findings="$(jq -r '.findings | length' "$json_file")"
    duplicate_groups="$(jq -r '.duplicate_groups' "$json_file")"

    if [[ "$duplicate_groups" -ne "$findings" ]]; then
        echo "error: duplicate-group count does not match findings: $repository" >&2
        echo "duplicate_groups: $duplicate_groups" >&2
        echo "findings:         $findings" >&2
        exit 2
    fi

    if [[ "$findings" -eq 0 ]]; then
        expected_status=0
    else
        expected_status=1
    fi

    if [[ "$status" -ne "$expected_status" ]]; then
        echo "error: exit status does not match finding state: $repository" >&2
        echo "exit:     $status" >&2
        echo "findings: $findings" >&2
        exit 2
    fi

    echo "Scan: PASS"
    echo "Discovery: PASS ($files files)"
    echo "stderr: empty"
    echo

    jq '{
        files,
        source_lines,
        analyzed_lines,
        duplicate_groups,
        duplicate_lines,
        duplication_percent,
        findings: (.findings | length)
    }' "$json_file"
}

print_distribution() {
    local repository="$1"
    local result="$RESULTS_DIR/$repository/arid.json"

    echo
    echo "=== distribution ==="

    jq '.findings
        | group_by(.distribution)
        | map({
            distribution: .[0].distribution,
            count: length
        })' "$result"
}

print_context() {
    local repository="$1"
    local result="$RESULTS_DIR/$repository/arid.json"

    echo
    echo "=== context ==="

    jq '.findings
        | group_by(.context)
        | map({
            context: .[0].context,
            count: length
        })' "$result"
}

print_scope() {
    local repository="$1"
    local result="$RESULTS_DIR/$repository/arid.json"

    echo
    echo "=== scope ==="

    jq '.findings
        | group_by(.scope)
        | map({
            scope: .[0].scope,
            count: length
        })' "$result"
}

print_duplicate_lengths() {
    local repository="$1"
    local result="$RESULTS_DIR/$repository/arid.json"

    echo
    echo "=== duplicate lengths ==="

    jq '{
        min_lines: ([.findings[].lines] | min),
        max_lines: ([.findings[].lines] | max),
        groups_4_to_9: (
            [.findings[] | select(.lines >= 4 and .lines <= 9)]
            | length
        ),
        groups_10_to_19: (
            [.findings[] | select(.lines >= 10 and .lines <= 19)]
            | length
        ),
        groups_20_plus: (
            [.findings[] | select(.lines >= 20)]
            | length
        )
    }' "$result"
}

print_ten_largest() {
    local repository="$1"
    local result="$RESULTS_DIR/$repository/arid.json"

    echo
    echo "=== ten largest groups ==="

    jq '.findings
        | sort_by(.lines)
        | reverse
        | .[:10]
        | map({
            lines,
            occurrences,
            files,
            distribution,
            context,
            scope,
            locations
        })' "$result"
}

print_rich_analysis() {
    local result="$RESULTS_DIR/rich/arid.json"

    echo
    echo "=== groups touching unicode data ==="

    jq '[
        .findings[]
        | select(
            any(
                .locations[];
                .path | contains("rich/_unicode_data/")
            )
        )
    ] | length' "$result"

    echo
    echo "=== groups not touching unicode data ==="

    jq '[
        .findings[]
        | select(
            all(
                .locations[];
                (.path | contains("rich/_unicode_data/") | not)
            )
        )
    ] | length' "$result"

    echo
    echo "=== non-unicode-data distribution ==="

    jq '[
        .findings[]
        | select(
            all(
                .locations[];
                (.path | contains("rich/_unicode_data/") | not)
            )
        )
    ]
    | group_by(.distribution)
    | map({
        distribution: .[0].distribution,
        count: length
    })' "$result"

    echo
    echo "=== ten largest findings outside unicode data ==="

    jq '[
        .findings[]
        | select(
            all(
                .locations[];
                (.path | contains("rich/_unicode_data/") | not)
            )
        )
    ]
    | sort_by(.lines)
    | reverse
    | .[:10]
    | map({
        lines,
        occurrences,
        files,
        distribution,
        context,
        scope,
        locations
    })' "$result"
}

print_mypy_analysis() {
    local corpus="$CORPUS_ROOT/mypy"
    local result="$RESULTS_DIR/mypy/arid.json"
    local tracked_file
    local discoverable_file

    echo
    echo "=== tracked included Python files ==="

    git -C "$corpus" \
        ls-files '*.py' '*.pyi' ':!test-data/**' |
        wc -l

    echo
    echo "=== included .pyi files ==="

    git -C "$corpus" \
        ls-files '*.pyi' ':!test-data/**' |
        wc -l

    echo
    echo "=== findings touching .pyi ==="

    jq '[
        .findings[]
        | select(
            any(
                .locations[];
                .path | endswith(".pyi")
            )
        )
    ] | length' "$result"

    tracked_file="$(mktemp)"
    discoverable_file="$(mktemp)"

    git -C "$corpus" \
        ls-files '*.py' '*.pyi' ':!test-data/**' |
        sort >"$tracked_file"

    (
        cd "$corpus"
        rg \
            --files \
            --hidden \
            -g '*.py' \
            -g '*.pyi' \
            -g '!test-data/**' |
            sort
    ) >"$discoverable_file"

    echo
    echo "=== tracked vs discoverable counts ==="

    wc -l "$tracked_file" "$discoverable_file"

    echo
    echo "=== tracked by Git but absent from normal discovery ==="

    comm -23 "$tracked_file" "$discoverable_file"

    rm -f "$tracked_file" "$discoverable_file"
}

run_django_determinism() {
    local corpus="$CORPUS_ROOT/django"
    local repository_results="$RESULTS_DIR/django"
    local baseline="$repository_results/arid.json"
    local worker
    local json_file
    local stderr_file
    local status
    local findings
    local expected_status

    echo
    echo "=== Django worker determinism ==="

    for worker in 2 4 8; do
        json_file="$repository_results/arid-w${worker}.json"
        stderr_file="$repository_results/arid-w${worker}.stderr"

        set +e
        "$ARID_BIN" \
            "$corpus" \
            --hidden \
            --exclude 'tests/test_runner_apps/tagged/tests_syntax_error.py' \
            --workers "$worker" \
            --json \
            >"$json_file" \
            2>"$stderr_file"
        status=$?
        set -e

        if [[ "$status" -ne 0 && "$status" -ne 1 ]]; then
            echo "error: Django workers=$worker failed with exit code $status" >&2

            if [[ -s "$stderr_file" ]]; then
                cat "$stderr_file" >&2
            fi

            exit "$status"
        fi

        if [[ -s "$stderr_file" ]]; then
            echo "error: Django workers=$worker produced unexpected stderr" >&2
            cat "$stderr_file" >&2
            exit 2
        fi

        if ! jq -e . "$json_file" >/dev/null; then
            die "Django workers=$worker did not produce valid JSON"
        fi

        findings="$(jq -r '.findings | length' "$json_file")"

        if [[ "$findings" -eq 0 ]]; then
            expected_status=0
        else
            expected_status=1
        fi

        if [[ "$status" -ne "$expected_status" ]]; then
            echo "error: Django workers=$worker exit status does not match findings" >&2
            echo "exit:     $status" >&2
            echo "findings: $findings" >&2
            exit 2
        fi

        if ! cmp -s "$baseline" "$json_file"; then
            echo "error: Django output is not deterministic at workers=$worker" >&2
            echo
            echo "workers=1:"
            sha256sum "$baseline"
            echo
            echo "workers=$worker:"
            sha256sum "$json_file"
            exit 2
        fi

        echo "workers 1 vs $worker: IDENTICAL"
    done

    echo
    echo "=== SHA-256 ==="

    sha256sum \
        "$repository_results/arid.json" \
        "$repository_results/arid-w2.json" \
        "$repository_results/arid-w4.json" \
        "$repository_results/arid-w8.json"
}

run_path_cases() {
    local repository_results="$RESULTS_DIR/path-cases"
    local temp_root
    local case_root
    local json_file="$repository_results/arid.json"
    local stderr_file="$repository_results/arid.stderr"
    local status

    echo
    echo "========================================"
    echo " Validation: path-cases"
    echo "========================================"

    temp_root="$(mktemp -d)"
    case_root="$temp_root/ünicode project"

    mkdir -p "$case_root/sub dir"

    cat >"$case_root/alpha file.py" <<'PY'
def first():
    value = calculate()
    save(value)
    report(value)
    return value
PY

    cat >"$case_root/sub dir/βeta.py" <<'PY'
def second():
    value = calculate()
    save(value)
    report(value)
    return value
PY

    set +e
    "$ARID_BIN" \
        "$case_root" \
        --json \
        >"$json_file" \
        2>"$stderr_file"
    status=$?
    set -e

    rm -rf "$temp_root"

    if [[ "$status" -ne 1 ]]; then
        echo "error: path validation expected exit 1, got $status" >&2

        if [[ -s "$stderr_file" ]]; then
            cat "$stderr_file" >&2
        fi

        exit 2
    fi

    if [[ -s "$stderr_file" ]]; then
        echo "error: path validation produced unexpected stderr" >&2
        cat "$stderr_file" >&2
        exit 2
    fi

    if ! jq -e '
        .files == 2
        and .duplicate_groups == 1
        and (.findings | length) == 1
        and .findings[0].lines == 4
        and (
            [.findings[0].locations[].path] | sort
        ) == [
            "alpha file.py",
            "sub dir/βeta.py"
        ]
        and all(
            .findings[0].locations[];
            .start_line == 2 and .end_line == 5
        )
    ' "$json_file" >/dev/null; then
        echo "error: path validation result did not match the expected contract" >&2
        jq . "$json_file" >&2
        exit 2
    fi

    echo "Path handling: PASS"
    echo "  spaces in directory names"
    echo "  Unicode directory names"
    echo "  non-ASCII filenames"
    echo "  relative JSON paths"
    echo "  source locations"
    echo

    jq '{
        files,
        duplicate_groups,
        findings: [
            .findings[] | {
                lines,
                locations
            }
        ]
    }' "$json_file"
}

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

    echo "global_root=$GLOBAL_ROOT"
    echo "corpus_root=$CORPUS_ROOT"
    echo "selected_repositories=$REPOS"
    echo "path_cases=true"
} >"$RESULTS_DIR/metadata.txt"

for repository in "${SELECTED_REPOS[@]}"; do
    corpus="$CORPUS_ROOT/$repository"

    {
        echo "${repository}_commit=$(git -C "$corpus" rev-parse HEAD)"
        echo "${repository}_remote=$(git -C "$corpus" remote get-url origin 2>/dev/null || true)"
    } >>"$RESULTS_DIR/metadata.txt"

    case "$repository" in
        black)
            echo "black_excludes=tests/data/**,profiling/**" \
                >>"$RESULTS_DIR/metadata.txt"
            ;;
        django)
            echo "django_excludes=tests/test_runner_apps/tagged/tests_syntax_error.py" \
                >>"$RESULTS_DIR/metadata.txt"
            echo "django_determinism_workers=1,2,4,8" \
                >>"$RESULTS_DIR/metadata.txt"
            ;;
        mypy)
            echo "mypy_excludes=test-data/**" \
                >>"$RESULTS_DIR/metadata.txt"
            ;;
        rich)
            echo "rich_excludes=" \
                >>"$RESULTS_DIR/metadata.txt"
            ;;
    esac
done

for repository in "${SELECTED_REPOS[@]}"; do
    echo
    echo "========================================"
    echo " Validation: $repository"
    echo "========================================"
    echo "Commit: $(git -C "$CORPUS_ROOT/$repository" rev-parse HEAD)"

    case "$repository" in
        black)
            echo
            echo "Checking intentional invalid-source fixture..."

            expect_invalid_source \
                black \
                'tests/data/cases/pep_572_do_not_remove_parens.py'

            echo
            echo "Running normal-source validation..."

            run_scan \
                black \
                'tests/data/**' \
                'profiling/**'

            print_distribution black
            print_context black
            print_duplicate_lengths black
            print_ten_largest black
            ;;

        django)
            echo
            echo "Checking intentional invalid-source fixture..."

            expect_invalid_source \
                django \
                'tests/test_runner_apps/tagged/tests_syntax_error.py'

            echo
            echo "Running normal-source validation..."

            run_scan \
                django \
                'tests/test_runner_apps/tagged/tests_syntax_error.py'

            print_duplicate_lengths django
            print_ten_largest django
            run_django_determinism
            ;;

        mypy)
            echo
            echo "Checking intentional invalid-source fixture..."

            expect_invalid_source \
                mypy \
                'test-data/unit/lib-stub/blocker.pyi'

            echo
            echo "Running normal-source validation..."

            run_scan \
                mypy \
                'test-data/**'

            print_distribution mypy
            print_context mypy
            print_duplicate_lengths mypy
            print_ten_largest mypy
            print_mypy_analysis
            ;;

        rich)
            echo
            echo "Running normal-source validation..."

            run_scan rich

            print_distribution rich
            print_context rich
            print_scope rich
            print_duplicate_lengths rich
            print_ten_largest rich
            print_rich_analysis
            ;;
    esac

    echo
    echo "$repository: PASS"
done

# Path handling is a core validation target rather than a repository corpus, so
# it runs regardless of --repos selection.
run_path_cases

echo
echo "========================================"
echo " Validation complete"
echo "========================================"
echo
echo "Repositories:"

for repository in "${SELECTED_REPOS[@]}"; do
    echo "  $repository: PASS"
done

echo "  path-cases: PASS"
echo
echo "Results:"

for repository in "${SELECTED_REPOS[@]}"; do
    echo "  $RESULTS_DIR/$repository"
done

echo "  $RESULTS_DIR/path-cases"
echo "  $RESULTS_DIR/metadata.txt"