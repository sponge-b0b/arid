# Migrating to Arid v2

Arid 2.0 is a machine-contract and automation release built around the same exact normalized duplicate detector used by the qualified v1.2 line.

For most developers who run Arid from the command line and read text output, migration is intentionally small. The breaking changes are concentrated in machine-readable report consumers and Rust embedding code.

## Who needs to migrate?

### Ordinary CLI users

If you use commands such as:

```bash
arid .
arid src tests
arid . --min-lines 8
arid . --baseline arid-baseline.json
```

and do not parse Arid's JSON/SARIF output or depend on Arid's Rust internals, you may require **no migration work**.

V2 preserves:

- exact normalized duplicate semantics
- `DUP001`
- normal CLI invocation
- existing CLI option names
- `[tool.arid]` configuration
- baseline schema v1 and existing baseline files
- normalization behavior
- source suppression through `# arid: disable` / `# arid: enable`
- serial execution as the default
- numeric `--workers N`
- `--workers auto`
- default exit meanings `0` / `1` / `2`
- pre-commit integration
- the supported platform set

The main visible terminology change is that occurrence distribution `mixed` is now called `hybrid`. Structural context and scope still use `mixed` where appropriate.

### Machine-output consumers

You **must** migrate if you consume Arid report JSON as a stable contract.

V2 changes report schema v3 to report schema v4 and introduces required report metadata and finding identity.

You should also review the SARIF identity change if you consume Arid SARIF results programmatically.

### Rust embedding consumers

You **must** migrate if your Rust code imports Arid implementation modules or internal report/detector types.

V2 deliberately narrows the supported Rust API to a small crate-root application interface.

---

## Report JSON: v3 to v4

The current v2 report schema is:

```text
schemas/report-v4.schema.json
```

The historical v1 report schema remains published unchanged at:

```text
schemas/report-v3.schema.json
```

### Before: report v3

A representative v1 report looked like:

```json
{
  "version": 3,
  "files": 2,
  "source_lines": 20,
  "analyzed_lines": 12,
  "duplicate_groups": 1,
  "duplicate_lines": 4,
  "duplication_percent": 33.33,
  "findings": [
    {
      "code": "DUP001",
      "lines": 4,
      "context": "executable",
      "scope": "function",
      "occurrences": 3,
      "files": 2,
      "distribution": "mixed",
      "locations": [
        {
          "path": "src/a.py",
          "start_line": 10,
          "end_line": 13
        },
        {
          "path": "src/a.py",
          "start_line": 30,
          "end_line": 33
        },
        {
          "path": "src/b.py",
          "start_line": 5,
          "end_line": 8
        }
      ]
    }
  ]
}
```

### After: report v4

The equivalent v2 shape is:

```json
{
  "schema_version": 4,
  "tool_version": "2.0.0",
  "complete": true,
  "analysis": {
    "min_lines": 4,
    "ignore_comments": true,
    "ignore_docstrings": true,
    "ignore_imports": true,
    "ignore_signatures": true,
    "same_file": true,
    "hidden": false,
    "exclude": [],
    "baseline_enabled": false,
    "focus": [],
    "virtual_source": null,
    "keep_going": false
  },
  "errors": [],
  "files": 2,
  "source_lines": 20,
  "analyzed_lines": 12,
  "duplicate_groups": 1,
  "duplicate_lines": 4,
  "duplication_percent": 33.33,
  "findings": [
    {
      "code": "DUP001",
      "fingerprint": "arid-finding-v1:sha256:<64-lowercase-hex-digits>",
      "lines": 4,
      "context": "executable",
      "scope": "function",
      "occurrences": 3,
      "files": 2,
      "distribution": "hybrid",
      "locations": [
        {
          "path": "src/a.py",
          "start_line": 10,
          "end_line": 13
        },
        {
          "path": "src/a.py",
          "start_line": 30,
          "end_line": 33
        },
        {
          "path": "src/b.py",
          "start_line": 5,
          "end_line": 8
        }
      ]
    }
  ]
}
```

The exact fingerprint value depends on the normalized duplicate content; the placeholder above is illustrative.

### `version` becomes `schema_version`

V1:

```json
{
  "version": 3
}
```

V2:

```json
{
  "schema_version": 4
}
```

Do not treat `tool_version` as the schema discriminator. Consumers should branch on `schema_version`.

### New required report metadata

Report v4 adds four required top-level fields:

```text
tool_version
complete
analysis
errors
```

`tool_version` records the Arid version that produced the report.

`complete` states whether the scan completed all requested source processing. Normal complete scans use:

```json
"complete": true
```

A `--keep-going` scan that encounters one or more independent source-processing errors returns a report with:

```json
"complete": false
```

and exits with status `2`.

