# **Arid v2.2 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.2.0`  
**Requirements:** `docs/arid-v2.2-requirements-specification.md`

---

# **1. Purpose**

This document defines the technical architecture for Arid 2.2.

The v2/v2.1 architecture remains the implementation base. V2.2 adds high-signal aggregation and presentation around the existing final reportable finding set without redesigning duplicate identity, report-v4, baseline-v1, suppression behavior, discovery, or the normal scan pipeline.

The architecture exists to implement:

- a rich normal-text Summary table;
- Context/Scope/Distribution breakdowns;
- objective top-file Hotspots;
- semantic summary color;
- `--summary-only`;
- `summary-v1`;
- compact GitHub Action outputs;
- deterministic output across workers/platforms;
- negligible aggregation overhead.

The detector itself is not redesigned.

---

# **2. Architectural Principles**

## **2.1 One authoritative detector**

V2.2 MUST continue to use the existing detector exactly once during a normal scan.

Summary generation is downstream of reportable findings:

```text
existing scan pipeline
        ↓
final reportable groups
        ↓
report-v4 typed Report
        ↓
one Summary derivation pass
        ↓
┌────────────────────┬────────────────────┬────────────────────┐
│ detailed text      │ summary-only text  │ summary-v1 JSON    │
└────────────────────┴────────────────────┴────────────────────┘
```

No summary feature may cause another detection pass.

## **2.2 The Report remains the source of finding truth**

The existing `Report` already represents the final active/focused finding set after baseline and focus policy.

V2.2 MUST derive summary metrics from that typed report rather than from:

- raw detector groups;
- rendered text;
- serialized JSON;
- filesystem traversal;
- a second metrics calculation over source files.

This preserves the current policy order automatically.

## **2.3 One typed Summary model**

Core Arid MUST construct one private/public-as-needed typed `Summary` value from the final `Report`.

The same `Summary` instance SHOULD feed:

- the rich footer appended to detailed text;
- summary-only text;
- `summary-v1` JSON serialization.

Renderers MUST NOT independently recalculate Context, Scope, Distribution, hotspot, occurrence, or distinct-file counts.

## **2.4 Aggregation is evidence, not scoring**

The aggregation layer counts existing facts.

It MUST NOT contain:

- thresholds;
- severity rules;
- health grades;
- quality scores;
- remediation ranking;
- AI advice.

## **2.5 Report-v4 remains frozen**

V2.2 MUST NOT add summary fields to report-v4 merely because they are convenient.

The full report already contains enough finding-level evidence to derive the new aggregates.

`summary-v1` is a separate compact contract with a different consumption purpose.

## **2.6 Human tables are presentation, not a framework**

The text renderer MAY gain small private helpers for table layout.

V2.2 MUST NOT introduce a generic reporter framework or broad table abstraction library merely to render three small deterministic tables.

A new external table-formatting dependency SHOULD be avoided.

If exact Unicode display-cell alignment cannot be implemented reliably with a small local helper, a narrowly scoped width dependency MAY be considered, but only after tests demonstrate the need.

## **2.7 Determinism is structural**

Summary counts use fixed bucket structures, not unordered public maps.

Hotspots use an explicit stable sort.

Human breakdown rows use fixed categorical order.

Machine serialization uses typed structs with explicit fields.

No output order depends on hash iteration or worker completion order.

## **2.8 Summary-only is presentation policy**

`--summary-only` MUST NOT create another invocation mode that forks the scan pipeline.

It is a scan-output modifier evaluated only after the normal report has been built.

## **2.9 Existing supplemental reports remain independent**

`--summary-only` changes primary output only.

`--report text|json|markdown|sarif=PATH` continues to render the existing full report representations from the same `Report`.

This allows one scan to produce a compact primary summary and full artifacts without rerunning detection.

---

# **3. Major Technical Decisions**

