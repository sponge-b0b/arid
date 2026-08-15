#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

FILES=(Cargo.toml Cargo.lock pyproject.toml README.md docs/arid-v1-release-roadmap.md)
MODE="prepare"
VERSION=""

usage() {
    cat <<'USAGE'
Usage:
  ./release.sh <version>
  ./release.sh --dry-run <version>
  ./release.sh --check

Supported versions: X.Y.Z-alpha.N, X.Y.Z-beta.N, X.Y.Z-rc.N, X.Y.Z

Prepares release metadata only. It does not commit, tag, push, or publish.
USAGE
}

die() {
    echo "error: $*" >&2
    exit 2
}

case "${1:-}" in
    --help|-h)
        usage
        exit 0
        ;;
    --check)
        [[ $# -eq 1 ]] || die "--check takes no arguments"
        MODE="check"
        ;;
    --dry-run)
        [[ $# -eq 2 ]] || die "--dry-run requires exactly one version"
        MODE="dry-run"
        VERSION="$2"
        ;;
    "")
        usage
        exit 2
        ;;
    -*)
        die "unknown option: $1"
        ;;
    *)
        [[ $# -eq 1 ]] || die "expected exactly one version"
        VERSION="$1"
        ;;
esac

command -v python3 >/dev/null 2>&1 ||
    die "required command not found: python3"

for file in "${FILES[@]}"; do
    [[ -f "$file" ]] || die "required file not found: $file"
done

current_version() {
    sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1
}

derive() {
    local version="$1"

    VERSION="$version"
    TAG="v$version"

    if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-alpha\.([0-9]+)$ ]]; then
        STAGE="Alpha"
        PYPI="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}a${BASH_REMATCH[4]}"
        CLASSIFIER="Development Status :: 3 - Alpha"
        PHASE="Alpha stabilization"
        BADGE='  <a href="#project-status"><img alt="Status: Alpha" src="https://img.shields.io/badge/status-alpha-orange"></a>'
        STATUS='> [!IMPORTANT]
> **Arid is currently in alpha.** The intended release functionality is code complete but remains under stabilization. Interfaces, behavior, defaults, and packaging details may still change before stable release.'

    elif [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-beta\.([0-9]+)$ ]]; then
        STAGE="Beta"
        PYPI="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}b${BASH_REMATCH[4]}"
        CLASSIFIER="Development Status :: 4 - Beta"
        PHASE="Beta"
        BADGE='  <a href="#project-status"><img alt="Status: Beta" src="https://img.shields.io/badge/status-beta-blue"></a>'
        STATUS='> [!IMPORTANT]
> **Arid is currently in beta.** The intended release feature set and core interfaces are frozen while prerelease validation and bug fixing continue toward stable release.'

    elif [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-rc\.([0-9]+)$ ]]; then
        STAGE="Release Candidate"
        PYPI="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}rc${BASH_REMATCH[4]}"
        CLASSIFIER="Development Status :: 4 - Beta"
        PHASE="Release Candidate"
        BADGE='  <a href="#project-status"><img alt="Status: Release Candidate" src="https://img.shields.io/badge/status-release%20candidate-blue"></a>'
        STATUS='> [!IMPORTANT]
> **Arid is currently a release candidate.** The release feature set and core interfaces are frozen, and the current build is believed ready for stable release without product-code changes.'

    elif [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        STAGE="Stable"
        PYPI="$version"
        CLASSIFIER="Development Status :: 5 - Production/Stable"
        PHASE="Stable"
        BADGE='  <a href="#project-status"><img alt="Status: Stable" src="https://img.shields.io/badge/status-stable-brightgreen"></a>'
        STATUS='> [!IMPORTANT]
> **Arid is stable.** The released interfaces and behavior are considered stable.'

    else
        die "unsupported release version: $version"
    fi

    export VERSION CLASSIFIER PHASE BADGE STATUS
}