`analysis` records the resolved semantics that affect the scan result, including normalization settings, same-file behavior, hidden/exclude settings, baseline use, focus selectors, virtual input, and keep-going mode.

`errors` contains deterministic structured source-processing errors for incomplete keep-going reports. Complete reports normally contain an empty array.

### Distribution `mixed` becomes `hybrid`

V1 used:

```json
"distribution": "mixed"
```

when a duplicate occurred across multiple files and at least one involved file contained multiple occurrences.

V2 calls that distribution:

```json
"distribution": "hybrid"
```

The full distribution vocabulary is now:

```text
same-file
cross-file
hybrid
```

This rename applies to **occurrence distribution only**.

Structural metadata retains `mixed`:

```json
{
  "context": "mixed",
  "scope": "mixed"
}
```

when a finding genuinely spans multiple structural contexts or scopes.

### Stable finding fingerprints

Every report-v4 finding has a required fingerprint:

```json
"fingerprint": "arid-finding-v1:sha256:..."
```

Finding fingerprint v1 identifies the normalized duplicate content rather than a physical source location.

It is intentionally independent of:

- file path
- physical line number
- occurrence order
- occurrence multiplicity
- structural context/scope metadata
- output format
- worker mode

Changing the normalized duplicate content changes the fingerprint.

Consumers that need to correlate the same logical duplicate across path moves or line-number changes should prefer the fingerprint over location-based identity.

Do not construct or reinterpret this value yourself. Treat the `arid-finding-v1` prefix as part of the versioned identity contract.

---

## Operational errors and incomplete reports

V2 separates a complete report from operational failure more explicitly.

For JSON-mode fatal operational errors, Arid emits the versioned error document described by:

```text
schemas/error-v1.schema.json
```

A top-level error document has the shape:

```json
{
  "schema_version": 1,
  "tool_version": "2.0.0",
  "error": {
    "kind": "parse",
    "message": "...",
    "path": "src/broken.py"
  }
}
```

The `path` member is present only when relevant.

With `--keep-going`, independent source read/parse/normalization failures are instead represented in report-v4 `errors`, the report uses `complete: false`, and the process still exits `2`.

Do not interpret `--no-fail-on-findings` as a general error suppression switch. It can map a complete findings-only exit `1` to `0`; it never masks exit `2`.

---

## SARIF identity

Arid continues to emit SARIF 2.1.0.

V2 adds Arid's stable finding identity to each SARIF result through standard `partialFingerprints`:

```json
{
  "partialFingerprints": {
    "aridFindingFingerprint/v1": "arid-finding-v1:sha256:..."
  }
}
```

That value is the same fingerprint exposed on the corresponding report-v4 finding.

Arid does not misuse GitHub-specific line hashes as content identity. Consumers should use `aridFindingFingerprint/v1` when they need Arid's path-independent duplicate identity.

Incomplete `--keep-going` reports cannot be rendered as SARIF because publishing partial SARIF results as if they represented a complete scan would be misleading.

---

## Baselines

Baseline schema v1 is unchanged.

Existing v1.2 baseline files remain valid in v2:

```text
schemas/baseline-v1.schema.json
```

Normal baseline enforcement remains compatible:

```bash
arid . --baseline arid-baseline.json
```

V2 adds lifecycle operations without introducing baseline schema v2:

```bash
arid . --baseline-status arid-baseline.json
arid . --prune-baseline arid-baseline.json
```

`--baseline-status` reports accepted, active/new, and stale debt.

`--prune-baseline` removes stale acceptance only. It does not accept new duplicate debt.

`--write-baseline` remains the explicit operation for replacing accepted debt with the current duplicate set.

---

## New v2 CLI workflow controls

The following v2 controls are additive. Existing CLI workflows do not need to adopt them merely because they exist.

### Explicit project and configuration selection

```bash
arid . --config path/to/pyproject.toml
arid . --no-config
arid . --project-root path/to/project
```

`--config` selects one exact `pyproject.toml` instead of ancestor discovery.

`--no-config` disables configuration discovery and uses built-in defaults plus CLI overrides.

`--project-root` sets the root used for configuration lookup and project-relative identity.

Contradictory root/config combinations fail rather than being silently guessed.

### Introspection

```bash
arid . --show-config
arid . --list-files
arid --capabilities
```

`--show-config` exposes the resolved project configuration.

`--list-files` exposes the exact Python files normal discovery would select, but stops before parsing and duplicate detection.

`--capabilities` emits deterministic capability JSON without requiring project discovery.

The capabilities contract is described by:

```text
schemas/capabilities-v1.schema.json
```

### Focused reporting

```bash
arid . --focus src/changed.py
arid . --focus src/package --focus tests/package
```

`--focus` changes which duplicate groups are reported, **not what code is compared**.

Arid still performs whole-corpus detection, then baseline enforcement, then focus filtering. A focused finding retains all of its occurrences, including occurrences outside the focused path.

