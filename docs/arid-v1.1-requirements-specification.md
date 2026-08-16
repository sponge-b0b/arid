# **Arid v1.1 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Version scope:** This document defines backward-compatible functionality targeted for Arid **1.1.0**.

---

# **1. Purpose**

Arid 1.1 improves adoption, developer usability, and CI integration without changing what Arid considers duplicate code.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The Arid v1 requirements specification remains the base product contract for behavior shipped in `1.0.0`.

This document defines only requirements added or changed for v1.1. Unless explicitly changed here, behavior specified by:

```text
docs/arid-v1-requirements-specification.md
```

remains in force.

---

# **2. Product Principle**

Arid 1.1 MUST improve how developers consume and enforce duplicate-code findings without broadening the detector itself.

The v1 duplicate definition remains unchanged:

> Two or more sufficiently long contiguous Python source regions that become identical after Arid's configured preprocessing rules are applied.

V1.1 MUST NOT introduce a second definition of duplication.

In particular, v1.1 MUST NOT add:

- semantic clone detection
- structural clone matching
- identifier-renaming clone detection
- fuzzy AST similarity
- embedding-based similarity
- general similarity scoring
- multi-language detection

Structural metadata remains descriptive only and MUST NOT participate in duplicate identity.

---

# **3. V1.1 Goals**

Arid 1.1 MUST add:

1. improved colored presentation for textual output
2. an explicit output-format model
3. Markdown output
4. SARIF output
5. baseline-based incremental adoption
6. official pre-commit integration

These features MUST preserve:

- Python-only scope
- exact duplication after normalization
- `DUP001`
- deterministic detection and reporting
- precise physical source locations
- existing v1 normalization behavior
- existing v1 configuration behavior unless explicitly extended
- existing JSON shape and meaning when baseline mode is not active
- existing exit-status semantics unless baseline behavior explicitly changes the set of active findings

---

# **4. Explicit V1.1 Non-Goals**

Arid 1.1 MUST NOT add:

- semantic or fuzzy duplicate detection
- identifier normalization
- structural clone detection
- autofix
- automated refactoring
- refactoring recommendations
- general lint rules
- code transformation
- multi-language support
- a plugin system
- a reporter plugin registry
- a language abstraction framework
- persistent analysis caching
- Git-diff-only detection
- Git-aware incremental indexing
- an LSP server
- editor-specific integration
- HTML output
- framework-specific severity or classification
- duplication severity levels
- historical duplication analytics
- a debt-management system
- a general waiver or policy engine

These features require separate evidence and product decisions and MUST NOT be introduced merely to support the v1.1 goals.

---

# **5. Detection Contract**

V1.1 MUST preserve the v1 detection model.

The following MUST remain unchanged unless a defect requires a backward-compatible correction:

- Python-aware preprocessing
- comment handling
- docstring handling
- import handling
- function-signature handling
- whitespace normalization
- suppression barriers
- normalized line interning
- generalized suffix-array detection
- LCP construction
- maximal-repeat extraction
- contained-match handling
- same-file overlap handling
- deterministic canonicalization
- structural context semantics
- structural scope semantics
- occurrence distribution semantics
- duplicate metrics

Adding output formats, terminal styling, baselines, or integrations MUST NOT alter whether two normalized source regions are duplicates.

---

# **6. Output Format Model**

V1.1 MUST provide one explicit output-format option:

```text
--format <FORMAT>
```

Supported formats MUST be:

```text
text
json
markdown
sarif
```

The default MUST be:

```text
text
```

Examples:

```bash
arid .
arid --format text .
arid --format json .
arid --format markdown .
arid --format sarif .
```

Output format describes the representation of a report, not its destination.

For example:

```bash
arid --format text . > report.txt
```

remains valid even though stdout is not a terminal.

Format names MUST NOT encode destination assumptions such as `terminal`, `console`, or `tty`.

Output-format selection SHOULD remain a CLI concern rather than project configuration. A repository SHOULD NOT force every consumer to receive the same representation.

---

# **7. Existing `--json` Compatibility**

The existing:

```text
--json
```

option MUST remain supported in v1.1.

It MUST be behaviorally equivalent to:

```text
--format json
```

Existing scripts using:

```bash
arid --json .
```

MUST continue to work.

Supplying `--json` together with `--format` MUST be treated as invalid CLI usage rather than requiring precedence rules for redundant output selectors.

V1.1 does not require deprecating `--json`.

---

# **8. Text Output**

