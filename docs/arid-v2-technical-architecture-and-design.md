# **Arid v2 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.0.0`  
**Requirements:** `docs/arid-v2-requirements-specification.md`

---

# **1. Purpose**

This document defines the technical architecture for Arid 2.0.

The v1, v1.1, and v1.2 architecture documents remain historical design context. V2 deliberately preserves the proven exact-duplicate detector while changing the public report contract and adding automation around one authoritative scan.

The v2 architecture exists to implement:

- report schema v4
- stable finding fingerprints
- occurrence distribution `hybrid`
- self-describing reports
- focused reporting
- keep-going partial analysis
- structured operational errors
- baseline status and safe pruning
- explicit project/configuration control
- configuration and discovery introspection
- virtual stdin source
- multiple report outputs from one scan
- machine capability discovery
- a deliberately narrow Rust public API
- an official GitHub Actions integration
- release and qualification support for the new contracts

The detector itself is not redesigned.

---

# **2. Architectural Principles**

V2 follows these principles.

## **2.1 One authoritative detector path**

There is one source-preparation path, one corpus, and one duplicate-detection pass.

Features such as baseline enforcement, focus, multiple outputs, GitHub integration, and virtual input operate around that path rather than creating alternate definitions of duplication.

Conceptually:

```text
project/config resolution
        ↓
disk discovery
        ↓
optional virtual-source merge
        ↓
source preparation
        ↓
corpus
        ↓
exact duplicate detection
        ↓
optional baseline enforcement
        ↓
optional focus filtering
        ↓
report v4
        ↓
concrete renderers / integrations
```

## **2.2 Breaking changes stop at public contracts**

V2 uses the major-version boundary to clean up public contracts that were intentionally deferred from 1.x.

It does not use the boundary as permission to churn unrelated CLI behavior, normalization, baseline files, detector semantics, or platform support.

## **2.3 Determinism is structural**

Determinism must come from explicit ordering and canonical encodings, not from incidental container iteration order or execution timing.

Parallel preparation may complete in any order internally, but externally observable collections are produced in deterministic source-path order.

## **2.4 Machine consumers receive typed facts**

JSON and SARIF consumers must not parse prose to determine:

- report schema version
- tool version
- scan completeness
- error category
- finding identity
- occurrence distribution
- effective analysis settings

Human-readable messages remain useful, but they are not the machine contract.

## **2.5 Administrative operations are not report formats**

Baseline writing/status/pruning, configuration display, file listing, and capability discovery are explicit invocation modes.

They do not become plugins, generic command pipelines, or a reporter framework.

## **2.6 Filesystem mutation is narrow**

Normal scans remain read-only except for explicitly requested report files.

Baseline mutation occurs only through:

```text
--write-baseline
--prune-baseline
```

Virtual stdin source is never written to disk.

## **2.7 Keep partial results honest**

`--keep-going` may preserve useful findings after independent source failures, but a partial scan never becomes a successful scan.

`complete = false` and exit status `2` are invariant for any incomplete analysis.

---

# **3. Major Technical Decisions**

| Area | V2 decision |
| --- | --- |
| Detection semantics | unchanged from 1.2 |
| Detection algorithm | retain suffix-array/LCP implementation unless evidence justifies replacement |
| Main orchestration | move from `lib.rs` into private `app.rs` |
| Rust API | small root-level re-exported surface; implementation modules private |
| Report schema | v4 only for Arid 2.x output |
| Historical report schema | v3 retained unchanged under `schemas/` |
| Baseline schema | v1 retained unchanged |
| Finding identity | SHA-256, dedicated `arid-finding-v1` domain |
| Finding identity path dependence | none |
| Distribution vocabulary | `same-file`, `cross-file`, `hybrid` |
| Structural vocabulary | existing `mixed` remains for context/scope |
| Operational errors | one typed internal/public machine model |
| Fatal machine errors | `schemas/error-v1.schema.json` |
| Capabilities | `schemas/capabilities-v1.schema.json` |
| Focus | post-baseline reporting filter; full corpus still detected |
| Keep-going | file-local read/parse/normalization failures only |
| Incomplete SARIF | not emitted/uploaded |
| Baseline maintenance | one comparison engine drives enforcement/status/pruning |
| Explicit project paths | canonical project-relative helper; no global path-format rewrite |
| Virtual source | one stdin source merged before preparation |
| Multi-output | concrete renderers consume one report; no registry |
| Supplemental text files | plain, non-ANSI text |
| GitHub Action | root composite `action.yml` in this repository |
| Action installation | exact PyPI Arid version selected by action metadata/input |
| Action metrics | parsed from one supplemental report-v4 JSON file |
| Release metadata | v2 release preparation also manages action package version |
| Competitive benchmark | reproducible pinned benchmark plus one pre-RC current-Pylint capture |

---

# **4. Intended Repository Delta**

V2 SHOULD evolve the current structure toward:

```text
src/
├── app.rs
├── baseline.rs
├── baseline_compare.rs
├── capabilities.rs
├── cli.rs
├── config.rs
├── corpus.rs
├── detect.rs
├── error.rs
├── files.rs
├── fingerprint.rs
├── focus.rs
├── lib.rs
├── main.rs
├── markdown.rs
├── metrics.rs
├── model.rs
├── normalize.rs
├── outcome.rs
├── output.rs
├── python.rs
├── report.rs
├── sarif.rs
├── source.rs
├── suffix.rs
└── text.rs

schemas/
├── baseline-v1.schema.json
├── capabilities-v1.schema.json
├── error-v1.schema.json
├── report-v3.schema.json
└── report-v4.schema.json

action.yml
action/
└── run.py

validation/
└── v2.sh

docs/
├── arid-v2-migration-guide.md
├── arid-v2-release-roadmap.md
└── releases/
    └── ... v2 release-note files
```

This is a responsibility split, not a requirement to create abstraction for its own sake.

Responsibilities:

```text
app.rs
    invocation-mode resolution
    scan orchestration
    administrative orchestration
    process-level outcome construction

error.rs
    ErrorKind
    OperationalError
    fatal error document
    stable machine error serialization

source.rs
    disk/virtual source inputs
    virtual replacement/addition
    read + prepare coordination
    keep-going collection

fingerprint.rs
    finding-fingerprint v1 encoding
    internal shared normalized-block byte helpers where safe

focus.rs
    focus selector normalization
    selector validation
    group filtering

baseline.rs
    unchanged baseline-v1 data contract
    baseline-v1 fingerprint contract
    deterministic read/write

baseline_compare.rs
    current-vs-baseline comparison
    normal enforcement
    status model
    safe pruned-baseline derivation

report.rs
    v4 report model
    finding construction
    analysis metadata
    canonical ordering
    JSON rendering

output.rs
    supplemental report-target validation
    concrete renderer dispatch
    atomic per-file report writing

capabilities.rs
    capabilities-v1 document

text.rs / markdown.rs / sarif.rs
    concrete presentation only
```

