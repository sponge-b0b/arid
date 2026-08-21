#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FINDING_FINGERPRINT="arid-finding-v1:sha256:572a68e97a82622bee89bc469c0d261ad28357d1753dd70128e424144f9d6443"
BASELINE_FINGERPRINT="sha256:80d31b6f8888c1118e22dcd6bfee46046848b0c9f944aa4a236a1cfd5c8fac2d"

usage() {
    cat <<'EOF'
Usage: validation/v2-identity.sh <arid-bin>

Validate Arid v2 stable identity, structural reporting, path handling, and
supported Rust API boundary.
EOF
}

die() {
    echo "error: $*" >&2
    exit 2
}

pass() {
    printf 'PASS: %s\n' "$1"
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

[[ $# -eq 1 ]] || {
    usage
    exit 2
}

ARID_BIN_INPUT="$1"

for command in cargo cmp cp mktemp python3 realpath; do
    command -v "$command" >/dev/null 2>&1 ||
        die "required command not found: $command"
done

[[ -f "$ARID_BIN_INPUT" ]] ||
    die "Arid executable does not exist: $ARID_BIN_INPUT"
[[ -x "$ARID_BIN_INPUT" ]] ||
    die "Arid executable is not executable: $ARID_BIN_INPUT"

ARID_BIN="$(realpath "$ARID_BIN_INPUT")"
ARID_VERSION="$("$ARID_BIN" --version)"
[[ "$ARID_VERSION" == arid\ * ]] ||
    die "executable does not identify itself as Arid: $ARID_BIN"

TMP_ROOT="$(mktemp -d)"

cleanup() {
    rm -rf "$TMP_ROOT"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

run_expect_status() {
    local expected="$1"
    local stdout_file="$2"
    local stderr_file="$3"
    shift 3

    local status
    set +e
    "$@" >"$stdout_file" 2>"$stderr_file"
    status=$?
    set -e

    [[ "$status" -eq "$expected" ]] || {
        [[ ! -s "$stderr_file" ]] || cat "$stderr_file" >&2
        die "expected exit $expected, got $status: $*"
    }

    [[ ! -s "$stderr_file" ]] || {
        cat "$stderr_file" >&2
        die "unexpected stderr: $*"
    }
}

write_two_line_duplicate() {
    local path="$1"
    cat >"$path" <<'PY'
alpha = 1
beta = 2
PY
}

IDENTITY_A="$TMP_ROOT/identity-a"
IDENTITY_B="$TMP_ROOT/identity-b"
mkdir -p "$IDENTITY_A" "$IDENTITY_B/renamed path"
write_two_line_duplicate "$IDENTITY_A/a.py"
write_two_line_duplicate "$IDENTITY_A/b.py"
cat >"$IDENTITY_B/renamed path/first.py" <<'PY'
prefix_a = 10
alpha = 1
beta = 2
PY
cat >"$IDENTITY_B/renamed path/second.py" <<'PY'
prefix_b = 20
alpha = 1
beta = 2
PY

run_expect_status 1 \
    "$TMP_ROOT/identity-a.json" \
    "$TMP_ROOT/identity-a.stderr" \
    "$ARID_BIN" "$IDENTITY_A" \
    --no-config \
    --project-root "$IDENTITY_A" \
    --min-lines 2 \
    --json

run_expect_status 1 \
    "$TMP_ROOT/identity-b.json" \
    "$TMP_ROOT/identity-b.stderr" \
    "$ARID_BIN" "$IDENTITY_B" \
    --no-config \
    --project-root "$IDENTITY_B" \
    --min-lines 2 \
    --json

run_expect_status 0 \
    "$TMP_ROOT/baseline.stdout" \
    "$TMP_ROOT/baseline.stderr" \
    "$ARID_BIN" "$IDENTITY_A" \
    --no-config \
    --project-root "$IDENTITY_A" \
    --min-lines 2 \
    --write-baseline "$TMP_ROOT/baseline.json"

python3 - \
    "$FINDING_FINGERPRINT" \
    "$BASELINE_FINGERPRINT" \
    "$TMP_ROOT/identity-a.json" \
    "$TMP_ROOT/identity-b.json" \
    "$TMP_ROOT/baseline.json" \
    <<'PY'
import json
import sys
from pathlib import Path

expected_finding = sys.argv[1]
expected_baseline = sys.argv[2]
first = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
moved = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))
baseline = json.loads(Path(sys.argv[5]).read_text(encoding="utf-8"))

if first["duplicate_groups"] != 1 or moved["duplicate_groups"] != 1:
    raise SystemExit("fingerprint fixtures do not each produce exactly one duplicate group")
first_fingerprint = first["findings"][0]["fingerprint"]
moved_fingerprint = moved["findings"][0]["fingerprint"]
if first_fingerprint != expected_finding:
    raise SystemExit(f"finding fingerprint golden vector changed: {first_fingerprint}")
if moved_fingerprint != expected_finding:
    raise SystemExit("finding fingerprint changed after path and physical-line relocation")
if [location["start_line"] for location in moved["findings"][0]["locations"]] != [2, 2]:
    raise SystemExit("relocation fixture did not actually move the duplicate block")
if baseline["version"] != 1 or len(baseline["groups"]) != 1:
    raise SystemExit("baseline fingerprint fixture has unexpected shape")
if baseline["groups"][0]["fingerprint"] != expected_baseline:
    raise SystemExit(
        f"baseline-v1 fingerprint golden vector changed: {baseline['groups'][0]['fingerprint']}"
    )
PY
pass "finding fingerprint golden vector is stable across path and line relocation"
pass "baseline-v1 fingerprint golden vector remains locked"

HYBRID_PROJECT="$TMP_ROOT/hybrid-project"
HYBRID_REPORTS="$TMP_ROOT/hybrid-reports"
mkdir -p "$HYBRID_PROJECT" "$HYBRID_REPORTS"
cat >"$HYBRID_PROJECT/a.py" <<'PY'
alpha = 1
beta = 2
separator = 999
alpha = 1
beta = 2
PY
write_two_line_duplicate "$HYBRID_PROJECT/b.py"

run_expect_status 1 \
    "$TMP_ROOT/hybrid.json" \
    "$TMP_ROOT/hybrid.stderr" \
    "$ARID_BIN" "$HYBRID_PROJECT" \
    --no-config \
    --project-root "$HYBRID_PROJECT" \
    --min-lines 2 \
    --json \
    --report "text=$HYBRID_REPORTS/report.txt" \
    --report "markdown=$HYBRID_REPORTS/report.md" \
    --report "sarif=$HYBRID_REPORTS/report.sarif"

python3 - \
    "$TMP_ROOT/hybrid.json" \
    "$HYBRID_REPORTS/report.txt" \
    "$HYBRID_REPORTS/report.md" \
    "$HYBRID_REPORTS/report.sarif" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
text = Path(sys.argv[2]).read_text(encoding="utf-8")
markdown = Path(sys.argv[3]).read_text(encoding="utf-8")
sarif = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))

