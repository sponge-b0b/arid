# Arid v1.1 Release Roadmap

**Product:** Arid  
**Stable target:** `1.1.0`  
**Current phase:** Phase 4 — SARIF Output

## Purpose

This document defines how Arid progresses from stable `1.0.0` to stable `1.1.0`.

The v1.1 requirements specification defines **what Arid 1.1 adds**. The v1.1 technical architecture defines **how those additions fit into the existing v1 design**. This roadmap defines **the implementation order, feature-complete gate, release-candidate qualification, and stable-promotion gate**.

Arid 1.1 is intentionally a backward-compatible minor release.

Its theme is:

> **Improve adoption, human readability, and CI integration without changing what Arid considers duplicate code.**

The stable v1 documents remain the historical contract for `1.0.0`. V1.1 documentation is a delta from that contract rather than a replacement for it.

---

## 1. V1.1 Scope

The committed v1.1 feature set is:

1. colored text output
2. explicit `--format text|json|markdown|sarif`
3. Markdown output
4. SARIF output
5. baseline-based incremental adoption
6. official pre-commit integration

The detector itself is not a v1.1 feature area.

V1.1 MUST preserve:

- Python-only scope
- exact duplication after documented normalization
- `DUP001`
- the existing suffix-array/LCP detector
- deterministic grouping
- source-location accuracy
- existing v1 suppression behavior
- existing JSON compatibility
- existing cold-scan performance expectations

---

## 2. Explicitly Out of Scope

The following MUST NOT be pulled into v1.1 merely because implementation work is underway:

- semantic clone detection
- fuzzy or structural clone detection
- identifier-renaming matching
- autofix or automated refactoring
- HTML reports
- duplication budgets
- persistent cache
- Git-diff detection
- Git-aware incremental indexing
- LSP/editor integration
- multi-language support
- plugins
- a reporter registry/framework
- historical duplication analytics

If a feature does not support the defined v1.1 contract, it waits.

---

## 3. Development Versioning

Normal v1.1 development does not require speculative version churn on every commit.

The repository may remain at the most recently released stable version while v1.1 functionality is developed.

Release metadata is prepared only when a release artifact is being cut.

The first planned v1.1 prerelease is:

```text
PyPI:   1.1.0rc1
GitHub: v1.1.0-rc.1
```

prepared with:

```bash
./release.sh 1.1.0-rc.1
```

There is no planned alpha or beta series for 1.1.

This does not weaken quality gates. The repository already has mature unit/integration tests, real-world validation, reproducible benchmarks, native release smoke tests, and published-artifact qualification.

If development evidence later justifies an alpha or beta, the release tooling can support one, but no prerelease stage is added merely for ceremony.

---

# Phase 0 — V1.1 Contract

**Status:** Complete

## Goal

Freeze the intended 1.1 product direction before implementation begins.

## Deliverables

- `docs/arid-v1.1-requirements-specification.md`
- `docs/arid-v1.1-technical-architecture-and-design.md`
- `docs/arid-v1.1-release-roadmap.md`

## Gate

Before feature implementation:

- every committed feature has explicit requirements
- detector semantics are explicitly unchanged
- baseline identity is defined before baseline code is written
- output-format names are fixed
- color semantics and precedence are fixed
- deferred and rejected ideas are recorded

No implementation should invent product behavior that contradicts these documents.

---

# Phase 1 — Output Foundation and Colored Text

**Status:** Complete

## Goal

Establish the v1.1 output model while preserving the current text and JSON behavior.

## Work

1. Add concrete output-format selection:

   ```text
   text
   json
   markdown
   sarif
   ```

2. Keep `text` as the default.

3. Preserve `--json` as the existing JSON selector.

4. Split the current reporting module only as needed to support concrete renderers.

5. Add semantic text styles.

6. Add:

   ```text
   --color auto
   --color always
   --color never
   ```

7. Implement environment handling for:

   ```text
   NO_COLOR
   CLICOLOR_FORCE
   CLICOLOR
   ```

8. Ensure redirected/piped auto text contains no ANSI sequences.

9. Keep Python source snippets unhighlighted while visually separating paths, locations, and gutters.

## Phase Gate