`lib.rs` becomes a small public boundary rather than the application implementation file.

---

# **5. Supported Rust Public API**

Arid remains primarily a CLI application. V2 intentionally stops exposing implementation modules as a supported semver surface.

The following modules SHOULD become private:

```text
baseline
baseline_compare
capabilities
cli
config
corpus
detect
error
files
fingerprint
focus
markdown
metrics
model
normalize
outcome
output
python
report
sarif
source
suffix
text
```

Selected types/functions are re-exported at the crate root.

The intended supported API is conceptually:

```rust
pub use cli::Cli;
pub use outcome::ExitStatus;

pub struct ColorEnvironment { /* private fields */ }
pub struct RunContext { /* private fields */ }
pub struct RunResult { /* private fields */ }

pub fn run(cli: &Cli) -> RunResult;
pub fn run_with_context(cli: &Cli, context: RunContext) -> RunResult;
```

`Cli` MAY remain publicly parseable through Clap while its fields become crate-private where practical. External callers should not depend on the internal field layout.

`RunContext` MUST provide constructors/builders sufficient for:

- deterministic non-terminal execution
- explicit terminal-color capability
- explicit color-environment values
- optional virtual stdin source text

Conceptually:

```rust
impl RunContext {
    pub const fn non_terminal() -> Self;
    pub fn new(text_color_capable: bool, color: ColorEnvironment) -> Self;
    pub fn with_stdin_source(self, source: String) -> Self;
}
```

`RunResult` MUST expose accessors for:

```text
stdout
stderr
exit_status
```

The exact field representation remains private.

Expected operational failures are represented as process outcomes rather than bubbling `Result<_, String>` through the public API.

This means:

```rust
let result = arid::run(&cli);
result.stdout();
result.stderr();
result.exit_status();
```

is sufficient for an embedding caller without exposing corpus, detector, baseline, or renderer internals.

Panics remain reserved for violated internal invariants; normal user/input failures become `ExitStatus::Error` outcomes.

---

# **6. Process Outcome Model**

V1 currently returns rendered stdout on success and a string error through `Result`.

V2 SHOULD centralize process behavior:

```rust
pub struct RunResult {
    stdout: String,
    stderr: String,
    exit_status: ExitStatus,
}
```

Internal execution MAY use:

```rust
fn execute(...) -> Result<Execution, OperationalError>;
```

but the public entry point converts that result into the correct process representation.

## **6.1 Text-oriented fatal errors**

For normal text, Markdown, or SARIF primary selection, a fatal pre-report error produces:

```text
stdout = ""
stderr = "error: ...\n"
exit = 2
```

## **6.2 JSON fatal errors**

When the primary output is JSON through either:

```text
--json
--format json
```

and CLI parsing itself has already succeeded, a fatal execution error produces:

```text
stdout = error-v1 JSON
stderr = ""
exit = 2
```

A supplemental `--report json=...` target does not change fatal-error stdout semantics. Supplemental report files are only report-v4 documents and therefore are not written when no report exists.

This prevents one output path from ambiguously containing either report-v4 or error-v1 based on runtime state.

## **6.3 CLI parser errors**

Clap remains responsible for malformed command-line syntax that prevents a valid invocation/output mode from being established.

Those errors retain standard CLI usage diagnostics.

---

# **7. Operational Error Model**

Add a stable error kind enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ErrorKind {
    Configuration,
    Discovery,
    Read,
    Parse,
    Normalization,
    Baseline,
    Output,
    Internal,
}
```

The logical error object is:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OperationalError {
    kind: ErrorKind,
    message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}
```

`message` remains human-readable and MAY improve without a schema change.

`kind` is the stable branchable machine value.

`path` is emitted as a canonical project-relative path when the failed artifact can be represented relative to the selected project root. If a path cannot safely be represented in that public form, the machine field is omitted rather than leaking a volatile machine-specific absolute path; the human message may still identify the offending input.

## **7.1 Fatal error document**

Add:

```rust
struct ErrorDocument {
    schema_version: u8,       // 1
    tool_version: &'static str,
    error: OperationalError,
}
```

Serialized shape:

```json
{
  "schema_version": 1,
  "tool_version": "2.0.0",
  "error": {
    "kind": "configuration",
    "message": "..."
  }
}
```

Schema:

```text
schemas/error-v1.schema.json
```

The error schema uses JSON Schema Draft 2020-12 and rejects unknown object properties.

## **7.2 Error conversion boundaries**

Existing domain-specific errors remain useful internally. They SHOULD be converted to `OperationalError` at subsystem boundaries rather than replacing every internal error enum with one giant application enum.

Examples:

```text
ConfigError        → configuration
DiscoveryError     → discovery
fs::read failure   → read
PrepareError       → parse or normalization
BaselineError      → baseline
serialization/I/O  → output
violated recovered invariant → internal
```

This preserves meaningful local error types while presenting one stable machine contract externally.

---

# **8. Project Context and Configuration Resolution**

V2 introduces an explicit resolved context rather than passing project-root/config decisions loosely through orchestration.

Conceptually:

```rust
struct ProjectContext {
    root: PathBuf,
    config_path: Option<PathBuf>,
    settings: Settings,
}
```

`Settings` remains the effective persistent detection/configuration state.

## **8.1 Default compatibility path**

With none of:

```text
--config
--no-config
--project-root
```

configuration discovery remains compatible with 1.x: start from the first effective scan path and walk ancestors for the nearest `pyproject.toml` containing `[tool.arid]`.

## **8.2 Exact config path**

`--config <PATH>`:

- requires an existing regular file
- requires the selected file to be a `pyproject.toml`
- disables ancestor search
- uses the file's parent as project root unless `--project-root` is also supplied

## **8.3 No-config mode**

`--no-config`:

- disables `[tool.arid]` loading
- retains built-in defaults
- applies explicit CLI overrides normally
- may be combined with `--project-root`

`--config` and `--no-config` conflict.

## **8.4 Explicit project root**

`--project-root <PATH>` requires an existing directory.

Without `--config` or `--no-config`, Arid considers only:

```text
<project-root>/pyproject.toml
```

for project configuration and never walks above the explicit root.

If the file exists without `[tool.arid]`, Arid behaves as though no Arid project configuration was present at that root.

## **8.5 Exact config plus explicit root**

When both are supplied, the exact config path MUST resolve to:

```text
<project-root>/pyproject.toml
```

Otherwise the invocation fails as contradictory.

