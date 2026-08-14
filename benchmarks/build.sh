#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

CORPUS_RELATIVE_PATH="benchmarks/arid-corpora"
REPOSITORIES=(polaris pydantic requests)

declare -A REPOSITORY_URLS=(
    [polaris]="https://github.com/sponge-b0b/Polaris.git"
    [pydantic]="https://github.com/pydantic/pydantic.git"
    [requests]="https://github.com/psf/requests.git"
)

declare -A REVISIONS=(
    [polaris]="HEAD"
    [pydantic]="HEAD"
    [requests]="HEAD"
)

usage() {
    cat <<EOF
Usage: $0 <global-root> [options]

Build or update Arid's benchmark corpora beneath:
  <global-root>/$CORPUS_RELATIVE_PATH

Options:
  --polaris <revision>   Polaris Git revision. Default: upstream HEAD
  --pydantic <revision>  Pydantic Git revision. Default: upstream HEAD
  --requests <revision>  Requests Git revision. Default: upstream HEAD
  --help                 Show this help

A revision may be a commit SHA, tag, or branch. Repositories are checked out at
an exact detached commit. Existing repositories must have clean working trees.

Examples:
  # Build all corpora at each repository's current upstream HEAD
  $0 /home/bobt

  # Build all corpora at pinned revisions
  $0 /home/bobt \
    --requests 6e83187b8feb273ed4c6cdab5efd8d54901dfab3 \
    --pydantic cf67d4b3193c3fe43ede18612ed62785eee11382 \
    --polaris 00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

normalize_git_url() {
    local url="${1%/}"

    url="${url%.git}"

    case "$url" in
        git@github.com:*)
            url="https://github.com/${url#git@github.com:}"
            ;;
        ssh://git@github.com/*)
            url="https://github.com/${url#ssh://git@github.com/}"
            ;;
    esac

    printf '%s\n' "$url"
}

resolve_revision() {
    local repository="$1"
    local revision="$2"
    local commit=""
    local remote_head=""

    if [[ "$revision" == "HEAD" ]]; then
        git -C "$repository" remote set-head origin -a >/dev/null 2>&1 ||
            die "unable to resolve upstream HEAD for $repository"

        remote_head="$(
            git -C "$repository" symbolic-ref --quiet refs/remotes/origin/HEAD
        )" || die "origin/HEAD is not configured for $repository"

        git -C "$repository" rev-parse --verify "${remote_head}^{commit}"
        return
    fi

    if commit="$(
        git -C "$repository" rev-parse --verify "${revision}^{commit}" 2>/dev/null
    )"; then
        printf '%s\n' "$commit"
        return
    fi

    if commit="$(
        git -C "$repository" \
            rev-parse --verify "refs/remotes/origin/${revision}^{commit}" 2>/dev/null
    )"; then
        printf '%s\n' "$commit"
        return
    fi

    die "unable to resolve revision '$revision' in $repository"
}

GLOBAL_ROOT_INPUT=""
declare -A seen_revision_options=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --polaris|--pydantic|--requests)
            [[ $# -ge 2 ]] || die "$1 requires a value"

            repository="${1#--}"

            if [[ -n "${seen_revision_options[$repository]:-}" ]]; then
                die "duplicate revision option: $1"
            fi

            REVISIONS["$repository"]="$2"
            seen_revision_options["$repository"]=1
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

for command in git realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

if [[ ! -d "$GLOBAL_ROOT_INPUT" ]]; then
    die "global root is not a directory: $GLOBAL_ROOT_INPUT"
fi

GLOBAL_ROOT="$(realpath "$GLOBAL_ROOT_INPUT")"
CORPUS_ROOT="$GLOBAL_ROOT/$CORPUS_RELATIVE_PATH"

mkdir -p "$CORPUS_ROOT"

for repository in "${REPOSITORIES[@]}"; do
    repository_path="$CORPUS_ROOT/$repository"
    repository_url="${REPOSITORY_URLS[$repository]}"
    revision="${REVISIONS[$repository]}"

    echo "=== $repository ==="

    if [[ ! -e "$repository_path" ]]; then
        echo "Cloning $repository_url"
        git clone "$repository_url" "$repository_path"
    elif [[ ! -d "$repository_path" ]]; then
        die "corpus path exists but is not a directory: $repository_path"
    fi

    if ! git -C "$repository_path" \
        rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        die "corpus is not a Git repository: $repository_path"
    fi

    repository_root="$(git -C "$repository_path" rev-parse --show-toplevel)"
    repository_root="$(realpath "$repository_root")"

    if [[ "$repository_root" != "$(realpath "$repository_path")" ]]; then
        die "corpus must be the Git repository root: $repository_path"
    fi

    if [[ -n "$(
        git -C "$repository_path" status --porcelain --untracked-files=all
    )" ]]; then
        echo "error: corpus working tree is not clean: $repository_path" >&2
        git -C "$repository_path" status --short >&2
        exit 2
    fi

    actual_origin="$(
        git -C "$repository_path" remote get-url origin 2>/dev/null || true
    )"

    if [[ -z "$actual_origin" ]]; then
        die "corpus has no origin remote: $repository_path"
    fi

    if [[ "$(normalize_git_url "$actual_origin")" != \
        "$(normalize_git_url "$repository_url")" ]]; then
        echo "error: unexpected origin remote for $repository_path" >&2
        echo "expected: $repository_url" >&2
        echo "actual:   $actual_origin" >&2
        exit 2
    fi

    echo "Fetching origin"
    git -C "$repository_path" fetch --prune --tags origin

    target_commit="$(resolve_revision "$repository_path" "$revision")"

    git -C "$repository_path" checkout --quiet --detach "$target_commit"

    resolved_commit="$(git -C "$repository_path" rev-parse HEAD)"

    echo "Revision: $revision"
    echo "Commit:   $resolved_commit"
    echo
done

echo "Benchmark corpora ready at:"
echo "  $CORPUS_ROOT"