# **Arid v2.2 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.2.0`  
**Version scope:** This document defines backward-compatible functionality targeted for Arid **2.2.0**.

---

# **1. Purpose**

Arid 2.2 turns the duplicate findings Arid already produces into a compact, high-signal view that is easier for developers, CI systems, tools, and coding agents to consume.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The stable v1, v1.1, v1.2, v2, and v2.1 requirements remain the base product contract.

This document defines only requirements added or changed for v2.2. Unless explicitly changed here, behavior specified by:

```text
docs/arid-v1-requirements-specification.md
docs/arid-v1.1-requirements-specification.md
docs/arid-v1.2-requirements-specification.md
docs/arid-v2-requirements-specification.md
docs/arid-v2.1-requirements-specification.md
```

remains in force.

Arid already reports rich per-finding information such as structural context, structural scope, occurrence count, file count, and occurrence distribution. That information is useful finding by finding but becomes difficult to interpret when a repository contains tens, hundreds, or thousands of duplicate groups.

V2.2 addresses that presentation and consumption problem without changing duplicate detection.

The v2.2 theme is:

> **High-signal duplicate intelligence. Same focused detector.**

---

# **2. Product Principle**

Arid 2.2 MUST make existing duplicate evidence easier to understand and consume without inventing synthetic quality judgments or alternate duplicate semantics.

The release MUST answer, at a glance:

- how much active duplication is being reported;
- how widely it is distributed across files;
- what structural contexts dominate the findings;
- what structural scopes dominate the findings;
- whether findings are primarily same-file, cross-file, or hybrid;
- which files participate in the largest number of reported duplicate groups; and
- whether the same information can be consumed compactly by automation and coding agents.

V2.2 MUST preserve Arid's distinction between evidence and judgment.

Arid reports duplicate structure. It does not assign severity, health scores, quality grades, or automated refactoring advice.

---

# **3. V2.2 Goals**

Arid 2.2 MUST add or complete:

1. a rich end-of-run summary for normal human-facing text scans;
2. aggregate totals for files with reported duplication and total reported occurrences;
3. deterministic group-count breakdowns by Context;
4. deterministic group-count breakdowns by Scope;
5. deterministic group-count breakdowns by Distribution;
6. an objective top-file Hotspots section;
7. semantic color for the summary and breakdown presentation using Arid's existing color policy;
8. a CLI-only `--summary-only` scan presentation modifier;
9. a compact deterministic `summary-v1` JSON machine contract;
10. `summary-v1` output through `--summary-only --json` / `--summary-only --format json`;
11. GitHub Action outputs for total occurrences, files with reported duplication, and compact summary JSON;
12. deterministic behavior across worker counts and supported platforms;
13. preservation of report-v4, baseline-v1, error-v1, suppression-status-v1, and path-explanation-v1.

---

# **4. Explicit V2.2 Non-Goals**

Arid 2.2 MUST NOT add:

- semantic clone detection;
- fuzzy clone detection;
- identifier-renaming clone detection;
- structural similarity as duplicate identity;
- token-based duplicate metrics merely to resemble multi-language clone tools;
- another diagnostic beyond `DUP001`;
- severity levels;
- a duplication health score;
- a repository quality grade;
- "good" or "bad" percentage thresholds;
- automatic refactoring recommendations;
- AI-generated remediation advice;
- automatic duplicate removal;
- Git blame or code ownership analysis;
- Git-history-aware changed-file discovery such as `--changed-since`;
- a generalized analytics/dashboard subsystem;
- average/median clone scores without a demonstrated user requirement;
- complexity scoring;
- persistent statistics storage;
- trend storage;
- persistent caching;
- a daemon or RPC service;
- an MCP server;
- LSP/editor integration;
- a plugin framework;
- a reporter framework;
- a new duplicate detector;
- a report-v4 schema change merely to duplicate derivable summary data;
- timing fields in `summary-v1`;
- a new `[tool.arid]` setting for `--summary-only`;
- a configurable hotspot ranking algorithm;
- a synthetic hotspot severity score.

V2.2 is a signal-extraction and presentation release, not an expansion of duplicate identity or Arid's product category.

---

# **5. Compatibility and Detection Contract**

Arid 2.2 MUST preserve the Arid 2.1 duplicate definition and normal scan pipeline.