This avoids ambiguous rules for whether configured relative paths are rooted at the config directory or at a separately selected project root.

## **8.6 Precedence**

The setting precedence remains:

```text
explicit CLI override
        ↓
selected [tool.arid]
        ↓
built-in default
```

Explicit configuration-selection flags choose where the middle layer comes from; they do not alter precedence.

---

# **9. Canonical Project-Relative Paths**

V2 needs consistent path identity for:

- baselines
- focus selectors
- virtual source
- report analysis metadata
- structured error paths
- action outputs/integrations

The existing baseline-v1 path rules already define a useful canonical form:

- relative to project root
- `/` separator
- no absolute paths
- no drive prefixes
- no empty components
- no `.` or `..` components
- no backslashes
- no NULs
- UTF-8

V2 SHOULD extract reusable internal path conversion/validation helpers from baseline-specific code without changing baseline-v1 serialization.

This does **not** require every normal Arid scan to reject a source outside the project root. Existing 1.x path behavior remains available for ordinary scanning.

Features that require stable project identity—baseline mutation, focus selectors, and virtual stdin source—MUST reject paths that cannot be represented safely inside the selected project root.

---

# **10. Invocation Modes**

The CLI remains flag-oriented rather than introducing subcommands solely for organization.

Internally, v2 resolves one mode early:

```rust
enum Mode {
    Scan,
    WriteBaseline(PathBuf),
    BaselineStatus(PathBuf),
    PruneBaseline(PathBuf),
    ShowConfig,
    ListFiles,
    Capabilities,
}
```

Mode resolution occurs after Clap parsing but before expensive work.

## **10.1 Scan-only options**

These apply only to `Mode::Scan`:

```text
--focus
--keep-going
--stdin-path
--report
--no-fail-on-findings
```

## **10.2 Detection administrative modes**

These require a complete current detector result:

```text
--write-baseline
--baseline-status
--prune-baseline
```

They do not accept `--keep-going`, because a partial corpus cannot safely determine accepted or stale baseline debt.

`--stdin-path` is also scan-only; Arid does not mutate or prune persistent baseline state based on unsaved virtual source.

## **10.3 Introspection modes**

`--show-config` resolves project/configuration but does not discover/parse source.

`--list-files` resolves project/configuration and discovery but does not read/parse source.

`--capabilities` does neither.

## **10.4 Output selection for administrative modes**

`ShowConfig`, `ListFiles`, `BaselineStatus`, and `PruneBaseline` support:

```text
text
json
```

through the existing primary output selector.

Markdown and SARIF are rejected for those operations.

`WriteBaseline` remains a file mutation with no normal report output.

`Capabilities` always emits JSON and rejects unrelated primary output selectors.

A small semantic validator in `app.rs` SHOULD enforce mode interactions rather than encoding every cross-product as Clap attributes.

Obvious local conflicts such as `--config` with `--no-config` remain Clap-level conflicts.

---

# **11. Source Input Model**

Current `prepare_file` already accepts a path plus source text. V2 should preserve that useful boundary and add one small source-input layer before preparation.

Conceptually:

```rust
enum SourceInput {
    Disk(PathBuf),
    Virtual {
        path: PathBuf,
        source: String,
    },
}
```

The path stored for virtual input is the absolute in-project path produced from the canonical `--stdin-path` identity. It behaves like a normal source path downstream without touching disk.

Source inputs are sorted deterministically by path before preparation.

---

# **12. Virtual Source from Standard Input**

The binary reads stdin only when `--stdin-path` is present. Normal Arid execution MUST NOT touch or block on stdin.

`main.rs` conceptually becomes:

```text
parse CLI
    ↓
if --stdin-path:
    read stdin to String
    place text in RunContext
    ↓
run_with_context
```

Library callers supply virtual source explicitly through `RunContext`.

## **12.1 Path resolution**

`--stdin-path <PATH>` is resolved against the selected project root.

The resulting path MUST:

- remain within project root
- have `.py` or `.pyi` extension
- satisfy canonical project-relative path rules
- not be excluded by configured Arid `exclude`

Ordinary `.gitignore` does not reject virtual source, matching explicit-file intent.

## **12.2 Merge behavior**

After disk discovery but before preparation:

```text
virtual path matches discovered disk file
    → replace disk SourceInput with Virtual

virtual path does not match discovered disk file
    → add Virtual SourceInput
```

There is never more than one source identity for the virtual path.

After merge, source inputs are sorted again deterministically.

## **12.3 Downstream behavior**

Virtual source follows normal:

```text
Python parse
normalization
corpus construction
detection
baseline enforcement
focus matching
report location mapping
```

No detector code knows whether a prepared file originated from disk or stdin.

---

# **13. Source Preparation and Keep-Going**

Replace `prepare_files(Vec<PathBuf>, ...)` with preparation over `Vec<SourceInput>`.

Conceptually:

```rust
struct PreparationResult {
    files: Vec<PreparedFile>,
    errors: Vec<OperationalError>,
}
```

## **13.1 Disk source**

A disk input:

1. reads UTF-8 source
2. maps read failure to `ErrorKind::Read`
3. calls existing `prepare_file`
4. maps Python parsing failure to `ErrorKind::Parse`

Recoverable future normalization failures map to `ErrorKind::Normalization`.

## **13.2 Virtual source**

A virtual input skips disk I/O and calls the same `prepare_file` function with the supplied source text.

## **13.3 Parallelism**

Serial, numeric-worker, and `auto` behavior remain unchanged.

Parallel preparation collects per-input results into an indexed vector preserving source-input order.

Therefore error selection/order is deterministic even if worker completion order differs.

## **13.4 Fail-fast mode**

Without `--keep-going`, any source-preparation error prevents corpus/detection/report construction.

When multiple parallel tasks fail, Arid reports the earliest failing input in deterministic source order.

The implementation does not need complex worker cancellation solely to stop already-running preparation tasks; fail-fast describes the externally visible scan result.

## **13.5 Keep-going mode**

With `--keep-going`:

- successful prepared files continue
- all file-local preparation errors are retained
- errors remain in deterministic source order
- corpus construction uses successful prepared files only
- duplicate detection runs once on that successful corpus
- report `complete` is false when at least one preparation error exists
- report `errors` contains those operational errors
- process exit status is `2`

If no file-local errors occur, a keep-going invocation produces the same complete report as a normal invocation.

## **13.6 Global errors**

Project/configuration, discovery, baseline loading/validation, output failures, and internal invariant failures remain fatal rather than keep-going candidates.

---

# **14. Detector Pipeline**

The existing detector pipeline remains technically intact:

```text
PreparedFile[]
    ↓
build_corpus
    ↓
detect_duplicates
    ↓
DuplicateGroup[]
```