`text` is Arid's normal developer-oriented textual report.

The underlying information and canonical ordering of v1 human-readable findings MUST remain stable unless changed by a documented v1.1 presentation requirement.

Text output MAY contain ANSI terminal styling only when color is enabled.

Text output MUST remain fully understandable when color is disabled.

Color MUST NOT be required to interpret:

- diagnostic identity
- source paths
- line ranges
- finding metadata
- source snippets
- summary metrics

---

# **9. Terminal Color**

## **9.1 Color option**

V1.1 MUST provide:

```text
--color <WHEN>
```

Supported values MUST be:

```text
auto
always
never
```

Default behavior MUST be equivalent to:

```text
--color auto
```

Examples:

```bash
arid --color auto .
arid --color always .
arid --color never .
```

Color configuration applies only to `--format text`.

Structured and document formats MUST NOT contain ANSI escape sequences. Therefore JSON, Markdown, and SARIF MUST always be ANSI-free.

An explicitly supplied `--color` together with a non-text `--format` MUST be rejected as invalid CLI usage rather than silently ignored.

---

## **9.2 Automatic color**

With `--color auto`, Arid MUST emit ANSI styling only when text output is directed to a terminal that supports colored presentation.

Redirected or piped text output MUST remain uncolored by default.

For example:

```bash
arid . > report.txt
```

MUST produce ANSI-free text unless color is explicitly forced.

---

## **9.3 Environment conventions**

When `--color` is not explicitly supplied, Arid SHOULD honor established terminal-color environment conventions.

The v1.1 precedence MUST be:

1. explicit `--color`
2. non-empty `NO_COLOR` disables color
3. nonzero `CLICOLOR_FORCE` forces color
4. `CLICOLOR=0` disables color
5. terminal capability detection

Environment color variables MUST have no effect on JSON, Markdown, or SARIF.

---

## **9.4 Semantic styling**

Color implementation MUST use semantic presentation roles rather than scattering raw color choices throughout report construction.

The initial text presentation SHOULD distinguish roles such as:

```text
diagnostic
heading
path
location
secondary
source-gutter
source
```

The initial visual mapping SHOULD be approximately:

```text
diagnostic     bold yellow
heading        bold
path           bold cyan
location       cyan
secondary      dim
source-gutter  dim
source          terminal default foreground
```

The exact ANSI implementation is a technical-design concern.

---

## **9.5 No severity through color**

Color MUST NOT assign or imply severity that is absent from Arid's data model.

In particular, Arid MUST NOT use different warning/error colors to imply that:

- executable duplication is more severe than declarative duplication
- cross-file duplication is more severe than same-file duplication
- mixed findings are inherently worse than other findings

Structural context, structural scope, and occurrence distribution remain descriptive facts.

---

# **10. Source Snippet Presentation**

When `--show-source` is used with text output, styling SHOULD improve the visual separation between locations and source snippets.

For example:

```text
Occurrences: 2 across 2 files (cross-file)

  test_observability_manager.py:66-74
      66 |             super().__init__()
      67 |             self.flush_count = 0
      68 |             self.shutdown_count = 0
```

The path and physical source range SHOULD receive stronger visual emphasis than the source body.

Source line numbers and gutter separators SHOULD use secondary styling.

V1.1 MUST NOT require Python syntax highlighting in terminal output.

The Python source itself SHOULD remain unmodified and SHOULD use the terminal's normal foreground presentation.

---

# **11. Markdown Output**

V1.1 MUST support:

```text
--format markdown
```

Markdown output MUST represent the same active duplicate findings and scan-level metrics available through Arid's report model.

Markdown output SHOULD optimize for:

- GitHub rendering
- pull-request or issue comments
- CI job summaries
- saved `.md` reports
- copy/paste into documentation

Markdown MUST NOT contain ANSI escape sequences.

Finding metadata SHOULD use standard Markdown constructs such as headings, emphasis, inline code, and lists.

When source display is requested, Markdown SHOULD use fenced Python code blocks containing the original source without an artificial line-number gutter. The location heading already supplies the physical source range, and preserving raw Python inside the fence allows the consuming Markdown renderer to provide syntax highlighting.

Arid MUST NOT implement its own Markdown syntax highlighter.

Markdown output MUST be deterministic. Repeated scans of identical inputs and configuration MUST produce byte-identical Markdown output.

---

# **12. SARIF Output**

V1.1 MUST support:

```text
--format sarif
```