### Virtual standard-input source

```bash
cat src/example.py | arid . --stdin-path src/example.py
```

`--stdin-path` provides exactly one virtual Python source through standard input.

If the virtual path corresponds to a disk file in the corpus, the virtual content replaces that file for the scan. Otherwise it is added as a virtual source when allowed by the resolved scan context.

Arid never writes the virtual source to disk.

### Keep-going analysis

```bash
arid . --keep-going --json
```

Arid normally fails fast on source-processing errors.

`--keep-going` allows independent file-local read/parse/normalization failures to be collected while valid files continue through detection. The resulting report is explicitly incomplete and exit status remains `2`.

### Multiple reports from one scan

```bash
arid . \
  --format text \
  --report json=artifacts/arid.json \
  --report markdown=artifacts/arid.md \
  --report sarif=artifacts/arid.sarif
```

Repeatable `--report FORMAT=PATH` writes supplemental text, JSON, Markdown, or SARIF from the same in-memory logical report. Arid does not reparse or redetect once per output.

### Findings-only exit control

```bash
arid . --no-fail-on-findings
```

On a complete scan, this maps findings-only exit status `1` to success `0`.

Operational/incomplete status `2` remains `2`.

---

## Source-level suppression

Source suppression behavior is unchanged from v1.2:

```python
# arid: disable

# intentionally accepted duplicate code

# arid: enable
```

Code inside the disabled region does not participate in duplicate detection, and the suppression creates a matching boundary so a duplicate cannot bridge across the ignored region.

---

## Rust API migration

Arid remains primarily a CLI application. V2 intentionally stops treating implementation modules as supported library API.

The supported crate-root API is:

```rust
use arid::{
    Cli,
    ColorEnvironment,
    ExitStatus,
    RunContext,
    RunResult,
    run,
    run_with_context,
};
```

A basic embedding flow is:

```rust
use arid::{Cli, RunResult, run};
use clap::Parser;

let cli = Cli::parse_from(["arid", "--capabilities"]);
let result: RunResult = run(&cli);

let stdout = result.stdout();
let stderr = result.stderr();
let status = result.exit_status();
```

Embedding callers that need an explicit execution context can use `RunContext`, `ColorEnvironment`, and `run_with_context`.

Imports through implementation modules are no longer supported. For example, code such as:

```rust,ignore
use arid::report::Report;
use arid::detect::detect;
```

must not be treated as part of Arid's semver-stable API in v2.

The supported boundary is intentionally an application-entry API rather than a public detector framework.

---

## Migration checklist

If you are upgrading an integration from Arid 1.2 to Arid 2.0:

1. **CLI-only users:** run your existing command first. It may require no change.
2. **JSON consumers:** switch from report schema v3 to v4.
3. Replace top-level `version` checks with `schema_version == 4`.
4. Accept the new required `tool_version`, `complete`, `analysis`, and `errors` fields.
5. Accept required finding `fingerprint`.
6. Replace occurrence distribution `mixed` with `hybrid`; do not replace structural context/scope `mixed`.
7. If correlating findings across runs, prefer the versioned fingerprint over paths/line numbers.
8. **SARIF consumers:** read `partialFingerprints["aridFindingFingerprint/v1"]` when stable Arid identity is needed.
9. **Baseline users:** keep existing baseline-v1 files; no conversion is required.
10. **Rust consumers:** migrate to the supported crate-root application API.
11. Preserve exit status `2` as operational/incomplete failure even when using `--no-fail-on-findings`.
12. Validate machine output against the published v2 schemas rather than inferring shape from examples.

---

## Compatibility summary

| Surface | v1.2 → v2 |
| --- | --- |
| Exact duplicate semantics | Preserved |
| `DUP001` | Preserved |
| Normal CLI invocation | Preserved |
| Existing CLI option names | Preserved |
| `[tool.arid]` | Preserved |
| Suppression directives | Preserved |
| Baseline schema v1 | Preserved |
| Existing baseline files | Preserved |
| Default exit meanings | Preserved |
| Report JSON | **Breaking: v3 → v4** |
| Report `version` | **Breaking: renamed to `schema_version`** |
| Report metadata | **Breaking: new required fields** |
| Finding fingerprint | **Breaking/additive required field in v4** |
| Distribution `mixed` | **Breaking: renamed to `hybrid`** |
| Structural context/scope `mixed` | Preserved |
| SARIF version | Preserved at 2.1.0 |
| SARIF Arid identity | Added through versioned `partialFingerprints` |
| Rust implementation modules | **No longer supported API** |
| Rust crate-root application API | Supported v2 boundary |

Arid v2 intentionally concentrates its breaking changes at machine-contract and Rust-boundary surfaces while keeping normal duplicate-checking workflows familiar.