Before advancing:

- plain text remains behaviorally compatible with v1
- default format is text
- `--json` remains green
- `--format json` is equivalent to JSON mode
- text `auto`, `always`, and `never` are tested
- explicit CLI color wins over environment settings
- no non-text output contains ANSI styling
- no TTY-dependent unit test depends on the actual test runner terminal
- detector tests are unchanged and green

Suggested commit scopes SHOULD remain small and cohesive, for example:

```text
feat(cli): add output format selection
refactor(report): split concrete renderers
feat(report): add colored text output
```

---

# Phase 2 — Baseline Domain and Enforcement

**Status:** Complete

## Goal

Allow established repositories to adopt Arid while failing when duplicate debt increases.

This is the most substantive v1.1 feature and receives its own phase.

## Work

1. Add baseline schema version `1`.

2. Implement deterministic SHA-256 group fingerprints from exact normalized duplicate blocks.

3. Normalize baseline paths to project-relative `/`-separated form.

4. Track accepted occurrence multiplicity per path.

5. Record normalization settings required for compatibility.

6. Add:

   ```text
   --write-baseline <PATH>
   --baseline <PATH>
   ```

7. Add optional persistent configuration:

   ```toml
   [tool.arid]
   baseline = "arid-baseline.json"
   ```

8. Write baselines atomically.

9. Filter fully accepted groups after detection and before report construction.

10. Keep complete occurrence lists on active groups so findings remain understandable.

11. Preserve JSON schema v3 by filtering groups rather than changing report structure.

## Required Behavior Cases

The phase MUST explicitly test:

```text
unchanged baseline               → no active finding
unrelated lines inserted above   → still accepted
new occurrence in existing file  → active finding
occurrence in new file           → active finding
file rename/move                 → active finding
normalized duplicate changed     → active finding
accepted duplicate removed       → no stale finding
normalization mismatch           → error
malformed baseline               → error
```

Same-file multiplicity MUST also be tested.

## Phase Gate

Before advancing:

- baseline serialization is byte-deterministic
- physical line numbers do not participate in identity
- no fuzzy/contextual matching exists
- added per-path multiplicity is detected
- renamed paths are conservatively treated as new
- baseline normalization mismatches fail clearly
- baseline errors return exit status `2`
- successful baseline creation returns exit status `0`
- normal scan semantics are unchanged when no baseline is configured
- detector output before baseline filtering remains identical to v1 behavior

---

# Phase 3 — Markdown Output

**Status:** Complete

## Goal

Provide a clean document-oriented report suitable for GitHub rendering, CI summaries, and saved reports.

## Work

1. Implement concrete Markdown rendering from the existing `Report` model.

2. Preserve canonical finding and occurrence ordering.

3. Use normal Markdown emphasis for metadata and locations.

4. With `--show-source`, render original source in fenced Python blocks without artificial line-number gutters.

5. Safely handle backticks in paths and source fences.

6. Ensure Markdown output is always ANSI-free.

## Phase Gate

- no detector/report-model change is required solely for Markdown
- Markdown contains the same essential finding information as text/JSON
- source fences cannot be broken by source containing backticks
- repeated identical scans produce byte-identical Markdown
- `--show-source` source is physically correct
- Markdown output does not alter exit status

---

# Phase 4 — SARIF Output

**Status:** In progress

## Goal

Provide standards-based code-scanning integration without adding GitHub-specific finding logic.

## Work

1. Implement a concrete SARIF 2.1.0 renderer.

2. Represent Arid as the tool driver.

3. Define `DUP001` as the rule.

4. Represent the canonical first occurrence as the primary location.

5. Represent remaining occurrences as related locations.

6. Preserve objective metadata through SARIF properties.

7. Omit invented severity.

8. Normalize project-relative paths to deterministic URI form.

9. Support source snippets when `--show-source` is requested where practical.

10. Keep the document deterministic and timestamp-free.

## Phase Gate

- one Arid duplicate group maps to one SARIF result
- all duplicate occurrences remain discoverable
- physical source ranges are correct
- Unicode and space-containing paths are represented correctly
- no volatile value makes repeated scans differ
- SARIF contains no ANSI sequences
- output validates against expected SARIF structure in tests
- SARIF does not alter detector or exit semantics