The suffix-array/LCP implementation remains the preferred current design because it is proven and not the measured v1.2 bottleneck.

V2 removes the algorithm choice from the product requirements but does not create implementation work merely to exercise that freedom.

No focus, baseline, output, error, action, or fingerprint feature may alter:

- normalized equality
- corpus sentinel semantics
- suffix-array ordering
- LCP meaning
- maximal-repeat qualification
- same-file overlap handling
- canonical duplicate grouping

---

# **15. Baseline Comparison Engine**

V1.1 enforcement currently computes current baseline identity group-by-group inside `baseline_filter.rs`.

V2 should consolidate baseline reasoning into one private comparison module so enforcement, status, and pruning cannot drift apart.

Rename/evolve:

```text
baseline_filter.rs
        ↓
baseline_compare.rs
```

The baseline-v1 storage/fingerprint contract remains in `baseline.rs`.

## **15.1 Comparison model**

For every current duplicate fingerprint/path multiplicity and every baseline fingerprint/path multiplicity, derive:

```text
accepted = min(current, baseline)
active   = max(current - baseline, 0)
stale    = max(baseline - current, 0)
```

A baseline fingerprint missing from current detection is wholly stale.

A current fingerprint missing from baseline is wholly active.

This path-count arithmetic generalizes the existing enforcement rules without changing them.

## **15.2 Comparison result**

Conceptually:

```rust
struct BaselineComparison {
    active_groups: Vec<DuplicateGroup>,
    status: BaselineStatus,
    pruned: Baseline,
}
```

`active_groups` contains each complete current duplicate group when any active debt exists for that fingerprint.

It never truncates a group to only newly introduced occurrences.

## **15.3 Normal enforcement**

Normal `--baseline` behavior consumes only `active_groups`.

This is behaviorally equivalent to v1.1/v1.2 enforcement.

## **15.4 Baseline status**

`BaselineStatus` is an administrative model grouped deterministically by fingerprint and path.

Each entry exposes enough counts to distinguish:

```text
accepted
active/new
stale
```

Text rendering summarizes these states for humans.

JSON rendering uses a small versioned administrative document with explicit counts. The architecture MAY use an inline `schema_version: 1` field without introducing another published schema file unless implementation demonstrates external schema value sufficient to justify one.

Exit:

```text
active count == 0 → 0
active count > 0  → 1
error             → 2
```

Stale acceptance alone does not fail status.

## **15.5 Safe pruning**

The pruned baseline is exactly the accepted intersection of baseline state with current duplicate state.

For each baseline fingerprint/path:

```text
new accepted count = min(old accepted count, current count)
```

Missing current paths/groups are removed.

New current paths/groups are never added.

Counts are never increased.

The resulting baseline may retain an accepted count that is only part of an otherwise active group. This is safe: the unaccepted portion remains active on subsequent enforcement.

Pruning:

1. requires a complete scan
2. builds the deterministic pruned baseline in memory
3. validates it under existing baseline-v1 rules
4. compares canonical serialized bytes
5. skips the filesystem write when bytes are unchanged
6. otherwise atomically replaces the baseline using the existing atomic-write dependency

Exit after pruning reflects whether active/new debt remains.

---

# **16. Focused Reporting**

Focus is implemented entirely after baseline enforcement.

Pipeline order:

```text
all detected groups
        ↓
optional baseline comparison
        ↓
active groups
        ↓
focus filter
        ↓
reportable groups
```

## **16.1 Focus selector representation**

CLI values are normalized to canonical project-relative path selectors.

A selector matches a source when either:

- selector equals the source's project-relative path, or
- selector is a path-component ancestor of the source path

Therefore one rule handles both file and directory focus without depending on filesystem type metadata.

Examples:

```text
focus: src/payments.py
matches: src/payments.py

focus: src/payments
matches: src/payments/api.py
         src/payments/model.py
```

It does not use raw string-prefix matching.

## **16.2 Validation timing**

Every focus selector is validated against the complete merged source-input set after virtual-source insertion and before source preparation.

This prevents a focused file that later fails parsing under `--keep-going` from being misreported as an unmatched selector.

An actual unmatched selector is a configuration error.

## **16.3 Group filtering**

After corpus construction, resolve focused project paths to corpus file IDs.

A group is retained if at least one occurrence uses a focused file ID.

The retained group keeps **all** occurrences.

No new `DuplicateGroup` identity is created.

## **16.4 Metrics**

For focus scans:

```text
files/source_lines/analyzed_lines
    → successful processed corpus

duplicate_groups/duplicate_lines/duplication_percent/findings
    → active groups after baseline + focus
```

This follows existing baseline-report behavior: scan metrics describe the analyzed corpus while duplicate metrics describe the enforced/reported group set.

---

# **17. Finding Fingerprint v1**

Finding fingerprints are implemented in a dedicated `fingerprint.rs` module.

They are not aliases for baseline-v1 fingerprints.

## **17.1 Algorithm**

Hash:

```text
SHA-256
```

Domain separator bytes:

```text
b"arid-finding-v1\0"
```

Canonical payload for one `DuplicateGroup`:

```text
domain separator
u32 effective_lines, big-endian
u32 normalized_len, big-endian
for each normalized line in the canonical first occurrence:
    u32 byte_length, big-endian
    exact normalized UTF-8 bytes
```

The canonical first occurrence is sufficient because every occurrence in a valid group has identical normalized content.

The occurrence's path or source line is never hashed.

## **17.2 Serialized representation**

The public string is:

```text
arid-finding-v1:sha256:<64 lowercase hex characters>
```

Example shape:

```text
arid-finding-v1:sha256:0123...abcd
```

The prefix makes both identity version and hash algorithm externally visible.

## **17.3 Baseline separation**

Baseline v1 retains its existing domain and encoding exactly.

Shared internal helpers MAY encode normalized line lengths/content, but no refactor may change baseline-v1 output bytes for the same duplicate block.

Golden-vector tests MUST lock both fingerprint contracts before v2 prerelease publication.

## **17.4 Report ordering**

The new fingerprint is data, not an ordering key.

Existing canonical finding ordering remains unless a tie-break defect requires an explicit documented correction.

---

# **18. Report v4 Model**

`report.rs` remains the canonical in-memory report model consumed by all concrete renderers.

Conceptually:

```rust
struct Report {
    schema_version: u8,
    tool_version: &'static str,
    complete: bool,
    analysis: AnalysisMetadata,
    errors: Vec<OperationalError>,
    files: u64,
    source_lines: u64,
    analyzed_lines: u64,
    duplicate_groups: u64,
    duplicate_lines: u64,
    duplication_percent: f64,
    findings: Vec<Finding>,
}
```

`schema_version` is always `4`.