Unless a separately documented correctness defect requires a compatible fix, v2.2 MUST NOT change:

- Python parsing semantics;
- normalization rules;
- comment handling;
- docstring handling;
- import handling;
- signature handling;
- exact normalized duplicate identity;
- same-file overlap semantics;
- maximal duplicate qualification;
- deterministic group canonicalization;
- finding fingerprints;
- structural context classification;
- structural scope classification;
- occurrence distribution classification;
- duplicate metrics;
- baseline-v1 fingerprints;
- baseline acceptance semantics;
- focus semantics;
- suppression semantics;
- discovery semantics;
- report-v4 meaning or shape;
- SARIF finding identity;
- normal scan exit meanings `0`, `1`, and `2`;
- `--no-fail-on-findings` semantics;
- `Total time:` placement and machine-output exclusion rules.

All v2.2 summary information MUST be derived from the same reportable finding set used by the existing normal scan after baseline and focus policy have been applied.

V2.2 MUST NOT run duplicate detection a second time to calculate summary information.

---

# **6. Authoritative Summary Input**

The authoritative v2.2 summary input is the final normal-scan finding set after the existing pipeline has applied:

```text
discovery
    ↓
source preparation
    ↓
corpus construction
    ↓
duplicate detection
    ↓
baseline filtering, when enabled
    ↓
focus filtering, when enabled
    ↓
reportable findings
    ↓
v2.2 summary derivation
```

The summary MUST be derived from the same typed findings used to render report-v4.

This requirement prevents the text summary, `summary-v1`, and GitHub Action outputs from disagreeing with detailed Arid output.

---

# **7. Summary Counting Definitions**

V2.2 MUST use the following counting definitions.

## **7.1 Files analyzed**

`files` retains the existing report-v4 meaning: the number of successfully processed source files represented by the scan report.

## **7.2 Source lines**

`source_lines` retains the existing report-v4 meaning.

## **7.3 Analyzed lines**

`analyzed_lines` retains the existing report-v4 meaning.

## **7.4 Duplicate groups**

`duplicate_groups` retains the existing report-v4 meaning: the number of reportable duplicate findings after baseline and focus policy.

## **7.5 Duplicate lines**

`duplicate_lines` retains the existing report-v4 metric and arithmetic.

V2.2 MUST NOT independently recalculate duplicate-line coverage from rendered locations.

## **7.6 Duplication percent**

`duplication_percent` retains the existing report-v4 metric and arithmetic.

## **7.7 Total occurrences**

`occurrences` MUST equal:

```text
sum(finding.occurrences for every reportable finding)
```

This is a count of duplicate-group occurrence memberships.

It is NOT defined as the number of globally unique physical source intervals across all findings.

## **7.8 Files with duplicates**

`files_with_duplicates` MUST equal the number of distinct reported location paths appearing in one or more reportable findings.

Each path contributes at most one to this count regardless of how many findings or occurrences touch it.

When no duplicate findings are reportable:

```text
files_with_duplicates = 0
```

## **7.9 Files-with-duplicates percentage**

Human text MAY display the derived percentage:

```text
files_with_duplicates / files * 100
```

when `files > 0`.

The machine contract does not need to duplicate this derivable percentage as a stored field.

---

# **8. Context Breakdown Contract**

Every reportable finding contributes exactly one group to one Context bucket:

```text
declarative
executable
mixed
```

The sum of Context bucket counts MUST equal `duplicate_groups`.

Context percentages in human output MUST use:

```text
context_bucket_groups / duplicate_groups * 100
```

Percentages MUST be presented as group share, not duplicate-line share.

When `duplicate_groups == 0`, the Context breakdown MAY be omitted from human text rather than rendering meaningless zero-percentage rows.

`summary-v1` MUST still expose deterministic zero counts for all Context buckets.

---

# **9. Scope Breakdown Contract**

Every reportable finding contributes exactly one group to one Scope bucket:

```text
module
class
function
mixed
```

The sum of Scope bucket counts MUST equal `duplicate_groups`.

Scope percentages in human output MUST use group share.

V2.2 MUST NOT reinterpret structural scope or introduce additional scope categories.

When `duplicate_groups == 0`, the Scope breakdown MAY be omitted from human text.

`summary-v1` MUST expose zero counts for all Scope buckets.

---