if report["duplicate_groups"] != 1:
    raise SystemExit(f"hybrid fixture produced {report['duplicate_groups']} groups instead of 1")
finding = report["findings"][0]
if finding["distribution"] != "hybrid":
    raise SystemExit(f"JSON distribution is not hybrid: {finding['distribution']}")
if finding["occurrences"] != 3 or finding["files"] != 2:
    raise SystemExit("hybrid finding does not contain three occurrences across two files")
if "Occurrences: 3 across 2 files (hybrid)" not in text:
    raise SystemExit("text output did not preserve hybrid distribution")
if "**Occurrences:** 3 across 2 files _(hybrid)_" not in markdown:
    raise SystemExit("Markdown output did not preserve hybrid distribution")
result = sarif["runs"][0]["results"][0]
if result["properties"]["distribution"] != "hybrid":
    raise SystemExit("SARIF output did not preserve hybrid distribution")
if result["properties"]["occurrences"] != 3 or result["properties"]["files"] != 2:
    raise SystemExit("SARIF hybrid cardinality differs from report-v4")
PY
pass "hybrid distribution is preserved across JSON/text/Markdown/SARIF"

MIXED_PROJECT="$TMP_ROOT/mixed-project"
MIXED_REPORTS="$TMP_ROOT/mixed-reports"
mkdir -p "$MIXED_PROJECT" "$MIXED_REPORTS"
cat >"$MIXED_PROJECT/module.py" <<'PY'
value = 1
PY
cat >"$MIXED_PROJECT/function.py" <<'PY'
def wrapper():
    value = 1