`tool_version` uses `env!("CARGO_PKG_VERSION")`.

`errors` is empty for a complete report.

## **18.1 Analysis metadata**

Conceptually:

```rust
struct AnalysisMetadata {
    min_lines: u32,
    ignore_comments: bool,
    ignore_docstrings: bool,
    ignore_imports: bool,
    ignore_signatures: bool,
    same_file: bool,
    hidden: bool,
    exclude: Vec<String>,
    baseline_enabled: bool,
    focus: Vec<String>,
    virtual_source: Option<String>,
    keep_going: bool,
}
```

`exclude` preserves effective pattern order.

`focus` is canonicalized, deduplicated, and sorted because selector order has no semantic meaning.

`virtual_source` is the canonical project-relative stdin path when present.

Do not include:

- worker count
- available CPUs
- current working directory
- absolute project root
- temp paths
- timestamps
- process IDs
- report destination paths
- color capability
- `--no-fail-on-findings`

Those values either do not affect finding semantics or would introduce environment volatility.

## **18.2 Complete vs partial metrics**

For a complete scan, metrics retain their normal meaning.

For an incomplete keep-going scan, `files`, `source_lines`, and `analyzed_lines` describe successfully prepared source only.

The report explicitly carries `complete: false` and structured errors so these metrics cannot be mistaken for a complete-project measurement.

## **18.3 Finding model**

`Finding` adds:

```rust
fingerprint: String
```

and changes only occurrence distribution vocabulary:

```rust
enum FindingDistribution {
    SameFile,
    CrossFile,
    Hybrid,
}
```

Structural context/scope retain `Mixed` where already defined.

## **18.4 JSON schema**

Add:

```text
schemas/report-v4.schema.json
```

Draft 2020-12, `additionalProperties: false` for stable objects, exact required fields, and reusable error definition consistent with `error-v1` semantics.

`report-v3.schema.json` remains byte-for-byte unchanged.

---

# **19. Text and Markdown for Incomplete Reports**

A keep-going partial report must be visually unmistakable.

Text SHOULD begin with a concise incomplete-analysis notice before findings, for example:

```text
Analysis incomplete: 2 source files could not be processed.
```

and include deterministic error summaries.

Markdown SHOULD include a prominent heading/callout-equivalent section before findings.

Neither format should relabel partial metrics as complete project metrics.

Finding rendering otherwise remains the same except `hybrid` distribution wording.

Normal text output should not print finding fingerprints by default.

Supplemental text files are always plain/non-ANSI even when stdout color is enabled.

---

# **20. SARIF v2 Mapping**

Arid continues to emit SARIF 2.1.0 through concrete Serde-owned structures in `sarif.rs`.

The existing mapping remains, with distribution `hybrid` and stable fingerprint additions.

## **20.1 Partial fingerprints**

Each SARIF result SHOULD emit:

```json
"partialFingerprints": {
  "primaryLocationLineHash": "arid-finding-v1:sha256:...",
  "aridFindingFingerprint/v1": "arid-finding-v1:sha256:..."
}
```

The Arid-specific key preserves explicit identity meaning for general SARIF consumers.

`primaryLocationLineHash` carries the same Arid logical identity so GitHub code scanning can correlate results using the partial-fingerprint key it recognizes.

The value remains independent of whichever occurrence happens to be selected as the primary SARIF location.

## **20.2 Primary/related locations**

V2 retains the existing model:

```text
canonical first occurrence → locations[0]
remaining occurrences      → relatedLocations
```

Finding fingerprint identity does not depend on that presentation choice.

## **20.3 Incomplete scans**

Arid MUST NOT emit SARIF for an incomplete keep-going report.

Reason: uploading a partial result set to a code-scanning service can incorrectly imply that absent findings were resolved.

If SARIF is the primary format and the scan becomes incomplete:

```text
stdout = empty
stderr = human incomplete-scan diagnostic
exit = 2
```

If SARIF is a supplemental report target, that target is not written; JSON/text/Markdown supplemental outputs MAY still be written from the partial report.

The official GitHub Action also skips SARIF upload whenever report `complete` is false.

---

# **21. Output Rendering and Multiple Report Files**

V2 keeps concrete renderers and adds a small dispatch/write layer.

No `Reporter` trait or runtime registry is required.

## **21.1 CLI target parsing**

Parse each:

```text
--report <FORMAT=PATH>
```

into:

```rust
struct ReportTarget {
    format: OutputFormat,
    path: PathBuf,
}
```

Supported formats remain:

```text
text
json
markdown
sarif
```

The same format MAY be requested for multiple different paths.

The same destination path MUST NOT be requested more than once.

## **21.2 Destination safety**

Before writing supplemental reports, reject destinations that resolve to:

- the active baseline file
- an explicitly selected baseline-administration file
- any discovered disk Python source input

This prevents accidental destructive self-overwrite.

The implementation does not create missing parent directories automatically. A missing destination parent is an `output` error rather than an implicit filesystem-layout mutation.

## **21.3 Render once per requested representation, detect once total**

The sequence is:

```text
build one Report
    ↓
render primary stdout representation
    ↓
render requested supplemental representations
    ↓
write supplemental files
```

No supplemental target reruns discovery, parsing, normalization, corpus construction, detection, baseline comparison, or focus filtering.

## **21.4 Text color**

`--color` controls primary stdout text only.

Supplemental text report files are rendered with color disabled so saved artifacts remain deterministic and free of terminal escapes.

`--show-source` applies consistently to the report model used by every renderer.

## **21.5 File writes**

Use the existing atomic-write dependency for each supplemental file.

All requested outputs are rendered in memory before mutations begin so serialization failures occur before any report file is replaced.

Each file replacement is atomic individually. V2 does not implement a cross-file transactional filesystem protocol.

Any report write failure returns exit `2`.

---

# **22. Exit Status Resolution**

Exit policy remains outside the report domain.

Conceptually:

```text
fatal/incomplete error
    → 2

complete + findings + normal enforcement
    → 1

complete + findings + --no-fail-on-findings
    → 0

complete + no findings
    → 0
```

`Report::has_findings()` remains a pure fact.

A small application policy function maps facts + invocation flags to `ExitStatus`.

`--no-fail-on-findings` never changes:

- report content
- metrics
- baseline decisions
- focus behavior
- structured errors

and never maps error `2` to success.

Baseline administrative modes retain their separately specified `0/1/2` meanings.

---

# **23. Configuration Introspection**

`--show-config` reuses the same project/configuration resolver as normal scans.

It does not create a second configuration parser.

Internal view conceptually contains:

```text
project_root
config_path or no-config state
effective Settings
resolved baseline path/state
```

Text rendering favors readable paths.