| Area | V2.2 decision |
| --- | --- |
| Detection semantics | unchanged from 2.1 |
| Summary input | final typed `Report` after baseline/focus |
| Summary derivation | one linear pass over findings/locations |
| Summary model | dedicated typed `Summary` in a small summary module |
| report-v4 | unchanged |
| summary machine contract | new `summary-v1` |
| `--summary-only` | CLI-only primary-output modifier |
| Text summary | always present for normal text scans reaching report rendering |
| Detailed findings | suppressed only when `--summary-only` is active |
| Breakdown percentages | group share, calculated for text presentation |
| Breakdown machine values | integer bucket counts only |
| Files with duplicates | unique location paths in final findings |
| Occurrences | sum of finding occurrence counts |
| Hotspot ranking | groups desc → occurrences desc → path asc |
| Human hotspots | top 5 only |
| Machine hotspots | same deterministic top 5 |
| Per-file duplicate lines | intentionally omitted in v2.2 |
| Color | reuse existing `TextStyles` / `--color` policy |
| Markdown/SARIF summary-only | rejected |
| Supplemental reports | remain full existing formats |
| Direct summary file report | not required in v2.2 |
| Timing | existing text-only `Total time:` contract unchanged |
| GitHub Action | expose new compact outputs without another detector run |
| Action parity | adapter behavior validated against core `summary-v1` fixtures |

---

# **4. Intended Repository Delta**

The expected source delta is intentionally small:

```text
src/
├── app.rs              # summary construction + primary-output selection
├── cli.rs              # --summary-only + conflicts
├── summary.rs          # typed aggregation model + summary-v1 JSON
├── text.rs             # rich summary/breakdown/hotspot tables + color
└── ... existing modules unchanged where possible

schemas/
└── summary-v1.schema.json

action/
├── run.py              # new outputs / compact summary adapter
└── test_run.py         # output and parity tests

action.yml              # expose new action outputs

validation/
└── v2.2.sh             # focused contract validation
```

`summary.rs` is justified because it owns a distinct typed domain result consumed by multiple output paths.

A separate `table.rs` SHOULD NOT be introduced initially.

A separate `hotspot.rs` SHOULD NOT be introduced; hotspots are one projection of the summary domain.

---

# **5. Pipeline Placement**

The current normal scan builds a `Report` only after baseline and focus filtering.

V2.2 inserts summary derivation immediately after `build_report` succeeds:

```rust
let report = build_report(...)?;
let summary = build_summary(&report, summary_options);

let output = match output_format {
    Text if cli.summary_only => render_summary_text(&summary, text_color),
    Text => render_text(&report, &summary, text_color),
    Json if cli.summary_only => render_summary_json(&summary)?,
    Json => render_json(&report)?,
    Markdown => render_markdown(&report),
    Sarif => render_sarif(&report)?,
};

write_report_targets(..., &report, ...)?;
```

The exact function names MAY differ, but the dependency direction MUST remain:

```text
Report → Summary → renderer
```

and never:

```text
renderer → recalculate summary
```

---

# **6. Summary Domain Model**

A representative internal model is:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Summary {
    pub(crate) schema_version: u8,
    pub(crate) tool_version: &'static str,
    pub(crate) complete: bool,
    pub(crate) analysis: SummaryAnalysis,
    pub(crate) errors: Vec<OperationalError>,
    pub(crate) files: u64,
    pub(crate) files_with_duplicates: u64,
    pub(crate) source_lines: u64,
    pub(crate) analyzed_lines: u64,
    pub(crate) duplicate_groups: u64,
    pub(crate) occurrences: u64,
    pub(crate) duplicate_lines: u64,
    pub(crate) duplication_percent: f64,
    pub(crate) context: ContextSummary,
    pub(crate) scope: ScopeSummary,
    pub(crate) distribution: DistributionSummary,
    pub(crate) hotspots: Vec<Hotspot>,
}
```

This is illustrative, not a requirement to expose the Rust type publicly.

## **6.1 SummaryAnalysis**

`SummaryAnalysis` SHOULD copy the stable effective analysis values already available from `Report.analysis` and add the effective ignore-file state required by `summary-v1`.

Representative shape:

```rust
struct SummaryAnalysis {
    min_lines: u32,
    ignore_comments: bool,
    ignore_docstrings: bool,
    ignore_imports: bool,
    ignore_signatures: bool,
    same_file: bool,
    hidden: bool,
    ignore_files: bool,
    exclude: Vec<String>,
    baseline_enabled: bool,
    focus: Vec<String>,
    virtual_source: Option<String>,
    keep_going: bool,
}
```

`ignore_files` MUST be supplied from effective invocation/discovery policy because report-v4 intentionally did not gain that v2.1 field.

## **6.2 Fixed bucket structs**

Use fixed structs rather than maps:

```rust
struct ContextSummary {
    executable: u64,
    declarative: u64,
    mixed: u64,
}

