# **Arid v2.1 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.1.0`  
**Requirements:** `docs/arid-v2.1-requirements-specification.md`

---

# **1. Purpose**

This document defines the technical architecture for Arid 2.1.

The v2 architecture remains the implementation base. V2.1 adds suppression maintenance and discovery auditability around the stable 2.0 detector without redesigning duplicate identity, report v4, baseline v1, or the normal scan pipeline.

The v2.1 architecture exists to implement:

- suppression status through `--suppression-status`
- active/stale suppression classification
- formally documented idempotent suppression directives
- generic stale-policy enforcement through `--fail-on-stale`
- stale-policy enforcement for existing `--baseline-status`
- targeted path explanation through `--explain-path <PATH>`
- ignore-file traversal override through `--no-ignore-files`
- `suppression-status-v1`
- `path-explanation-v1`
- deterministic administrative JSON output and direct-file equivalence

The detector itself is not redesigned.

---

# **2. Architectural Principles**

## **2.1 One authoritative detector**

V2.1 MUST continue to use the existing exact duplicate detector.

Suppression auditing creates an alternate **preparation view**, not an alternate detector.

Conceptually:

```text
normal scan
    source preparation with suppression applied
        ↓
    existing corpus
        ↓
    existing detector

suppression audit
    source preparation with suppression ignored for matching
        ↓
    existing corpus
        ↓
    existing detector
        ↓
    map otherwise-reportable findings to suppression regions
```

The detector receives prepared source in both cases and does not know whether the source came from a normal or suppression-audit preparation view.

## **2.2 One audit detector pass**

Suppression status MUST NOT run the detector once per suppression region.

All effective suppression regions are exposed in one suppression-disabled audit view and the existing detector runs once over that view.

This is required both for efficiency and correctness when multiple suppressed regions duplicate one another.

## **2.3 Suppression directives are state, not nesting**

`# arid: disable` and `# arid: enable` are idempotent state setters.

The implementation MUST NOT introduce pairing stacks, nesting depth, unmatched-directive errors, or balancing rules.

## **2.4 Discovery has one policy definition**

Normal traversal, `--list-files`, `--explain-path`, and `--no-ignore-files` MUST derive decisions from the same discovery configuration and matching primitives.

Path explanation is not permitted to become a hand-written approximation of normal discovery.

## **2.5 Administrative models remain separate**

The new maintenance and discovery results are not report-v4 findings.

```text
report-v4
    duplicate scan report

suppression-status-v1
    suppression maintenance state

path-explanation-v1
    discovery decision
```

## **2.6 Determinism is explicit**

All public administrative collections, paths, classifications, reasons, and summary counts have explicit deterministic ordering.

Filesystem traversal arrival order, worker completion order, and unordered map iteration MUST NOT affect serialized output.

## **2.7 Ordinary scans stay cheap**

A normal invocation such as:

```bash
arid .
```

MUST NOT run suppression-audit detection, path explanation, or extra discovery matching merely because v2.1 supports those operations.

## **2.8 Mutation remains narrow**

The new v2.1 administrative operations are read-only.

Existing baseline mutation remains limited to:

```text
--write-baseline
--prune-baseline
```

`--fail-on-stale` is exit policy only.

---

# **3. Major Technical Decisions**

