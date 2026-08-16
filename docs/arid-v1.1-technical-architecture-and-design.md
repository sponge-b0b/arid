# **Arid v1.1 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Version scope:** This document defines the technical delta targeted for Arid **1.1.0**.

---

# **1. Purpose**

The v1 technical architecture remains the base implementation contract for Arid.

This document defines only the architectural changes required to implement the v1.1 requirements:

- explicit output formats
- colored text output
- Markdown output
- SARIF output
- baseline enforcement
- official pre-commit integration

Unless explicitly changed here, the architecture described by:

```text
docs/arid-v1-technical-architecture-and-design.md
```

remains in force.

The v1 detection pipeline is intentionally preserved.

---

# **2. Architectural Principle**

V1.1 adds behavior **after exact duplicate detection**.

The core pipeline remains:

```text
Python source
    ↓
discovery
    ↓
Python parse + tokenize
    ↓
normalization
    ↓
corpus
    ↓
suffix array + LCP
    ↓
maximal exact duplicate groups
```

V1.1 extends the tail of the pipeline:

```text
DuplicateGroup[]
    │
    ├── baseline write mode ───────────────→ baseline file
    │
    └── optional baseline enforcement
                    ↓
             active groups
                    ↓
                Report
       ┌────────────┼────────────┐
       ↓            ↓            ↓            ↓
      text         JSON       Markdown      SARIF
```

Neither output selection nor baseline enforcement may change normalized equality, suffix-array construction, LCP computation, maximal-repeat extraction, or canonical group construction.

---

# **3. Major V1.1 Technical Decisions**

| Area | V1.1 decision |
| --- | --- |
| Detection model | unchanged from v1 |
| Output selector | `--format text|json|markdown|sarif` |
| Default format | `text` |
| JSON compatibility | existing `--json` retained; schema v3 preserved |
| Color selector | `--color auto|always|never` |
| Color scope | text output only |
| Styling model | semantic roles using `anstyle` |
| Terminal adaptation | `anstream` for capability-aware stdout behavior |
| Markdown | concrete deterministic renderer |
| SARIF | concrete typed SARIF 2.1.0 renderer |
| Baseline storage | deterministic JSON |
| Baseline group identity | SHA-256 of exact normalized block encoding |
| Baseline occurrence identity | project-relative path + per-path multiplicity |
| Baseline line numbers | never part of identity |
| Baseline enforcement | filter fully accepted groups before `Report` construction |
| Baseline configuration | optional `[tool.arid].baseline` plus CLI override |
| Baseline writing | explicit CLI-only `--write-baseline` mode |
| Pre-commit | official `language: system` hook; whole-project scan |
| Reporter abstraction | concrete modules; no plugin/trait registry |
| Cache | none |
| Git-diff engine | none |

Cargo dependency versions remain controlled by `Cargo.toml` and `Cargo.lock`; this design does not require hard-coding dependency versions into the architecture document.

---

# **4. Repository Structure**

The current `report.rs` already owns the stable report model plus human and JSON rendering. Adding multiple concrete renderers makes a module split justified in v1.1.

The intended structure is:

```text
src/
├── baseline.rs
├── report/
│   ├── mod.rs
│   ├── text.rs
│   ├── json.rs
│   ├── markdown.rs
│   └── sarif.rs
└── ... existing v1 modules
```

Responsibilities:

```text
baseline.rs
    baseline schema
    exact group fingerprinting
    baseline read/write
    normalization compatibility check
    active-group filtering

report/mod.rs
    Report
    Finding
    Location
    report construction
    canonical report ordering

report/text.rs
    plain/colored developer output

report/json.rs
    existing schema-v3 serialization

report/markdown.rs
    Markdown document rendering

report/sarif.rs
    SARIF 2.1.0 model + rendering
```

This is a concrete split, not a reporter framework.

V1.1 MUST NOT add:

- a `Reporter` trait solely because four formats exist
- a renderer registry
- runtime renderer discovery
- plugins
- dependency injection

A simple `match OutputFormat` is sufficient.

---

# **5. CLI Model**

## **5.1 Output format**

`cli.rs` SHOULD define a Clap value enum conceptually equivalent to:

```rust
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
    Sarif,
}
```

The CLI adds:

```text
--format <FORMAT>
```

The field SHOULD be optional at raw parse time so the application can distinguish an explicit selector from the default.

Resolution:

```text
--json present
    → JSON

--format present
    → selected format

neither present
    → text
```

