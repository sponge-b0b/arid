# Arid v1.2 Release Roadmap

**Product:** Arid  
**Stable target:** `1.2.0`  
**Current phase:** Phase 8 — Release Candidate

## Purpose

Arid v1.2 is a focused engineering release centered on three outcomes:

1. **Faster to run** — add opt-in automatic worker selection and use measurement to guide any justified performance work.
2. **Easier to install** — expand Linux distribution coverage to x86_64 and ARM64 with an explicit compatibility floor.
3. **Harder to integrate incorrectly** — publish machine-readable schemas and harden release qualification around PyPI publication timing.

The duplicate-detection model, findings semantics, existing output contracts, baseline semantics, and serial default remain unchanged.

## Scope boundary

### In scope

- `--workers auto`, opt-in only.
- Linux x86_64 and ARM64 wheel and standalone distribution.
- Explicit manylinux compatibility policy and pinned release-build tooling.
- JSON Schema documents for report schema v3 and baseline schema v1.
- Benchmark stage timing and performance profiling.
- Bounded PyPI release-readiness retry behavior in qualification.
- Release workflow and artifact-matrix hardening.
- Documentation, validation, and benchmark evidence for all v1.2 changes.

### Explicitly out of scope

The following remain candidates for the later breaking release rather than v1.2:

- `mixed` → `hybrid` vocabulary change.
- Report `version` → `schema_version` rename.
- A new report schema version.
- New stable finding-fingerprint contract.
- Rust API surface cleanup or deliberate public API redesign.
- Making automatic parallelism the default.
- Baseline pruning/maintenance commands.
- Semantic or fuzzy duplicate detection.
- Diff-only detection, plugins, LSP support, HTML output, or general debt-management features.

## Phase 0 — Contract

**Goal:** freeze requirements, architecture, and release gates before product implementation.

### Artifacts

- `docs/arid-v1.2-requirements-specification.md`
- `docs/arid-v1.2-technical-architecture-and-design.md`
- `docs/arid-v1.2-release-roadmap.md`

### Gate

- Requirements, architecture, and roadmap agree on scope and compatibility boundaries.
- No implementation begins until the three documents are internally consistent.

## Phase 1 — Linux distribution expansion

**Goal:** establish a reproducible Linux x86_64 + ARM64 artifact matrix.

### Work

- Update release workflow to build both Linux architectures.
- Establish the selected manylinux compatibility floor.
- Pin maturin-action and maturin versions.
- Ensure the standalone binary is the exact executable packaged into the wheel where the packaging design permits.
- Add artifact naming and manifest checks.
- Add native ARM64 post-build smoke validation.

### Gate

Every target produces the expected wheel and standalone artifact, artifacts pass package validation, and both architectures execute successfully in their target environments.

## Phase 2 — Automatic worker selection

**Goal:** add opt-in automatic parallelism without changing the default.

### Work

- Accept `--workers auto`.
- Resolve `auto` to a bounded worker count during CLI parsing.
- Preserve `--workers` numeric behavior.
- Preserve serial default when the option is omitted.
- Test zero/one/small/large corpus behavior and unavailable-parallelism fallback.

### Gate

CLI behavior and detection results are identical between serial and automatic execution; only execution parallelism differs. Existing numeric worker tests remain green.

## Phase 3 — Machine-readable schemas

**Goal:** publish formal schemas for existing machine contracts without changing them.

### Work

- Add `schemas/report-v3.schema.json`.
- Add `schemas/baseline-v1.schema.json`.
- Validate representative generated documents against the schemas.
- Document schema locations and compatibility expectations.

### Gate

Existing JSON and baseline fixtures validate against their published schemas with no report or baseline contract changes.

## Phase 4 — Qualification and release hardening

**Goal:** make release automation resistant to the PyPI propagation race encountered during v1.1.0 qualification.

### Work

- Add a small stdlib-only PyPI readiness helper.
- Retry only a bounded, release-specific “not yet visible” response.
- Fail immediately on unrelated HTTP, DNS, TLS, or service errors.
- Integrate readiness into the stable qualification path.
- Add explicit artifact completeness checks for the expanded Linux matrix.
- Keep release tooling pinned and explicit.

### Gate

A newly published version that is accepted by PyPI but temporarily absent from the simple/install index no longer produces a false qualification failure, while genuine publication failures still fail fast.

## Phase 5 — Performance measurement

**Goal:** establish evidence before changing performance-sensitive implementation.

**Status:** Complete. Profiling identified Ruff Python parsing as the dominant cost, and the reproducible corpus/worker/cross-tool benchmark campaign established the v1.2 baseline with no meaningful serial regression.

### Work

- Measure the major execution regions before considering optimization.
- Run representative small, medium, large, and duplicate-heavy corpora.
- Compare serial, explicit multi-worker, and `auto` execution.
- Continue the established Pylint and jscpd comparisons.
- Identify the actual dominant stage before considering optimization.

### Evidence

- `docs/arid-v1.2-performance-report.md` — complete profiling findings, worker matrix, cross-tool results, regression comparison, and Phase 6 decision.

### Gate

A benchmark report identifies the dominant cost centers and establishes a reproducible baseline. No algorithmic optimization is accepted without measured benefit and no semantic regression.