# **10. Distribution Breakdown Contract**

Every reportable finding contributes exactly one group to one Distribution bucket:

```text
same-file
cross-file
hybrid
```

The sum of Distribution bucket counts MUST equal `duplicate_groups`.

Distribution percentages in human output MUST use group share.

V2.2 MUST reuse the existing finding distribution classification rather than recompute file relationships independently.

When `duplicate_groups == 0`, the Distribution breakdown MAY be omitted from human text.

`summary-v1` MUST expose zero counts for all Distribution buckets.

---

# **11. Human Summary Presentation**

Every completed normal text scan that reaches report rendering MUST end with a compact summary derived from the final reportable finding set.

The existing trailing lines:

```text
Found N duplicate groups.
N duplicate lines (P%).
```

SHOULD be replaced by the richer summary rather than duplicated beneath it.

The summary MUST include at least:

```text
Files analyzed
Files with duplicates
Source lines
Analyzed lines
Duplicate groups
Occurrences
Duplicate lines
Duplication
```

A representative shape is:

```text
Summary

┌───────────────────────┬────────────────┐
│ Files analyzed        │          2,927 │
│ Files with duplicates │    418 (14.28%)│
│ Source lines          │        418,632 │
│ Analyzed lines        │        301,447 │
│ Duplicate groups      │          5,558 │
│ Occurrences           │         12,084 │
│ Duplicate lines       │         48,297 │
│ Duplication           │         11.54% │
└───────────────────────┴────────────────┘
```

Exact spacing is presentation-level and MAY differ so long as the table remains deterministic, legible, and covered by golden tests.

Human integer values SHOULD use thousands separators for readability.

The summary MUST remain concise enough to be useful in a terminal and CI log.

---

# **12. Human Breakdown Presentation**

When one or more reportable duplicate groups exist, normal text output MUST include a breakdown table containing Context, Scope, and Distribution.

A representative shape is:

```text
Breakdown

┌──────────────┬─────────────┬────────┬─────────┐
│ Dimension    │ Value       │ Groups │ Percent │
├──────────────┼─────────────┼────────┼─────────┤
│ Context      │ executable  │  3,201 │  57.60% │
│              │ declarative │  2,117 │  38.09% │
│              │ mixed       │    240 │   4.32% │
├──────────────┼─────────────┼────────┼─────────┤
│ Scope        │ function    │  3,842 │  69.13% │
│              │ module      │  1,491 │  26.83% │
│              │ class       │    117 │   2.10% │
│              │ mixed       │    108 │   1.94% │
├──────────────┼─────────────┼────────┼─────────┤
│ Distribution │ cross-file  │  4,021 │  72.35% │
│              │ same-file   │  1,411 │  25.39% │
│              │ hybrid      │    126 │   2.27% │
└──────────────┴─────────────┴────────┴─────────┘
```

Rows within each dimension MUST use a fixed documented order rather than sorting dynamically by count.

The required order is:

```text
Context:      executable, declarative, mixed
Scope:        function, module, class, mixed
Distribution: cross-file, same-file, hybrid
```

Stable categorical order is preferred over count-based ordering because it keeps repeated scans visually comparable.

---

# **13. Semantic Color Contract**

V2.2 MUST add color to summary presentation only through the existing text color policy:

```text
--color auto
--color always
--color never
```

No new color option or theme system is permitted.

Color MUST be semantic rather than decorative.

The intended semantic mapping is:

- headings and structural labels: existing heading/bold treatment;
- Context values: existing classification color;
- Scope values: existing classification color;
- Distribution values: existing distribution color;
- duplicate-group and occurrence emphasis: existing group/count emphasis where appropriate;
- nonzero duplicate totals: existing problem color where appropriate;
- zero duplicate state: existing success color where appropriate;
- ordinary corpus totals and table borders: neutral/default terminal color.

V2.2 MUST NOT color percentages green/red based on arbitrary notions of healthy or unhealthy duplication.

Color MUST NOT introduce severity semantics.

`--color never` and non-color environments MUST produce plain deterministic text with no ANSI escape sequences.

---

# **14. Hotspots Contract**

V2.2 MUST add an objective `Hotspots` section for reportable duplicate findings.

A hotspot is a file path participating in one or more reportable duplicate groups.

For each hotspot path:

```text
groups      = number of distinct reportable findings containing at least one location in that path
occurrences = number of reported finding locations in that path across all reportable findings
```

If one hybrid finding contains multiple occurrences in the same file:

- that finding contributes `1` to the path's `groups` count;
- every occurrence in that file contributes to the path's `occurrences` count.

Hotspots MUST NOT use duplicate-line counts unless a future release explicitly defines interval-union semantics for per-file duplicate lines.

This avoids accidental double-counting from overlapping findings.

## **14.1 Hotspot ordering**

Hotspots MUST sort deterministically by:

1. `groups` descending;
2. `occurrences` descending;
3. `path` ascending.

## **14.2 Human hotspot limit**

Human text MUST display at most the top **5** hotspot paths.

The limit is intentionally fixed for v2.2 to keep normal output concise.

V2.2 MUST NOT add a hotspot-count tuning flag without a demonstrated requirement.

When no findings are reportable, the Hotspots section MAY be omitted.

A representative shape is:

```text
Hotspots

┌──────────────────────────────────┬────────┬─────────────┐
│ Path                             │ Groups │ Occurrences │
├──────────────────────────────────┼────────┼─────────────┤
│ src/api/generated.py             │     47 │          83 │
│ src/services/orders.py           │     31 │          42 │
│ tests/helpers/factories.py       │     27 │          51 │
└──────────────────────────────────┴────────┴─────────────┘
```

---

# **15. `--summary-only` Contract**

V2.2 MUST add:

```text
--summary-only
```

`--summary-only` is a normal-scan presentation modifier.

It MUST NOT be a new detector mode.

It MUST NOT change:

- discovery;
- parsing;
- normalization;
- duplicate detection;
- baseline filtering;
- focus filtering;
- finding identity;
- metrics;
- exit status;
- `--no-fail-on-findings` behavior;
- worker behavior;
- suppression behavior.

## **15.1 Text behavior**

For primary text output, `--summary-only` MUST suppress individual `DUP001` finding blocks and render only the compact summary/breakdown/hotspot presentation plus the existing normal text `Total time:` footer when the scan is complete.

The same derived summary values MUST appear whether individual findings were rendered or suppressed.

## **15.2 JSON behavior**

For primary JSON output:

```bash
arid . --summary-only --json
arid . --summary-only --format json
```

MUST emit `summary-v1`, not report-v4.

## **15.3 Markdown and SARIF**

V2.2 MUST NOT silently invent summary-only variants of existing Markdown or SARIF contracts.

`--summary-only` with primary Markdown or SARIF output MUST be rejected with a clear configuration error unless the requirements are explicitly amended before implementation.

## **15.4 Configuration**

`--summary-only` MUST remain CLI-only in v2.2.

It MUST NOT be added to `[tool.arid]`.

This avoids project configuration silently hiding detailed findings from ordinary developer invocations.

---

# **16. Supplemental Report Interaction**

`--summary-only` applies to the **primary output only**.

Existing supplemental `--report FORMAT=PATH` behavior MUST remain unchanged.

For example:

```bash
arid . --summary-only --report json=artifacts/arid.json
```

MUST allow:

- compact human summary on primary stdout; and
- the existing full report-v4 JSON in `artifacts/arid.json`.

Likewise, existing Markdown and SARIF supplemental reports MUST retain their existing full finding-oriented semantics.

V2.2 MUST NOT change the meaning of existing `--report json=PATH` to `summary-v1` merely because `--summary-only` is present.

Direct file output for `summary-v1` is not required in v2.2.

Users and tools may redirect primary JSON stdout when a standalone summary document is required.

---

# **17. `summary-v1` Machine Contract**

V2.2 MUST publish:

```text
schemas/summary-v1.schema.json
```

`summary-v1` MUST be deterministic, versioned, compact, and sufficient for tools or coding agents to understand the high-level duplicate profile without parsing every finding location.

The top-level document MUST contain:

```text
schema_version
tool_version
complete
analysis
errors
files
files_with_duplicates
source_lines
analyzed_lines
duplicate_groups
occurrences
duplicate_lines
duplication_percent
context
scope
distribution
hotspots
```

## **17.1 Schema version**

The schema version MUST be:

```json
"schema_version": 1
```

## **17.2 Analysis metadata**

`summary-v1.analysis` MUST expose the effective scan settings needed to interpret summary results.