| Area | V2.1 decision |
| --- | --- |
| Normal detection semantics | unchanged from 2.0 |
| Suppression semantics | idempotent enabled/disabled state machine |
| EOF while disabled | valid implicit region termination |
| Repeated same-state directive | valid no-op |
| Suppression audit | one suppression-disabled preparation view + one existing detector pass |
| Suppression audit identity | effective regions only; no-op directives are not audit records |
| Suppression status | `active` or `stale` only |
| Stale enforcement | centralized exit-policy helper used by suppression and baseline status |
| Discovery implementation | continue using `ignore::WalkBuilder` |
| Ignore-file override | disable ignore-file filters individually; do not disable hidden filtering |
| Targeted ignore evaluation | use the same configured `WalkBuilder` matching semantics rather than reimplement ignore precedence |
| Arid excludes | remain separate from ignore-file filtering |
| Path explanation | one target only; no recursive explanation trace |
| Administrative schemas | `suppression-status-v1` and `path-explanation-v1` |
| Schema version field | numeric `schema_version: 1` |
| Administrative file output | reuse `--report json=PATH` only for the two new administrative models |
| File writes | same atomic output primitive used by existing report files |
| Capabilities v1 | unchanged in 2.1; no mutation of the published schema |
| Report v4 | unchanged |
| Baseline v1 | unchanged |
| Error v1 | reused; no new error taxonomy |
| Rust public API | unchanged from 2.0 unless implementation requires a private refactor |

---

# **4. Intended Repository Delta**

V2.1 SHOULD require only a small source delta:

```text
src/
├── app.rs                  # extend invocation orchestration
├── cli.rs                  # new CLI flags / validation inputs
├── exit_policy.rs          # generic stale-policy helper
├── files.rs                # discovery policy + targeted explanation support
├── normalize.rs            # suppression-aware preparation mode
├── output.rs               # reusable atomic administrative JSON target write
├── python.rs               # retain directive collection; expose required location data
├── suppression.rs          # effective regions + audit model + renderers
└── ... existing modules unchanged where possible

schemas/
├── suppression-status-v1.schema.json
└── path-explanation-v1.schema.json

validation/
└── v2.1.sh                 # or equivalent focused validation additions
```

A separate `path_explanation.rs` MAY be introduced only if `files.rs` becomes materially harder to understand without it.

A separate `suppression_status.rs` MAY likewise be introduced only if rendering/model concerns make `suppression.rs` unreasonably broad.

The design does not require new framework layers.

---

# **5. Invocation Modes**

Extend the internal invocation-mode model conceptually to:

```rust
enum Mode {
    Scan,
    WriteBaseline(PathBuf),
    BaselineStatus(PathBuf),
    PruneBaseline(PathBuf),
    SuppressionStatus,
    ExplainPath(PathBuf),
    ShowConfig,
    ListFiles,
    Capabilities,
}
```

`--no-ignore-files` is not a mode. It changes discovery policy for modes that perform discovery.

`--fail-on-stale` is not a mode. It changes exit policy for `SuppressionStatus` and `BaselineStatus` only.

Mode resolution remains early in `app.rs` before expensive work.

## **5.1 New mode conflicts**

`SuppressionStatus` conflicts with other terminal administrative operations, including:

```text
--show-config
--list-files
--baseline-status
--prune-baseline
--write-baseline
--explain-path
```

`ExplainPath` is likewise terminal and conflicts with detection-oriented administrative modes.

`--fail-on-stale` requires one of:

```text
--suppression-status
--baseline-status
```

Obvious local relations SHOULD be expressed in Clap. Cross-mode semantic validation remains centralized in `app.rs` where doing so avoids a brittle attribute cross-product.

---

# **6. Suppression State Model**

The parser continues to recognize only exact comment directives:

```text
arid: disable
arid: enable
```

after the existing comment-prefix/whitespace handling.

Existing `SuppressionEvent` values remain ordered by physical source offset.

V2.1 adds a small pure state reduction over those events.

Conceptually:

```rust
enum SuppressionState {
    Enabled,
    Disabled,
}

struct SuppressionRegion {
    disable_line: u32,
    end: SuppressionEnd,
}

enum SuppressionEnd {
    Enable { line: u32 },
    Eof,
}
```

The exact internal integer type is not public.

## **6.1 Effective transitions**

Starting state is `Enabled`.

```text
Enabled  + Disable → Disabled; open region
Disabled + Disable → Disabled; no-op
Disabled + Enable  → Enabled; close region
Enabled  + Enable  → Enabled; no-op
Disabled + EOF     → close region at EOF
Enabled  + EOF     → nothing to close
```

Only the first and third rows create effective directive boundaries.

