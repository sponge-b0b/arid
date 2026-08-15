#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULTS_ROOT="$ROOT_DIR/qualification/results"

LINUX_ARCHIVE="arid-linux-x86_64.tar.gz"
BENCHMARK_WARMUP=3
BENCHMARK_RUNS=10
MIN_PYLINT_SPEEDUP=10

POLARIS_REVISION="00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031"
PYDANTIC_REVISION="cf67d4b3193c3fe43ede18612ed62785eee11382"
REQUESTS_REVISION="6e83187b8feb273ed4c6cdab5efd8d54901dfab3"

RELEASE_METADATA_FILES=(
    Cargo.toml
    Cargo.lock
    pyproject.toml
    README.md
    docs/arid-v1-release-roadmap.md
)

usage() {
    cat <<EOF
Usage: $0 <global-root> <version>

Qualify a published Arid release candidate or stable release.

Supported versions:
  X.Y.Z-rc.N    Full release-candidate qualification
  X.Y.Z         Stable promotion qualification

Examples:
  $0 /home/bobt 1.0.1-rc.1
  $0 /home/bobt 1.0.1

Release-candidate qualification verifies the source/release workflow, exercises
both the published Linux standalone artifact and exact PyPI package through the
real-world validation campaign, compares their JSON output byte-for-byte, and
benchmarks the published standalone artifact.

Stable qualification requires the latest RC for the same base version to have a
local PASS record, verifies the RC-to-stable change is exactly the metadata
transition produced by release.sh, and smoke-tests the stable published
artifacts.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

pass() {
    printf 'PASS: %s\n' "$1"
}

require_command() {
    command -v "$1" >/dev/null 2>&1 ||
        die "required command not found: $1"
}

GLOBAL_ROOT_INPUT="${1:-}"
VERSION="${2:-}"

if [[ "$GLOBAL_ROOT_INPUT" == "--help" || "$GLOBAL_ROOT_INPUT" == "-h" ]]; then
    usage
    exit 0
fi

[[ $# -eq 2 ]] || {
    usage
    exit 2
}

if [[ "$VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-rc\.([0-9]+)$ ]]; then
    RELEASE_KIND="rc"
    BASE_VERSION="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
    PYPI_VERSION="${BASE_VERSION}rc${BASH_REMATCH[4]}"
    EXPECTED_PRERELEASE="true"
elif [[ "$VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    RELEASE_KIND="stable"
    BASE_VERSION="$VERSION"
    PYPI_VERSION="$VERSION"
    EXPECTED_PRERELEASE="false"
else
    die "supported qualification versions are X.Y.Z-rc.N and X.Y.Z"
fi

TAG="v$VERSION"
EXPECTED_ARID_VERSION="arid $VERSION"

for command in \
    awk \
    cmp \
    cp \
    diff \
    find \
    gh \
    git \
    grep \
    head \
    jq \
    mktemp \
    python3 \
    realpath \
    sed \
    sha256sum \
    sort \
    tail \
    tar \
    uname \
    wc
do
    require_command "$command"
done

if [[ "$RELEASE_KIND" == "rc" ]]; then
    for command in cargo rustc; do
        require_command "$command"
    done
fi

[[ -d "$GLOBAL_ROOT_INPUT" ]] ||
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"

[[ "$(uname -s)" == "Linux" ]] ||
    die "published-artifact qualification currently requires Linux"

[[ "$(uname -m)" == "x86_64" ]] ||
    die "published-artifact qualification currently requires x86_64"

cd "$ROOT_DIR"

[[ -x "$ROOT_DIR/release.sh" ]] ||
    die "required executable not found: release.sh"

[[ -x "$ROOT_DIR/validation/run.sh" ]] ||
    die "required executable not found: validation/run.sh"

[[ -x "$ROOT_DIR/benchmarks/build.sh" ]] ||
    die "required executable not found: benchmarks/build.sh"

[[ -x "$ROOT_DIR/benchmarks/run.sh" ]] ||
    die "required executable not found: benchmarks/run.sh"

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "error: Arid working tree must be clean" >&2
    git status --short >&2
    exit 2
fi

CURRENT_BRANCH="$(
    git symbolic-ref --quiet --short HEAD 2>/dev/null || true
)"

[[ "$CURRENT_BRANCH" == "main" ]] ||
    die "qualification must run from the main branch"

echo "Fetching release refs..."

git fetch origin main --tags --prune

HEAD_COMMIT="$(git rev-parse HEAD)"
ORIGIN_MAIN_COMMIT="$(git rev-parse refs/remotes/origin/main)"

[[ "$HEAD_COMMIT" == "$ORIGIN_MAIN_COMMIT" ]] ||
    die "main is not synchronized with origin/main"

if ! git rev-parse --verify --quiet "refs/tags/$TAG" >/dev/null; then
    die "release tag does not exist locally after fetch: $TAG"
fi

TAG_COMMIT="$(git rev-parse "$TAG^{commit}")"

[[ "$TAG_COMMIT" == "$HEAD_COMMIT" ]] ||
    die "$TAG does not point at the current main commit"

REMOTE_TAG="$(
    {
        git ls-remote \
            --tags \
            origin \
            "refs/tags/$TAG" \
            "refs/tags/$TAG^{}" ||
            true
    } |
        awk 'END {print $1}'
)"

[[ -n "$REMOTE_TAG" ]] ||
    die "release tag does not exist on origin: $TAG"

[[ "$REMOTE_TAG" == "$TAG_COMMIT" ]] ||
    die "origin tag $TAG does not resolve to the local tag commit"

CURRENT_VERSION="$(
    sed -n \
        's/^version = "\([^"]*\)"/\1/p' \
        Cargo.toml |
        head -n 1
)"

[[ "$CURRENT_VERSION" == "$VERSION" ]] ||
    die "Cargo.toml version is $CURRENT_VERSION, expected $VERSION"

./release.sh --check

pass "release metadata"

git diff --check

[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
    die "working tree changed during preflight"

pass "repository cleanliness"

REPOSITORY="$(
    gh repo view \
        --json nameWithOwner \
        --jq '.nameWithOwner'
)"

[[ -n "$REPOSITORY" ]] ||
    die "unable to determine GitHub repository"

verify_release_workflow() {
    local runs_json
    local run_json
    local run_status
    local run_conclusion

    echo "Verifying production release workflow..."

    runs_json="$(
        gh api \
            -X GET \
            "repos/$REPOSITORY/actions/workflows/release.yml/runs" \
            -f event=push \
            -f head_sha="$TAG_COMMIT" \
            -f per_page=20
    )"

    run_json="$(
        jq \
            -c \
            --arg sha "$TAG_COMMIT" \
            '[
                .workflow_runs[]
                | select(.head_sha == $sha)
            ]
            | sort_by(.created_at)
            | last // empty' \
            <<<"$runs_json"
    )"

    [[ -n "$run_json" ]] ||
        die "no Release workflow run found for $TAG at $TAG_COMMIT"

    RELEASE_RUN_ID="$(
        jq -r '.id' <<<"$run_json"
    )"

    RELEASE_RUN_URL="$(
        jq -r '.html_url' <<<"$run_json"
    )"

    run_status="$(
        jq -r '.status' <<<"$run_json"
    )"

    run_conclusion="$(
        jq -r '.conclusion' <<<"$run_json"
    )"

    [[ "$run_status" == "completed" ]] ||
        die "release workflow has not completed: run $RELEASE_RUN_ID"

    [[ "$run_conclusion" == "success" ]] ||
        die "release workflow did not succeed: run $RELEASE_RUN_ID ($run_conclusion)"

    pass "production release workflow"
}