At minimum it MUST include:

```text
min_lines
ignore_comments
ignore_docstrings
ignore_imports
ignore_signatures
same_file
hidden
ignore_files
exclude
baseline_enabled
focus
virtual_source
keep_going
```

`ignore_files` MUST describe the effective ignore-file traversal policy:

```text
true  = ignore-file-derived traversal filters are honored
false = --no-ignore-files is active
```

This is a new summary contract and is not constrained to the older report-v4 `analysis` shape.

## **17.3 Errors and completeness**

`summary-v1` MUST include:

```text
complete
errors
```

so a tool does not mistake partial keep-going analysis for a complete scan.

Structured source-processing errors MUST use the same underlying error representation already used by report-v4 rather than inventing a second error taxonomy.

## **17.4 Breakdown objects**

`summary-v1.context` MUST contain integer counts for:

```text
executable
declarative
mixed
```

`summary-v1.scope` MUST contain integer counts for:

```text
function
module
class
mixed
```

`summary-v1.distribution` MUST contain integer counts for:

```text
cross_file
same_file
hybrid
```

Machine keys use snake_case even where human text uses hyphenated labels.

Percentages for breakdown buckets are intentionally omitted because they are deterministic derivatives of counts and `duplicate_groups`.

## **17.5 Hotspots**

`summary-v1.hotspots` MUST contain the same deterministic top-five hotspot ranking used by human summary output.

Each entry MUST contain:

```text
path
groups
occurrences
```

The array length MUST be between `0` and `5`.

## **17.6 Timing exclusion**

`summary-v1` MUST NOT contain elapsed timing.

The v2.1 rule that volatile timing is human presentation only remains in force.

---

# **18. Representative `summary-v1` Document**

A representative complete document is:

```json
{
  "schema_version": 1,
  "tool_version": "2.2.0",
  "complete": true,
  "analysis": {
    "min_lines": 4,
    "ignore_comments": true,
    "ignore_docstrings": true,
    "ignore_imports": true,
    "ignore_signatures": true,
    "same_file": true,
    "hidden": false,
    "ignore_files": true,
    "exclude": [],
    "baseline_enabled": false,
    "focus": [],
    "virtual_source": null,
    "keep_going": false
  },
  "errors": [],
  "files": 2927,
  "files_with_duplicates": 418,
  "source_lines": 418632,
  "analyzed_lines": 301447,
  "duplicate_groups": 5558,
  "occurrences": 12084,
  "duplicate_lines": 48297,
  "duplication_percent": 11.54,
  "context": {
    "executable": 3201,
    "declarative": 2117,
    "mixed": 240
  },
  "scope": {
    "function": 3842,
    "module": 1491,
    "class": 117,
    "mixed": 108
  },
  "distribution": {
    "cross_file": 4021,
    "same_file": 1411,
    "hybrid": 126
  },
  "hotspots": [
    {
      "path": "src/api/generated.py",
      "groups": 47,
      "occurrences": 83
    }
  ]
}
```

The numbers above are illustrative only.

---

# **19. Determinism Contract**

V2.2 MUST preserve deterministic output for all new machine-readable state.

The following MUST NOT affect `summary-v1` bytes for logically identical scans:

- worker count;
- worker completion order;
- filesystem traversal arrival order;
- hash-map iteration order;
- terminal color capability;
- elapsed execution time.

Hotspot ordering MUST use the explicit ordering contract in this document.

Breakdown objects MUST serialize in a fixed field order chosen by the implementation and frozen by golden tests/schema examples.

Human text MUST also use stable row order.

---

# **20. Baseline Interaction**

When a baseline is active during a normal scan, summary values MUST describe the **active reportable findings after baseline acceptance is removed**.

Accepted baseline debt MUST NOT appear in:

- `duplicate_groups`;
- `occurrences`;
- files-with-duplicates counts;
- Context breakdown;
- Scope breakdown;
- Distribution breakdown;
- Hotspots.

This is consistent with normal report-v4 finding output.

Baseline administrative modes remain unchanged.

`--summary-only` MUST NOT apply to `--baseline-status`, `--prune-baseline`, or `--write-baseline`.

---

# **21. Focus Interaction**

When `--focus` is active, summary values MUST describe the same focused group set used by report-v4.