EOF closure is implicit and valid.

## **6.2 Directive-line semantics**

Existing normal-scan behavior remains authoritative: a directive takes effect after its physical source line.

Therefore an inline directive such as:

```python
do_work()  # arid: disable
```

leaves `do_work()` in the preceding enabled segment and disables matching beginning with the following physical line.

An effective `# arid: enable` line is still part of the disabled physical region for matching purposes because re-enabling takes effect after that line.

The region model MUST preserve this behavior exactly.

---

# **7. Suppression Region Location Model**

Public suppression-region location is directive-oriented rather than byte-offset-oriented.

Each region is identified by:

```text
path
start_line
end_line
termination
```

where:

- `start_line` is the 1-based physical line containing the effective `disable`
- `end_line` is the 1-based physical line containing the effective `enable`, or `null` for EOF termination
- `termination` is `enable` or `eof`

This gives users a stable location for the directive they need to inspect without exposing parser byte offsets as public API.

No-op directives do not receive public region identities.

---

# **8. Preparation Modes**

Source preparation needs one private suppression control.

Conceptually:

```rust
enum SuppressionMode {
    Apply,
    Audit,
}
```

Normal preparation uses `Apply`.

Suppression status uses `Audit`.

## **8.1 Apply mode**

`Apply` preserves the existing 2.0 normalization behavior exactly:

- directive comments remain masked
- disabled physical lines do not participate in normalized matching
- effective suppression boundaries split normalized segments

## **8.2 Audit mode**

`Audit` uses the same parser, masks, normalization rules, structural classification, and line mapping, but:

- directive comments remain masked
- suppression state does not remove source from normalized matching
- suppression boundaries do not split matching segments
- effective suppression regions are retained separately for later classification

This produces the source view Arid would analyze if its suppression directives did not hide or separate duplicate source.

## **8.3 Refactor boundary**

`prepare_file` SHOULD remain the normal public/private call used by ordinary execution.

Implementation MAY introduce a private lower-level helper that accepts `SuppressionMode` and optionally returns derived regions.

Normal callers SHOULD NOT be forced to allocate or retain audit metadata they do not need.

---

# **9. Suppression Audit Pipeline**

`--suppression-status` uses the following pipeline:

```text
project/config resolution
        ↓
discovery using effective discovery policy
        ↓
source preparation in SuppressionMode::Audit
        ↓
effective suppression regions retained by source
        ↓
build_corpus
        ↓
detect_duplicates exactly once
        ↓
map duplicate occurrences to effective regions
        ↓
active/stale region classification
        ↓
SuppressionStatus model
        ↓
text / suppression-status-v1 JSON
```

Baseline enforcement and focus filtering do not participate in this classification pipeline.

The audit requires a complete source set. It does not use `--keep-going`.

Any source-processing failure returns exit `2` because stale classification would otherwise be incomplete.

---

# **10. Active/Stale Mapping**

Every effective region begins `stale` in the audit model.

A region becomes `active` when at least one otherwise-reportable duplicate occurrence in the audit corpus depends on that region being unsuppressed.

The mapping MUST cover both:

1. duplicate source physically inside the disabled region; and
2. a duplicate occurrence that crosses a suppression boundary which would split the occurrence during a normal scan.

The implementation SHOULD map using physical source-line ranges derived from the audit corpus's existing normalized-to-source mapping rather than compare source text separately.

Conceptually, for each audit occurrence:

```text
physical start line
physical end line
```

are compared with the effective region boundaries for the same file.

A region is active when an occurrence:

- overlaps physical lines whose matching participation is disabled by the region, or
- spans across an effective region boundary that would prevent the occurrence from existing as one contiguous normal-scan segment.

This ensures an empty or normalization-empty suppression region can still be active when its boundary alone prevents an otherwise valid contiguous duplicate.

Once active, a region remains active; no per-region detector rerun is required.

## **10.1 Multiple suppressed regions**

All suppressions are ignored simultaneously in the audit preparation view.