---

# Phase 5 — Official Pre-commit Integration

## Goal

Make local enforcement easy without compromising global duplicate detection.

## Work

Add and validate an official hook manifest based on:

```yaml
- id: arid
  name: arid
  entry: arid .
  language: system
  pass_filenames: false
  always_run: true
```

The exact manifest is finalized against current supported pre-commit behavior during implementation.

Documentation MUST explain that Arid must already be installed and available on `PATH`.

## Phase Gate

- hook performs a whole-project scan
- pre-commit does not append staged filenames
- hook returns Arid's normal exit status
- baseline configuration in `[tool.arid]` works through the hook
- no hidden Rust build or package-manager workflow is introduced
- a representative repository can install and execute the hook successfully

---

# Phase 6 — V1.1 Integration and Release-Tooling Hardening

## Goal

Make the existing validation/release system aware of the v1.1 surface before any RC is published.

## Validation Harness

Extend targeted integration coverage for:

- plain text
- auto-color redirection
- forced color
- JSON compatibility
- Markdown
- SARIF
- baseline creation
- unchanged baseline enforcement
- deliberate new duplicate against a baseline

Do not multiply every real-world corpus by every output format unless evidence shows that is necessary.

The established Black, Django, mypy, Rich, path-case, and worker-determinism campaign remains the detector-correctness gate.

## Benchmark Harness

Keep the existing benchmark methodology.

The exact published RC artifact continues to be benchmarked with `--arid-bin`.

No cache is introduced to hide v1.1 overhead.

Investigate any meaningful unexplained regression relative to v1.

## Release Metadata Tooling

Before `1.1.0-rc.1`, update `release.sh` so the active roadmap is derived from the target release series.

Required mapping includes:

```text
1.0.x  → docs/arid-v1-release-roadmap.md
1.1.x  → docs/arid-v1.1-release-roadmap.md
```

The historical v1 roadmap must no longer be mutated by 1.1 release preparation.

## Qualification Tooling

Update `qualification/run.sh` to derive the same active roadmap for RC-to-stable metadata verification.

The existing qualification responsibilities remain:

```text
tagged source gate
release workflow verification
GitHub release verification
published standalone smoke
clean PyPI installation
standalone full validation
PyPI full validation
JSON equivalence
published-binary benchmarks
performance gate
qualification evidence
```

V1.1 targeted validation must be included in the qualification path before RC1 is considered qualified.

## Phase Gate

- release metadata preparation targets the v1.1 roadmap
- historical v1 release documentation remains untouched
- qualification knows the v1.1 metadata file set
- validation covers each v1.1 feature at least once end-to-end
- benchmark harness works with the current v1.1 binary
- generated results remain ignored
- repository documentation matches actual CLI behavior

---

# Phase 7 — Feature Complete

## Meaning

V1.1 becomes feature complete when all committed requirements are implemented and no planned v1.1 code change remains.

At feature complete:

> **The next release artifact is expected to be a release candidate, not another development milestone.**

## Feature-Complete Gate

Run at minimum:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
git diff --check
```

Then run:

- full real-world validation
- targeted v1.1 integration validation
- deterministic worker checks
- full reproducible benchmark suite when performance-sensitive changes justify it
- baseline determinism tests
- output determinism tests
- documented CLI examples
- pre-commit integration test

The feature-complete source MUST have:

- no known correctness blocker
- no known baseline false-accept defect
- no known source-location defect
- no known ANSI leakage into machine/document output
- no known nondeterminism
- no known packaging blocker
- no planned feature work

Any feature request discovered after this gate waits unless it fixes a defined v1.1 requirement.

---

# Phase 8 — `1.1.0-rc.1`

## Prepare

From a clean synchronized `main`:

```bash
./release.sh 1.1.0-rc.1
./release.sh --check
git diff --check
```

Commit the metadata transition with the repository's required scoped Conventional Commit form.

Tag:

```text
v1.1.0-rc.1
```

Push the tag and allow the production release workflow to build and publish the release.

## Qualification

After the tagged release workflow is green:

```bash
qualification/run.sh <global-root> 1.1.0-rc.1
```

The qualification harness MUST exercise the exact published artifacts.

## RC Gate

The RC passes only when:

- tagged source gate passes
- native release workflow passes
- GitHub prerelease is correct
- exact PyPI RC installs and runs
- exact standalone artifact runs
- complete real-world validation passes
- v1.1 targeted validation passes
- standalone/PyPI deterministic outputs agree where required
- full published-artifact benchmarks pass
- Pydantic and Polaris remain at least 10x faster than isolated Pylint duplicate detection
- qualification evidence records `PASS`

A failed RC is never promoted directly to stable.

---

# Phase 9 — Additional Release Candidates

Additional RCs are defect-driven only.

If RC1 reveals a release-blocking defect:

```text
v1.1.0-rc.1
      ↓
    fix only
      ↓
