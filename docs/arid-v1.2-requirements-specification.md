# **Arid v1.2 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Version scope:** This document defines backward-compatible functionality targeted for Arid **1.2.0**.

---

# **1. Purpose**

Arid 1.2 makes the existing product faster to operate, easier to install across common Linux environments, and more robust to integrate without changing what Arid considers duplicate code.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The Arid v1 and v1.1 requirements remain the base product contract.

This document defines only requirements added or changed for v1.2. Unless explicitly changed here, behavior specified by:

```text
docs/arid-v1-requirements-specification.md
docs/arid-v1.1-requirements-specification.md
```

remains in force.

---

# **2. Product Principle**

Arid 1.2 MUST strengthen execution, portability, packaging, and integration while preserving the narrow detector.

The duplicate definition remains unchanged:

> Two or more sufficiently long contiguous Python source regions that become identical after Arid's configured preprocessing rules are applied.

V1.2 MUST NOT introduce another duplicate definition or change duplicate identity.

The release theme is:

> **Faster to run. Easier to install. Harder to integrate incorrectly. Same duplicate semantics.**

---

# **3. V1.2 Goals**

Arid 1.2 MUST add or complete:

1. broader Linux release portability
2. opt-in automatic worker selection
3. benchmark-driven performance hardening
4. published JSON Schema documents for Arid-owned machine contracts
5. release-pipeline hardening for Linux compatibility
6. qualification resilience to normal PyPI publication propagation

These changes MUST preserve:

- Python-only scope
- exact duplication after normalization
- `DUP001`
- deterministic findings and metrics
- deterministic report ordering
- precise physical source locations
- existing normalization behavior
- existing baseline behavior
- existing output representations
- JSON report schema version `3`
- baseline schema version `1`
- serial execution as the default
- existing numeric `--workers N` behavior
- existing exit-status semantics

---

# **4. Explicit V1.2 Non-Goals**

Arid 1.2 MUST NOT add:

- semantic, fuzzy, structural, or renamed-identifier clone detection
- another diagnostic beyond `DUP001`
- autofix or automated refactoring
- duplication budgets
- Git-diff-only detection
- persistent analysis caching
- Git-aware incremental indexing
- LSP/editor integration
- plugins
- a reporter registry or framework
- HTML output
- additional output formats
- multi-language support
- baseline pruning or baseline debt-management commands
- first-class finding fingerprints in the report schema
- SARIF fingerprint changes that require a report-model compatibility break
- musllinux packaging as a release requirement
- Windows ARM64 packaging as a release requirement
- default parallel execution
- public Rust API cleanup or visibility narrowing

The following known breaking cleanups remain reserved for 2.0:

- distribution vocabulary `mixed` → `hybrid`
- report `version` → `schema_version`
- report schema version `4`
- deliberate Rust public-API boundary cleanup
- any decision to make automatic parallelism the default

---

# **5. Detection Contract**

V1.2 MUST preserve the v1/v1.1 detector and report semantics.

Unless a correctness defect requires a backward-compatible fix, v1.2 MUST NOT change:

- Python parsing semantics
- normalization rules
- suppression barriers
- normalized line interning
- suffix-array duplicate detection semantics
- LCP semantics
- maximal-repeat qualification
- contained-match handling
- same-file overlap handling
- deterministic group canonicalization
- structural context semantics
- structural scope semantics
- occurrence distribution semantics
- duplicate metrics
- baseline fingerprint identity
- baseline acceptance semantics

Execution strategy MAY change performance only. It MUST NOT change findings.

---

# **6. Linux Portability**

## **6.1 Supported Linux architectures**

V1.2 MUST publish Linux artifacts for:

```text
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

For each architecture, the production release MUST publish:

- a PyPI wheel
- a standalone GitHub release archive

The Linux ARM64 standalone archive SHOULD be named consistently with the existing release artifacts, for example:

```text
arid-linux-aarch64.tar.gz
```

## **6.2 Linux compatibility floor**

The Linux wheel build MUST use an explicit manylinux target rather than allowing the effective glibc floor to drift with the GitHub runner image.

The v1.2 Linux compatibility target MUST be no newer than:

```text
manylinux_2_28
```

for both x86_64 and aarch64.

If `manylinux2014` / glibc 2.17 proves reproducible for Arid's dependency set without special-case build complexity, the implementation MAY choose that broader compatibility target instead.

The exact selected target MUST be identical across release preparation, smoke testing, documentation, and qualification expectations.

## **6.3 Standalone Linux compatibility**

Linux standalone binaries MUST NOT silently retain a newer glibc requirement than the corresponding wheel.

The release design MUST build or otherwise verify the standalone Linux binaries against the selected v1.2 Linux compatibility floor.

## **6.4 Existing platforms**

V1.2 MUST preserve the existing release coverage for:

```text
macOS aarch64
macOS x86_64
Windows x86_64
```

Linux ARM64 is additive. V1.2 does not require additional operating systems or architectures.

---

# **7. Automatic Worker Selection**

## **7.1 CLI contract**

The existing option:

```text
--workers <N>
```

MUST additionally accept:

```text
--workers auto
```

Examples:

```bash
arid .
arid . --workers 1
arid . --workers 4
arid . --workers auto
```

## **7.2 Default remains serial**

The default MUST remain equivalent to:

```text
--workers 1
```

Running:

```bash
arid .
```

MUST NOT automatically enable parallel preparation.

V1.2 MUST NOT change default CPU-consumption behavior merely because multiple processors are available.

## **7.3 Existing numeric behavior**

Positive numeric worker counts MUST retain their current meaning.

```text
--workers 0
```

MUST remain invalid CLI input.

## **7.4 `auto` heuristic**

Automatic worker selection MUST be bounded and conservative.

The initial v1.2 heuristic MUST choose no more than:

```text
4 workers
```

and no more workers than:

- available parallelism
- the number of discovered Python files

The resolved worker count MUST always be at least `1`.

Conceptually:

```text
max(1, min(4, available_parallelism, discovered_file_count))
```

Equivalent implementation details MAY differ where needed for empty-input handling or platform APIs.

## **7.5 Semantics and determinism**

For the same source and settings, these modes MUST produce identical findings, metrics, report ordering, and exit status:

```text
--workers 1
--workers N
--workers auto
```

Output bytes MUST remain identical for deterministic non-colored formats when all other options are equal.

## **7.6 Configuration boundary**

V1.2 does not add worker selection to `[tool.arid]`.

Worker count remains an execution-time CLI choice rather than repository detection policy.

---

# **8. Performance Hardening**

V1.2 performance work MUST be evidence-driven.

The benchmark harness MUST make it possible to determine where scan time is being spent before substantial algorithmic optimization is accepted.

At minimum, investigation MUST distinguish the cost of the major pipeline regions:

```text
discovery
read / parse / normalize
corpus construction
suffix-array / LCP construction
duplicate extraction
report construction / rendering
```

This instrumentation MAY remain benchmark/development-only. V1.2 does not require a user-facing timing mode.

Optimization work MUST follow these rules:

1. measure before changing an algorithm
2. preserve cold-scan behavior
3. prefer allocation and implementation improvements before replacing a proven algorithm
4. reject substantial code complexity for negligible benchmark gain
5. preserve deterministic output
6. preserve serial mode as a first-class path

V1.2 MUST NOT introduce persistent caching to improve benchmark results.

The release benchmark campaign MUST continue to compare Arid with Pylint and jscpd on the established pinned corpora.

Pydantic and Polaris MUST remain at least `10x` faster than isolated Pylint duplicate detection under the established qualification methodology.

Any meaningful unexplained serial regression relative to the qualified v1.1 baseline MUST be investigated before release.

The `--workers auto` benchmark path MUST also be measured on the established corpora so the heuristic is validated rather than assumed.

---

# **9. Published Machine-Contract Schemas**

V1.2 MUST publish JSON Schema documents for Arid-owned JSON contracts that already exist.

Required files:

```text
schemas/report-v3.schema.json
schemas/baseline-v1.schema.json
```

## **9.1 Report schema**

`report-v3.schema.json` MUST describe the existing JSON report schema version `3` exactly.

V1.2 MUST NOT change report JSON solely to make the schema easier to express.

In particular, the v1.2 report MUST retain:

```json
"version": 3
```

and the existing distribution vocabulary, including:

```text
same-file
cross-file
mixed
```

## **9.2 Baseline schema**

`baseline-v1.schema.json` MUST describe the existing baseline schema version `1` exactly.

The schema MUST preserve the current normalization snapshot, fingerprint, line-count, path, and multiplicity contract.

## **9.3 Schema lifecycle**

Published schema files are historical compatibility artifacts.

Once released, an existing schema file MUST NOT be silently rewritten to describe a later incompatible contract.

A future incompatible report or baseline format MUST receive a new schema version and new schema file.

## **9.4 SARIF**

Arid MUST continue to emit SARIF 2.1.0 against the official SARIF schema.

V1.2 does not require an Arid-owned SARIF schema.

---

# **10. Release Pipeline Hardening**

V1.2 MUST make Linux compatibility an explicit release-workflow input rather than an accidental property of the selected runner image.

For Linux wheel production, the release workflow MUST:

- specify the selected manylinux target explicitly
- keep `--compatibility pypi`
- use the same compatibility policy for x86_64 and aarch64 unless architecture constraints require a documented exception

The maturin release tooling SHOULD pin both:

- the `maturin-action` revision
- the maturin version

so platform-tag behavior cannot change silently between otherwise identical Arid releases.

Any pinning added for v1.2 MUST remain easy to update deliberately and MUST NOT introduce a general dependency-update framework.

---

# **11. Qualification Resilience for PyPI Publication**

A successful PyPI upload may become installable through the package index slightly after the production release workflow completes.

V1.2 qualification MUST distinguish this normal publication-propagation state from a genuine missing or broken release.

Before exact PyPI installation, qualification MUST perform a bounded readiness check for the exact requested release version.

The readiness mechanism MUST:

- check the exact requested version
- retry only the not-yet-visible publication condition
- stop after a bounded period
- fail clearly if the exact release never becomes visible
- proceed immediately once the exact release is visible

Qualification MUST NOT mask genuine failures such as:

- wheel installation failure
- unsupported platform tags after the release is visible
- `arid --version` mismatch
- runtime failure
- missing expected artifact files

The readiness check SHOULD use standard tooling already available to the qualification harness where practical rather than adding a heavyweight dependency.

---

# **12. Compatibility Requirements**

V1.2 is a backward-compatible minor release.

The following MUST remain compatible with v1.1:

- existing CLI invocations
- default serial execution
- numeric `--workers N`
- `[tool.arid]` configuration
- baseline files using schema version `1`
- JSON consumers using report schema version `3`
- Markdown output contract
- SARIF output contract
- pre-commit integration
- exit statuses `0`, `1`, and `2`

V1.2 MUST NOT require users to regenerate existing baselines solely because they upgraded from 1.1.

---

# **13. Validation Requirements**

V1.2 validation MUST add targeted coverage for:

```text
--workers auto
serial / numeric / auto finding equivalence
serial / numeric / auto deterministic ordering
worker auto cap and lower bound
Linux x86_64 published wheel
Linux x86_64 standalone artifact
Linux aarch64 published wheel
Linux aarch64 standalone artifact
report JSON Schema document
baseline JSON Schema document
PyPI qualification readiness behavior
```

The established real-world detector validation campaign remains required.

V1.2 changes to packaging, worker selection, schemas, or qualification MUST NOT weaken existing detector validation.

---

# **14. Release Requirements**

Before stable `1.2.0`:

- all v1.2 requirements MUST be implemented
- all existing source gates MUST pass fail-fast
- targeted v1.2 integration validation MUST pass
- full real-world validation MUST pass for the release candidate
- full published-artifact benchmarks MUST pass for the release candidate
- published Linux x86_64 artifacts MUST satisfy the selected compatibility floor
- published Linux aarch64 artifacts MUST install/run successfully on the qualification path
- machine-contract schema files MUST be present and documented
- the latest release candidate MUST have a complete qualification PASS
- stable promotion MUST remain metadata-only after the qualified RC

The existing RC-first release discipline remains in force.

---

# **15. V1.2 Completion Criterion**

Arid 1.2 is complete when the following statement is true:

> **Arid detects the same duplicates as 1.1, remains serial by default, can opt into bounded automatic parallel preparation, publishes explicit and broader Linux artifacts, exposes formal schemas for its existing machine contracts, and qualifies published releases without mistaking normal PyPI propagation for a release defect.**

No additional feature is required merely because the minor-version cycle is open.