Therefore two suppressed regions that duplicate only one another appear in the same audit detector pass and both become active.

This collective audit interpretation is intentional.

---

# **11. Suppression Audit vs Reporting Policy**

Suppression activity is determined from underlying duplicate identity before acceptance and presentation layers.

The suppression-status pipeline therefore does not apply:

```text
--baseline
configured baseline filtering
--focus
normal report filtering
```

A duplicate accepted by a baseline still demonstrates that a suppression is active.

A duplicate omitted from a focused report still demonstrates that a suppression is active.

This preserves the architecture ordering:

```text
duplicate identity
        ↓
suppression maintenance state
        ↓
separate baseline acceptance / report presentation concerns
```

---

# **12. Suppression Status Model**

Conceptually:

```rust
enum SuppressionStatusKind {
    Active,
    Stale,
}

struct SuppressionStatusRegion {
    path: String,
    start_line: u32,
    end_line: Option<u32>,
    termination: SuppressionTermination,
    status: SuppressionStatusKind,
}

struct SuppressionStatus {
    analysis: SuppressionAnalysis,
    files: u64,
    regions: Vec<SuppressionStatusRegion>,
}
```

`regions` is sorted by:

```text
path
start_line
end_line, with EOF after a numeric end for an otherwise identical key
```

The summary is derived from the sorted region set rather than maintained independently during scanning.

---

# **13. suppression-status-v1**

Add:

```text
schemas/suppression-status-v1.schema.json
```

using JSON Schema Draft 2020-12 and `additionalProperties: false` for stable objects.

The exact v1 document shape is:

```json
{
  "schema_version": 1,
  "tool_version": "2.1.0",
  "files": 42,
  "analysis": {
    "min_lines": 4,
    "ignore_comments": false,
    "ignore_docstrings": false,
    "ignore_imports": false,
    "ignore_signatures": false,
    "same_file": true,
    "hidden": false,
    "exclude": [],
    "ignore_files": true
  },
  "summary": {
    "total": 2,
    "active": 1,
    "stale": 1
  },
  "regions": [
    {
      "path": "src/example.py",
      "start_line": 20,
      "end_line": 32,
      "termination": "enable",
      "status": "active"
    },
    {
      "path": "tests/example.py",
      "start_line": 80,
      "end_line": null,
      "termination": "eof",
      "status": "stale"
    }
  ]
}
```

## **13.1 Analysis fields**

`analysis` contains only stable settings that materially affect corpus selection or suppression activity:

```text
min_lines
ignore_comments
ignore_docstrings
ignore_imports
ignore_signatures
same_file
hidden
exclude
ignore_files
```

`ignore_files` is `false` only when `--no-ignore-files` is active.

Worker count is omitted because it is execution strategy, not audit semantics.

Baseline/focus state is omitted because those layers do not define suppression activity.

## **13.2 Path representation**

Paths use the existing canonical project-relative representation when possible.

If an explicitly valid source cannot be represented relative to project root under existing compatible scan behavior, the implementation MAY use the same deterministic fallback already used by discovery introspection rather than reject an otherwise valid scan solely for suppression status.

## **13.3 Schema lifecycle**

Once released, `suppression-status-v1.schema.json` is immutable.

An incompatible future shape requires a new schema version/file.

---

# **14. Suppression Text Output**

Text output is presentation, not a parsing contract.

It SHOULD contain:

- one concise summary
- one deterministic line or block per effective region
- path and directive line
- active/stale state
- explicit EOF indication where applicable

No-op directives are not listed independently.

The renderer consumes `SuppressionStatus`; it does not recompute classification.

---

# **15. Generic Stale Exit Policy**

Extend `exit_policy.rs` with one generic administrative helper rather than duplicate exit logic in orchestration.

Conceptually:

```rust
fn apply_fail_on_stale(
    base: ExitStatus,
    has_stale: bool,
    fail_on_stale: bool,
) -> ExitStatus
```

Policy:

```text
base Error                 → Error
base Findings              → Findings
base Success + no policy   → Success
base Success + policy + stale → Findings
```

For suppression status, the base successful result is always `Success`; active suppressions do not represent failure.

For baseline status, the existing active/new-debt result is first calculated exactly as in 2.0 and stale policy is then layered on top.

No normal scan calls this helper.

---

# **16. Baseline Status Integration**

The baseline comparison engine remains unchanged.

Existing `BaselineStatus` already knows whether active/new and stale acceptance exist.

V2.1 only changes final policy selection:

```text
comparison.status.has_active()
        ↓
existing base exit
        ↓
comparison.status.has_stale()
        ↓
apply_fail_on_stale(...)
```

No baseline-v1 bytes, fingerprints, comparison arithmetic, pruning rules, or status JSON meaning are changed.

`--fail-on-stale` never invokes pruning automatically.

---

# **17. Discovery Policy Object**

V2.1 SHOULD make the current traversal choices explicit in one small private policy value.

Conceptually:

```rust
struct DiscoveryPolicy<'a> {
    settings: &'a Settings,
    project_root: &'a Path,
    ignore_files: bool,
}
```

`ignore_files` defaults to `true` and becomes `false` with `--no-ignore-files`.

The policy is passed to normal discovery and targeted explanation rather than threading a bare boolean through unrelated functions.

Arid configured/CLI excludes continue to use the existing `GitignoreBuilder` matcher rooted at project root.

---

# **18. WalkBuilder Construction**

Directory traversal SHOULD be constructed through one helper so normal discovery and targeted ignore matching use identical `ignore` crate configuration.

Conceptually:

```rust
fn configured_walk_builder(
    root: &Path,
    include_hidden: bool,
    ignore_files: bool,
) -> WalkBuilder
```

Always preserve:

```text
follow_links(false)
hidden(!include_hidden)
```

When `ignore_files == true`, retain the `ignore` crate's normal ignore-file behavior.

When `ignore_files == false`, disable only ignore-file-derived filters:

```rust
builder
    .parents(false)
    .ignore(false)
    .git_ignore(false)
    .git_global(false)
    .git_exclude(false);
```

Do **not** call:

```rust
standard_filters(false)
```

because that would also disable hidden-path filtering and broaden the meaning of `--no-ignore-files` beyond the requirements.

Under the current `ignore` crate behavior, normal traversal may read `.ignore`, `.gitignore`, `.git/info/exclude`, global Git ignore rules, and applicable parent ignore files. V2.1's override disables those ignore-file sources while leaving Arid's own exclude matcher separate.

If Arid later adds another ignore-file source to normal traversal, `--no-ignore-files` must disable that source too unless its contract explicitly says otherwise.

---

# **19. --no-ignore-files Discovery Flow**

Normal directory discovery becomes conceptually:

```text
for each explicit input path
    ↓
metadata / symlink handling
    ↓
explicit file?
    ├─ yes → existing explicit-file rules
    └─ no  → configured WalkBuilder
                  ↓
             ignore-file rules (optional)
                  ↓
             hidden policy
                  ↓
             Arid exclude filter_entry
                  ↓
             .py/.pyi selection
```

Explicit files continue to bypass normal ignore-file traversal rules.

They also retain current explicit-file behavior for hidden paths; v2.1 does not retroactively force directory-only hidden filtering onto explicitly named files.

Configured Arid excludes remain authoritative for explicit files and directory discovery.

---

# **20. Targeted Path Explanation**

`--explain-path <PATH>` performs:

```text
project/config resolution
        ↓
normalize scan roots and target
        ↓
target metadata / symlink classification
        ↓
determine explicit-input vs descendant context
        ↓
apply same Arid exclude policy
        ↓
apply same configured ignore/hidden matching semantics where traversal applies
        ↓
apply Python source-type or directory-traversal rule
        ↓
PathExplanation model
```

It does not parse Python source or run duplicate detection.

## **20.1 Target must exist**