`--json` and `--format` conflict at the CLI layer.

The existing `--json` flag remains visible and supported.

---

## **5.2 Color**

`cli.rs` SHOULD define:

```rust
pub enum ColorWhen {
    Auto,
    Always,
    Never,
}
```

The raw CLI field SHOULD be:

```text
Option<ColorWhen>
```

rather than eagerly defaulting to `Auto`, because the application must know whether the user explicitly supplied `--color`.

An explicit `--color` conflicts with an explicit non-text output format.

Environment variables are considered only when color was not explicitly selected.

---

## **5.3 Baseline options**

The CLI adds:

```text
--baseline <PATH>
--write-baseline <PATH>
```

They conflict with each other.

`--write-baseline` SHOULD also conflict with output-presentation options that have no meaning in baseline-generation mode, including:

```text
--format
--json
--color
--show-source
```

This keeps baseline generation an explicit administrative operation rather than a second report format.

`--baseline` remains compatible with all normal report formats.

---

# **6. Configuration Model**

The persistent v1 settings remain unchanged except for one optional key:

```toml
[tool.arid]
baseline = "arid-baseline.json"
```

The resolved baseline path MUST use the same project-root model used by the rest of Arid configuration.

A relative configured baseline path is resolved relative to the project root containing the active `pyproject.toml`.

CLI precedence:

```text
--baseline <PATH>
        ↓
[tool.arid].baseline
        ↓
no baseline
```

The following remain CLI-only:

```text
format
color
write-baseline
workers
```

`format` and `color` are intentionally not project policy. Different consumers of the same repository may require text, JSON, Markdown, or SARIF without editing project configuration.

---

# **7. Run Pipeline**

The current v1 orchestration is conceptually:

```text
discover
→ prepare
→ corpus
→ detect
→ build report
→ render
```

V1.1 becomes:

```text
discover
→ prepare
→ corpus
→ detect
→ baseline branch
→ build report
→ render selected format
```

Conceptually:

```rust
let groups = detect_duplicates(...)?;

if let Some(path) = cli.write_baseline {
    write_baseline(path, &corpus, &groups, normalization)?;
    return success;
}

let active_groups = if let Some(path) = resolved_baseline {
    load_and_filter(path, &corpus, &groups, normalization)?
} else {
    groups
};

let report = build_report(&corpus, &active_groups, ...)?;
let output = render(report, format, render_context)?;
```

The detector is called exactly once.

Baseline enforcement does not require reparsing or redetection.

---

# **8. Library Determinism and Terminal Context**

The current public `run(&Cli)` returns a `RunResult` containing a rendered `String` and exit status.

TTY-dependent behavior must not make library tests or programmatic callers nondeterministic.

V1.1 SHOULD therefore introduce an additional execution entry point conceptually equivalent to:

```rust
pub struct RunContext {
    pub text_color_capable: bool,
}

pub fn run(cli: &Cli) -> Result<RunResult, String> {
    run_with_context(cli, RunContext::non_terminal())
}

pub fn run_with_context(
    cli: &Cli,
    context: RunContext,
) -> Result<RunResult, String>;
```

The exact names MAY differ, but the invariant is required:

> Library execution must have a deterministic non-terminal default; the binary entry point owns real stdout capability detection.

Explicit `--color always` still overrides the non-terminal default.

`main.rs` SHOULD determine stdout capability and pass it into orchestration rather than allowing deep reporting code to query global process state.

---

# **9. Color Resolution**

Color resolution occurs once before text rendering.

The decision function MUST be pure after receiving:

- explicit CLI choice, if any
- relevant environment values
- stdout terminal/color capability

Precedence:

```text
explicit --color
        ↓
NO_COLOR
        ↓
CLICOLOR_FORCE
        ↓
CLICOLOR
        ↓
terminal capability
```

Exact rules:

```text
--color always       → enabled
--color never        → disabled
--color auto         → terminal capability

NO_COLOR non-empty   → disabled
CLICOLOR_FORCE != 0  → enabled
CLICOLOR=0           → disabled
otherwise            → terminal capability
```

An explicitly supplied CLI value wins over every environment variable.

For non-text formats, color resolution is skipped entirely.

---

# **10. Color Implementation**

V1.1 SHOULD use the Rust CLI `anstyle` / `anstream` ecosystem rather than hand-building escape sequences.

`anstyle` provides semantic ANSI styles with low coupling.