JSON includes an administrative `schema_version: 1` field so consumers can detect future incompatible shape changes even though v2 does not require a separately published schema file for this view.

Because introspection intentionally describes a concrete environment, absolute project/config paths MAY appear here. This is different from portable report-v4 analysis metadata.

`--show-config` exits `0` after successful resolution and does not discover or analyze source.

---

# **24. File Discovery Introspection**

`--list-files` invokes the same disk discovery code used by normal scans.

It does not duplicate ignore logic.

Pipeline:

```text
resolve project/config
    ↓
discover disk Python files
    ↓
canonical deterministic ordering
    ↓
render list
```

It does not:

- read file contents
- parse Python
- normalize
- build corpus
- detect duplicates

Text emits one path per line.

JSON conceptually emits:

```json
{
  "schema_version": 1,
  "files": [
    "src/a.py",
    "src/b.py"
  ]
}
```

Paths are project-relative when representable, matching normal report path conventions.

---

# **25. Capabilities v1**

`--capabilities` is a compile/build capability query and must not inspect the current repository.

Add a typed document:

```rust
struct Capabilities {
    schema_version: u8,
    tool_version: &'static str,
    report_schema_versions: Vec<u8>,
    baseline_schema_versions: Vec<u8>,
    error_schema_versions: Vec<u8>,
    finding_fingerprint_versions: Vec<u8>,
    formats: Vec<&'static str>,
    features: Vec<&'static str>,
}
```

Initial values conceptually include:

```text
report_schema_versions: [4]
baseline_schema_versions: [1]
error_schema_versions: [1]
finding_fingerprint_versions: [1]
formats: [json, markdown, sarif, text]
features:
    baseline-prune
    baseline-status
    focus
    keep-going
    multi-report
    no-fail-on-findings
    stdin-path
    workers-auto
```

String arrays are sorted lexically before serialization.

Feature names are additive capability tokens. Adding a new feature token later does not require changing the meaning of an existing token.

Schema:

```text
schemas/capabilities-v1.schema.json
```

`--capabilities` always exits `0` unless serialization itself violates an internal invariant.

---

# **26. CLI Additions**

`cli.rs` adds fields conceptually equivalent to:

```rust
focus: Vec<PathBuf>,
keep_going: bool,
no_fail_on_findings: bool,
config: Option<PathBuf>,
no_config: bool,
project_root: Option<PathBuf>,
show_config: bool,
list_files: bool,
stdin_path: Option<PathBuf>,
report: Vec<ReportTarget>,
baseline_status: Option<PathBuf>,
prune_baseline: Option<PathBuf>,
capabilities: bool,
```

Existing options remain.

## **26.1 Mode validation**

Do not create a massive web of brittle Clap conflicts for every administrative combination.

Use:

1. Clap for local syntax/value validation and obvious pairwise conflicts
2. `resolve_mode` for mutually exclusive operation selection
3. `validate_mode_options` for semantic compatibility

Errors must name the conflicting options clearly.

## **26.2 Raw CLI vs resolved state**

CLI types remain parse-oriented.

Do not stuff resolved project roots, effective config, normalized focus selectors, or discovered files back into `Cli`.

Those belong to application/context types after parsing.

---

# **27. Official GitHub Action**

Arid 2.0 ships a composite GitHub Action in the **same repository**.

Root file:

```text
action.yml
```

Helper:

```text
action/run.py
```

Keeping the metadata file at repository root allows normal external use:

```yaml
- uses: sponge-b0b/arid@v2.0.0
```

and keeps action source versioned with the exact Arid release tag it invokes.

## **27.1 Why composite + Python helper**

A composite action is sufficient because Arid remains the real executable.

A small Python helper is justified for:

- cross-platform argument construction
- exact subprocess exit-code capture without prematurely terminating the action
- report-v4 JSON parsing
- GitHub output-file writing
- avoiding fragile shell parsing across Linux/macOS/Windows

It does not implement detection or reporting semantics.

## **27.2 Installation**

The composite action SHOULD use a pinned `actions/setup-python` revision to establish a predictable Python runtime, then install the exact requested PyPI Arid version.

`action.yml` exposes:

```text
version
```

whose default is release-managed metadata corresponding to the action's source release.

Examples:

```text
Git tag v2.0.0-rc.1 → PyPI default 2.0.0rc1
Git tag v2.0.0      → PyPI default 2.0.0
```

Callers MAY override `version` deliberately.

Normal action use does not compile Rust and does not install project dependencies.

## **27.3 Inputs**

Initial action inputs SHOULD remain small:

```text
version           exact Arid package version; release-managed default
paths             newline-separated scan paths; default "."
focus             optional newline-separated focus paths
arguments         optional additional shell-style Arid arguments
fail-on-findings  boolean; default true
sarif             boolean; default false
job-summary       boolean; default true
```

`arguments` is parsed by `action/run.py` using explicit documented shell-style tokenization rather than concatenated directly into a command string.

Focus receives a dedicated input because it is especially useful for PR workflows and avoids forcing callers to quote repeated flags manually.

## **27.4 One action scan**

The helper creates temporary paths and invokes Arid once, adding an internal supplemental JSON target:

```text
--report json=<temp-report>
```

When requested it also adds:

```text
--report markdown=<temp-summary>
--report sarif=<temp-sarif>
```

User-selected primary stdout format remains independent.

The helper captures Arid's original exit code instead of letting the scan step immediately fail, so outputs/summary/SARIF decisions can be completed first.

## **27.5 Outputs**

Parse the supplemental report-v4 JSON and expose at minimum:

```text
has-findings
duplicate-groups
duplicate-lines
duplication-percent
files
complete
scan-exit-code
```

These values describe the same active report set Arid produced after baseline/focus filtering.

No action-side metric recomputation is allowed.

## **27.6 Job summary**

When enabled and a Markdown report exists, append it to `GITHUB_STEP_SUMMARY`.

Do not reformat findings in the action helper.

## **27.7 SARIF upload**

When enabled:

1. request one supplemental SARIF file from the same Arid scan
2. upload it through a pinned `github/codeql-action/upload-sarif` action
3. require the caller workflow to grant the permissions GitHub requires
4. skip upload when report `complete` is false

The action does not manufacture severity or fingerprints; it uploads Arid's SARIF.

## **27.8 Final action status**

After outputs/integrations are processed:

```text
Arid scan exit 2
    → action fails

complete findings + fail-on-findings=true
    → action fails

complete findings + fail-on-findings=false
    → action succeeds

complete no findings
    → action succeeds
```

The action's `fail-on-findings` input is integration policy and does not modify the Arid report.

## **27.9 No second action repository required**

V2 does not require `arid-action` as a separate repository.

A separate repository would add release synchronization and dependency-management work without improving detector functionality.

---