verify_github_release() {
    local release_json
    local actual_prerelease

    echo "Verifying GitHub release..."

    release_json="$(
        gh release view \
            "$TAG" \
            --repo "$REPOSITORY" \
            --json tagName,isDraft,isPrerelease,url,assets
    )"

    [[ "$(jq -r '.tagName' <<<"$release_json")" == "$TAG" ]] ||
        die "GitHub release tag mismatch"

    [[ "$(jq -r '.isDraft' <<<"$release_json")" == "false" ]] ||
        die "GitHub release is still a draft"

    actual_prerelease="$(
        jq -r '.isPrerelease' <<<"$release_json"
    )"

    [[ "$actual_prerelease" == "$EXPECTED_PRERELEASE" ]] ||
        die "GitHub release prerelease state is $actual_prerelease, expected $EXPECTED_PRERELEASE"

    jq \
        -e \
        --arg archive "$LINUX_ARCHIVE" \
        'any(.assets[]; .name == $archive)' \
        <<<"$release_json" \
        >/dev/null ||
        die "GitHub release is missing $LINUX_ARCHIVE"

    GITHUB_RELEASE_URL="$(
        jq -r '.url' <<<"$release_json"
    )"

    pass "GitHub release"
}

run_rc_source_gate() {
    echo
    echo "Running release-candidate source gate..."

    cargo fmt --check

    cargo test \
        --locked

    cargo clippy \
        --locked \
        --all-targets \
        --all-features \
        -- \
        -D warnings

    git diff --check

    [[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
        die "source gate modified the working tree"

    pass "release-candidate source gate"
}

find_latest_rc() {
    local latest

    latest="$(
        git tag \
            -l "v${BASE_VERSION}-rc.*" |
            grep \
                -E '^v[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$' |
            sort -V |
            tail -n 1
    )"

    [[ -n "$latest" ]] ||
        die "no release candidate tag exists for $BASE_VERSION"

    printf '%s\n' "$latest"
}

verify_stable_promotion() {
    local qualification_file
    local recorded_commit
    local actual_commit
    local changed_file
    local allowed
    local release_file
    local predicted_worktree="$TMP_ROOT/predicted-stable"
    local actual_file="$TMP_ROOT/actual-release-file"

    LATEST_RC="$(find_latest_rc)"

    qualification_file="$RESULTS_ROOT/$LATEST_RC/qualification.txt"

    [[ -f "$qualification_file" ]] ||
        die "latest release candidate has no local qualification record: $LATEST_RC"

    grep \
        -Fxq \
        'qualification=PASS' \
        "$qualification_file" ||
        die "latest release candidate is not qualified: $LATEST_RC"

    recorded_commit="$(
        sed -n \
            's/^tag_commit=//p' \
            "$qualification_file" |
            head -n 1
    )"

    actual_commit="$(
        git rev-parse "$LATEST_RC^{commit}"
    )"

    [[ -n "$recorded_commit" && "$recorded_commit" == "$actual_commit" ]] ||
        die "qualification record does not match current $LATEST_RC tag"

    while IFS= read -r changed_file; do
        [[ -n "$changed_file" ]] ||
            continue

        allowed=false

        for release_file in "${RELEASE_METADATA_FILES[@]}"; do
            if [[ "$changed_file" == "$release_file" ]]; then
                allowed=true
                break
            fi
        done

        [[ "$allowed" == true ]] ||
            die "stable release changes non-metadata file after $LATEST_RC: $changed_file"
    done < <(
        git diff \
            --name-only \
            "$LATEST_RC" \
            "$TAG"
    )

    echo "Verifying exact RC-to-stable metadata transition..."

    git worktree add \
        --detach \
        "$predicted_worktree" \
        "$LATEST_RC" \
        >/dev/null

    PREDICTED_WORKTREE="$predicted_worktree"

    (
        cd "$predicted_worktree"

        ./release.sh "$VERSION" \
            >/dev/null
    )

    for release_file in "${RELEASE_METADATA_FILES[@]}"; do
        git show \
            "$TAG:$release_file" \
            >"$actual_file"

        if ! cmp \
            -s \
            "$predicted_worktree/$release_file" \
            "$actual_file"
        then
            die "stable $release_file does not match the release.sh transition from $LATEST_RC"
        fi
    done

    git worktree remove \
        --force \
        "$predicted_worktree" \
        >/dev/null

    PREDICTED_WORKTREE=""

    pass "latest RC qualification ($LATEST_RC)"
    pass "RC-to-stable metadata-only transition"
}

