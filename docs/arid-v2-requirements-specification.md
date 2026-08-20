# **Arid v2 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.0.0`  
**Version scope:** This document defines the product contract targeted for Arid **2.0.0**.

---

# **1. Purpose**

Arid 2.0 is a deliberate contract, automation, and integration release.

It strengthens Arid's public machine interfaces, makes whole-project duplicate detection easier to target and automate, improves failure and configuration transparency, and narrows accidental implementation exposure without changing the product's core definition of duplicate code.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The stable v1, v1.1, and v1.2 requirements remain the historical base contract. Unless this document explicitly changes a behavior, the existing contract remains in force.

The v2 theme is:

> **Cleaner contracts. Better automation. Same focused detector.**

---

# **2. Product Principle**

A major version gives Arid permission to make justified incompatible changes. It does not justify redesign for its own sake.

Arid 2.0 MUST preserve the product properties that remained stable throughout 1.x:

- Python-only scope
- exact duplication after documented normalization
- arbitrary contiguous source-block detection
- cross-file and same-file detection
- `DUP001` as the duplicate-code diagnostic
- deterministic findings, metrics, and ordering
- accurate physical source locations
- Python-aware normalization
- project configuration through `[tool.arid]`
- CI-friendly exit statuses
- text, JSON, Markdown, and SARIF output
- baseline-based incremental adoption
- pre-commit integration
- substantial performance advantage over Pylint duplicate detection

V2 MUST NOT broaden duplicate identity merely because a breaking release is available.

New automation features MUST operate around the same detector rather than create alternate definitions of duplication.

---

# **3. V2 Goals**

Arid 2.0 MUST:

1. introduce JSON report schema version `4`
2. rename the JSON report's top-level `version` field to `schema_version`
3. replace occurrence-distribution vocabulary `mixed` with `hybrid`
4. add a stable first-class fingerprint for each reported duplicate finding
5. expose stable finding identity through machine-readable integrations and SARIF
6. make report v4 self-describing with deterministic analysis metadata and scan completeness
7. add focused reporting that scans the whole corpus while reporting only groups relevant to selected paths
8. add opt-in keep-going analysis for file-local failures without allowing partial scans to masquerade as success
9. define a stable machine-readable operational-error contract
10. add safe baseline status and pruning operations without turning baselines into a general policy database
11. add an explicit successful-reporting mode that does not fail solely because findings exist
12. add explicit project-root/configuration control and configuration/discovery introspection
13. support one virtual Python source file supplied through standard input for pre-write or agent workflows
14. support writing multiple concrete report formats from one detection run
15. expose deterministic machine-readable capability discovery
16. provide an official GitHub Actions integration
17. preserve baseline schema version `1` and existing baseline compatibility
18. deliberately narrow Arid's supported Rust public API boundary
19. provide a clear 1.x-to-2.0 migration guide
20. preserve established detector semantics, configuration compatibility, determinism, and performance except where this specification explicitly changes them

---

# **4. Explicit V2 Non-Goals**

Arid 2.0 MUST NOT add merely because it is a major release:

- semantic clone detection
- fuzzy clone detection
- structural similarity as duplicate identity
- identifier-renaming clone detection
- embedding-based similarity
- multi-language duplicate detection
- general lint rules
- autofix or automated duplicate removal
- automated refactoring
- framework-specific severity or classification
- duplication severity levels
- a plugin system
- a reporter registry or framework
- HTML output
- persistent scan caching
- Git-aware incremental indexing
- built-in Git blame
- a watch daemon
- an MCP server
- LSP/editor integration
- historical duplication analytics
- a general waiver or policy engine
- a second proprietary configuration format
- another duplicate diagnostic beyond `DUP001`

The following ideas remain uncommitted and require separate evidence before inclusion in a later 2.x release:

- duplication budgets or percentage thresholds
- native Git-diff-oriented reporting
- persistent caching
- editor/LSP integration
- additional output formats
- long-running daemon/RPC operation
- MCP integration

Focused reporting is not Git-diff detection. It is a generic post-detection reporting primitive that other tools may use with any selected paths.

Deferral does not imply eventual inclusion.

---

# **5. Detection Contract**

Arid 2.0 MUST preserve the 1.x duplicate definition:

> Two or more sufficiently long contiguous Python source regions that become identical after Arid's configured preprocessing rules are applied.

V2 MUST continue to detect **exact duplication after normalization**.

The following MUST remain unchanged unless a correctness defect requires an explicitly documented fix:

- Python parsing semantics
- comment handling
- docstring handling
- import handling
- function-signature handling
- whitespace normalization
- suppression barriers
- normalized source-location mapping
- same-file overlap semantics
- maximal duplicate qualification
- deterministic group canonicalization
- structural context semantics
- structural scope semantics
- duplicate metrics