struct ScopeSummary {
    function: u64,
    module: u64,
    class: u64,
    mixed: u64,
}

struct DistributionSummary {
    cross_file: u64,
    same_file: u64,
    hybrid: u64,
}
```

Benefits:

- deterministic serialization order;
- compile-time completeness when enums change;
- no public hash ordering;
- direct schema correspondence.

## **6.3 Hotspot model**

Representative shape:

```rust
struct Hotspot {
    path: String,
    groups: u64,
    occurrences: u64,
}
```

Only the final top-five list is stored in `Summary` unless implementation evidence shows retaining all hotspot rows is simpler and still cheap.

---

# **7. Summary Build Options**

The summary builder needs one item not recoverable from frozen report-v4:

```text
effective ignore-file policy
```

Use a small options value rather than widening report-v4:

```rust
struct SummaryOptions {
    ignore_files: bool,
}
```

The builder signature SHOULD remain conceptually small:

```rust
fn build_summary(report: &Report, options: SummaryOptions) -> Summary
```

Summary construction should be infallible for a valid typed `Report` unless checked integer overflow policy requires a deliberate internal error path.

If overflow is checked, it MUST map to existing internal operational-error handling rather than saturating silently.

---

# **8. Aggregation Algorithm**

Summary derivation is one pass over `report.findings`.

Conceptually:

```text
initialize zero buckets
initialize files_with_duplicates set
initialize per-path hotspot accumulator

for each finding:
    occurrences += finding.occurrences
    increment one Context bucket
    increment one Scope bucket
    increment one Distribution bucket

    initialize per-finding seen_paths set

    for each location:
        files_with_duplicates.insert(location.path)
        hotspot[path].occurrences += 1

        if path not in seen_paths:
            hotspot[path].groups += 1
            seen_paths.insert(path)

sort hotspot rows
truncate to 5
```

The per-finding `seen_paths` rule is necessary for hybrid groups with multiple occurrences in one file.

## **8.1 Complexity**

Let:

```text
F = number of reportable findings
L = total locations across findings
P = number of distinct paths in reportable findings
```

Aggregation SHOULD be:

```text
O(F + L + P log P)
```

where the sort is only over hotspot paths.

Because only top five rows are needed, implementation MAY use a bounded selection strategy if profiling demonstrates value, but full deterministic sorting is preferred initially for simplicity.

---

# **9. Distinct Path Identity**

Summary path identity MUST use the already rendered/project-relative `Location.path` values from the typed `Report`.

Do not re-resolve filesystem paths during aggregation.

This guarantees:

- agreement with detailed findings;
- project-relative hotspot output;
- no filesystem work;
- consistent focus behavior;
- deterministic string identity.

---

# **10. Bucket Invariants**

The builder MUST enforce or test the following invariants:

```text
context.executable
+ context.declarative
+ context.mixed
= duplicate_groups
```

```text
scope.function
+ scope.module
+ scope.class
+ scope.mixed
= duplicate_groups
```

```text
distribution.cross_file
+ distribution.same_file
+ distribution.hybrid
= duplicate_groups
```

These SHOULD be covered by unit tests and MAY use debug assertions internally after construction.

---

# **11. Hotspot Determinism**

Hotspot rows sort by:

```rust
right.groups.cmp(&left.groups)
    .then_with(|| right.occurrences.cmp(&left.occurrences))
    .then_with(|| left.path.cmp(&right.path))