v1.1.0-rc.2
      ↓
full qualification again
```

Any non-metadata source change after a fully qualified RC invalidates direct stable promotion and requires another RC.

There is no required minimum number of RCs.

---

# Phase 10 — Stable `1.1.0`

## Stable Preconditions

Stable may be prepared only when:

- the latest `1.1.0-rc.N` has a complete qualification PASS
- no known release-blocking defect remains
- no product code has changed since that qualified RC
- no planned v1.1 work remains

## Prepare

```bash
./release.sh 1.1.0
./release.sh --check
git diff --check
```

The stable transition MUST contain release metadata only.

Tag:

```text
v1.1.0
```

After publication:

```bash
qualification/run.sh <global-root> 1.1.0
```

Stable qualification MUST prove:

- the latest RC qualification record is valid
- the RC-to-stable delta contains only managed release metadata
- the stable metadata exactly matches the transition produced by `release.sh`
- the production stable release workflow passed
- the stable GitHub release is not a prerelease
- the exact stable PyPI package smoke-tests
- the exact stable standalone artifact smoke-tests

The complete real-world and benchmark campaign does not need to be repeated for stable when stable differs from the fully qualified RC only by validated release metadata.

---

# Release-Blocking Defects

A defect blocks v1.1 advancement when it materially violates the v1 or v1.1 product contract.

Examples include:

- any v1 detector regression
- false duplicate findings
- missed duplicates required by the existing contract
- incorrect physical source locations
- nondeterministic findings
- worker-dependent semantics
- ANSI leakage into JSON, Markdown, or SARIF
- auto color polluting redirected text
- incorrect color precedence
- malformed Markdown structure that loses finding information
- incorrect SARIF locations
- SARIF dropping duplicate occurrences
- baseline accepting increased per-path multiplicity
- baseline depending on physical line numbers
- normalization mismatch silently accepted
- malformed baseline silently treated as empty
- baseline mode changing detector equality
- pre-commit scanning only changed files
- broken JSON compatibility
- packaging failure
- substantial unexplained performance regression
- release qualification failure

The following normally do not block v1.1:

- deferred HTML output
- deferred caching
- deferred Git-diff mode
- deferred LSP integration
- cosmetic styling preferences that do not affect readability or contract
- speculative optimization work

---

# Planned Progression

The intended v1.1 path is:

```text
1.0.0
Stable v1 baseline
   │
   │ v1.1 contract
   ▼
Output foundation + color
   │
   ▼
Baselines
   │
   ▼
Markdown + SARIF
   │
   ▼
Pre-commit integration
   │
   ▼
Validation + release-tooling hardening
   │
   ▼
Feature complete
   │
   ▼
1.1.0-rc.1
   │
   │ automated published-artifact qualification
   ▼
1.1.0
Stable v1.1
```

Additional RCs are inserted only when a release-blocking fix requires them.

---

# After `1.1.0`

The `1.1.0` release closes the committed v1.1 feature set.

Future work should continue to be selected from demonstrated user needs rather than automatically consuming the deferred list.

In particular, the existence of multiple renderers does not justify a plugin system, and the existence of baselines does not justify turning Arid into a debt-management platform.

The continuing product test is:

> **Does this make exact Python duplicate detection easier to adopt, understand, or enforce without diluting Arid's narrow purpose?**

If not, it probably does not belong in Arid.