**Gate result:** PASS.

## Phase 6 — Justified performance optimization

**Goal:** optimize only the measured bottleneck.

**Status:** Complete — no product-code change. The dominant measured cost is Ruff parsing, Arid's post-parse analysis is already inexpensive, and the benchmark campaign does not justify a parser redesign, normalization rewrite, detector algorithm change, or additional performance machinery for v1.2.

### Work

- Select the smallest implementation change that materially improves the measured bottleneck.
- Preserve deterministic output and duplicate semantics.
- Avoid persistent caching or unrelated architectural expansion.
- Re-run the complete benchmark suite.

### Gate

The optimization demonstrates a meaningful improvement on representative workloads without unacceptable regression on other workloads, memory use, correctness, or maintainability.

If measurement does not identify a worthwhile optimization, this phase is explicitly allowed to produce **no product-code change**.

**Gate result:** PASS by the explicit no-change path. Phase 5 measurement did not identify a worthwhile product-code optimization for v1.2.

## Phase 7 — Integration and validation

**Goal:** prove the complete v1.2 feature set together before publication.

**Status:** Complete. Source, targeted integration, real-world detector, performance, and pre-publication release-matrix validation all pass.

### Required validation

- Rust formatting, tests, and clippy with warnings denied.
- CLI compatibility tests.
- JSON and baseline schema validation.
- Serial versus numeric versus `auto` equivalence tests.
- Linux x86_64 wheel and standalone smoke tests.
- Linux ARM64 wheel and standalone smoke tests.
- Existing macOS and Windows artifact tests remain green.
- Real-world repository validation.
- Performance benchmark suite.
- Pre-publication release workflow dry run with publication jobs skipped.

Published PyPI/GitHub artifact checks and the complete release qualification campaign require a release tag and therefore belong to Phase 8 RC qualification rather than this pre-publication phase.

### Evidence

- Source gate: `cargo fmt --check`, `cargo test --locked`, clippy with `-D warnings`, `git diff --check`, and clean working tree — PASS.
- `validation/run.sh`: Black, Django, mypy, Rich, and path cases — PASS.
- `validation/v1.2.sh`: inherited v1.1 surface, serial/numeric/auto deterministic JSON equivalence, report-v3 schema, and baseline-v1 schema — PASS.
- `docs/arid-v1.2-performance-report.md`: complete Phase 5 benchmark campaign — PASS.
- GitHub Actions Release workflow dispatch `32246005708` on `v1.2-contract`: verify plus Linux x86_64, Linux ARM64, macOS ARM64, macOS x86_64, and Windows x86_64 build/smoke jobs — PASS; PyPI/GitHub publication and published Linux ARM64 verification correctly skipped for the branch dispatch.

### Gate

All pre-publication validation passes and no out-of-scope behavior has entered the release.

**Gate result:** PASS.

## Phase 8 — Release Candidate

**Goal:** freeze the v1.2 feature set and qualify a release candidate.

### Rules

- No new product features after RC.
- Fixes are limited to qualification failures, regressions, packaging defects, or release-process defects.
- Any source-code change after RC requires the affected validation gates to be rerun.

### Gate

The RC passes the complete qualification suite, including artifact, schema, PyPI, real-world, and performance validation.

## Phase 9 — Stable promotion

**Goal:** promote the qualified RC to `1.2.0`.

### Rules

- Stable promotion is metadata-only when the RC remains qualified.
- The stable tag must point to the exact promoted commit.
- GitHub and PyPI publication must be verified.
- Final qualification verifies the RC-to-stable delta and published artifacts without unnecessarily repeating the full RC campaign when the delta is metadata-only.

### Gate

`1.2.0` is published, installable from PyPI on all supported targets, the GitHub release is complete, and stable qualification passes.

## Phase 10 — Closeout

**Goal:** record the actual release outcome and close v1.2.

### Work

- Mark the roadmap Stable.
- Record RC and stable qualification results.
- Record any deferred performance work explicitly.
- Ensure README/release metadata reflects stable `1.2.0`.

### Gate

The roadmap accurately reflects the shipped release and its evidence, with no unresolved release-process state.

## Version and compatibility policy

v1.2 is a **minor, backward-compatible feature release**.

The following must remain compatible throughout v1.2:

- Existing duplicate-detection semantics.
- Existing default serial execution.
- Existing numeric `--workers N` behavior.
- Existing text, JSON, Markdown, and SARIF output contracts.
- Report schema v3.
- Baseline schema v1.
- Existing baseline enforcement semantics.
- Existing supported non-Linux artifact targets.

Any change that violates these boundaries requires explicit re-scoping and review before implementation.

## Completion criteria

v1.2 is complete only when:

- `--workers auto` is implemented, tested, and remains opt-in.
- Linux x86_64 and ARM64 artifacts are published and verified.
- The selected manylinux compatibility floor is explicit and reproducible.
- Report v3 and baseline v1 schemas are published and validated.
- Qualification handles the observed PyPI propagation race without masking real failures.
- Performance is measured and any optimization is justified by evidence.
- Full real-world and release validation passes.
- `1.2.0` stable qualification passes.
- The roadmap is updated to record the actual release outcome.