# **28. GitHub Action Validation**

Action validation occurs at two levels.

## **28.1 Source-level helper tests**

Before publication, tests validate:

- argument construction
- newline path/focus handling
- shell-style additional arguments
- report-v4 metric extraction
- exit-policy mapping
- incomplete-report SARIF suppression
- GitHub output encoding

These tests do not require a published v2 package.

## **28.2 Published end-to-end verification**

Because the official action intentionally installs the released PyPI artifact, exact end-to-end action validation belongs after a prerelease is published.

The production release workflow adds a post-publication job conceptually named:

```text
Verify published GitHub Action
```

It runs after PyPI/GitHub publication, waits for exact PyPI availability using the existing readiness helper, checks out the exact tag, invokes the root action against a deterministic fixture, and verifies:

- exact Arid version installed
- action completes
- expected finding outcome
- expected metric outputs
- focus behavior when exercised
- job-summary path when practical

SARIF upload itself MAY remain separately workflow-tested to avoid requiring code-scanning permissions in every release verification path.

Release qualification verifies that the expected published-action verification job succeeded.

---

# **29. Performance Architecture**

V2 does not introduce a new performance subsystem.

The v1.2 evidence remains the starting point:

- source preparation dominates
- Ruff parsing dominates preparation
- post-parse Arid analysis is relatively inexpensive
- bounded auto workers capture useful parallelism
- default automatic parallelism was not justified

Therefore:

- serial remains default
- `auto` remains bounded as established unless new benchmarks justify change
- no persistent cache is added
- no parser replacement is assumed
- focus does not reduce detection scope and is not marketed as a speed optimization
- multi-output explicitly avoids repeated scans
- baseline status/pruning reuses one current detection
- official Action uses one scan for console, metrics, summary, and SARIF

## **29.1 Current Pylint comparison**

The pinned historical benchmark remains the regression gate.

Before final RC, one performance phase resolves the then-current stable Pylint release, records the exact version, and runs the established comparison methodology.

That exact competitor version is then frozen in the v2 performance evidence for RC/stable qualification rather than re-resolving a moving latest version during stable promotion.

This gives both:

```text
pinned historical reproducibility
+
current competitive relevance
```

without making qualification nondeterministic.

---

# **30. Schema Lifecycle**

V2 ships three Arid-owned current machine schemas plus one preserved historical report schema:

```text
schemas/report-v4.schema.json
schemas/error-v1.schema.json
schemas/capabilities-v1.schema.json
schemas/baseline-v1.schema.json
schemas/report-v3.schema.json   # historical, unchanged
```

All new schemas use JSON Schema Draft 2020-12.

Schemas are validated with pinned development/validation tooling, not a runtime Arid dependency.

Once published:

- report-v4 is immutable for incompatible changes
- error-v1 is immutable for incompatible changes
- capabilities-v1 is immutable for incompatible changes
- baseline-v1 remains its existing immutable contract
- report-v3 remains historical and untouched

Additive capability tokens do not require a capability schema version bump when the schema permits additional valid tokens.

---

# **31. Migration Architecture**

The v2 migration guide is generated from explicit contract changes, not from commit history.

Required before/after material includes:

```json
// v1.x
{
  "version": 3,
  "findings": [
    {
      "distribution": "mixed"
    }
  ]
}
```

```json
// v2
{
  "schema_version": 4,
  "tool_version": "2.0.0",
  "complete": true,
  "analysis": { "...": "..." },
  "errors": [],
  "findings": [
    {
      "fingerprint": "arid-finding-v1:sha256:...",
      "distribution": "hybrid"
    }
  ]
}
```

The guide explicitly states that users relying only on normal CLI text behavior and existing `[tool.arid]`/baseline-v1 configuration do not need a migration step.

Rust API consumers receive a separate before/after public-surface table.

---

# **32. Release Tooling Delta**

`release.sh` must recognize:

```text
2.0.* → docs/arid-v2-release-roadmap.md
```

For v2 versions, release-managed metadata additionally includes the default PyPI package version in root `action.yml`.

The action default uses the already-derived `PYPI` value, not the Git tag spelling.

Conceptually the v2 managed release files become:

```text
Cargo.toml
Cargo.lock
pyproject.toml
README.md
action.yml
docs/arid-v2-release-roadmap.md
```

`action.yml` is release metadata only for versions whose action is part of the release contract; existing 1.x `release.sh --check` behavior must not be retroactively broken by the new file.

The RC-to-stable metadata-only allowlist in qualification must be extended accordingly for v2.

Curated release-note files are **not** generated by `release.sh` and are not stable-promotion metadata. They must already exist per the release-notes policy.

---

# **33. Production Release Workflow Delta**

The existing production workflow retains:

- release metadata verification
- platform wheel/standalone builds
- PyPI publication
- GitHub Release publication using curated notes
- Linux ARM64 post-publication verification

V2 additionally verifies:

- report-v4/error-v1/capabilities-v1 schemas exist in tagged source
- root `action.yml` release version matches exact PyPI version
- published GitHub Action end-to-end behavior succeeds

The action verification job runs only on release tags after publication dependencies succeed.

A production release is not considered fully successful until required post-publication verification jobs pass.

---

# **34. Targeted V2 Validation**

Add/extend:

```text
validation/v2.sh
```

It composes existing v1/v1.1/v1.2 validation instead of copying the entire campaign.

Targeted areas:

## **34.1 Report contract**

```text
schema_version == 4
no top-level version
tool_version exact
complete true/false behavior
analysis metadata exact
errors deterministic
report-v4 schema validation
report-v3 historical checksum unchanged
```

## **34.2 Distribution vocabulary**

```text
hybrid in JSON
hybrid in text
hybrid in Markdown
hybrid in SARIF
mixed retained for structural context
mixed retained for structural scope
```

## **34.3 Finding identity**

```text
golden fingerprint vectors
line movement invariant
path movement invariant
occurrence multiplicity invariant
source snippet invariant
worker-mode invariant
normalized-content change changes fingerprint
baseline-v1 fingerprint golden vectors unchanged
```

## **34.4 Focus**

```text
whole corpus still participates
matching group retains out-of-focus occurrences
file focus
directory focus
multiple selectors
unmatched selector error
baseline then focus ordering
Unicode/space paths
virtual-source focus
```

## **34.5 Keep-going/errors**

```text
read failure collection
parse failure collection
deterministic error order
complete false
exit 2
no-fail cannot mask 2
fatal configuration remains immediate
error-v1 schema validation
primary JSON fatal-error document
```

## **34.6 Baseline maintenance**

```text
accepted status
active status
stale status
new path
increased multiplicity
reduced multiplicity
missing group
prune removes stale only
prune never adds/increases acceptance
atomic rewrite
no-op prune avoids rewrite
existing baseline-v1 files remain compatible
```