`anstream` provides stream adaptation including non-terminal stripping and Windows console handling.

Arid still owns its own product-level environment precedence; terminal adaptation is delegated to the stream layer.

The text renderer SHOULD define a small fixed stylesheet:

```rust
struct TextStyles {
    diagnostic: Style,
    heading: Style,
    path: Style,
    location: Style,
    secondary: Style,
    source_gutter: Style,
}
```

The enabled stylesheet maps approximately to:

```text
diagnostic     bold yellow
heading        bold
path           bold cyan
location       cyan
secondary      dim
source_gutter  dim
```

The disabled stylesheet uses plain styles.

Python source text itself is never syntax-highlighted by Arid.

The renderer MUST reset styling after every styled fragment so one field cannot leak style into later text.

---

# **11. Text Renderer**

`render_human` SHOULD be renamed or wrapped as `render_text` while preserving the existing output contract when color is disabled.

Text rendering accepts explicit rendering context rather than reading environment variables itself.

Conceptually:

```rust
struct TextRenderOptions {
    color: bool,
    baseline_active: bool,
}
```

The renderer keeps existing finding ordering and content.

For a source location:

```text
  test_observability_manager.py:66-74
      66 |             super().__init__()
```

semantic roles are:

```text
test_observability_manager.py  path
66-74                          location
66                             source-gutter
|                              source-gutter
Python source                  plain
```

No color choice changes the underlying text, source range, or finding order.

When baseline enforcement removes every finding, text MAY say:

```text
No new duplicate code found.
```

instead of implying that the repository contains no duplication at all.

---

# **12. JSON Renderer**

The v1 `Report` structure and JSON schema version `3` remain the JSON contract for v1.1.

`render_json` continues to pretty-serialize `Report` deterministically.

Baseline enforcement happens before `build_report`, so JSON needs no baseline-specific fields in v1.1.

Without a baseline, schema-v3 semantics are unchanged.

With a baseline, the report contains active duplicate groups only.

The complete scan still determines:

```text
files
source_lines
analyzed_lines
```

while duplicate metrics are calculated from active groups passed to `build_report`.

This avoids a gratuitous JSON schema revision.

---

# **13. Markdown Renderer**

Markdown rendering consumes the same `Report` used by text and JSON.

The renderer SHOULD produce a compact document shaped approximately as:

```markdown
# Arid duplicate-code report

## DUP001 — 9 duplicated lines

**Context:** executable  
**Scope:** function  
**Occurrences:** 2 across 2 files _(cross-file)_

### `test_observability_manager.py:66-74`

```python
super().__init__()
self.flush_count = 0
...
```
```

The actual implementation MUST correctly handle Markdown delimiter characters in user-controlled paths and source.

### Inline code

Path/location code spans MUST choose a delimiter long enough to avoid collision with backticks contained in a path.

### Source fences

When `--show-source` is active, the renderer MUST choose a backtick fence longer than the longest contiguous backtick run in the source snippet, with a minimum fence length of three.

The fenced language identifier SHOULD be:

```text
python
```

Source fences contain raw source only, without an artificial line-number gutter.

Markdown uses no ANSI styling.

---

# **14. SARIF Renderer**

V1.1 emits SARIF 2.1.0 through concrete Serde-owned structures in `report/sarif.rs`.

Arid SHOULD NOT expose SARIF types throughout the detection or report model.

Conversion occurs only at the renderer boundary:

```text
Report
    ↓
SARIF document
```

The top-level document contains one run with Arid as the tool driver.

The driver MUST identify:

```text
name: Arid
version: current Arid package version
rule: DUP001
```

One Arid `Finding` becomes one SARIF result.

Mapping:

```text
Finding.code              → ruleId
canonical first Location  → locations[0]
remaining Locations       → relatedLocations
Finding.lines             → properties.lines
Finding.context           → properties.context
Finding.scope             → properties.scope
Finding.occurrences       → properties.occurrences
Finding.files             → properties.files
Finding.distribution      → properties.distribution
```

Arid SHOULD omit SARIF severity/level rather than manufacturing a severity not present in the product model.

The result message SHOULD remain descriptive, for example:

```text
9 duplicated lines across 2 occurrences in 2 files
```

If `--show-source` is active, a location region MAY include a snippet containing the original source.

---

# **15. SARIF Paths**

SARIF artifact locations are URI strings rather than native OS paths.

Arid MUST convert project-relative paths deterministically:

- `/` is used as the separator on every platform
- path bytes requiring URI escaping are percent-encoded from UTF-8
- no current-working-directory prefix is embedded when a project-relative path is available

V1.1 SHOULD implement the small required path-to-relative-URI conversion directly rather than adding a large URL abstraction solely for SARIF output.

The same source path MUST render identically on Linux, macOS, and Windows after project-relative normalization.

---

# **16. Baseline Schema**

`baseline.rs` owns a versioned JSON schema independent of the report JSON schema.

Conceptually:

```rust
struct Baseline {
    version: u8,
    normalization: BaselineNormalization,
    groups: Vec<BaselineGroup>,
}

struct BaselineNormalization {
    ignore_comments: bool,
    ignore_docstrings: bool,
    ignore_imports: bool,
    ignore_signatures: bool,
}

struct BaselineGroup {
    fingerprint: String,
    lines: u32,
    occurrences: Vec<BaselinePathCount>,
}

struct BaselinePathCount {
    path: String,
    count: u32,
}
```

Initial baseline schema version:

```text
1
```

The file contains no source snippets and no physical line numbers.

Example shape:

```json
{
  "version": 1,
  "normalization": {
    "ignore_comments": true,
    "ignore_docstrings": true,
    "ignore_imports": true,
    "ignore_signatures": true
  },
  "groups": [
    {
      "fingerprint": "sha256:...",
      "lines": 9,
      "occurrences": [
        {"path": "tests/a.py", "count": 1},
        {"path": "tests/b.py", "count": 1}
      ]
    }
  ]
}
```

Pretty serialization is deterministic because vectors are sorted before serialization rather than relying on map iteration order.

---

# **17. Baseline Group Fingerprint**

The baseline fingerprint is computed from the exact normalized line sequence represented by a duplicate group.

Because every occurrence in a valid duplicate group is exactly equal after normalization, the canonical first occurrence is sufficient to obtain the block content.

V1.1 SHOULD use SHA-256 through a small direct dependency such as `sha2`.

The input encoding MUST be stable and unambiguous.

Conceptually:

```text
ASCII "arid-baseline-group-v1\0"
+ effective line count as fixed-width big-endian integer
+ for each normalized line:
    byte length as fixed-width big-endian integer
    exact normalized UTF-8 bytes
```

Length-prefixing prevents different line segmentations from producing the same byte stream accidentally.

The serialized fingerprint is lowercase hexadecimal with an algorithm prefix:

```text
sha256:<64 lowercase hex digits>
```

The fingerprint intentionally excludes:

- physical line numbers
- file paths
- structural context
- structural scope
- occurrence distribution
- source snippets

Those properties do not define duplicate identity.

---

# **18. Baseline Path Identity**

Baseline paths MUST be normalized relative to the resolved project root.

Serialized paths use `/` separators on every platform.

A baseline-writing scan that cannot represent a duplicate occurrence relative to the project root SHOULD fail rather than write a machine-specific absolute path into a source-controlled baseline.

Path identity is intentionally conservative:

```text
same block + same project-relative path  → same path identity
same block + renamed/moved file          → new path identity
```

Arid does not invoke Git rename detection or fuzzy path matching.

Within one path, baseline identity tracks occurrence multiplicity rather than physical positions.

---

# **19. Baseline Construction**

Baseline writing runs after detection and before report construction.

Algorithm:

1. iterate canonical duplicate groups
2. compute each exact group fingerprint
3. count occurrences by normalized project-relative path
4. create one `BaselineGroup`
5. sort path-count entries by path
6. sort groups by fingerprint, then line count, then occurrence vector
7. pretty-serialize baseline JSON
8. write atomically

Atomic writing SHOULD use a temporary file in the destination directory followed by rename/replacement.

A successful `--write-baseline` operation returns success even when groups exist because the requested operation was baseline creation, not enforcement.

---

# **20. Baseline Enforcement Algorithm**

Baseline enforcement occurs after detection and before `build_report`.

First validate:

- supported baseline schema version
- required fields
- normalization settings equal current normalization settings

Then construct an in-memory lookup keyed by group fingerprint.

For each current duplicate group:

1. compute its fingerprint
2. build current per-path occurrence counts
3. find the baseline group with the same fingerprint
4. compare every current path/count with the baseline path/count

A group is fully accepted only when:

```text
for every current path:
    baseline contains path
    AND current_count <= baseline_count
```

Otherwise the group is active.