PY

run_expect_status 1 \
    "$TMP_ROOT/mixed.json" \
    "$TMP_ROOT/mixed.stderr" \
    "$ARID_BIN" "$MIXED_PROJECT" \
    --no-config \
    --project-root "$MIXED_PROJECT" \
    --min-lines 1 \
    --json \
    --report "text=$MIXED_REPORTS/report.txt" \
    --report "markdown=$MIXED_REPORTS/report.md" \
    --report "sarif=$MIXED_REPORTS/report.sarif"

python3 - \
    "$TMP_ROOT/mixed.json" \
    "$MIXED_REPORTS/report.txt" \
    "$MIXED_REPORTS/report.md" \
    "$MIXED_REPORTS/report.sarif" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
text = Path(sys.argv[2]).read_text(encoding="utf-8")
markdown = Path(sys.argv[3]).read_text(encoding="utf-8")
sarif = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))

mixed = [
    finding
    for finding in report["findings"]
    if finding["context"] == "mixed" and finding["scope"] == "mixed"
]
if len(mixed) != 1:
    raise SystemExit("fixture did not produce exactly one mixed context/scope finding")
if "Context: mixed" not in text or "Scope: mixed" not in text:
    raise SystemExit("text output did not preserve mixed structure")
if "**Context:** mixed" not in markdown or "**Scope:** mixed" not in markdown:
    raise SystemExit("Markdown output did not preserve mixed structure")
results = sarif["runs"][0]["results"]
if not any(
    result["properties"]["context"] == "mixed"
    and result["properties"]["scope"] == "mixed"
    for result in results
):
    raise SystemExit("SARIF output did not preserve mixed structure")
PY
pass "structural mixed context/scope is preserved across all formats"

PATH_PROJECT="$TMP_ROOT/path project"
PATH_REPORTS="$TMP_ROOT/path reports"
mkdir -p "$PATH_PROJECT/space dir" "$PATH_PROJECT/other dir" "$PATH_REPORTS"
write_two_line_duplicate "$PATH_PROJECT/space dir/naïve.py"
write_two_line_duplicate "$PATH_PROJECT/other dir/über.py"

run_expect_status 1 \
    "$TMP_ROOT/paths.json" \
    "$TMP_ROOT/paths.stderr" \
    "$ARID_BIN" "$PATH_PROJECT" \
    --no-config \
    --project-root "$PATH_PROJECT" \
    --min-lines 2 \
    --json \
    --report "text=$PATH_REPORTS/report.txt" \
    --report "markdown=$PATH_REPORTS/report.md" \
    --report "sarif=$PATH_REPORTS/report.sarif"

python3 - \
    "$TMP_ROOT/paths.json" \
    "$PATH_REPORTS/report.txt" \
    "$PATH_REPORTS/report.md" \
    "$PATH_REPORTS/report.sarif" \
    <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
text = Path(sys.argv[2]).read_text(encoding="utf-8")
markdown = Path(sys.argv[3]).read_text(encoding="utf-8")
sarif = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))

expected = ["other dir/über.py", "space dir/naïve.py"]
paths = [location["path"] for location in report["findings"][0]["locations"]]
if paths != expected:
    raise SystemExit(f"report-v4 changed Unicode/space paths: {paths}")
for path in expected:
    if path not in text or path not in markdown:
        raise SystemExit(f"textual output lost path: {path}")
result = sarif["runs"][0]["results"][0]
uris = [
    result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
    *[
        item["physicalLocation"]["artifactLocation"]["uri"]
        for item in result.get("relatedLocations", [])
    ],
]
expected_uris = ["other%20dir/%C3%BCber.py", "space%20dir/na%C3%AFve.py"]
if uris != expected_uris:
    raise SystemExit(f"SARIF URI encoding changed: {uris}")
PY
pass "Unicode and space paths are stable across report and SARIF output"

(
    cd "$ROOT_DIR"
    cargo test --locked --quiet --test public_api
    cargo test --locked --quiet --doc
)
pass "supported Rust API surface and private-module boundary compile as documented"

echo
echo "V2 identity/structure integration validation PASS"
echo "Arid: $ARID_VERSION"