The detector MUST continue to ignore structural metadata when deciding whether source is duplicated.

Structural context and scope remain descriptive facts attached after exact duplicate identity is established.

Features such as focus filtering, baseline enforcement, multi-output rendering, GitHub integration, virtual input, and exit-status control MUST NOT create a second detection path or change duplicate identity.

---

# **6. Implementation Freedom**

The v2 product contract MUST specify externally observable behavior rather than freeze an internal algorithm unnecessarily.

Arid MUST retain:

- exact normalized duplicate semantics
- deterministic output
- required correctness behavior
- established performance expectations

The technical architecture MAY continue using the existing generalized suffix-array and LCP design, and SHOULD do so unless evidence justifies a change.

The v2 requirements MUST NOT make one internal detection algorithm a permanent public product contract when equivalent behavior can be preserved by another implementation.

Any future algorithm change MUST satisfy the same correctness, determinism, and performance gates before acceptance.

---

# **7. JSON Report Schema Version 4**

V2 MUST introduce:

```text
schemas/report-v4.schema.json
```

The existing:

```text
schemas/report-v3.schema.json
```

MUST remain unchanged as a historical compatibility artifact.

## **7.1 Schema version field**

The v1.x report field:

```json
"version": 3
```

MUST become:

```json
"schema_version": 4
```

The old top-level `version` field MUST NOT also be emitted in report v4.

`schema_version` describes the JSON report contract, not the Arid executable version.

## **7.2 Required v4 report metadata**

Report v4 MUST include deterministic top-level metadata sufficient to identify the emitting tool and whether the scan completed:

```text
schema_version
tool_version
complete
analysis
errors
```

`tool_version` MUST identify the Arid executable version.

`complete` MUST be `true` only when all source required by the scan was processed successfully.

`errors` MUST contain structured operational errors collected during an incomplete keep-going scan and MUST be empty for a complete scan.

## **7.3 Existing report meaning**

Except for changes explicitly defined by v2, report v4 MUST preserve the meaning of the existing report fields:

```text
files
source_lines
analyzed_lines
duplicate_groups
duplicate_lines
duplication_percent
findings
```

V2 MUST NOT rename other existing report fields merely for stylistic consistency.

## **7.4 Schema lifecycle**

Published report schemas are immutable historical contracts.

A future incompatible report format MUST receive a new schema version and a new schema file rather than silently rewriting report v4.

---

# **8. Self-Describing Analysis Metadata**

Report v4 MUST include an `analysis` object describing settings that materially affect corpus selection, normalization, detection, baseline filtering, or focus filtering.

At minimum the effective analysis metadata MUST represent:

```text
min_lines
ignore_comments
ignore_docstrings
ignore_imports
ignore_signatures
same_file
hidden
exclude
baseline_enabled
focus
```

The values MUST reflect effective settings after configuration and CLI precedence are resolved.

`focus` MUST use canonical project-relative path representations and MUST be deterministically ordered.

`exclude` MUST preserve effective pattern order because pattern ordering may affect ignore semantics.

Report metadata MUST NOT include volatile timestamps, temporary directories, process identifiers, machine-specific absolute paths, or other values that make otherwise equivalent reports unstable.

Execution strategy is not finding semantics. Resolved worker count MUST NOT be included in report v4 in a way that makes serial, explicit-worker, and automatic-worker reports differ solely because of worker selection.

`--show-source` MAY change location representation as already documented but MUST NOT change finding identity.

---

# **9. Occurrence Distribution Vocabulary**

V2 MUST replace the occurrence-distribution value:

```text
mixed
```

with:

```text
hybrid
```

The complete v2 occurrence-distribution vocabulary MUST be:

```text
same-file
cross-file
hybrid
```

`hybrid` means a duplicate group contains both same-file and cross-file occurrence relationships.

This rename applies wherever occurrence distribution is represented, including:

- JSON report output
- text output
- Markdown output
- SARIF properties
- Rust report-domain vocabulary

## **9.1 Structural `mixed` remains unchanged**

This change MUST NOT rename structural context or structural scope values.

The following remain valid:

```text
context: declarative | executable | mixed
scope: module | class | function | mixed
```

`mixed` remains correct when a finding aggregates multiple structural contexts or scopes.

Only occurrence distribution changes from `mixed` to `hybrid`.

---

# **10. Stable Finding Fingerprints**

V2 MUST add a stable public fingerprint to every reported duplicate finding.

The JSON finding field MUST be named:

```text
fingerprint
```

The fingerprint exists to support:

- stable CI result identity
- cross-run machine correlation
- SARIF result deduplication
- external integrations that need identity independent of physical line movement