```

or equivalent.

After sorting:

```text
truncate to first 5
```

No filesystem metadata, path depth, source size, or duplicate-line estimate participates in ranking.

The ranking is descriptive, not a severity score.

---

# **12. Text Rendering Architecture**

`text.rs` remains the owner of human-facing text composition and semantic styles.

V2.2 SHOULD extend `TextStyles` minimally rather than introducing a theme object.

Existing concepts already map well:

```text
heading
classification
distribution
group
problem
success
path
```

A new style field MAY be added only if an existing semantic style cannot represent a table value cleanly.

## **12.1 Detailed text path**

Detailed normal text becomes conceptually:

```text
DUP001 finding blocks

Summary table

Breakdown table, when findings exist

Hotspots table, when findings exist
```

The existing application-level timing footer remains appended afterward.

## **12.2 Summary-only text path**

Summary-only text uses the same summary renderer but omits finding blocks:

```text
Summary table

Breakdown table, when findings exist

Hotspots table, when findings exist
```

No second summary implementation is permitted.

## **12.3 Zero findings**

For zero findings:

- retain a clear success message;
- render the overall Summary;
- omit Breakdown and Hotspots to avoid zero-row noise.

Exact placement of `No duplicate code found.` relative to the Summary should be frozen in golden tests before alpha.

## **12.4 Incomplete scan**

Incomplete keep-going text SHOULD add a concise explicit marker such as:

```text
Summary (incomplete)
```

or an equivalent status row.

Do not color partial metrics as success.

---

# **13. Small Table Renderer**

Use a small private rendering helper sufficient for the three v2.2 tables.

Required responsibilities:

- compute deterministic column widths from known labels/values;
- right-align numeric columns;
- left-align labels/paths;
- emit UTF-8 box-drawing characters;
- insert ANSI styles around cell content without counting escape bytes as visible width;
- preserve plain-text alignment when color is disabled.

The helper SHOULD accept already-formatted logical cell text plus semantic style intent rather than becoming a generic data-table API.

Runtime terminal-width detection SHOULD NOT be used because it would make output environment-dependent.

Hotspot paths SHOULD remain complete in v2.2 rather than being truncated based on terminal width.

---

# **14. Number Formatting**

Human integer counts SHOULD use comma thousands separators.

Implement a tiny deterministic integer formatter rather than adding a localization dependency.

Examples:

```text
0
999
1,000
12,084
418,632
```

Percentages retain two decimal places where shown.

Machine JSON retains numeric types and MUST NOT serialize formatted strings such as `"12,084"`.

---

# **15. Percentage Rendering**

Human breakdown percentages are derived at render time from integer bucket counts and `duplicate_groups`.

Do not store redundant bucket percentages in `Summary`.

Conceptually:

```rust
fn group_percent(groups: u64, total: u64) -> f64
```

When `total == 0`, Breakdown is omitted in text and no division is needed.

Machine consumers derive percentages from counts.

---

# **16. `--summary-only` CLI Architecture**

Add one boolean CLI field:

```rust
pub summary_only: bool
```

It is not part of `[tool.arid]`.

It is not an `OutputFormat` variant.

It is not an invocation mode.

## **16.1 Validation**

Validation should occur with existing output-option validation in `app.rs` / CLI conflict declarations.

Reject `--summary-only` with terminal administrative operations.

Reject:

```text
--summary-only --format markdown
--summary-only --format sarif
```

Allow:

```text
--summary-only
--summary-only --format text
--summary-only --json
--summary-only --format json
--summary-only --report json=...
--summary-only --report markdown=...
--summary-only --report sarif=...
```

Existing color validation remains sufficient when primary output is not text.

---

# **17. Primary Output Selection**

Primary output selection extends existing normal scan rendering only.

Conceptually:

```rust
match (output_format, cli.summary_only) {
    (Text, false) => detailed text + summary,
    (Text, true) => summary text only,
    (Json, false) => report-v4,
    (Json, true) => summary-v1,
    (Markdown, false) => existing Markdown,
    (Sarif, false) => existing SARIF,
    (Markdown | Sarif, true) => rejected before execution,
}
```

This explicit matrix is preferred over making `summary-v1` another general output-format enum member.

---

# **18. Supplemental Reports**

`write_report_targets` continues to receive the full `Report` and remains unchanged unless implementation requires a small signature refactor.

`--summary-only` MUST NOT alter supplemental target meaning.

This preserves the useful one-scan composition:

```text
compact stdout + full JSON + full Markdown + SARIF
```

No `summary` supplemental format is required in v2.2.

---

# **19. `summary-v1` Serialization**

`summary.rs` SHOULD own:

```text
SUMMARY_SCHEMA_VERSION = 1
render_summary_json(&Summary)
```

Use deterministic pretty JSON consistent with existing Arid machine contracts:

```rust
serde_json::to_string_pretty(summary)
```

The serialization type MUST correspond exactly to `schemas/summary-v1.schema.json`.

Timing is not part of the type and therefore cannot accidentally serialize.

---

# **20. `summary-v1.analysis` Construction**

Most fields can be copied directly from `report.analysis`.

The effective ignore-file value is:

```text
ignore_files = !cli.no_ignore_files
```

or an equivalent value from an explicit discovery policy object if implementation has one available at the construction site.

Prefer the effective policy value over reinterpreting raw CLI flags when practical.

The summary schema is new, so its analysis shape may be more complete than frozen report-v4 without requiring report-v4 mutation.

---

# **21. Errors and Incomplete Reports**

`Summary.errors` SHOULD clone/reuse the report's structured errors.

Do not introduce `SummaryError` as a second source-processing error taxonomy.

For keep-going scans:

```text
Report.complete = false
        ↓
