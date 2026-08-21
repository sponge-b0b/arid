#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

usage() {
    cat <<'EOF'
Usage: validation/v2-project.sh <arid-bin>

Validate Arid v2 project/configuration introspection and virtual-source behavior.
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

for command in cmp cp mktemp python3 realpath; do
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

run_with_stdin_expect_status() {
    local expected="$1"
    local stdin_file="$2"
    local stdout_file="$3"
    local stderr_file="$4"
    shift 4

    local status
    set +e
    "$@" <"$stdin_file" >"$stdout_file" 2>"$stderr_file"
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

write_duplicate_source() {
    local path="$1"
    cat >"$path" <<'PY'
def duplicated_calculation():
    alpha = 1
    beta = 2
    gamma = alpha + beta
    delta = gamma * 2
    return delta
PY
}

CONFIG_PROJECT="$TMP_ROOT/config-project"
mkdir -p "$CONFIG_PROJECT/src" "$CONFIG_PROJECT/excluded"
cat >"$CONFIG_PROJECT/pyproject.toml" <<'TOML'
[tool.arid]
min-lines = 2
exclude = ["excluded/**"]
TOML
write_duplicate_source "$CONFIG_PROJECT/src/a.py"
write_duplicate_source "$CONFIG_PROJECT/src/b.py"
write_duplicate_source "$CONFIG_PROJECT/excluded/skip.py"

run_expect_status 0 \
    "$TMP_ROOT/show-config.json" \
    "$TMP_ROOT/show-config.stderr" \
    "$ARID_BIN" "$CONFIG_PROJECT/src" \
    --project-root "$CONFIG_PROJECT" \
    --show-config \
    --json

run_expect_status 0 \
    "$TMP_ROOT/show-no-config.json" \
    "$TMP_ROOT/show-no-config.stderr" \
    "$ARID_BIN" "$CONFIG_PROJECT/src" \
    --project-root "$CONFIG_PROJECT" \
    --no-config \
    --min-lines 7 \
    --show-config \
    --json

EXACT_PROJECT="$TMP_ROOT/exact-config-project"
mkdir -p "$EXACT_PROJECT/src"
cat >"$EXACT_PROJECT/pyproject.toml" <<'TOML'
[tool.arid]
min-lines = 3
same-file = false
TOML
write_duplicate_source "$EXACT_PROJECT/src/a.py"

run_expect_status 0 \
    "$TMP_ROOT/show-exact-config.json" \
    "$TMP_ROOT/show-exact-config.stderr" \
    "$ARID_BIN" "$EXACT_PROJECT/src" \
    --project-root "$EXACT_PROJECT" \
    --config "$EXACT_PROJECT/pyproject.toml" \
    --show-config \
    --json

run_expect_status 0 \
    "$TMP_ROOT/list-files.json" \
    "$TMP_ROOT/list-files.stderr" \
    "$ARID_BIN" "$CONFIG_PROJECT" \
    --project-root "$CONFIG_PROJECT" \
    --list-files \
    --json

run_expect_status 0 \
    "$TMP_ROOT/list-files.txt" \
    "$TMP_ROOT/list-files-text.stderr" \
    "$ARID_BIN" "$CONFIG_PROJECT" \
    --project-root "$CONFIG_PROJECT" \
    --list-files

python3 - \
    "$CONFIG_PROJECT" \
    "$EXACT_PROJECT" \
    "$TMP_ROOT/show-config.json" \
    "$TMP_ROOT/show-no-config.json" \
    "$TMP_ROOT/show-exact-config.json" \
    "$TMP_ROOT/list-files.json" \
    "$TMP_ROOT/list-files.txt" \
    <<'PY'
import json
import os
import sys
from pathlib import Path

config_project = os.path.realpath(sys.argv[1])
exact_project = os.path.realpath(sys.argv[2])
discovered = json.loads(Path(sys.argv[3]).read_text(encoding="utf-8"))
disabled = json.loads(Path(sys.argv[4]).read_text(encoding="utf-8"))
exact = json.loads(Path(sys.argv[5]).read_text(encoding="utf-8"))
files = json.loads(Path(sys.argv[6]).read_text(encoding="utf-8"))
text_files = Path(sys.argv[7]).read_text(encoding="utf-8").splitlines()

if os.path.realpath(discovered["project_root"]) != config_project:
    raise SystemExit("discovered configuration reports the wrong project root")
if discovered["configuration"]["state"] != "file":
    raise SystemExit("explicit project root did not load its pyproject.toml")
if os.path.realpath(discovered["configuration"]["path"]) != os.path.join(config_project, "pyproject.toml"):
    raise SystemExit("discovered configuration path is not the project-root pyproject.toml")
if discovered["settings"]["min_lines"] != 2:
    raise SystemExit("project configuration did not set min-lines=2")
if discovered["settings"]["exclude"] != ["excluded/**"]:
    raise SystemExit("project configuration did not preserve excludes")

if disabled["configuration"] != {"state": "disabled", "path": None}:
    raise SystemExit("--no-config did not disable configuration loading")
if disabled["settings"]["min_lines"] != 7:
    raise SystemExit("CLI override was not applied with --no-config")
if os.path.realpath(disabled["project_root"]) != config_project:
    raise SystemExit("--no-config changed the explicit project root")
if disabled["settings"]["exclude"]:
    raise SystemExit("--no-config retained project excludes")

if os.path.realpath(exact["project_root"]) != exact_project:
    raise SystemExit("exact configuration reports the wrong project root")
if exact["configuration"]["state"] != "file":
    raise SystemExit("--config did not select a configuration file")
if os.path.realpath(exact["configuration"]["path"]) != os.path.join(exact_project, "pyproject.toml"):
    raise SystemExit("--config did not select the exact requested pyproject.toml")
if exact["settings"]["min_lines"] != 3 or exact["settings"]["same_file"] is not False:
    raise SystemExit("exact configuration settings were not applied")

expected_files = ["src/a.py", "src/b.py"]
if files != expected_files:
    raise SystemExit(f"--list-files JSON returned unexpected files: {files}")
if text_files != expected_files:
    raise SystemExit(f"--list-files text returned unexpected files: {text_files}")
PY

pass "config, no-config, exact config, and project-root resolution"
pass "show-config exposes resolved settings and project identity"
pass "list-files is deterministic and honors project excludes"

VIRTUAL_ADD_PROJECT="$TMP_ROOT/virtual-add-project"
mkdir -p "$VIRTUAL_ADD_PROJECT"
write_duplicate_source "$VIRTUAL_ADD_PROJECT/a.py"
write_duplicate_source "$TMP_ROOT/virtual-add.py"

run_with_stdin_expect_status 1 \
    "$TMP_ROOT/virtual-add.py" \
    "$TMP_ROOT/virtual-add.json" \
    "$TMP_ROOT/virtual-add.stderr" \
    "$ARID_BIN" "$VIRTUAL_ADD_PROJECT" \
    --no-config \
    --project-root "$VIRTUAL_ADD_PROJECT" \
    --min-lines 4 \
    --stdin-path proposed.py \
    --json

[[ ! -e "$VIRTUAL_ADD_PROJECT/proposed.py" ]] ||
    die "virtual source add wrote proposed.py to disk"

python3 - "$TMP_ROOT/virtual-add.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report["analysis"]["virtual_source"] != "proposed.py":
    raise SystemExit("virtual add did not record project-relative identity")
if report["files"] != 2 or report["duplicate_groups"] != 1:
    raise SystemExit("virtual add did not participate in the two-source scan")
paths = [location["path"] for location in report["findings"][0]["locations"]]
if paths != ["a.py", "proposed.py"]:
    raise SystemExit(f"virtual add produced unexpected locations: {paths}")
PY
pass "virtual source add participates in scanning without disk writes"

VIRTUAL_REPLACE_PROJECT="$TMP_ROOT/virtual-replace-project"
mkdir -p "$VIRTUAL_REPLACE_PROJECT"
cat >"$VIRTUAL_REPLACE_PROJECT/a.py" <<'PY'
def disk_only_calculation():
    alpha = 10
    beta = 20
    gamma = alpha + beta
    delta = gamma + 1
    return delta
PY
write_duplicate_source "$VIRTUAL_REPLACE_PROJECT/b.py"
cp "$VIRTUAL_REPLACE_PROJECT/a.py" "$TMP_ROOT/virtual-replace-before.py"
write_duplicate_source "$TMP_ROOT/virtual-replacement.py"

run_with_stdin_expect_status 1 \
    "$TMP_ROOT/virtual-replacement.py" \
    "$TMP_ROOT/virtual-replace.json" \
    "$TMP_ROOT/virtual-replace.stderr" \
    "$ARID_BIN" "$VIRTUAL_REPLACE_PROJECT" \
    --no-config \
    --project-root "$VIRTUAL_REPLACE_PROJECT" \
    --min-lines 4 \
    --stdin-path a.py \
    --json

cmp -s "$TMP_ROOT/virtual-replace-before.py" "$VIRTUAL_REPLACE_PROJECT/a.py" ||
    die "virtual replacement mutated the matching disk source"

python3 - "$TMP_ROOT/virtual-replace.json" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if report["analysis"]["virtual_source"] != "a.py":
    raise SystemExit("virtual replacement identity was not recorded")
if report["files"] != 2 or report["duplicate_groups"] != 1:
    raise SystemExit("virtual replacement did not replace disk identity exactly once")
paths = [location["path"] for location in report["findings"][0]["locations"]]
if paths != ["a.py", "b.py"]:
    raise SystemExit(f"virtual replacement produced unexpected locations: {paths}")
PY
pass "virtual source replaces disk identity exactly once without mutation"

echo
echo "V2 project/input integration validation PASS"
echo "Arid: $ARID_VERSION"