The target is inspected with filesystem metadata.

A missing or unreadable target is a discovery error and exits `2`.

## **20.2 Explicit context**

A target equal to an explicitly supplied file path uses explicit-file semantics.

A target equal to an explicitly supplied directory root uses root traversal semantics.

A target below an explicitly supplied directory is evaluated as a traversal descendant.

A target outside all effective scan roots is excluded with reason `outside-scan-roots`.

## **20.3 Ignore matching**

For a traversal descendant, targeted ignore evaluation SHOULD use an `IncrementalIgnore` matcher built from the same configured `WalkBuilder` used for normal traversal.

This avoids reimplementing `.ignore` / Git ignore precedence and parent-file handling.

The matcher is appropriate because explanation checks one target, not an entire tree.

Arid `exclude` remains a separate check because it is implemented separately from the walker's ignore-file rules in normal discovery.

---

# **21. Path Explanation Decision Model**

Conceptually:

```rust
enum PathKind {
    File,
    Directory,
}

enum PathDecision {
    Include,
    Exclude,
}

enum PathReason {
    ExplicitFile,
    ExplicitDirectory,
    Discovered,
    OutsideScanRoots,
    AridExclude,
    IgnoreFile,
    Hidden,
    UnsupportedSourceType,
    SymlinkDirectory,
    SymlinkTraversal,
}

struct PathExplanation {
    path: String,
    kind: PathKind,
    symlink: bool,
    explicit: bool,
    decision: PathDecision,
    reasons: Vec<PathReason>,
}
```

The final implementation MAY collapse the two symlink reasons if one stable value describes all excluded symlink cases without ambiguity.

## **21.1 Stable reason ordering**

When more than one exclusion fact applies, `reasons` is ordered by a fixed explanatory priority:

```text
outside scan roots
symlink policy
Arid exclude
hidden policy
ignore-file policy
unsupported source type
```

Positive included/traversed results use one positive reason:

```text
explicit-file
explicit-directory
discovered
```

Human text may display only the first decisive reason plus concise supporting details.

JSON preserves the ordered machine reason set.

## **21.2 Decision meaning**

For a file:

```text
include → selected as a Python source input
exclude → not selected
```

For a directory:

```text
include → normal discovery would traverse the directory
exclude → normal discovery would not traverse it
```

Explaining a directory does not explain or enumerate descendants.

---

# **22. path-explanation-v1**

Add:

```text
schemas/path-explanation-v1.schema.json
```

using JSON Schema Draft 2020-12 and `additionalProperties: false`.

The exact v1 document shape is:

```json
{
  "schema_version": 1,
  "tool_version": "2.1.0",
  "path": "src/generated/example.py",
  "kind": "file",
  "symlink": false,
  "explicit": false,
  "decision": "exclude",
  "reasons": ["ignore-file"]
}
```

Stable v1 values are:

```text
kind:
    file
    directory

decision:
    include
    exclude

reason:
    explicit-file
    explicit-directory
    discovered
    outside-scan-roots
    arid-exclude
    ignore-file
    hidden
    unsupported-source-type
    symlink-directory
    symlink-traversal
```

If implementation evidence before the first prerelease demonstrates that the two symlink reason values cannot be distinguished reliably across supported platforms, they MUST be replaced with one `symlink-policy` value before the schema is published rather than guessed at runtime.

Once released, the v1 vocabulary is immutable.

---

# **23. Path Representation and Determinism**

Path explanation SHOULD use canonical project-relative paths whenever the target is inside project root.

The target presented in output is normalized independently of how the user spelled equivalent `.` components.

For compatible existing scans involving a path outside project root, Arid MAY retain an absolute normalized representation rather than reject the diagnostic. Such a path is result-relevant and therefore not considered unrelated volatile metadata.

No temporary path, process ID, timestamp, or current worker information is emitted.

---

# **24. Administrative JSON Rendering**

Both new administrative models receive dedicated render functions:

```text
render_suppression_status_json
render_path_explanation_json
```