## **10.1 Identity semantics**

A finding fingerprint MUST identify the exact normalized duplicate block itself.

It MUST be deterministic for the same normalized duplicate content and MUST be independent of:

- physical source line numbers
- source paths
- canonical occurrence ordering
- occurrence multiplicity
- structural context
- structural scope
- occurrence distribution
- source snippets
- output format
- terminal environment

Moving an otherwise unchanged duplicate block within or between files MUST NOT change its finding fingerprint.

Changing the normalized duplicate content MUST change the fingerprint except for the theoretical collision properties of the selected cryptographic hash.

## **10.2 Public fingerprint contract**

The fingerprint encoding MUST be explicitly versioned and documented before implementation is considered complete.

The technical architecture MUST define:

- the cryptographic hash algorithm
- the canonical byte encoding
- a versioned domain separator
- the serialized string representation

The initial v2 representation SHOULD use a deterministic SHA-256-based value with an explicit versioned Arid finding-fingerprint domain and a human-recognizable serialized form.

Once released, the fingerprint algorithm and encoding become a compatibility contract. A future incompatible fingerprint algorithm MUST use a new identity version rather than silently changing existing fingerprint meaning.

## **10.3 Baseline fingerprints remain separate**

V2 MUST NOT silently redefine the existing baseline-v1 fingerprint contract.

Baseline group fingerprints and report finding fingerprints serve related but distinct public contracts and MUST use independently versioned identity domains.

Implementations MAY share internal encoding helpers only when doing so does not couple the two public compatibility contracts.

The same duplicate block is not required to have the same serialized report fingerprint and baseline-v1 fingerprint.

---

# **11. SARIF Finding Identity**

SARIF output MUST continue to use SARIF 2.1.0 and the official SARIF schema.

V2 MUST expose the same stable Arid finding identity used by report v4 through the SARIF result representation.

The technical architecture MUST evaluate standard SARIF fingerprint facilities, including `partialFingerprints`, and use a standards-compatible representation suitable for result correlation by code-scanning consumers when practical.

The selected SARIF fingerprint property name and semantics MUST be stable and versioned.

Each SARIF result MUST map to the same logical fingerprint as the corresponding Arid report finding.

SARIF fingerprint identity MUST NOT depend on the primary-location choice.

The existing objective SARIF metadata remains required:

- duplicate lines
- structural context
- structural scope
- occurrence count
- distinct-file count
- occurrence distribution

SARIF MUST use `hybrid` for hybrid occurrence distribution while retaining `mixed` where structural context or scope is mixed.

Arid MUST continue not to invent severity solely to satisfy SARIF.

---

# **12. Focused Reporting**

V2 MUST add repeatable focused reporting:

```text
--focus <PATH>
```

Examples:

```bash
arid . --focus src/payments.py
arid . --focus src/payments.py --focus tests/payments
```

Focus MUST be a reporting filter, not a detection scope.

Arid MUST still discover, parse, normalize, and compare the complete scan corpus. A focused file MUST therefore still be compared with unchanged source elsewhere in the project.

A duplicate group is reportable under focus mode when at least one occurrence belongs to a file matched by at least one focus path.

When a group matches focus, Arid MUST report the complete duplicate group, including occurrences outside the focused paths, so the duplicate remains understandable in context.

Focus paths MAY identify files or directories. Directory focus applies recursively to corpus files beneath that directory.

Focus paths MUST be interpreted relative to the selected project root unless absolute paths are explicitly supported by the technical design and normalized to the same project-relative identity.

Every supplied focus path MUST match at least one disk or virtual Python source file in the scan corpus. An unmatched explicit focus selector MUST fail clearly rather than silently producing a misleading empty report.

When baseline enforcement and focus are both active, processing order MUST conceptually be:

```text
whole-project detection
    ↓
baseline enforcement
    ↓
focus filtering
    ↓
report construction
```

`files`, `source_lines`, and `analyzed_lines` continue to describe the complete processed corpus. Duplicate findings and duplicate metrics describe active groups after baseline and focus filtering.

Focus MUST NOT be added to `[tool.arid]` in v2. It is an invocation-specific reporting concern.

---

# **13. Keep-Going Analysis and Scan Completeness**

Default Arid behavior MUST remain fail-fast for source-processing errors.

V2 MUST add:

```text
--keep-going
```

`--keep-going` allows Arid to continue after independent file-local read, parse, or normalization failures when the remaining corpus can still be analyzed safely.

Global failures that make scan meaning unreliable MUST remain fatal and immediate. These include at minimum:

- invalid CLI usage
- invalid project configuration
- invalid explicit project root or configuration selection
- discovery failures that make corpus membership unreliable
- invalid baseline state
- output serialization/write failures
- internal invariant failures

A keep-going scan with one or more file-local failures MUST:

- analyze every remaining valid source file
- report findings derived only from successfully processed source
- set report-v4 `complete` to `false`
- include the collected structured errors in deterministic order
- return exit status `2`

A partial scan MUST NEVER return success or findings-only exit status merely because useful findings were produced.

`--no-fail-on-findings` MUST NOT override an incomplete scan's exit status `2`.

---

# **14. Machine-Readable Operational Errors**

V2 MUST define a stable operational-error object for automation and agent use.

At minimum each structured error MUST contain:

```text
kind
message
```

and MAY contain a project-relative `path` when the failure is associated with source or a project artifact.

The `kind` value is the stable machine classification. Human-readable `message` wording MUST NOT be treated as a stable parsing contract.

The initial stable error-kind vocabulary MUST distinguish at least:

```text
configuration
discovery
read
parse
normalization
baseline
output
internal
```

The technical architecture MAY refine these categories before implementation, but the final vocabulary MUST be explicit, tested, and documented before the first v2 prerelease that publishes the contract.

## **14.1 JSON fatal-error document**

V2 MUST publish a versioned schema for machine-readable fatal execution errors:

```text
schemas/error-v1.schema.json
```

When JSON output has been successfully selected and Arid encounters a post-CLI-parse fatal execution error before a report can be produced, stdout MUST contain a document conforming to this schema and the process MUST exit `2`.

The error document MUST include at minimum:

```text
schema_version
tool_version
error
```

CLI parser failures that occur before a valid output mode can be established MAY retain normal command-line usage diagnostics.

## **14.2 Keep-going error reuse**

Report-v4 `errors` MUST reuse the same logical operational-error object rather than define an unrelated second error taxonomy.

Machine consumers MUST be able to branch on `kind` without scraping diagnostic prose.

---

# **15. Exit-Status Control**

The default exit-status contract remains:

```text
0  successful complete scan with no active findings
1  successful complete scan with active findings
2  usage, configuration, parsing, I/O, incomplete scan, baseline, output, or internal error
```

V2 MUST add:

```text
--no-fail-on-findings
```

For a complete successful scan with this option:

```text
no active findings → 0
active findings    → 0
Arid error         → 2
```

The option exists for reporting-only CI and automation workflows and MUST NOT suppress genuine Arid failures.

It MUST NOT alter findings, metrics, report content, or baseline decisions.

It MUST NOT apply to administrative operations whose exit semantics are defined separately.

---

# **16. Baseline Compatibility and Maintenance**

Arid 2.0 MUST continue to read and enforce existing baseline schema version `1` files.

V2 MUST NOT require users to regenerate a valid 1.x baseline merely because they upgraded to 2.0.

The existing baseline contract remains:

```text
version = 1
normalization snapshot
exact normalized-block fingerprint
project-relative paths
accepted occurrence multiplicity per path
```

The existing file remains:

```text
schemas/baseline-v1.schema.json
```

and MUST remain unchanged unless a discovered schema defect can be corrected without changing the serialized baseline-v1 contract.

V2 MUST NOT introduce `baseline-v2` without a separately justified incompatible baseline requirement.

The report rename `version` → `schema_version` applies only to report v4. It MUST NOT be generalized to baseline-v1.

## **16.1 Baseline status**

V2 MUST add a read-only baseline inspection operation:

```text
--baseline-status <PATH>
```

It MUST distinguish at minimum:

- currently accepted duplicate debt
- active/new duplicate debt not fully accepted by the baseline
- stale baseline groups or path multiplicity that no longer exist in current source

The operation MUST NOT modify the baseline.

It MUST support deterministic human-readable output and deterministic JSON suitable for automation.

Its exit status MUST be:

```text
0  status completed and no active/new debt exists
1  status completed and active/new debt exists
2  baseline or scan error
```

## **16.2 Baseline pruning**

V2 MUST add safe pruning:

```text
--prune-baseline <PATH>
```

Pruning MAY remove only baseline acceptance that is stale relative to the current detected corpus.

It MUST NOT:

- add a new baseline group
- increase accepted path multiplicity
- accept newly introduced duplicate debt
- change normalization settings
- invent metadata

The baseline MUST be rewritten deterministically and atomically.

After pruning, exit status MUST be:

```text
0  prune completed and no active/new debt exists
1  prune completed but active/new debt remains
2  baseline, scan, or write error
```

`--write-baseline` remains the explicit administrative operation for replacing accepted debt with the complete current duplicate set.

Baseline maintenance MUST NOT evolve into owners, expiration dates, approvals, tickets, severity, arbitrary annotations, or a general waiver database.