Summary.complete = false
Report.errors
        ↓
Summary.errors
```

Summary metrics are still useful partial evidence, but exit policy remains `Error` / process exit `2`.

The existing application-level timing rule continues to suppress `Total time:` on exit `2`.

---

# **22. Baseline and Focus Preservation**

Because `Summary` is downstream of `Report`, baseline and focus semantics are inherited automatically.

No summary-specific baseline or focus filtering should exist.

This is a hard architecture boundary.

If implementation code attempts to inspect baseline files or focus paths while counting findings, that is evidence the layer boundary is wrong.

---

# **23. GitHub Action Architecture**

The Action currently obtains a full report-v4 JSON from the same scan via supplemental `--report json=PATH` and parses it for metrics.

V2.2 MUST preserve the one-scan Action architecture.

Two acceptable implementation strategies exist:

### **Strategy A — Action adapter from report-v4**

Extend the existing Python adapter to derive the new compact outputs from the already-produced report-v4 document.

This is the lowest-risk option for preserving current Action stdout/log behavior.

Requirements:

- implement the same documented counts and hotspot ordering;
- derive `ignore_files` from validated Action arguments/effective invocation state because frozen report-v4 does not expose it;
- serialize the Action `summary-json` with the same field names and deterministic ordering as `summary-v1`;
- add parity fixtures that compare Action-derived summary JSON against core Arid `summary-v1` for the same logical scan fixture.

### **Strategy B — Consume primary `summary-v1`**

The Action MAY run the single Arid process with summary-v1 as primary output and existing full formats as supplemental reports, then parse the captured primary JSON.

This is acceptable only if current Action user-visible logging and argument compatibility can be preserved cleanly.

### **Preferred v2.2 choice**

Prefer **Strategy A** unless implementation evidence shows Strategy B is simpler without regressions.

The Action is an integration adapter; duplicating a small deterministic projection there is acceptable when parity-tested and avoids changing core CLI/report semantics merely to service one integration.

A second Arid scan is prohibited.

---

# **24. Action Data Model**

Extend the Action's parsed report representation or add a compact derived summary representation.

At minimum it must expose:

```text
occurrences
files_with_duplicates
summary_json
```

Existing outputs remain unchanged.

`summary-json` SHOULD be compact single-line JSON when written as a GitHub output unless multiline encoding is simpler and already supported safely by `write_github_outputs`.

The logical JSON document MUST remain schema-equivalent to `summary-v1`.

---

# **25. Action Parity Testing**

If Strategy A is used, parity testing is mandatory because the Action reconstructs aggregation from report-v4.

Fixtures MUST cover:

- repeated occurrences in one file;
- hybrid findings;
- distinct files;
- Context/Scope/Distribution counts;
- hotspot ties;
- top-five truncation;
- focus-preserved outside paths;
- incomplete reports;
- `--no-ignore-files` analysis metadata.

Parity tests SHOULD compare parsed JSON objects rather than whitespace unless Action output promises byte identity.

---

# **26. Schema Architecture**

Add:

```text
schemas/summary-v1.schema.json
```

Follow existing Arid schema conventions:

- explicit object types;
- explicit required fields;
- `additionalProperties: false` where consistent with existing schemas;
- non-negative integer constraints;
- exact schema version constant;
- explicit hotspot item schema;
- explicit analysis object;
- existing structured error shape copied/referenced consistently with current schema practice.

Do not mutate report-v4 schema.

---

# **27. Public Rust API Boundary**

V2.2 does not require a new public Rust API.

The summary model MAY remain crate-private unless a concrete existing public API requirement demonstrates otherwise.

Do not export `Summary` merely because it is serializable.

If future demand exists for library consumers, that is a separate API-versioning decision.

---

# **28. Color Architecture**

The summary renderer MUST reuse the existing `color` boolean resolved by application policy.

Do not inspect TTY state inside summary code.

Conceptually:

```text
app resolves color policy
        ↓