Because existing focus semantics retain every occurrence of a selected group, a focused finding MAY contain locations outside the focused path.

Therefore:

- `files_with_duplicates` MAY include paths outside the explicit focus paths;
- Hotspots MAY include paths outside the explicit focus paths;
- occurrence totals include all retained occurrences of focused groups.

This behavior is required for consistency with existing focus semantics and MUST be documented clearly.

---

# **22. Incomplete / Keep-Going Scans**

When `--keep-going` produces a partial report:

- summary derivation MUST use the partial report's successfully prepared corpus and reportable findings;
- `summary-v1.complete` MUST be `false`;
- `summary-v1.errors` MUST contain the structured source-processing errors;
- process exit status MUST remain `2`;
- machine output MUST remain deterministic;
- the normal `Total time:` footer MUST remain absent under the existing v2.1 exit-2 timing contract.

Human summary output SHOULD make incompleteness visually explicit rather than presenting partial metrics as an ordinary complete scan.

A fatal failure that prevents report construction MUST continue to use existing operational error behavior and does not need to fabricate a summary document.

---

# **23. Zero-Finding Behavior**

When a complete scan has zero reportable duplicate groups:

- `duplicate_groups = 0`;
- `occurrences = 0`;
- `duplicate_lines = 0`;
- `duplication_percent` retains existing zero semantics;
- `files_with_duplicates = 0`;
- all summary breakdown counts are `0`;
- `hotspots` is empty;
- text summary MUST clearly communicate that no duplicate code was found;
- Breakdown and Hotspots tables MAY be omitted from text to avoid zero-only noise;
- `summary-v1` MUST remain structurally complete with zero counts.

---

# **24. GitHub Action Integration**

The official GitHub Action MUST expose the following new outputs:

```text
occurrences
files-with-duplicates
summary-json
```

## **24.1 `occurrences`**

The value MUST equal the `summary-v1.occurrences` count for the same scan.

## **24.2 `files-with-duplicates`**

The value MUST equal `summary-v1.files_with_duplicates` for the same scan.

## **24.3 `summary-json`**

The output MUST expose the compact deterministic summary document in a form safely consumable by subsequent GitHub Actions steps.

The Action MUST NOT require a second Arid detector run solely to produce these outputs.

Existing Action outputs and failure-policy behavior MUST remain backward compatible.

V2.2 does not require changing the default job-summary behavior beyond what is necessary to expose the new compact data safely.

---

# **25. Human Output and Existing Formats**

The rich summary is a normal text presentation feature.

Existing full JSON, Markdown, and SARIF report semantics remain unchanged unless `--summary-only` explicitly selects the new `summary-v1` JSON contract.

Normal scan examples:

```text
arid .                         -> detailed text findings + rich summary + Total time
arid . --summary-only          -> rich summary only + Total time
arid . --json                  -> report-v4 JSON
arid . --summary-only --json   -> summary-v1 JSON
arid . --format markdown       -> existing Markdown report
arid . --format sarif          -> existing SARIF report
```

No normal machine format gains volatile color or timing data.

---

# **26. Performance Requirements**

Summary derivation MUST be linear in the number of already-built report findings and locations.

V2.2 MUST NOT:

- rerun detection;
- reread source files;
- reparse Python;
- renormalize source;
- rerun discovery;
- perform Git operations;
- allocate a repository-wide secondary source model merely for summary presentation.

The expected additional cost is one deterministic aggregation pass over reportable findings plus text/JSON rendering.

Normal-scan performance regression qualification MUST compare v2.2 against the stable 2.1 release on established benchmark corpora.

No material unexplained detector-path regression is acceptable.

---

# **27. CLI Validation Requirements**

The CLI MUST reject invalid combinations clearly.

At minimum:

- `--summary-only` is valid only for normal scan mode;
- `--summary-only` MUST conflict with terminal administrative modes;
- `--summary-only --format markdown` MUST fail clearly;
- `--summary-only --format sarif` MUST fail clearly;
- `--summary-only --json` MUST be valid;
- `--summary-only --format json` MUST be valid;
- `--summary-only --color auto|always|never` MUST be valid only when primary output is text;
- existing `--report` supplemental report combinations MUST remain valid where normal scan mode already permits them.

---

# **28. Machine Schema Requirements**

`schemas/summary-v1.schema.json` MUST:

- use JSON Schema conventions consistent with existing Arid schemas;
- reject unknown top-level fields unless Arid's existing schema policy requires otherwise;
- define all required fields explicitly;
- constrain `schema_version` to `1`;
- constrain counts to non-negative integers;
- constrain `duplication_percent` consistently with existing report semantics;
- constrain hotspot arrays to at most 5 entries;
- define hotspot object fields and required ordering-independent semantics;
- define the complete analysis object;
- reuse compatible structured error definitions or shapes without changing error-v1 semantics;
- include representative schema validation fixtures.

The schema file MUST be published with release artifacts/source exactly as other Arid-owned schemas are published.

---

# **29. Documentation Requirements**

Before the first v2.2 prerelease, README/reference documentation MUST explain:

- the new rich Summary section;
- Context/Scope/Distribution breakdown meaning;
- group-share percentage semantics;
- total occurrence semantics;
- files-with-duplicates semantics;
- Hotspots ranking and top-five limit;
- `--summary-only`;
- `summary-v1`;
- the difference between `--json` and `--summary-only --json`;
- baseline interaction;
- focus interaction;
- incomplete keep-going behavior;
- semantic color behavior;
- GitHub Action outputs.

Documentation MUST avoid implying that high or low percentages represent universal code-quality thresholds.

---

# **30. Validation Requirements**

V2.2 validation MUST include at least:

```text
summary overall-count fixtures
occurrence aggregation fixtures
files-with-duplicates distinct-path fixtures
Context bucket fixtures
Scope bucket fixtures
Distribution bucket fixtures
bucket-sum invariants
human percentage formatting
zero-finding text behavior
colored summary output
--color never ANSI absence
hotspot group counting
hotspot occurrence counting
hybrid same-path hotspot counting
hotspot tie ordering
hotspot top-five truncation
--summary-only detailed-finding suppression
--summary-only exit-status equivalence
--summary-only baseline equivalence
--summary-only focus equivalence
--summary-only keep-going partial output
summary-v1 schema validation
summary-v1 worker determinism
summary-v1 timing absence
summary-v1 zero-finding document
summary-v1 incomplete document
supplemental report-v4 preservation under --summary-only
report-v4 byte/semantic compatibility where required
Markdown compatibility
SARIF compatibility
GitHub Action output extraction
GitHub Action no-second-scan invariant
real-world regression validation
performance regression validation
```

---

# **31. Release and Compatibility Requirements**

V2.2 MUST follow the established prerelease qualification discipline.

A prerelease stage MUST exist only when it reduces a distinct release risk.

The release process MUST verify:

- supported platform builds;
- PyPI installation;
- standalone artifacts;
- `summary-v1` schema publication;
- deterministic machine output from published artifacts;
- human summary rendering from published artifacts;
- GitHub Action integration;
- report-v4 compatibility;
- baseline compatibility;
- detector regression behavior;
- performance regression behavior.

Stable promotion MUST remain product-code-free after the final qualified prerelease unless a release-blocking defect requires another qualified prerelease.

---

# **32. Acceptance Criteria**

Arid 2.2 is functionally complete when all of the following are true:

1. normal text scans render a compact Summary with the required overall metrics;
2. normal text scans with findings render deterministic Context, Scope, and Distribution breakdowns;
3. normal text scans with findings render deterministic top-five Hotspots;
4. semantic color obeys the existing `--color` contract;
5. `--summary-only` suppresses detailed findings without changing analysis or exit status;
6. `--summary-only --json` emits deterministic `summary-v1`;
7. `summary-v1` validates against its published schema;
8. summary counts match the same final reportable finding set used by report-v4;
9. baseline and focus semantics remain unchanged;
10. incomplete keep-going summaries remain explicitly incomplete and exit `2`;
11. supplemental reports remain full existing report formats;
12. GitHub Action exposes the three new outputs without a second detector run;
13. report-v4, baseline-v1, error-v1, suppression-status-v1, and path-explanation-v1 remain compatible;
14. real-world validation passes;
15. no material unexplained performance regression exists.

The release-completion statement MUST be true:

> **Arid 2.2 preserves the focused exact detector while turning its existing duplicate evidence into a concise, deterministic summary that developers can understand at a glance and tools or coding agents can consume without parsing every finding.**