---

# **17. Explicit Project Root and Configuration Control**

The default 1.x configuration-discovery behavior MUST remain available for compatibility.

V2 MUST additionally provide explicit control for monorepos, automation, and agent execution.

## **17.1 Exact configuration**

Add:

```text
--config <PATH>
```

It MUST select the exact `pyproject.toml` to use rather than performing ancestor discovery.

Unless an explicit project root is also provided, the selected configuration file's parent directory becomes the project root.

## **17.2 Disable project configuration**

Add:

```text
--no-config
```

It MUST disable `[tool.arid]` discovery and use built-in defaults plus explicit CLI overrides.

`--config` and `--no-config` MUST conflict.

## **17.3 Explicit project root**

Add:

```text
--project-root <PATH>
```

The project root controls project-relative path identity and resolution for features such as:

- configured excludes
- configured baseline paths
- focus paths
- report paths
- virtual stdin paths
- source locations

When `--project-root` is supplied without `--config` or `--no-config`, automatic configuration lookup MUST be anchored to that root rather than walking above it.

The technical architecture MUST define the exact interaction of an explicit root and exact config path and MUST reject contradictory combinations rather than guessing.

---

# **18. Configuration and Discovery Introspection**

V2 MUST make Arid's effective behavior inspectable without requiring a user or agent to infer it from source code.

## **18.1 Effective configuration**

Add:

```text
--show-config
```

It MUST resolve configuration and CLI precedence and report at minimum:

- selected project root
- selected configuration file or explicit no-config state
- effective detection and normalization settings
- effective hidden/exclude behavior
- effective baseline path or disabled state

`--show-config` MUST exit without running duplicate detection.

It MUST support deterministic text and JSON output. Markdown and SARIF are not required for this administrative operation.

## **18.2 Discovered source list**

Add:

```text
--list-files
```

It MUST perform configuration resolution and file discovery but MUST NOT parse or detect duplicates.

It MUST emit the exact Python files Arid would pass to source preparation, using deterministic project-relative ordering where possible.

It MUST support deterministic text and JSON output.

Explicit-file behavior, `.gitignore`, hidden-path handling, and Arid `exclude` semantics MUST remain consistent with normal scans.

These operations MUST make configuration and discovery easier to debug without introducing a verbose tracing subsystem into normal execution.

---

# **19. Virtual Source from Standard Input**

V2 MUST support analyzing one virtual Python source file supplied through standard input:

```text
--stdin-path <PATH>
```

Example:

```bash
cat proposed.py | arid . --stdin-path src/proposed.py --focus src/proposed.py
```

The supplied path MUST provide the virtual source's project identity and source locations.

The virtual path MUST resolve within the selected project root and MUST represent a Python source path supported by Arid.

If the virtual path corresponds to a disk file that would otherwise be present in the corpus, the virtual source MUST replace that disk file for the scan rather than create a second file with the same identity.

If the virtual path does not already exist, the virtual source MUST be added to the corpus as a new file.

The virtual source MUST:

- use the same Python parsing and normalization pipeline as disk source
- participate in whole-project duplicate detection
- participate in baseline comparison using its project-relative path
- participate in focus matching
- preserve deterministic findings
- never be written to disk by Arid

Ordinary `.gitignore` discovery MUST NOT prevent an explicitly supplied virtual source from being analyzed, consistent with explicit-file intent. Configured Arid `exclude` rules MUST still apply.

Invalid virtual source MUST follow the same fail-fast or `--keep-going` policy as an equivalent disk source.

V2 does not require multiple virtual files, a daemon, an RPC protocol, or a persistent in-memory workspace.

---

# **20. Multiple Report Outputs from One Scan**

V2 MUST allow one completed detection run to render additional report files without rerunning discovery, parsing, normalization, or duplicate detection.

Add a repeatable option conceptually equivalent to:

```text
--report <FORMAT=PATH>
```

Example:

```bash
arid . \
    --report json=artifacts/arid.json \
    --report sarif=artifacts/arid.sarif \
    --report markdown=artifacts/arid.md
```

Supported report formats MUST remain the existing concrete set:

```text
text
json
markdown
sarif
```

Normal stdout behavior remains controlled by the existing `--format` / `--json` selection, with text as the default.

Supplemental report files MUST be rendered from the same in-memory logical report/finding set as stdout.

For equivalent formats and presentation options, supplemental output MUST be semantically identical to output that would have been produced as the primary representation.

Report-file write failures MUST return exit status `2`.

The implementation SHOULD use safe replacement semantics for report files where practical.

This feature MUST NOT introduce a reporter registry, plugin framework, second detector pass, or general pipeline framework.