text renderer receives color bool
        ↓
TextStyles::colored() or plain path
```

Table borders remain unstyled/default.

Color is applied to cell values after width calculation or through helpers that keep visible width separate from ANSI bytes.

---

# **29. Plain and Colored Renderer Parity**

The plain and colored text paths MUST have identical visible characters after ANSI escape removal.

Tests SHOULD assert:

```text
strip_ansi(render_colored(summary)) == render_plain(summary)
```

for representative summary fixtures.

This protects alignment and avoids maintaining two logical table renderers.

Implementation SHOULD therefore share table composition and apply styles at cell emission time rather than maintaining separate table layouts.

---

# **30. Existing `Total time:` Integration**

V2.2 MUST leave timing at the application presentation boundary established in v2.1.

Expected completed text order:

```text
[detailed findings unless summary-only]
Summary
Breakdown
Hotspots
Total time: ...
```

For zero findings:

```text
No duplicate code found.
Summary
Total time: ...
```

Exact blank-line spacing is frozen by golden tests.

Machine output remains timing-free.

---

# **31. Performance Architecture**

No expensive operation is added before report construction.

The only normal-scan cost is:

1. build one Summary from already-built findings;
2. render it when needed.

Even full JSON/Markdown/SARIF scans do not inherently need a Summary unless:

- the primary output is summary-only JSON; or
- Action/core orchestration specifically requires one.

For efficiency, application code MAY construct `Summary` conditionally when the selected primary text/summary output needs it.

However, avoid multiple conditional builders that risk divergent behavior.

A small summary over even thousands of findings should be negligible compared with parsing/normalization/detection.

---

# **32. Failure Boundaries**

Summary aggregation should have no ordinary user-facing failure mode for a valid `Report`.

Potential implementation failures are limited to:

- checked integer overflow;
- JSON serialization failure;
- output/write failure through existing mechanisms.

Table rendering to `String` uses infallible formatting expectations consistent with existing text rendering.

No filesystem operation belongs in summary aggregation.

---

# **33. Testing Strategy**

## **33.1 Summary unit tests**

Test direct `Report → Summary` derivation for:

- zero findings;
- one finding;
- multiple contexts;
- multiple scopes;
- every distribution;
- repeated paths;
- hybrid findings;
- hotspot ties;
- top-five truncation;
- incomplete reports;
- integer totals.

## **33.2 Text golden tests**

Freeze:

- detailed findings + summary;
- summary-only output;
- zero findings;
- incomplete summary;
- large numbers with separators;
- colored/plain visible parity;
- Unicode hotspot paths.

## **33.3 CLI tests**

Cover the complete summary-only format/conflict matrix.

## **33.4 Schema tests**

Validate complete, zero-finding, and incomplete `summary-v1` documents.

## **33.5 Integration tests**

Prove:

- detailed and summary-only scans have identical exit status;
- detailed and summary-only summary values match;
- baseline behavior matches;
- focus behavior matches;
- supplemental report-v4 remains full;
- JSON summary is worker-deterministic;
- timing is absent from machine output.

## **33.6 Action tests**

Cover new outputs and one-scan behavior.

---

# **34. Compatibility Test Boundary**

Existing report-v4 machine fixtures SHOULD remain byte-identical unless unrelated existing version metadata makes byte identity impossible.

At minimum their schema shape and logical content MUST remain unchanged.

Existing Markdown and SARIF output SHOULD remain unchanged.

Normal text output intentionally changes because v2.2 replaces the old two-line summary with richer tables.

Validation that compares v2.1 and v2.2 normal text MUST normalize only the intentional summary/timing presentation differences rather than weakening finding comparison.

---

# **35. Validation Architecture**

Add a durable `validation/v2.2.sh` or smallest equivalent extension.

It SHOULD include inherited v2.1 validation plus focused v2.2 checks.

The validation suite MUST exercise published-style binaries rather than only unit tests.

Real-world repositories SHOULD include at least the established benchmark/validation corpus so summary counts are tested at meaningful scale.

---

# **36. Release Tooling**

`release.sh` MUST map `2.2.*` to the v2.2 roadmap and release metadata state.

Pre-publication workflow validation MUST include supported platform builds.

Release assets/source MUST contain `schemas/summary-v1.schema.json`.

The first published prerelease freezes the `summary-v1` field names, bucket vocabulary, hotspot ordering, and summary-only JSON semantics for the 2.2 release line.

---

# **37. Toolchain Reproducibility Follow-up**

V2.1 closeout identified that unpinned local `stable` rustfmt could drift and reject byte-identical previously qualified source.

V2.2 planning SHOULD include a small release-process/tooling correction before release qualification:

- choose and document a known-good Rust/rustfmt toolchain policy;
- pin it in the repository if that is the simplest durable solution;
- make formatting qualification reproducible across v2.2 stages.

This work is release-process infrastructure, not a v2.2 product feature.

It MUST remain separate from summary semantics and MUST NOT trigger gratuitous source reformatting solely to satisfy a newly advanced formatter.

---

# **38. Implementation Order**

Implementation SHOULD proceed inside-out:

```text
1. summary domain/counting model
2. summary-v1 schema + JSON rendering
3. hotspot aggregation
4. plain text summary tables
5. semantic color integration
6. --summary-only orchestration
7. supplemental-report compatibility
8. GitHub Action outputs/parity
9. documentation/validation
10. release tooling/toolchain reproducibility
11. prerelease qualification
```

Do not begin with CLI cosmetics before the counting model is frozen and tested.

---

# **39. Architectural Rejection Criteria**

An implementation is architecturally wrong if it:

- reruns detection for summary output;
- calculates summary from rendered text or serialized report-v4;
- changes report-v4 to avoid creating `summary-v1`;
- makes baseline/focus decisions inside summary aggregation;
- computes per-file duplicate lines by naïvely summing overlapping finding lengths;
- ranks hotspots using an undocumented score;
- uses count-based dynamic breakdown row ordering;
- introduces environment-dependent terminal-width output;
- adds a general reporter/table framework;
- adds MCP/server infrastructure;
- places timing in `summary-v1`;
- allows text, JSON, and Action summary counts to drift semantically;
- runs Arid twice in the GitHub Action solely to obtain summary data.

---

# **40. Architecture Completion Criterion**

The v2.2 architecture is correctly implemented when this dependency structure is true:

```text
stable Arid detector
        ↓
existing baseline/focus policy
        ↓
existing typed Report
        ↓
one deterministic Summary projection
        ↓
┌──────────────────────┬──────────────────────┐
│ human signal         │ machine/agent signal │
│                      │                      │
│ Summary              │ summary-v1           │
│ Breakdown            │ deterministic counts │
│ Hotspots             │ top-five hotspots    │
│ semantic color       │ no timing/color      │
└──────────────────────┴──────────────────────┘
```

and the existing detailed report formats remain authoritative and compatible.