prepare_results() {
    QUALIFICATION_DIR="$RESULTS_ROOT/$TAG"
    ARTIFACT_DIR="$QUALIFICATION_DIR/artifacts"
    STANDALONE_RESULTS="$QUALIFICATION_DIR/standalone/validation"
    PYPI_RESULTS="$QUALIFICATION_DIR/pypi/validation"

    rm -rf "$QUALIFICATION_DIR"

    mkdir -p \
        "$ARTIFACT_DIR/linux-x86_64" \
        "$QUALIFICATION_DIR/standalone" \
        "$QUALIFICATION_DIR/pypi"
}

download_standalone() {
    local archive="$ARTIFACT_DIR/$LINUX_ARCHIVE"

    echo
    echo "Downloading published Linux standalone artifact..."

    gh release download \
        "$TAG" \
        --repo "$REPOSITORY" \
        --pattern "$LINUX_ARCHIVE" \
        --dir "$ARTIFACT_DIR"

    [[ -f "$archive" ]] ||
        die "downloaded standalone archive not found: $archive"

    tar \
        -xzf "$archive" \
        -C "$ARTIFACT_DIR/linux-x86_64"

    STANDALONE_BIN="$ARTIFACT_DIR/linux-x86_64/arid"

    [[ -x "$STANDALONE_BIN" ]] ||
        die "standalone archive did not contain executable arid"

    STANDALONE_VERSION="$(
        "$STANDALONE_BIN" --version
    )"

    [[ "$STANDALONE_VERSION" == "$EXPECTED_ARID_VERSION" ]] ||
        die "standalone reports '$STANDALONE_VERSION', expected '$EXPECTED_ARID_VERSION'"

    "$STANDALONE_BIN" \
        --help \
        >/dev/null

    STANDALONE_SHA256="$(
        sha256sum "$STANDALONE_BIN" |
            awk '{print $1}'
    )"

    ARCHIVE_SHA256="$(
        sha256sum "$archive" |
            awk '{print $1}'
    )"

    {
        sha256sum "$archive"
        sha256sum "$STANDALONE_BIN"
    } >"$ARTIFACT_DIR/sha256.txt"

    pass "standalone artifact smoke test"
}