---

# **21. Machine Capability Discovery**

V2 MUST provide deterministic machine-readable capability discovery:

```text
--capabilities
```

`--capabilities` MUST emit JSON and exit without configuration discovery or source analysis.

The capability document MUST identify at minimum:

```text
tool_version
report_schema_versions
baseline_schema_versions
finding_fingerprint_versions
formats
features
```

The feature set MUST allow a machine consumer to determine support for important optional behavior such as:

```text
workers_auto
focus
keep_going
baseline_status
baseline_prune
multi_report
stdin_path
no_fail_on_findings
```

Capability output MUST be deterministic for a given Arid build and MUST NOT contain host-specific capacity such as CPU count, current directory, or discovered configuration.

The capability document is a public machine interface. Its schema MUST be explicitly versioned and published, initially as:

```text
schemas/capabilities-v1.schema.json
```

---

# **22. CLI Compatibility**

The normal Arid CLI remains compatible with 1.x except for explicitly documented machine-output changes.

The following existing invocations MUST continue to work:

```bash
arid .
arid src tests
arid . --json
arid . --format json
arid . --format markdown
arid . --format sarif
arid . --show-source
arid . --exclude 'generated/**'
arid . --workers 4
arid . --workers auto
arid . --baseline arid-baseline.json
arid . --write-baseline arid-baseline.json
```

The existing `--json` shorthand MUST remain behaviorally equivalent to `--format json`.

V2 MUST NOT introduce subcommands merely to reorganize existing options.

Boolean configuration MUST continue to support explicit positive and negative CLI overrides where currently provided.

New invocation-specific controls such as focus, keep-going, output selection, project-root selection, and administrative introspection MUST remain CLI-oriented rather than becoming repository detection policy without a separate justification.

---

# **23. Configuration Compatibility**

`[tool.arid]` remains Arid's project configuration namespace.

Existing valid 1.x configuration MUST continue to work in 2.0.

Default configuration precedence remains:

```text
CLI arguments
    ↓
pyproject.toml
    ↓
built-in defaults
```

Explicit v2 configuration-selection flags MAY alter where the middle layer is sourced but MUST NOT invert the precedence model.

V2 MUST NOT rename existing configuration keys solely because the report schema changes.

Worker selection remains an execution-time CLI choice and MUST NOT be added to `[tool.arid]` in 2.0.

Focus, output format, keep-going mode, no-fail-on-findings, and report destinations MUST remain invocation-specific in v2.

Arid MUST NOT introduce another proprietary configuration file format.

---

# **24. Parallelism and Performance**

V2 MUST preserve serial execution as the default.

Running:

```bash
arid .
```

MUST remain equivalent to:

```bash
arid . --workers 1
```

Automatic parallelism remains opt-in:

```bash
arid . --workers auto
```

The existing bounded automatic-selection policy MUST remain unless new benchmark evidence demonstrates that a change is materially beneficial without unacceptable resource cost or determinism risk.

V2 MUST preserve:

- numeric `--workers N`
- `--workers auto`
- deterministic findings across worker modes
- deterministic metrics and ordering across worker modes
- byte-identical deterministic non-colored structured output for equivalent scans

The v1.2 performance evidence established Python parsing as the dominant measured cost and found no justification for detector redesign, persistent caching, or default automatic parallelism. V2 MUST NOT override that evidence without new measurements.

New v2 functionality SHOULD reuse one scan/report model wherever possible. Focus, multi-output rendering, baseline maintenance, and integration features MUST NOT trigger avoidable duplicate parsing or detection passes.

Arid 2.0 MUST continue to demonstrate at least an order-of-magnitude performance advantage over isolated Pylint duplicate detection on the established qualifying medium and large corpora.

Meaningful unexplained regression from the qualified 1.2 baseline MUST be investigated before release.

## **24.1 Competitive benchmark freshness**

Pinned historical benchmark versions remain necessary for reproducible regression evidence.

Before stable 2.0 performance claims are published, Arid MUST also run a documented comparison against the then-current stable Pylint release so public competitive claims are not based solely on an obsolete competitor version.

The current-version comparison does not replace the pinned historical benchmark; both serve different purposes.

Public performance claims MUST identify the compared versions and methodology.

---

# **25. Rust Public API Boundary**

Arid is primarily a CLI product, but the published Rust package currently exposes implementation modules that create an unnecessarily broad semver surface.

V2 MUST deliberately define a narrow supported Rust API boundary.

Internal implementation areas SHOULD no longer be public modules merely because the binary uses them internally. This includes implementation details such as:

- corpus construction
- normalization internals
- suffix-array/LCP machinery
- duplicate-extraction internals
- parser-specific internals
- renderer implementation details
- baseline filtering internals