Serialization occurs from typed logical models, not ad-hoc `serde_json::json!` fragments spread across orchestration.

Text renderers consume the same logical model.

Schema tests MUST deserialize/validate representative documents and assert deterministic ordering.

---

# **25. Administrative Direct-File Output**

V2.1 reuses the existing repeatable:

```text
--report <FORMAT=PATH>
```

for direct JSON output from the two new administrative modes only.

Examples:

```bash
arid . --suppression-status \
    --report json=artifacts/arid-suppressions.json

arid . --explain-path src/generated.py \
    --report json=artifacts/arid-path.json
```

For these modes:

- only `json=PATH` is valid as a supplemental target
- text/Markdown/SARIF supplemental targets are rejected
- the file is rendered from the same in-memory administrative model as stdout
- the existing atomic replacement behavior is reused
- duplicate destination validation remains in force

Normal scan `--report` semantics remain unchanged.

Other existing administrative modes do not gain supplemental report files in 2.1.

## **25.1 Semantic equivalence**

If primary stdout is JSON and a supplemental JSON file is requested, both are produced by the same renderer and SHOULD be byte-identical apart from process-level newline handling.

If stdout is text and supplemental JSON is requested, both still represent the same underlying administrative model.

No separate abbreviated file model exists.

---

# **26. Output Module Refactor**

`output.rs` SHOULD extract only the smallest reusable pieces required for administrative JSON files:

```text
report-target parsing / destination validation
atomic string write
```

Normal report rendering remains report-specific.

The implementation MUST NOT generalize this into a reporter registry or polymorphic output framework.

A private helper conceptually equivalent to:

```rust
fn write_atomic_output(
    path: &Path,
    contents: &str,
    project_root: &Path,
) -> Result<(), OperationalError>
```

is sufficient.

---

# **27. Error Semantics**

V2.1 reuses the v2 `OperationalError` and `error-v1` taxonomy.

Expected mappings include:

```text
invalid mode combination        → configuration
missing explain target          → discovery
ignore matcher / traversal fail → discovery
source read failure in audit    → read
Python parse failure in audit   → parse
JSON serialization failure      → output
supplemental file write failure → output
internal mapping invariant      → internal
```

No `suppression` or `path-explanation` error kind is added merely to identify which administrative mode was running.

When primary output is JSON and execution fails after CLI parsing, existing `error-v1` behavior remains authoritative.

A partial suppression audit is never emitted as a successful suppression-status-v1 document.

---

# **28. Capability Discovery Compatibility**

`schemas/capabilities-v1.schema.json` is already a published stable schema with a closed feature vocabulary.

V2.1 MUST NOT mutate that historical schema merely to append the new feature names.

V2.1 therefore leaves capabilities-v1 unchanged.

The new versioned administrative schemas and CLI version provide the 2.1 machine-contract boundary.

If a future release requires `--capabilities` to enumerate the 2.1 feature set explicitly, that work requires a separately versioned capabilities contract rather than silently changing capabilities-v1.

---

# **29. Performance Design**

## **29.1 Normal scans**

Normal scans retain the 2.0 path and SHOULD incur no additional detector pass.

The only acceptable normal-path overhead is negligible state handling required to preserve/refactor existing suppression semantics.

## **29.2 Suppression status**

Suppression status performs one complete preparation/detection analysis over the audit view.

It does not first run a normal suppressed detector and then rerun unsuppressed detection; the administrative question requires only the audit result.

Therefore the expected major cost is approximately one ordinary complete Arid analysis, not one analysis per suppression.

## **29.3 Path explanation**

Path explanation performs configuration resolution, target metadata, and targeted discovery matching only.

It does not recursively rescan the project solely to answer one path question when the existing targeted ignore matcher can evaluate the requested path.

## **29.4 --no-ignore-files**

The flag changes only walker filtering configuration.

Any performance difference is a consequence of discovering a larger source corpus, not a second discovery pass.

---

