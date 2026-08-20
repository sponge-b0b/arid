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

Arid 2.0 is a deliberate contract-cleanup release.

It strengthens Arid's public machine interfaces and narrows accidental implementation exposure without changing the product's core definition of duplicate code.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The stable v1, v1.1, and v1.2 requirements remain the historical base contract. Unless this document explicitly changes a behavior, the existing contract remains in force.

The v2 theme is:

> **Cleaner contracts. Stronger integrations. Same focused detector.**

---

# **2. Product Principle**

A major version gives Arid permission to make justified incompatible changes. It does not justify redesign for its own sake.

Arid 2.0 MUST preserve the product properties that have remained stable throughout 1.x:

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

---

# **3. V2 Goals**

Arid 2.0 MUST:

1. introduce JSON report schema version `4`
2. rename the JSON report's top-level `version` field to `schema_version`
3. replace occurrence-distribution vocabulary `mixed` with `hybrid`
4. add a stable first-class fingerprint for each reported duplicate finding
5. expose that stable finding identity through machine-readable integrations
6. preserve baseline schema version `1` and existing baseline compatibility
7. deliberately narrow Arid's supported Rust public API boundary
8. provide a clear 1.x-to-2.0 migration guide
9. preserve the established detector, CLI, configuration, determinism, and performance contracts except where this specification explicitly changes them

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
- LSP/editor integration
- historical duplication analytics
- a general waiver or policy engine
- a second proprietary configuration format
- another duplicate diagnostic beyond `DUP001`

The following previously deferred ideas remain uncommitted and require separate evidence before inclusion in a later 2.x release:

- duplication budgets
- Git-diff-oriented reporting
- baseline pruning or debt-management commands
- persistent caching
- editor/LSP integration
- additional output formats

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

## **7.2 Existing report meaning**

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

V2 MUST NOT rename other report fields merely for stylistic consistency.

## **7.3 Schema lifecycle**

Published report schemas are immutable historical contracts.

A future incompatible report format MUST receive a new schema version and a new schema file rather than silently rewriting report v4.

---

# **8. Occurrence Distribution Vocabulary**

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

## **8.1 Structural `mixed` remains unchanged**

This change MUST NOT rename structural context or structural scope values.

The following remain valid:

```text
context: declarative | executable | mixed
scope: module | class | function | mixed
```

`mixed` remains correct when a finding aggregates multiple structural contexts or scopes.

Only occurrence distribution changes from `mixed` to `hybrid`.

---

# **9. Stable Finding Fingerprints**

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

## **9.1 Identity semantics**

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

## **9.2 Public fingerprint contract**

The fingerprint encoding MUST be explicitly versioned and documented before implementation is considered complete.

The technical architecture MUST define:

- the cryptographic hash algorithm
- the canonical byte encoding
- a versioned domain separator
- the serialized string representation

The initial v2 representation SHOULD use a deterministic SHA-256-based value with an explicit versioned Arid finding-fingerprint domain and a human-recognizable serialized form.

Once released, the fingerprint algorithm and encoding become a compatibility contract. A future incompatible fingerprint algorithm MUST use a new identity version rather than silently changing existing fingerprint meaning.

## **9.3 Baseline fingerprints remain separate**

V2 MUST NOT silently redefine the existing baseline-v1 fingerprint contract.

Baseline group fingerprints and report finding fingerprints serve related but distinct public contracts and MUST use independently versioned identity domains.

Implementations MAY share internal encoding helpers only when doing so does not couple the two public compatibility contracts.

The same duplicate block is not required to have the same serialized report fingerprint and baseline-v1 fingerprint.

---

# **10. SARIF Finding Identity**

SARIF output MUST continue to use SARIF 2.1.0 and the official SARIF schema.

V2 MUST expose the same stable Arid finding identity used by report v4 through the SARIF result representation.

The technical architecture MUST define one stable, versioned SARIF fingerprint property name and MUST map each SARIF result to the same logical fingerprint as the corresponding Arid report finding.

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

# **11. Baseline Compatibility**

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

---

# **12. CLI Compatibility**

The normal Arid CLI remains compatible with 1.x.

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

Exit-status semantics MUST remain:

```text
0  successful scan with no active findings
1  successful scan with active findings
2  usage, configuration, parsing, I/O, baseline, or other scan error
```

Output format MUST NOT change exit status.

---

# **13. Configuration Compatibility**

`[tool.arid]` remains Arid's project configuration namespace.

Existing valid 1.x configuration MUST continue to work in 2.0.

Configuration precedence remains:

```text
CLI arguments
    ↓
pyproject.toml
    ↓
built-in defaults
```

V2 MUST NOT rename existing configuration keys solely because the report schema changes.

Worker selection remains an execution-time CLI choice and MUST NOT be added to `[tool.arid]` in 2.0.