SARIF output exists to integrate Arid with standards-based code-scanning systems.

SARIF MUST NOT alter detection or finding semantics.

Each Arid duplicate group MUST remain fundamentally one `DUP001` result.

The canonical earliest occurrence SHOULD be represented as the primary result location. Remaining duplicate occurrences SHOULD be represented as related locations so the full clone group remains discoverable to consumers.

SARIF SHOULD expose objective Arid metadata including:

- effective duplicate lines
- structural context
- structural scope
- occurrence count
- distinct-file count
- occurrence distribution

Arid MUST NOT invent a severity solely to satisfy SARIF. If the selected SARIF representation permits omission of severity, Arid SHOULD omit it.

SARIF MUST preserve original physical source locations and project-relative paths where possible.

When `--show-source` is requested, SARIF MAY include source snippets associated with reported regions.

SARIF output MUST be deterministic and MUST NOT contain volatile timestamps, random identifiers, temporary paths, or ANSI escape sequences.

---

# **13. Baselines**

## **13.1 Purpose**

V1.1 MUST support baseline-based incremental adoption.

The purpose of a baseline is:

> Allow a project to acknowledge duplicate relationships that already exist while continuing to fail when duplicate debt increases.

Baselines enable established repositories to adopt Arid without requiring all existing duplicate code to be removed first.

A baseline MUST NOT redefine whether source is duplicated. Detection runs normally first; baseline comparison determines which detected groups are already accepted for enforcement.

---

## **13.2 Baseline CLI**

V1.1 MUST provide an enforcement option:

```text
--baseline <PATH>
```

and an explicit baseline-writing mode:

```text
--write-baseline <PATH>
```

Examples:

```bash
arid . --write-baseline arid-baseline.json
arid . --baseline arid-baseline.json
```

`--baseline` and `--write-baseline` MUST conflict.

`--write-baseline` is an administrative operation. A successful baseline write MUST exit with status `0` even when the scan contains duplicate groups. Scan or write errors MUST exit with status `2`.

The baseline path MAY also be configured persistently through:

```toml
[tool.arid]
baseline = "arid-baseline.json"
```

An explicit CLI baseline path MUST override the configured path.

There MUST be no default baseline file. Baseline enforcement is opt-in.

---

## **13.3 Baseline file**

The v1.1 baseline file MUST be deterministic JSON with an explicit baseline schema version.

It MUST be suitable for source control and MUST NOT store source snippets or complete duplicated source text.

A baseline MUST record enough information to identify:

- the exact normalized duplicate block
- the number of effective duplicate lines
- the project-relative files containing accepted occurrences
- the accepted occurrence multiplicity within each file
- the normalization settings under which the baseline was created

Baseline entries and per-file occurrence records MUST be canonically sorted.

---

## **13.4 Exact block fingerprint**

Baseline group identity MUST derive from the exact normalized duplicate block using a deterministic cryptographic fingerprint.

The fingerprint MUST be independent of physical source line numbers.

Two differently normalized blocks MUST NOT intentionally share one baseline identity.

No fuzzy, semantic, structural, or approximate baseline matching is permitted.

The technical design MUST define a stable fingerprint encoding so the same normalized block produces the same fingerprint across supported platforms.

---

## **13.5 Occurrence identity**

V1.1 baseline occurrence identity MUST use:

```text
normalized block fingerprint
+
project-relative file path
+
accepted occurrence multiplicity in that file
```

Physical line numbers MUST NOT participate in baseline identity.

Therefore:

- inserting unrelated lines above a duplicate does not invalidate the baseline
- moving an occurrence to another physical line in the same file does not invalidate the baseline when file-level multiplicity is unchanged
- adding another occurrence of the same duplicate block to a file increases multiplicity and is new duplicate debt
- adding the duplicate block to a new file is new duplicate debt
- renaming or moving a file changes its path identity and is conservatively treated as new until the baseline is intentionally regenerated

The baseline intentionally tracks accepted multiplicity per path rather than exact physical positions inside a file. Replacing one occurrence with another occurrence of the same normalized block in the same file while keeping multiplicity unchanged does not increase baseline debt and MAY remain accepted.

This model is deterministic, line-number independent, and intentionally avoids fuzzy contextual matching.

---

## **13.6 Baseline enforcement**

For each currently detected duplicate group, Arid MUST compare the current per-path occurrence multiplicity with the matching baseline group.

A current duplicate group is fully baselined only when:

- a baseline group with the same exact fingerprint exists
- every current path exists in the baseline group
- the current occurrence count for every path is less than or equal to the accepted count for that path

A group MUST be active when any of those conditions is false.

When an active group contains both previously accepted and newly introduced occurrences, Arid MUST report the complete current group so the new duplicate remains understandable in context.

Fully baselined groups MUST be omitted from normal finding output while baseline enforcement is active.

Running Arid without `--baseline` MUST continue to report all detected duplicate groups.

---

## **13.7 Baseline metrics and JSON compatibility**

Baseline enforcement SHOULD filter duplicate groups before the existing report is built.

When baseline enforcement is active:

- `files`, `source_lines`, and `analyzed_lines` continue to describe the complete scan
- duplicate finding fields and duplication metrics describe active groups only
- fully baselined groups are absent from the report

This design MUST preserve the existing JSON document shape in v1.1 and MUST NOT require an unrelated JSON schema revision merely to support baseline enforcement.

Text output MAY use baseline-aware wording such as:

```text
No new duplicate code found.
```

when no active groups remain.

---

## **13.8 Normalization compatibility**

A baseline MUST record the normalization settings that determine normalized block content:

- `ignore-comments`
- `ignore-docstrings`
- `ignore-imports`
- `ignore-signatures`

Arid MUST reject baseline enforcement with a clear configuration error when those settings differ from the settings recorded in the baseline.

Discovery scope, `min-lines`, and `same-file` MAY legitimately change without making the baseline unreadable; changes in those settings naturally affect which current groups exist and therefore which groups become active.

---

## **13.9 No general suppression database**

A baseline MUST NOT become a general policy or waiver system.

V1.1 baselines MUST NOT add:

- owners
- expiration dates
- severity
- approval workflow
- ticket references
- per-finding remediation state
- arbitrary user metadata
- fuzzy matching
- manually authored similarity rules

Native source suppression remains the mechanism for intentionally excluding source from duplicate detection.

---

# **14. Pre-commit Integration**

V1.1 MUST provide an official supported pre-commit integration.

The integration MUST preserve whole-project duplicate detection.

It MUST NOT pretend that analyzing only filenames supplied by pre-commit is equivalent to scanning the project. A changed file can duplicate unchanged source elsewhere in the repository.

The official hook SHOULD therefore conceptually run:

```bash
arid .
```

and MUST disable automatic staged-filename passing.

The initial supported integration SHOULD use the Arid executable already installed in the user's environment rather than introducing a hidden package-manager or source-build workflow inside the hook.

Installation documentation MUST state that Arid must be installed and available on `PATH` for the official hook.

---

# **15. Configuration**

V1.1 MUST preserve `[tool.arid]` as the primary persistent configuration namespace.

The baseline path MAY be persistent project configuration.

The following SHOULD remain CLI-oriented in v1.1:

```text
format
color
write-baseline
```

This avoids repository configuration unexpectedly forcing a developer, CI job, or downstream tool to consume one presentation format.

The default invocation:

```bash
arid .
```

MUST remain useful with no v1.1 configuration.

---

# **16. JSON Compatibility**

The existing v1 JSON format is a public machine-readable interface.

V1.1 MUST NOT change the JSON schema merely because additional output formats exist.

Without baseline enforcement, `--format json` and `--json` MUST retain the existing v1 JSON shape and meaning.

With baseline enforcement, the same JSON shape MUST describe active groups after baseline filtering.

Fields MUST NOT silently acquire unrelated meanings.

If implementation discovers that an incompatible JSON change is genuinely required, that change MUST be separately justified and the schema version MUST be incremented rather than silently modifying schema v3.

---

# **17. Exit Status**

The base v1 exit-status contract remains:

```text
0  successful scan with no active findings
1  successful scan with active findings
2  usage, configuration, parsing, I/O, baseline, or other scan error
```

Without a baseline, v1.1 MUST preserve existing behavior.

With a baseline, exit status `1` MUST mean that at least one currently detected group is not fully accepted by the baseline.

Fully baselined duplication alone MUST NOT cause exit status `1`.

A successful `--write-baseline` operation MUST return `0`.

Output format MUST NOT alter exit status.

For the same scan and baseline state, text, JSON, Markdown, and SARIF MUST produce equivalent success/finding/error outcomes.

---

# **18. Determinism**

All new v1.1 functionality MUST preserve Arid's deterministic behavior.

For identical source files, paths, configuration, baseline, Arid version, and output format, Arid MUST produce stable:

- finding identity
- occurrence ordering
- baseline acceptance decisions
- text content
- Markdown output
- JSON output
- SARIF output
- baseline files

Terminal color auto-detection MAY change ANSI presentation according to the output environment, but it MUST NOT change underlying finding content or ordering.

---

# **19. Performance**

V1.1 features MUST NOT materially compromise Arid's established performance characteristics.

In particular:

- color styling MUST occur only during reporting
- Markdown rendering MUST occur only after detection
- SARIF rendering MUST occur only after detection
- baseline comparison MUST operate on already-detected canonical groups
- output formatting MUST NOT trigger additional Python parsing
- output formatting MUST NOT trigger duplicate detection a second time

The existing v1 performance qualification remains applicable to the detection pipeline.

V1.1 release qualification SHOULD compare performance against the established v1 baseline and investigate meaningful unexplained regressions.

Persistent caching MUST NOT be introduced merely to compensate for overhead added by v1.1 reporting features.

---

# **20. Architecture Boundaries**

V1.1 SHOULD preserve the existing architectural separation:

```text
discovery
    ↓
Python frontend
    ↓
normalization
    ↓
corpus
    ↓
detection
    ↓
baseline enforcement
    ↓
report model
    ↓
presentation
```

The new functionality belongs after exact duplicate detection.

Conceptually:

```text
DuplicateGroup[]
    │
    ├── optional baseline filtering
    │
    ▼
DuplicateReport
    ├── text renderer
    ├── JSON renderer
    ├── Markdown renderer
    └── SARIF renderer
```

Baseline comparison MAY require new domain types but MUST NOT alter normalized equality or the detector.

V1.1 MUST NOT introduce a generic reporter plugin system merely because multiple concrete renderers exist.

Concrete renderers are sufficient.

---

# **21. Backward Compatibility**

Arid 1.1 is a backward-compatible minor release.

The following existing usages MUST continue to work:

```bash
arid .
arid src tests
arid --json .
arid --show-source .
arid --exclude generated/** .
arid --workers 4 .
```

Existing `[tool.arid]` configuration MUST continue to work.

Existing suppression directives MUST continue to work.

Existing valid exit-status expectations MUST continue to work except when a user explicitly opts into baseline enforcement.

The default invocation MUST NOT require a baseline, output-format argument, or color argument.

---

# **22. V1.1 Release Criteria**

Arid 1.1 is ready for release when:

1. the v1 detection contract remains green
2. existing v1 tests remain green
3. `--format text` is the default
4. `--json` remains compatible with existing use
5. `--format json` is equivalent to `--json`
6. text color supports `auto`, `always`, and `never`
7. automatic color does not pollute redirected output
8. JSON contains no ANSI styling
9. Markdown contains no ANSI styling
10. SARIF contains no ANSI styling
11. Markdown findings preserve all essential duplicate information
12. SARIF findings preserve all essential duplicate locations
13. baseline creation is deterministic
14. unchanged baselined duplication does not fail enforcement
15. new duplicate content against a baseline does fail enforcement
16. a new path or increased per-path occurrence multiplicity is detected as new duplicate debt
17. unrelated line-number movement does not invalidate an otherwise unchanged baseline
18. baseline normalization mismatches fail clearly
19. official pre-commit integration preserves whole-project detection
20. deterministic-output tests cover all output formats
21. real-world validation remains green
22. benchmark qualification shows no unacceptable regression
23. published-artifact qualification passes

---

# **23. Deferred Beyond V1.1**

The following remain deferred rather than committed:

- HTML output
- duplication budgets
- Git-diff reporting
- persistent cache
- Git-aware incremental indexing
- LSP/editor integration
- non-UTF-8 PEP 263 codec expansion absent demonstrated compatibility need
- portable remote qualification records

Deferral does not imply eventual inclusion.

Each item requires demonstrated user value sufficient to justify its complexity.

---

# **24. Rejected Product Directions**

The following remain outside Arid's intended product direction unless the product itself is deliberately redefined in a future major design effort:

- semantic duplicate detection
- fuzzy clone detection
- renamed-variable clone detection
- embedding similarity
- structural similarity as duplicate identity
- multi-language support
- general linting
- automated duplicate removal
- automatic refactoring
- framework-specific severity
- generalized code-quality analysis

Arid's value depends partly on the strength and predictability of its definition:

> **If Arid reports duplicated code, the reported regions are exactly equal under documented Python-aware normalization rules.**

V1.1 MUST preserve that guarantee.