metadata() {
    python3 - "$1" <<'PY'
from pathlib import Path
import os
import re
import sys

action = sys.argv[1]
version = os.environ["VERSION"]
classifier = os.environ["CLASSIFIER"]
phase = os.environ["PHASE"]
badge = os.environ["BADGE"]
status = os.environ["STATUS"]


def fail(message):
    raise SystemExit(f"error: {message}")


def sub_once(path, pattern, replacement, label, flags=re.M):
    file = Path(path)
    text = file.read_text()
    text, count = re.subn(
        pattern,
        replacement,
        text,
        count=1,
        flags=flags,
    )

    if count != 1:
        fail(f"expected exactly one {label} in {path}")

    file.write_text(text)


def block(text, start, end, label):
    if text.count(start) != 1 or text.count(end) != 1:
        fail(f"expected exactly one {label} marker pair")

    _, rest = text.split(start, 1)

    if end not in rest:
        fail(f"invalid {label} marker ordering")

    return rest.split(end, 1)[0].strip()


def replace_block(path, start, end, value, label):
    file = Path(path)
    text = file.read_text()

    block(text, start, end, label)

    left, rest = text.split(start, 1)
    _, right = rest.split(end, 1)

    file.write_text(
        f"{left}{start}\n{value}\n{end}{right}"
    )


if action == "update":
    sub_once(
        "Cargo.toml",
        r'(^\[package\]\n(?:(?!^\[).)*?^version = ")[^"]+("$)',
        rf'\g<1>{version}\2',
        "package version",
        re.M | re.S,
    )

    sub_once(
        "Cargo.lock",
        r'(^\[\[package\]\]\nname = "arid-cli"\nversion = ")[^"]+("$)',
        rf'\g<1>{version}\2',
        "arid-cli lock version",
    )

    sub_once(
        "pyproject.toml",
        r'^(\s*)"Development Status :: [^"]+",$',
        rf'\1"{classifier}",',
        "Development Status classifier",
    )

    replace_block(
        "README.md",
        "<!-- release-badge:start -->",
        "<!-- release-badge:end -->",
        badge,
        "README release badge",
    )

    replace_block(
        "README.md",
        "<!-- release-status:start -->",
        "<!-- release-status:end -->",
        status,
        "README release status",
    )

    replace_block(
        "docs/arid-v1-release-roadmap.md",
        "<!-- release-phase:start -->",
        "<!-- release-phase:end -->",
        f"**Current phase:** {phase}",
        "roadmap release phase",
    )


checks = [
    (
        "Cargo.toml",
        rf'^\[package\]\n(?:(?!^\[).)*?^version = "{re.escape(version)}"$',
        "Cargo.toml version",
        re.M | re.S,
    ),
    (
        "Cargo.lock",
        rf'^\[\[package\]\]\nname = "arid-cli"\nversion = "{re.escape(version)}"$',
        "Cargo.lock version",
        re.M,
    ),
    (
        "pyproject.toml",
        rf'^\s*"{re.escape(classifier)}",$',
        "pyproject.toml classifier",
        re.M,
    ),
]

for path, pattern, label, flags in checks:
    if len(re.findall(pattern, Path(path).read_text(), flags)) != 1:
        fail(f"{label} does not match expected release metadata")


readme = Path("README.md").read_text()

if block(
    readme,
    "<!-- release-badge:start -->",
    "<!-- release-badge:end -->",
    "README release badge",
) != badge.strip():
    fail("README release badge does not match")

if block(
    readme,
    "<!-- release-status:start -->",
    "<!-- release-status:end -->",
    "README release status",
) != status.strip():
    fail("README release status does not match")


roadmap = Path("docs/arid-v1-release-roadmap.md").read_text()

if block(
    roadmap,
    "<!-- release-phase:start -->",
    "<!-- release-phase:end -->",
    "roadmap release phase",
) != f"**Current phase:** {phase}":
    fail("roadmap current phase does not match")
PY
}

summary() {
    echo "$1"
    echo

    printf '  %-20s %s\n' \
        "Version:" "$VERSION" \
        "Stage:" "$STAGE" \
        "Git tag:" "$TAG" \
        "PyPI version:" "$PYPI" \
        "Python classifier:" "$CLASSIFIER" \
        "Roadmap phase:" "$PHASE"
}

CURRENT="$(current_version)"
REQUESTED="$VERSION"

[[ -n "$CURRENT" ]] ||
    die "unable to determine current Cargo.toml version"

if [[ "$MODE" == "check" ]]; then
    derive "$CURRENT"
    metadata check
    summary "Release metadata is consistent."
    exit 0
fi

# Refuse to build a new release on top of inconsistent current metadata.
derive "$CURRENT"
metadata check

derive "$REQUESTED"

[[ "$VERSION" != "$CURRENT" ]] ||
    die "requested version is already current: $VERSION"

if [[ "$MODE" == "dry-run" ]]; then
    echo "Current version: $CURRENT"
    echo

    summary "Would prepare release metadata:"

    echo
    echo "Managed files:"
    printf '  %s\n' "${FILES[@]}"
    exit 0
fi

command -v cargo >/dev/null 2>&1 ||
    die "required command not found: cargo"

command -v git >/dev/null 2>&1 ||
    die "required command not found: git"

[[ -z "$(git status --porcelain --untracked-files=all)" ]] ||
    die "working tree must be clean"

restore() {
    trap - ERR INT TERM
    git restore -- "${FILES[@]}"
    die "release metadata preparation failed; original files restored"
}

trap restore ERR INT TERM

metadata update
cargo check --locked --quiet
git diff --check

trap - ERR INT TERM

summary "Release metadata prepared successfully."

echo
echo "Review with:"
echo "  git diff"