# **30. Test Architecture**

Tests SHOULD be organized around domain boundaries rather than only CLI snapshots.

## **30.1 Suppression state tests**

Unit tests lock:

```text
disable → enable
disable → EOF
repeated disable no-op
repeated enable no-op
leading enable no-op
multiple effective regions
inline directive takes effect after its line
```

## **30.2 Audit classification tests**

Integration/domain tests cover:

```text
suppressed region duplicates unsuppressed source → active
suppressed regions duplicate one another → both active
suppressed region no longer duplicates → stale
boundary-only prevention → active
no effective regions → successful empty audit
baseline acceptance does not alter status
focus does not alter status
```

## **30.3 Discovery tests**

Existing file-discovery fixtures should be extended for:

```text
.ignore
.gitignore
.git/info/exclude where practical
global ignore fixture where hermetic setup is practical
parent ignore file
Arid exclude
hidden path
explicit ignored file
.py / .pyi / unsupported extension
file symlink
directory symlink
--no-ignore-files composition
```

Tests that depend on user-global Git configuration MUST isolate environment state rather than consume the developer machine's actual configuration.

## **30.4 Schema tests**

Golden representative documents validate against:

```text
suppression-status-v1.schema.json
path-explanation-v1.schema.json
```

Ordering and EOF serialization receive explicit regression tests.

## **30.5 Exit policy tests**

One matrix test SHOULD lock `--fail-on-stale` behavior for both suppression and baseline status.

---

# **31. Validation Design**

The v2.1 validation path SHOULD prove four things independently:

1. ordinary 2.0 duplicate results remain unchanged;
2. suppression status classifies known active/stale fixtures correctly;
3. path explanation agrees with actual `--list-files`/scan discovery outcomes;
4. `--no-ignore-files` changes only ignore-file-derived traversal decisions.

A strong discovery consistency test SHOULD, for representative files, compare:

```text
--explain-path decision
        ↕
actual --list-files membership
```

under both normal traversal and `--no-ignore-files`.

The real-world validation corpus remains the final regression guard for detector stability.

---

# **32. Implementation Order**

Implementation SHOULD proceed inside-out:

```text
1. suppression state/region derivation
2. audit preparation mode
3. suppression audit classification
4. suppression status model/renderers/schema
5. generic stale exit policy + baseline integration
6. discovery policy / WalkBuilder construction
7. --no-ignore-files
8. targeted path explanation
9. path explanation model/renderers/schema
10. administrative supplemental JSON file output
11. CLI/app orchestration
12. documentation / validation / qualification
```

Lower-level domain behavior is made correct first even if application orchestration temporarily requires follow-up changes.

Do not preserve an awkward upper-layer interface merely to avoid updating its callers.

---

# **33. Architecture Acceptance Criteria**

The v2.1 architecture is correctly implemented when:

- normal scans use the same detector semantics and report-v4 contracts as 2.0
- suppression directives behave as idempotent state setters
- EOF while disabled is valid
- no-op directives do not create audit regions
- suppression status uses one unsuppressed audit detector pass
- every effective suppression region is deterministically active or stale
- `--fail-on-stale` layers policy onto suppression and baseline status without mutation
- path explanation evaluates the same effective discovery policy as actual traversal
- `--no-ignore-files` disables ignore-file filtering without disabling hidden or Arid exclude policy
- the two new JSON contracts validate against immutable v1 schemas
- stdout and direct JSON files render the same logical administrative state
- no new detector, reporter framework, waiver database, or discovery implementation is introduced

---

# **34. V2.1 Technical Completion Criterion**

Arid 2.1 is technically complete when one stable 2.0 detector can be used both normally and through a suppression-disabled preparation view to audit suppression maintenance; the existing baseline comparison can enforce stale cleanup without changing baseline-v1; the existing discovery system can both explain one target and selectively bypass ignore files without bypassing Arid policy; and both new administrative domains expose deterministic, versioned, directly writable JSON contracts without changing ordinary scan behavior.