install_pypi() {
    local venv="$TMP_ROOT/pypi-venv"

    echo
    echo "Installing exact PyPI release in a clean environment..."

    python3 -m venv "$venv"

    "$venv/bin/python" \
        -m pip install \
        --disable-pip-version-check \
        --no-cache-dir \
        --index-url https://pypi.org/simple \
        "arid==$PYPI_VERSION"

    PYPI_BIN="$venv/bin/arid"

    [[ -x "$PYPI_BIN" ]] ||
        die "PyPI installation did not provide an arid executable"

    PYPI_ARID_VERSION="$(
        "$PYPI_BIN" --version
    )"

    [[ "$PYPI_ARID_VERSION" == "$EXPECTED_ARID_VERSION" ]] ||
        die "PyPI executable reports '$PYPI_ARID_VERSION', expected '$EXPECTED_ARID_VERSION'"

    "$PYPI_BIN" \
        --help \
        >/dev/null

    PYPI_SHA256="$(
        sha256sum "$PYPI_BIN" |
            awk '{print $1}'
    )"

    pass "PyPI package smoke test"
}

preserve_validation_results() {
    local destination="$1"

    rm -rf "$destination"

    mkdir -p "$destination"

    cp \
        -a \
        "$ROOT_DIR/validation/results/." \
        "$destination/"
}

run_full_validation() {
    echo
    echo "Validating published standalone artifact..."

    "$ROOT_DIR/validation/run.sh" \
        "$GLOBAL_ROOT" \
        --arid-bin "$STANDALONE_BIN"

    preserve_validation_results "$STANDALONE_RESULTS"

    pass "standalone real-world validation"

    echo
    echo "Validating exact PyPI-installed executable..."

    "$ROOT_DIR/validation/run.sh" \
        "$GLOBAL_ROOT" \
        --arid-bin "$PYPI_BIN"

    preserve_validation_results "$PYPI_RESULTS"

    pass "PyPI real-world validation"
}