Arid MUST NOT introduce another proprietary configuration file format.

---

# **14. Parallelism and Performance**

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

Arid 2.0 MUST continue to demonstrate at least an order-of-magnitude performance advantage over isolated Pylint duplicate detection on the established qualifying medium and large corpora.

Meaningful unexplained regression from the qualified 1.2 baseline MUST be investigated before release.

---

# **15. Rust Public API Boundary**

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

# **16. Output Compatibility Beyond JSON**

Text and Markdown output MUST preserve their existing purpose and information content except for the occurrence-distribution rename to `hybrid`.

V2 MUST NOT add fingerprints to normal text output if doing so would materially reduce readability.

Markdown MAY expose the fingerprint when it materially improves CI or saved-report usefulness, but this is not required merely because JSON and SARIF expose it.

Color behavior and precedence remain unchanged.

No ANSI styling may appear in JSON, Markdown, or SARIF.

`--show-source` semantics remain unchanged across supported formats.

---

# **17. Distribution and Platform Support**

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

# **18. Migration Guide**

V2 MUST add:

```text
docs/arid-v2-migration-guide.md
```

The migration guide MUST clearly distinguish users who are affected from users who are not.

At minimum it MUST document:

1. report v3 → v4
2. `version` → `schema_version`
3. distribution `mixed` → `hybrid`
4. the new finding `fingerprint`
5. SARIF fingerprint identity and distribution terminology
6. supported Rust API changes
7. continued compatibility of `[tool.arid]`
8. continued compatibility of baseline-v1 files
9. unchanged CLI exit statuses
10. unchanged duplicate-detection semantics

The guide MUST include concrete before/after JSON examples.

A normal CLI user who does not consume JSON/SARIF programmatically and does not depend on Arid's Rust library SHOULD be told explicitly when no migration work is required.

---

# **19. Validation Requirements**

V2 validation MUST cover all intentional compatibility changes and all preserved contracts.

At minimum, targeted v2 validation MUST verify:

```text
report schema_version = 4
absence of top-level report version
report-v4 JSON Schema validation
report-v3 schema remains unchanged as a historical artifact
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
baseline-v1 files remain readable
baseline-v1 enforcement remains behaviorally compatible
existing 1.x pyproject configuration remains valid
serial/numeric/auto finding equivalence
existing exit-status behavior
```

The established real-world validation campaign remains required.

V2 changes to report contracts or Rust visibility MUST NOT weaken detector validation.

Published schema files MUST be validated against representative generated documents with pinned validation tooling.

---

# **20. Release and Qualification Requirements**

Before stable `2.0.0`:

- all committed v2 requirements MUST be implemented
- source formatting, tests, and lint gates MUST pass
- targeted v2 integration validation MUST pass
- full real-world validation MUST pass for the release candidate
- full release benchmark qualification MUST pass for the release candidate
- report-v4 schema validation MUST pass
- baseline-v1 compatibility validation MUST pass
- the migration guide MUST be complete
- curated RC release notes MUST exist
- curated stable `docs/releases/v2.0.0.md` MUST be substantively complete before final RC qualification
- the latest release candidate MUST receive a complete qualification PASS
- stable promotion MUST remain metadata-only from the qualified RC
- published stable artifacts MUST pass the established post-publication qualification path

The release workflow MUST continue to reject a published tag whose curated release-notes file is missing or invalid.

---

# **21. Backward-Compatibility Boundary**

Arid 2.0 is intentionally breaking only where this specification says so.

## **21.1 Intentionally breaking**

The following are intentional 2.0 compatibility breaks:

```text
JSON report schema 3 → 4
JSON report version → schema_version
occurrence distribution mixed → hybrid
new required JSON finding fingerprint
SARIF occurrence distribution mixed → hybrid
supported Rust public API narrowed
```

## **21.2 Preserved**

The following remain compatible with 1.2:

```text
exact duplicate semantics
DUP001
normal CLI invocation
CLI option names unless explicitly changed above
[tool.arid] configuration
baseline schema v1 and existing baseline files
normalization behavior
suppression behavior
text/Markdown information model except distribution vocabulary
SARIF 2.1.0 format
serial default
numeric workers
--workers auto
exit statuses 0 / 1 / 2
pre-commit integration
published platform set
```

No additional incompatibility should be introduced merely because the major-version boundary exists.

---

# **22. V2 Completion Criterion**

Arid 2.0 is complete when the following statement is true:

> **Arid still detects the same exact normalized Python duplicates as 1.2, but exposes a cleaner versioned report contract, precise hybrid distribution terminology, stable finding identity for machine integrations, and an intentionally narrow Rust API—while existing CLI configuration and baseline files continue to work.**

A feature is not required merely because 2.0 is a major release.