The output of baseline enforcement is simply:

```rust
Vec<DuplicateGroup>
```

containing active groups.

An active group is not trimmed to only new occurrences. All current occurrences remain in the group so the resulting `DUP001` finding continues to explain what the new copy duplicates.

This preserves the existing `Report`, `Finding`, and `Location` types.

---

# **21. Baseline Semantics Examples**

Baseline:

```text
fingerprint F
    tests/a.py  1
    tests/b.py  1
```

Current:

```text
fingerprint F
    tests/a.py  1
    tests/b.py  1
```

Result:

```text
fully baselined
```

Current after unrelated lines are inserted above both blocks:

```text
fingerprint F
    tests/a.py  1
    tests/b.py  1
```

Result:

```text
fully baselined
```

Current after another copy is added to `tests/a.py`:

```text
fingerprint F
    tests/a.py  2
    tests/b.py  1
```

Result:

```text
active group
```

Current after the occurrence in `tests/b.py` moves to `tests/c.py`:

```text
fingerprint F
    tests/a.py  1
    tests/c.py  1
```

Result:

```text
active group
```

Current after the duplicate content changes identically in both files:

```text
fingerprint G
```

Result:

```text
active group
```

No approximate matching occurs.

---

# **22. Report and Metric Semantics Under Baselines**

The existing `build_report(corpus, groups, options)` design is retained.

Without a baseline:

```text
groups = all detected groups
```

With a baseline:

```text
groups = active groups only
```

Therefore:

- file count remains the full analyzed corpus
- physical source lines remain the full analyzed corpus
- analyzed effective lines remain the full analyzed corpus
- duplicate group count reflects active groups
- duplicate line metrics reflect active groups
- findings contain active groups

This is an explicit enforcement-mode view, not a redefinition of detection.

To inspect every duplicate regardless of baseline, run without baseline enforcement.

---

# **23. Error Model**

Baseline-specific errors SHOULD be typed internally and converted into Arid's existing user-facing error path.

Required errors include:

- baseline file not found
- baseline read failure
- invalid JSON
- unsupported baseline schema version
- normalization mismatch
- invalid fingerprint
- duplicate baseline group identity
- duplicate path entry within a baseline group
- non-project-relative occurrence during baseline write
- atomic write/rename failure

All baseline errors produce exit status `2`.

Malformed baselines MUST NOT silently degrade to an empty baseline.

---

# **24. Pre-commit Integration**

The repository SHOULD add:

```text
.pre-commit-hooks.yaml
```

The initial hook SHOULD be conceptually:

```yaml
- id: arid
  name: arid
  description: Check Python source for duplicate code with Arid
  entry: arid .
  language: system
  pass_filenames: false
  always_run: true
```

The exact manifest syntax MUST be validated against supported pre-commit behavior during implementation.

`language: system` is deliberate:

- Arid is already distributed as an installable CLI
- the hook should use the exact Arid executable selected by the developer/CI environment
- pre-commit should not trigger an implicit Rust source build
- pre-commit should not hide another package-management layer

`pass_filenames: false` is required because duplicate detection is repository-global.

`always_run: true` ensures the whole-project check remains eligible even when pre-commit's changed-file classification would otherwise skip it.

---

# **25. Testing Strategy**

V1 tests remain mandatory.

V1.1 adds focused tests rather than duplicating the detector suite.

## **25.1 CLI tests**

Cover:

- every `--format` value
- default text format
- `--json` compatibility
- `--json` / `--format` conflict
- every `--color` value
- explicit color with non-text format rejection
- `--baseline`
- `--write-baseline`
- baseline/write conflict
- baseline-writing presentation conflicts

## **25.2 Color tests**

Use deterministic injected context and environment inputs.

Cover:

- auto terminal enabled
- auto non-terminal disabled
- explicit always over `NO_COLOR`
- explicit never over `CLICOLOR_FORCE`
- `NO_COLOR`
- `CLICOLOR_FORCE`
- `CLICOLOR=0`
- no ANSI in disabled text
- no ANSI in JSON, Markdown, or SARIF

Do not make unit tests depend on the test runner's real TTY state.

## **25.3 Renderer tests**

Use stable expected-output fixtures for:

- text plain
- text colored
- text with source
- Markdown
- Markdown with source
- SARIF
- SARIF with source

Repeated rendering of one `Report` must be byte-identical.

## **25.4 Baseline tests**

Cover:

- deterministic baseline serialization
- line-number movement
- path rename
- per-path multiplicity increase
- new file occurrence
- removed duplicate
- changed normalized block
- normalization mismatch
- malformed baseline
- empty baseline
- atomic baseline replacement
- same-file multiple occurrences
- cross-platform path separator normalization

## **25.5 End-to-end tests**

Cover the same source scan rendered as text, JSON, Markdown, and SARIF and verify equivalent finding/exit outcomes.

Baseline end-to-end tests MUST prove:

```text
existing debt → exit 0
new occurrence → exit 1
scan error → exit 2
```

---

# **26. Validation Harness Changes**

The existing real-world validation campaign remains authoritative for detector correctness.

V1.1 SHOULD extend it with a small targeted integration layer rather than multiplying every corpus by every output format.

At minimum validation MUST exercise:

- text output with color disabled
- redirected text under auto color contains no ANSI
- Markdown output
- SARIF output
- JSON compatibility
- baseline creation
- unchanged baseline enforcement
- deliberate new duplicate against baseline

Full Black/Django/mypy/Rich/path-case detector validation remains unchanged.

Worker determinism remains applicable to every machine-readable format where practical.

---

# **27. Benchmark Strategy**

No new detector benchmark architecture is required.

The existing benchmark harness remains the performance source of truth.

V1.1 SHOULD add only targeted reporting/baseline micro-measurements if a real regression is observed; speculative Criterion infrastructure is not required.

Release qualification continues to benchmark the exact published standalone artifact.

The medium/large Pylint speedup gate remains in force.

If v1.1 materially regresses cold-scan performance relative to the established v1 results, the regression MUST be investigated before release rather than hidden behind caching.

---

# **28. Release Tooling Delta**

The v1 release tooling currently treats the v1 roadmap as the active release document.

Before the first `1.1.0` release candidate, release tooling MUST be updated so a 1.1 release changes the v1.1 roadmap rather than rewriting the historical v1 roadmap.

`release.sh` SHOULD derive the active roadmap from the target release series.

Required behavior:

```text
1.0.x  → docs/arid-v1-release-roadmap.md
1.1.x  → docs/arid-v1.1-release-roadmap.md
```

The implementation SHOULD be structured so adding a later minor release does not require rewriting release logic throughout the script.

The qualification harness MUST use the same derived active roadmap when verifying an RC-to-stable metadata-only transition.

The historical v1 roadmap MUST remain stable after v1.1 development begins.

---

# **29. Dependency Policy**

V1.1 SHOULD add only dependencies that directly support the committed features.

Expected direct additions are limited to:

```text
anstyle   semantic ANSI styles
anstream  terminal-aware output adaptation
sha2      deterministic cryptographic baseline fingerprints
```

SARIF and Markdown SHOULD be implemented with existing standard-library and Serde capabilities unless implementation evidence justifies a small focused dependency.

No template engine, Markdown parser, syntax-highlighting library, async runtime, plugin framework, or Git library is required.

---

# **30. Implementation Order**

The preferred inside-out implementation order is:

1. CLI output/color domain enums and resolution
2. report module split without changing behavior
3. plain text compatibility tests
4. semantic color rendering
5. baseline domain model and fingerprinting
6. baseline write/read/enforcement pipeline
7. Markdown renderer
8. SARIF renderer
9. pre-commit manifest and documentation
10. validation harness extensions
11. release/qualification tooling update for the v1.1 roadmap
12. complete regression and release qualification

Each layer should be made correct according to the v1.1 model before downstream callers are adapted.

---

# **31. Architecture Acceptance Criteria**

The v1.1 architecture is satisfied when:

- parser-specific types remain isolated in the Python frontend
- detector code has no knowledge of output format, color, baseline storage, Markdown, SARIF, or pre-commit
- baseline filtering occurs only after canonical duplicate groups exist
- `Report` remains the shared concrete report model
- JSON schema v3 remains valid
- concrete renderers do not require a reporter framework
- color decisions are injected rather than discovered deep inside report code
- terminal auto-detection is testable without a real TTY
- baseline fingerprints are deterministic and line-number independent
- baseline path serialization is platform-stable
- baseline matching is exact and non-fuzzy
- pre-commit invokes a whole-project scan
- no new feature requires a Python interpreter at runtime
- v1 performance and determinism invariants remain intact

V1.1 should make Arid easier to adopt and easier to read without making the duplicate detector more complicated than it needs to be.