## **34.7 Configuration/introspection**

```text
legacy ancestor discovery
exact --config
--no-config
explicit --project-root
contradictory config/root rejected
show-config precedence
list-files matches normal discovery
```

## **34.8 Virtual source**

```text
new virtual file
replace disk file
never writes disk
normalization/detection equivalence
baseline enforcement
focus matching
configured exclude
invalid syntax fail-fast
invalid syntax keep-going
```

## **34.9 Multi-output**

```text
one scan logical equivalence
JSON supplemental equals primary JSON semantics
Markdown/SARIF/text equivalence
plain supplemental text
same destination rejected
source/baseline destination rejected
atomic per-file write
write failure exit 2
incomplete SARIF suppressed
```

## **34.10 Capabilities**

```text
schema validation
stable ordering
no config/discovery access
expected feature tokens
exact tool version
```

## **34.11 Public Rust boundary**

Compile-level tests/documentation ensure only the intended root-level API is supported.

Do not preserve old public module visibility merely to keep 1.x Rust imports compiling; that break is intentional in 2.0.

---

# **35. Real-World Validation**

The established real-world corpus campaign remains mandatory.

V2 must demonstrate that the contract/automation work did not change duplicate identity on the established repositories.

For equivalent 1.2 settings, detector-level canonical groups before v2 post-detection transformations should match the qualified 1.2 expectation.

V2-specific real-world exercises SHOULD additionally include:

- focus on representative changed directories/files
- baseline enforcement with focus
- one controlled malformed file under keep-going
- multi-output on a large corpus

The normal campaign remains fail-fast by default.

---

# **36. Qualification Delta**

`qualification/run.sh` retains existing published-artifact qualification and adds v2 evidence for:

```text
report-v4 schema
error-v1 schema
capabilities-v1 schema
finding fingerprint
baseline-v1 compatibility
focus
keep-going
multi-output
virtual stdin
baseline status/prune
explicit config/root behavior
published GitHub Action verification
v2 migration/release-note presence
```

The exact published x86_64 artifact remains the local full qualification target.

Native Linux aarch64 verification remains in GitHub Actions.

Stable qualification still requires a fully qualified latest RC and an exact allowed metadata-only RC-to-stable delta.

For v2 that allowed release metadata includes `action.yml` because its default exact PyPI version changes from RC to stable.

---

# **37. Implementation Order**

Implementation SHOULD proceed bottom-up so each layer becomes correct before downstream consumers are repaired.

Recommended order:

```text
1.  repository/release scaffolding for v2
2.  operational-error types and process outcome model
3.  canonical project-path helpers
4.  finding-fingerprint v1 + golden vectors
5.  baseline-v1 fingerprint regression lock
6.  report domain v4 + hybrid distribution
7.  report-v4/error-v1 schemas
8.  SARIF fingerprint + hybrid changes
9.  baseline comparison engine
10. baseline status + prune operations
11. explicit project/config resolver
12. source-input abstraction + virtual stdin
13. keep-going preparation
14. focus selector/filter
15. self-describing analysis metadata final wiring
16. exit-status control
17. supplemental multi-output writing
18. show-config + list-files
19. capabilities-v1 + schema
20. narrow/re-export Rust public API
21. extract orchestration into app.rs / slim lib.rs
22. official GitHub Action source + helper tests
23. targeted v2 integration validation
24. real-world validation
25. performance/regression + current-Pylint comparison
26. migration guide + README reconciliation
27. release roadmap/release-note freeze
28. prerelease publication + published Action verification
29. RC qualification
```

This ordering intentionally moves report-domain breakage before renderer/CLI repair and moves shared baseline/source primitives before higher-level automation.

Do not preserve an incorrect intermediate public contract solely to keep downstream code compiling. Downstream consumers should be repaired after the owning layer is made correct.

---

# **38. Explicitly Rejected V2 Architecture**

V2 does not introduce:

- a generic reporter trait/registry
- a plugin framework
- an event bus
- a dependency-injection container
- a daemon
- an MCP server
- a language abstraction layer
- a generalized policy engine
- a Git abstraction layer
- a persistent cache subsystem
- a second detection engine
- a special agent-only detector
- a task scheduler around workers
- a cross-file transaction manager for report writes
- a new baseline schema merely to support maintenance

The new capabilities are implemented through small domain modules around the existing pipeline.

---

# **39. Final V2 Architecture**

The resulting architecture is:

```text
                         Cli
                          │
                          ▼
                    mode resolution
                          │
             ┌────────────┼─────────────┐
             │            │             │
     capabilities   config/list     scan/admin
                                      │
                                      ▼
                              ProjectContext
                                      │
                                      ▼
                               disk discovery
                                      │
                                      ▼
                           optional virtual merge
                                      │
                                      ▼
                            SourceInput[] sorted
                                      │
                                      ▼
                         prepare (serial/parallel)
                                      │
                       ┌──────────────┴──────────────┐
                       │                             │
                 PreparedFile[]              OperationalError[]
                       │                             │
                       └──────────────┬──────────────┘
                                      ▼
                                   Corpus
                                      │
                                      ▼
                              detect_duplicates
                                      │
                                      ▼
                              DuplicateGroup[]
                                      │
                    ┌─────────────────┴──────────────────┐
                    │                                    │
             baseline admin                     optional enforcement
                    │                                    │
          status / safe prune                    active groups
                                                         │
                                                         ▼
                                                  optional focus
                                                         │
                                                         ▼
                                                   reportable groups
                                                         │
                                                         ▼
                                                    Report v4
                                       ┌─────────────────┼─────────────────┐
                                       │                 │                 │
                                      text              JSON            Markdown
                                       │                                   │
                                       └─────────────────┬─────────────────┘
                                                         │
                                                      SARIF*

* SARIF only when the scan is complete.
```

The official GitHub Action is an external orchestration layer around the same released CLI:

```text
GitHub Action
    ↓
install exact released Arid
    ↓
one Arid scan
    ↓
report-v4 metrics + optional Markdown/SARIF
    ↓
GitHub outputs / summary / code scanning
```

No duplicate-detection logic exists in the action.

---

# **40. Completion Criterion**

The v2 architecture is complete when the implementation can truthfully state:

> **Arid still performs one deterministic exact-duplicate scan, but the scan now produces a self-describing v4 contract with stable finding identity, can safely expose partial failures, can focus whole-project results without narrowing detection, can inspect and maintain baseline state without accepting new debt, can evaluate unsaved virtual source, can fan one report into multiple outputs and GitHub integrations, and exposes only a deliberately small Rust API—without changing the detector definition or baseline-v1 compatibility.**