compare_validation_json() {
    local left_list="$TMP_ROOT/standalone-json.txt"
    local right_list="$TMP_ROOT/pypi-json.txt"
    local relative
    local count

    find \
        "$STANDALONE_RESULTS" \
        -type f \
        -name '*.json' \
        -printf '%P\n' |
        sort \
            >"$left_list"

    find \
        "$PYPI_RESULTS" \
        -type f \
        -name '*.json' \
        -printf '%P\n' |
        sort \
            >"$right_list"

    if ! cmp \
        -s \
        "$left_list" \
        "$right_list"
    then
        echo "error: standalone and PyPI validation JSON file sets differ" >&2

        diff \
            -u \
            "$left_list" \
            "$right_list" \
            >&2 ||
            true

        exit 2
    fi

    count="$(
        wc -l \
            <"$left_list"
    )"

    count="${count//[[:space:]]/}"

    [[ "$count" -gt 0 ]] ||
        die "validation produced no JSON files to compare"

    while IFS= read -r relative; do
        if ! cmp \
            -s \
            "$STANDALONE_RESULTS/$relative" \
            "$PYPI_RESULTS/$relative"
        then
            echo "error: standalone and PyPI validation output differ: $relative" >&2

            echo "standalone:" >&2

            sha256sum \
                "$STANDALONE_RESULTS/$relative" \
                >&2

            echo "PyPI:" >&2

            sha256sum \
                "$PYPI_RESULTS/$relative" \
                >&2

            exit 2
        fi
    done <"$left_list"

    VALIDATION_JSON_COUNT="$count"

    pass "standalone/PyPI validation equivalence ($count JSON files)"
}

benchmark_speedup() {
    local result_file="$1"

    python3 \
        - \
        "$result_file" \
        <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as file:
    data = json.load(file)

results = data.get("results", [])

if len(results) < 2:
    raise SystemExit(
        "benchmark result does not contain both comparison commands"
    )

arid = results[0].get("mean")
pylint = results[1].get("mean")

if (
    not isinstance(arid, (int, float))
    or not isinstance(pylint, (int, float))
    or arid <= 0
):
    raise SystemExit(
        "benchmark result contains invalid mean values"
    )

print(f"{pylint / arid:.2f}")
PY
}

require_minimum_speedup() {
    local name="$1"
    local speedup="$2"

    python3 \
        - \
        "$name" \
        "$speedup" \
        "$MIN_PYLINT_SPEEDUP" \
        <<'PY'
import sys

name = sys.argv[1]
speedup = float(sys.argv[2])
minimum = float(sys.argv[3])

if speedup < minimum:
    raise SystemExit(
        f"error: {name} Arid/Pylint speedup is {speedup:.2f}x; "
        f"release target requires at least {minimum:.2f}x"
    )
PY
}

run_benchmarks() {
    local pydantic_result
    local polaris_result

    echo
    echo "Provisioning canonical benchmark revisions..."

    "$ROOT_DIR/benchmarks/build.sh" \
        "$GLOBAL_ROOT" \
        --polaris "$POLARIS_REVISION" \
        --pydantic "$PYDANTIC_REVISION" \
        --requests "$REQUESTS_REVISION"

    echo
    echo "Benchmarking published standalone artifact..."

    "$ROOT_DIR/benchmarks/run.sh" \
        "$GLOBAL_ROOT" \
        --arid-bin "$STANDALONE_BIN" \
        --label "$VERSION" \
        --warmup "$BENCHMARK_WARMUP" \
        --runs "$BENCHMARK_RUNS"

    pydantic_result="$ROOT_DIR/benchmarks/results/pydantic-$VERSION/pylint.json"
    polaris_result="$ROOT_DIR/benchmarks/results/polaris-$VERSION/pylint.json"

    [[ -f "$pydantic_result" ]] ||
        die "missing Pydantic Pylint benchmark result"

    [[ -f "$polaris_result" ]] ||
        die "missing Polaris Pylint benchmark result"

    PYDANTIC_SPEEDUP="$(
        benchmark_speedup "$pydantic_result"
    )"

    POLARIS_SPEEDUP="$(
        benchmark_speedup "$polaris_result"
    )"

    require_minimum_speedup \
        "Pydantic" \
        "$PYDANTIC_SPEEDUP"

    require_minimum_speedup \
        "Polaris" \
        "$POLARIS_SPEEDUP"

    pass "benchmark suite"
    pass "Pydantic performance target (${PYDANTIC_SPEEDUP}x vs Pylint)"
    pass "Polaris performance target (${POLARIS_SPEEDUP}x vs Pylint)"
}