The supported public Rust surface SHOULD be limited to the smallest coherent set required for intentional programmatic invocation and result handling.

The technical architecture MUST enumerate the supported public Rust items before implementation begins.

Any public item retained in the supported v2 API becomes part of the v2 semver contract and MUST therefore justify its exposure.

Arid 2.0 MUST NOT create a large embedding framework, plugin API, or generic library abstraction merely to replace the accidental 1.x surface.

---

# **26. Output Compatibility Beyond JSON**

Text and Markdown output MUST preserve their existing purpose and information content except for explicitly documented v2 additions and the occurrence-distribution rename to `hybrid`.

V2 MUST NOT add fingerprints to normal text output if doing so would materially reduce readability.

Markdown MAY expose the fingerprint when it materially improves CI or saved-report usefulness, but this is not required merely because JSON and SARIF expose it.

Keep-going text/Markdown output MUST make incomplete analysis unmistakable and MUST not present partial metrics as though the scan were complete.

Color behavior and precedence remain unchanged.

No ANSI styling may appear in JSON, Markdown, or SARIF.

`--show-source` semantics remain unchanged across supported formats.

---

# **27. Official GitHub Actions Integration**

Arid 2.0 MUST provide an official supported GitHub Actions integration.

The integration MUST use released Arid behavior rather than implement a second duplicate detector.

It MUST preserve whole-project duplicate detection by default.

The integration SHOULD make common CI usage low-friction while exposing explicit inputs for additional Arid arguments rather than hiding the underlying CLI contract.

At minimum it MUST support:

- selecting the scan path or paths
- normal Arid enforcement semantics
- exposing whether active findings exist
- exposing core scan metrics as action outputs
- optional SARIF artifact/upload integration where GitHub permissions permit it
- optional GitHub job-summary output

The action SHOULD be able to use Arid focus mode for pull-request workflows when a caller supplies the relevant paths, while preserving whole-project detection.

GitHub annotations or summaries MUST NOT invent Arid severity levels.

The implementation location and packaging strategy for the official action are technical-architecture decisions. The core Arid CLI MUST remain fully useful without GitHub Actions.

---

# **28. Distribution and Platform Support**

V2 MUST preserve the established release targets unless a platform becomes technically unavailable and the exception is explicitly documented:

```text
Linux x86_64
Linux aarch64
macOS x86_64
macOS aarch64
Windows x86_64
```

Linux release artifacts MUST preserve an explicit compatibility floor rather than inheriting the host runner's glibc version accidentally.

V2 MUST preserve publication through:

- PyPI wheels
- standalone GitHub release archives

Normal installation MUST continue to support:

```bash
uv tool install arid
```

and:

```bash
python -m pip install arid
```

Rust-native installation SHOULD continue to support:

```bash
cargo install arid-cli
```

---

# **29. Migration Guide**

V2 MUST add:

```text
docs/arid-v2-migration-guide.md
```

The migration guide MUST clearly distinguish users who are affected from users who are not.

At minimum it MUST document:

1. report v3 → v4
2. `version` → `schema_version`
3. new report metadata including `tool_version`, `complete`, `analysis`, and `errors`
4. distribution `mixed` → `hybrid`
5. the new finding `fingerprint`
6. SARIF fingerprint identity and distribution terminology
7. supported Rust API changes
8. continued compatibility of `[tool.arid]`
9. continued compatibility of baseline-v1 files
10. unchanged default CLI exit statuses
11. unchanged duplicate-detection semantics
12. the purpose and semantics of focus and keep-going modes for automation consumers

The guide MUST include concrete before/after JSON examples.

A normal CLI user who does not consume JSON/SARIF programmatically and does not depend on Arid's Rust library SHOULD be told explicitly when no migration work is required.

New additive CLI capabilities do not themselves require migration.

---

# **30. Validation Requirements**

V2 validation MUST cover all intentional compatibility changes, new automation contracts, and preserved behavior.

At minimum, targeted v2 validation MUST verify:

```text
report schema_version = 4
absence of top-level report version
report tool_version
report complete = true for complete scans
report-v4 JSON Schema validation
report-v3 schema remains unchanged as a historical artifact
self-describing effective analysis metadata
distribution hybrid in JSON
distribution hybrid in text
distribution hybrid in Markdown
distribution hybrid in SARIF
context mixed remains mixed
scope mixed remains mixed
stable finding fingerprint determinism
fingerprint independence from line movement
fingerprint independence from path movement
fingerprint independence from occurrence multiplicity
fingerprint change when normalized duplicate content changes
SARIF identity matches report finding identity
focus scans full corpus but reports only matching complete groups
focus retains out-of-focus occurrences in a matching group
focus composes correctly with baseline enforcement
unmatched focus selector fails clearly
keep-going retains valid-file findings
keep-going reports deterministic structured errors
keep-going sets complete = false
keep-going always exits 2 when source processing is incomplete
JSON fatal-error document validates against error-v1
structured error kind does not require parsing message text
--no-fail-on-findings maps findings-only exit 1 to 0
--no-fail-on-findings never masks exit 2
baseline-v1 files remain readable
baseline-v1 enforcement remains behaviorally compatible
baseline status identifies accepted, active, and stale debt
baseline pruning removes stale acceptance only
baseline pruning never accepts new debt
explicit --config selection
--no-config behavior
--project-root anchoring
--show-config effective precedence
--list-files exact deterministic discovery
stdin virtual file addition
stdin virtual file replacement
stdin virtual file baseline/focus behavior
multi-report output equivalence from one scan
multi-report write failure returns 2
capabilities-v1 JSON Schema validation
capability output determinism
existing 1.x pyproject configuration remains valid
serial/numeric/auto finding equivalence
existing default exit-status behavior
official GitHub Action whole-project semantics
official GitHub Action metric outputs
```

The established real-world validation campaign remains required.

V2 changes to reporting, automation, or Rust visibility MUST NOT weaken detector validation.

Published schema files MUST be validated against representative generated documents with pinned validation tooling.

Focus, virtual input, baseline maintenance, and keep-going tests MUST include path cases with spaces and Unicode where relevant.

---

# **31. Release and Qualification Requirements**

Before stable `2.0.0`:

- all committed v2 requirements MUST be implemented
- source formatting, tests, and lint gates MUST pass
- targeted v2 integration validation MUST pass
- full real-world validation MUST pass for the release candidate
- full release benchmark qualification MUST pass for the release candidate
- current-stable Pylint comparison MUST be recorded for public v2 performance claims
- report-v4 schema validation MUST pass
- error-v1 schema validation MUST pass
- capabilities-v1 schema validation MUST pass
- baseline-v1 compatibility validation MUST pass
- focus/keep-going/multi-output integration validation MUST pass
- baseline maintenance validation MUST pass
- virtual stdin validation MUST pass
- official GitHub Actions integration validation MUST pass
- the migration guide MUST be complete
- README documentation MUST cover all normal-user v2 CLI additions
- curated RC release notes MUST exist
- curated stable `docs/releases/v2.0.0.md` MUST be substantively complete before final RC qualification
- the latest release candidate MUST receive a complete qualification PASS
- stable promotion MUST remain metadata-only from the qualified RC
- published stable artifacts MUST pass the established post-publication qualification path

The release workflow MUST continue to reject a published tag whose curated release-notes file is missing or invalid.

Release qualification MUST exercise the exact published artifact for machine-contract features rather than relying only on source-checkout execution.

---

# **32. Backward-Compatibility Boundary**

Arid 2.0 is intentionally breaking only where this specification says so.

## **32.1 Intentionally breaking**

The following are intentional 2.0 compatibility breaks:

```text
JSON report schema 3 → 4
JSON report version → schema_version
new required report tool_version / complete / analysis / errors metadata
occurrence distribution mixed → hybrid
new required JSON finding fingerprint
SARIF occurrence distribution mixed → hybrid
SARIF stable finding identity
supported Rust public API narrowed
```

JSON consumers MUST migrate to report v4 rather than expecting Arid 2.0 to emit the old v3 shape.

## **32.2 Preserved**

The following remain compatible with 1.2:

```text
exact duplicate semantics
DUP001
normal CLI invocation
existing CLI option names
[tool.arid] configuration
baseline schema v1 and existing baseline files
normalization behavior
suppression behavior
text/Markdown core information model except distribution vocabulary
SARIF 2.1.0 format
serial default
numeric workers
--workers auto
default exit statuses 0 / 1 / 2
pre-commit integration
published platform set
```

New focus, keep-going, introspection, multi-output, virtual-input, capability, baseline-maintenance, and exit-control options are additive and opt-in.

No additional incompatibility should be introduced merely because the major-version boundary exists.

---

# **33. V2 Completion Criterion**

Arid 2.0 is complete when the following statement is true:

> **Arid still detects the same exact normalized Python duplicates as 1.2, but exposes cleaner versioned machine contracts, stable finding identity, precise hybrid distribution terminology, focused whole-project reporting, safe partial-result diagnostics, baseline maintenance, transparent configuration, multi-output and virtual-source automation, capability discovery, first-class GitHub CI integration, and an intentionally narrow Rust API—while existing project configuration and baseline files continue to work.**

A feature is not required merely because 2.0 is a major release.