write_report() {
    local report="$QUALIFICATION_DIR/qualification.txt"

    {
        echo "qualification=PASS"
        echo "date_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "version=$VERSION"
        echo "tag=$TAG"
        echo "release_kind=$RELEASE_KIND"
        echo "pypi_version=$PYPI_VERSION"
        echo "repository=$REPOSITORY"
        echo "tag_commit=$TAG_COMMIT"
        echo "harness_commit=$HEAD_COMMIT"
        echo "release_run_id=$RELEASE_RUN_ID"
        echo "release_run_url=$RELEASE_RUN_URL"
        echo "github_release_url=$GITHUB_RELEASE_URL"
        echo "standalone_archive=$ARTIFACT_DIR/$LINUX_ARCHIVE"
        echo "standalone_archive_sha256=$ARCHIVE_SHA256"
        echo "standalone_binary=$STANDALONE_BIN"
        echo "standalone_sha256=$STANDALONE_SHA256"
        echo "standalone_version=$STANDALONE_VERSION"
        echo "pypi_binary_sha256=$PYPI_SHA256"
        echo "pypi_arid_version=$PYPI_ARID_VERSION"

        if [[ "$RELEASE_KIND" == "rc" ]]; then
            echo "standalone_validation=$STANDALONE_RESULTS"
            echo "pypi_validation=$PYPI_RESULTS"
            echo "validation_json_files=$VALIDATION_JSON_COUNT"
            echo "validation_equivalence=PASS"
            echo "benchmark_warmup=$BENCHMARK_WARMUP"
            echo "benchmark_runs=$BENCHMARK_RUNS"
            echo "benchmark_label=$VERSION"
            echo "pydantic_pylint_speedup=${PYDANTIC_SPEEDUP}x"
            echo "polaris_pylint_speedup=${POLARIS_SPEEDUP}x"
            echo "minimum_required_pylint_speedup=${MIN_PYLINT_SPEEDUP}x"
        else
            echo "base_rc=$LATEST_RC"
            echo "base_rc_qualification=PASS"
            echo "metadata_delta=PASS"
        fi
    } >"$report"
}

TMP_ROOT="$(mktemp -d)"
PREDICTED_WORKTREE=""

cleanup() {
    if [[ -n "$PREDICTED_WORKTREE" && -d "$PREDICTED_WORKTREE" ]]; then
        git \
            -C "$ROOT_DIR" \
            worktree remove \
            --force \
            "$PREDICTED_WORKTREE" \
            >/dev/null 2>&1 ||
            true
    fi

    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ "$RELEASE_KIND" == "rc" ]]; then
    run_rc_source_gate
fi

verify_release_workflow
verify_github_release
prepare_results

if [[ "$RELEASE_KIND" == "stable" ]]; then
    verify_stable_promotion
fi

download_standalone
install_pypi

if [[ "$RELEASE_KIND" == "rc" ]]; then
    run_full_validation
    compare_validation_json
    run_benchmarks
fi

write_report

trap - EXIT INT TERM

cleanup

echo
echo "========================================"
echo " Release qualification complete"
echo "========================================"
echo

printf '  %-28s %s\n' \
    "Version:" "$VERSION" \
    "Tag:" "$TAG" \
    "Release kind:" "$RELEASE_KIND" \
    "Release workflow:" "PASS" \
    "GitHub release:" "PASS" \
    "Standalone smoke:" "PASS" \
    "PyPI smoke:" "PASS"

if [[ "$RELEASE_KIND" == "rc" ]]; then
    printf '  %-28s %s\n' \
        "Standalone validation:" "PASS" \
        "PyPI validation:" "PASS" \
        "Artifact equivalence:" "PASS" \
        "Benchmarks:" "PASS" \
        "Pydantic vs Pylint:" "${PYDANTIC_SPEEDUP}x" \
        "Polaris vs Pylint:" "${POLARIS_SPEEDUP}x"
else
    printf '  %-28s %s\n' \
        "Base RC:" "$LATEST_RC" \
        "Base RC qualification:" "PASS" \
        "RC -> stable delta:" "METADATA ONLY"
fi

echo

printf '  %-28s %s\n' \
    "QUALIFICATION:" \
    "PASS"

echo
echo "Evidence:"
echo "  $QUALIFICATION_DIR/qualification.txt"