# Arid v2 Release Roadmap

**Product:** Arid  
**Stable target:** `2.0.0`  
**Current phase:** Real-world validation and beta stabilization

## Purpose

Arid v2 is a major contract, automation, and integration release built around one principle:

> **Cleaner contracts. Better automation. Same focused detector.**

V2 deliberately preserves Arid's exact normalized duplicate-detection semantics while improving the surfaces around that detector for developers, CI systems, coding agents, and other tools.

The release is intended to deliver four outcomes:

1. **Cleaner machine contracts** — report schema v4, stable finding identity, structured operational errors, capability discovery, and stronger SARIF integration.
2. **Better developer and agent workflows** — focused reporting, virtual stdin source, explicit project/configuration control, introspection, and safe keep-going analysis.
3. **Stronger automation** — baseline maintenance, findings-only exit control, multiple report outputs from one scan, and an official GitHub Actions integration.
4. **A smaller supported Rust API** — intentional public boundaries instead of exposing detector implementation details as accidental semver commitments.

The detector itself is not redesigned for v2.

## Governing documents

The v2 implementation contract is defined by:

- `docs/arid-v2-requirements-specification.md`
- `docs/arid-v2-technical-architecture-and-design.md`
- this roadmap

The requirements define externally observable product behavior. The architecture defines the intended technical boundaries and implementation order. This roadmap defines delivery phases, evidence, and release gates.

If these documents disagree, implementation MUST stop and the contract documents MUST be reconciled before proceeding.

## Scope boundary

### In scope

- JSON report schema v4.
- `version` → `schema_version` in report JSON.
- Report metadata: `tool_version`, `complete`, deterministic `analysis`, and structured `errors`.
- Occurrence distribution `mixed` → `hybrid`, while structural context/scope retain `mixed`.
- Stable path-independent finding fingerprint v1.
- SARIF exposure of Arid finding identity through a versioned `partialFingerprints` entry.
- Continued SARIF 2.1.0 compatibility.
- Focused reporting through repeatable `--focus <PATH>` while preserving whole-project detection.
- `--keep-going` with deterministic partial results and mandatory exit status `2` for incomplete scans.
- Structured operational-error taxonomy and `schemas/error-v1.schema.json`.
- `--no-fail-on-findings`.
- Baseline-v1 compatibility.
- `--baseline-status` and safe `--prune-baseline`.
- `--config`, `--no-config`, and `--project-root`.
- `--show-config` and `--list-files`.
- One virtual Python source file through `--stdin-path`.
- Multiple concrete report outputs from one scan through repeatable report destinations.
- Deterministic `--capabilities` output and `schemas/capabilities-v1.schema.json`.
- Deliberately narrow supported Rust public API.
- Official GitHub Actions integration.
- Current-stable Pylint comparison before public v2 performance claims.
- Migration guide, README reconciliation, curated release notes, validation, qualification, and release-tooling support for v2.

### Explicitly out of scope

V2 does not add:

- semantic clone detection
- fuzzy clone detection
- identifier-renaming clone detection
- structural similarity as duplicate identity
- embedding-based similarity
- multi-language duplicate detection
- general-purpose linting
- autofix or automated duplicate removal
- automated refactoring
- severity levels
- plugin architecture
- reporter registry/framework
- HTML output
- persistent scan cache
- Git-aware incremental indexing
- built-in Git blame
- native Git-diff detection
- LSP/editor integration
- historical duplication analytics
- watch daemon
- RPC service
- MCP server
- generalized policy/waiver engine
- percentage duplication budgets or thresholds

These remain separate future decisions and are not implied by the existence of a major-version boundary.

## Release policy

V2 is intentionally breaking only where the requirements explicitly say so.

The principal compatibility breaks are:

```text
report schema v3 → v4
version → schema_version
new required report metadata
occurrence distribution mixed → hybrid
new finding fingerprint
SARIF Arid finding identity
supported Rust API narrowing
```

The following remain compatible with v1.2:

```text
exact duplicate semantics
DUP001
normal CLI invocation
existing CLI option names
[tool.arid]
baseline schema v1
existing baseline files
normalization behavior
suppression behavior
serial default
numeric --workers N
--workers auto
exit status meanings 0 / 1 / 2 by default
pre-commit integration
supported platform set
```

New automation controls are additive and opt-in.

## Development and release discipline

Implementation proceeds inside-out / bottom-up.

A lower layer is made correct according to the v2 contract even when doing so temporarily breaks downstream callers. Downstream layers are repaired afterward rather than preserving an incorrect compatibility shim.

No phase may introduce a second detector path.

The release sequence is expected to be:

```text
2.0.0-alpha.N
    ↓
2.0.0-beta.N
    ↓
2.0.0-rc.N
    ↓
2.0.0
```

Additional prereleases are created only when required by failed gates or material fixes. The roadmap does not require an arbitrary number of alpha, beta, or RC releases.

The final RC-to-stable promotion MUST remain metadata-only.

Curated release notes are required for every published v2 tag and MUST exist before publication. Stable `docs/releases/v2.0.0.md` must be substantively complete before the final RC is qualified so stable promotion does not require product/documentation changes.

---

## Phase 0 — Contract

**Goal:** freeze the v2 product boundary, technical architecture, and release plan before implementation begins.

**Status:** Complete.

### Artifacts

- `docs/arid-v2-requirements-specification.md`
- `docs/arid-v2-technical-architecture-and-design.md`
- `docs/arid-v2-release-roadmap.md`

### Gate

- All three documents agree on product scope, compatibility breaks, preserved behavior, pipeline ordering, and release gates.
- All twelve committed automation/integration capabilities have explicit behavioral boundaries.
- No implementation begins while a known contract contradiction remains.

---

## Phase 1 — V2 foundation and release scaffolding

**Goal:** establish the shared v2 foundations before changing report consumers or higher-level automation.

**Status:** Complete.

### Work

- Update `release.sh` so `2.0.*` maps to `docs/arid-v2-release-roadmap.md`.
- Preserve existing 1.x `release.sh --check` behavior.
- Prepare v2 release metadata handling for root `action.yml` once the Action exists.
- Introduce the v2 process outcome model with deterministic stdout, stderr, and exit status.
- Add stable operational-error types and conversion boundaries.
- Add canonical project-relative path helpers used by new v2 features.
- Keep internal subsystem errors local; convert to the stable machine model only at application boundaries.
- Establish or update test fixtures needed for golden compatibility vectors.

### Required invariants

- Normal v1.2 CLI behavior remains unchanged unless an explicitly breaking v2 contract requires otherwise.
- Expected user/input failures are process outcomes, not panics.
- Path helpers do not silently change baseline-v1 serialized path semantics.

### Gate

Foundation tests pass, 1.x release metadata checks remain valid, canonical path behavior is covered across Linux/Windows-style path cases where relevant, and downstream v2 work has a stable outcome/error/path foundation.

---

## Phase 2 — Finding identity and report v4

**Goal:** establish the core breaking machine contract before repairing every renderer and consumer.

**Status:** Complete.

### Work

- Implement finding-fingerprint v1 with the architecture-defined SHA-256 domain and canonical encoding.
- Add golden fingerprint vectors.
- Lock existing baseline-v1 fingerprint vectors against regression.
- Introduce report schema v4.
- Rename report `version` to `schema_version`.
- Add `tool_version`, `complete`, `analysis`, and `errors`.
- Add required finding `fingerprint`.
- Change occurrence distribution `Mixed` to `Hybrid` in the report domain.
- Preserve structural `mixed` for context and scope.
- Add `schemas/report-v4.schema.json`.
- Add `schemas/error-v1.schema.json`.
- Preserve `schemas/report-v3.schema.json` unchanged as a historical contract.
- Preserve `schemas/baseline-v1.schema.json` unchanged.

### Required invariants

- Finding fingerprints are independent of path, physical line, occurrence multiplicity, structural metadata, output format, and worker mode.
- Normalized-content change changes the fingerprint.
- Report-v4 serialization is deterministic.
- Worker selection does not create report differences merely because execution strategy changed.

### Gate

Report-v4 and error-v1 schema validation pass, fingerprint golden vectors pass, baseline-v1 vectors remain unchanged, and no downstream renderer is allowed to redefine the v4 domain independently.

---

## Phase 3 — Renderer and SARIF migration

**Goal:** make every existing output representation consume the v2 report contract consistently.

**Status:** Complete.

### Work

- Update text output for `hybrid` terminology.
- Update Markdown output for `hybrid` terminology and incomplete-scan presentation.
- Update JSON rendering to report v4.
- Update SARIF to preserve SARIF 2.1.0 while exposing:
  - `hybrid` occurrence distribution
  - `aridFindingFingerprint/v1` in standard `partialFingerprints`
- Do not put Arid's path-independent fingerprint into GitHub-specific `primaryLocationLineHash`.
- Ensure incomplete keep-going scans cannot be emitted/uploaded as SARIF.
- Preserve source-snippet and color contracts.

### Gate

All four renderers represent the same logical findings, JSON conforms to report-v4, SARIF validates as SARIF 2.1.0, Arid's SARIF fingerprint equals the report finding fingerprint, and incomplete scans cannot produce misleading SARIF.

---

## Phase 4 — Baseline comparison and maintenance

**Goal:** consolidate baseline reasoning and add safe lifecycle operations without changing baseline-v1.

**Status:** Complete.

### Work

- Replace duplicated baseline-enforcement reasoning with one private comparison engine.
- Preserve existing enforcement behavior.
- Model accepted, active/new, and stale debt from one comparison result.
- Implement `--baseline-status <PATH>`.
- Implement deterministic text and JSON baseline status output.
- Implement `--prune-baseline <PATH>`.
- Prune only stale acceptance.
- Preserve atomic baseline writes.
- Avoid rewriting the baseline when pruning produces no content change where practical.
- Keep `--write-baseline` as the only operation that intentionally replaces accepted debt with the full current set.

### Required invariants

Pruning MUST NOT:

- add a baseline group
- increase accepted multiplicity
- accept a new path
- accept new duplicate content
- modify normalization settings
- introduce baseline-v2

### Gate

Existing baseline-v1 fixtures still load and enforce identically, status classifies all debt states correctly, pruning is monotonic toward less accepted debt only, and all baseline writes remain deterministic and atomic.

---

## Phase 5 — Project/configuration control and introspection

**Goal:** make Arid's project context explicit and inspectable for monorepos, tools, and agents.

**Status:** Complete.

### Work

- Introduce resolved `ProjectContext` ownership of root, config source, and effective settings.
- Implement exact `--config <PATH>`.
- Implement `--no-config`.
- Implement `--project-root <PATH>`.
- Preserve legacy nearest-config behavior when no explicit selector is used.
- Reject contradictory root/config combinations rather than guessing.
- Implement `--show-config` with deterministic text and JSON.
- Implement `--list-files` with deterministic text and JSON.
- Ensure list-files uses the exact normal discovery rules but stops before parsing/detection.

### Gate

Legacy configuration discovery remains compatible, explicit selection is unambiguous, effective precedence can be inspected, discovered-file output matches normal scan input exactly, and path/exclude/hidden behavior remains deterministic.

---

## Phase 6 — Source model, virtual stdin, and keep-going

**Goal:** make source preparation flexible and resilient without creating another Python-analysis path.

**Status:** Complete.

### Work

- Introduce a small source-input model for disk and virtual source.
- Implement `--stdin-path <PATH>` for exactly one virtual Python source.
- Replace an equivalent disk file when the virtual path already exists in the corpus.
- Add the virtual file when no disk counterpart exists.
- Never write virtual source to disk.
- Apply configured Arid excludes to virtual input while preserving explicit-source intent relative to ordinary ignore rules.
- Implement `--keep-going` for independent file-local read/parse/normalization failures.
- Collect structured errors in deterministic order.
- Keep global configuration/discovery/baseline/output/internal failures fatal.
- Set `complete = false` and exit `2` whenever source processing is incomplete.

### Gate

Disk and virtual source share the same parser/normalizer, virtual replacement/addition is deterministic, keep-going preserves valid findings without pretending the scan is complete, and fail-fast remains the default.

---

## Phase 7 — Focused reporting and analysis metadata

**Goal:** support changed-area and agent workflows without sacrificing whole-project detection.

**Status:** Complete.

### Work

- Implement repeatable `--focus <PATH>`.
- Normalize selectors against the project root.
- Support file and directory focus.
- Validate that every explicit selector matches at least one disk or virtual Python source in the scan corpus.
- Keep full corpus discovery, preparation, and duplicate detection.
- Apply processing order:

```text
whole-project detection
    ↓
baseline enforcement
    ↓
focus filtering
    ↓
report construction
```

- Retain every occurrence in a reportable matching group, including out-of-focus occurrences.
- Finalize report-v4 `analysis` metadata from resolved effective settings and normalized focus selectors.
- Keep resolved worker count out of finding-semantics metadata so worker modes remain deterministic-equivalent.

### Gate

Focus cannot miss duplication against unchanged files, focused groups retain complete occurrence context, focus composes correctly with baseline and virtual input, unmatched selectors fail clearly, and report metadata accurately describes the effective scan semantics.

---

## Phase 8 — Exit policy, multi-output, and capabilities

**Goal:** make one scan more useful to humans, CI systems, and machine consumers.

**Status:** Complete.

### Work

- Implement `--no-fail-on-findings`.
- Preserve default `0/1/2` behavior.
- Ensure no-fail can map findings-only `1` to `0` but can never mask `2`.
- Implement repeatable supplemental report destinations conceptually equivalent to `--report FORMAT=PATH`.
- Support existing concrete formats only: text, JSON, Markdown, SARIF.
- Render all outputs from one in-memory logical report.
- Prevent destination collisions and unsafe overlap with source/baseline files.
- Use atomic per-file replacement semantics where practical.
- Emit supplemental text without ANSI styling.
- Suppress SARIF targets for incomplete scans.
- Implement deterministic `--capabilities` JSON.
- Add `schemas/capabilities-v1.schema.json`.
- Ensure capabilities requires no project/configuration discovery.

### Gate

One scan can safely feed multiple consumers without reparsing or redetection, output write errors remain exit `2`, capabilities validate against the published schema, and no-fail never weakens operational failure handling.

---

## Phase 9 — Rust API and orchestration cleanup

**Goal:** finish the intentional v2 Rust boundary and simplify application ownership before external integration work.

**Status:** Complete.

### Work

- Move application orchestration out of `lib.rs` into private `app.rs`.
- Make implementation modules private.
- Re-export only the supported root-level API defined by the architecture.
- Keep the public surface centered on:
  - `Cli`
  - `RunContext`
  - `ColorEnvironment`
  - `RunResult`
  - `ExitStatus`
  - `run`
  - `run_with_context`
- Make `RunResult` expose stdout, stderr, and exit status through stable accessors.
- Keep expected failures represented as process outcomes.
- Add compile/documentation tests for the supported API boundary.

### Gate

The binary uses the same supported application entry points as embedding callers where practical, old accidental public implementation modules are no longer part of the v2 API, and no replacement framework is introduced merely to hide them.

---

## Phase 10 — Official GitHub Action

**Goal:** make correct Arid CI integration low-friction without duplicating detector behavior.

**Status:** Complete.

### Work

- Add root `action.yml` as a composite action.
- Add the minimal helper under `action/` required for argument construction, metrics extraction, summaries, and status mapping.
- Install/use the exact released Arid PyPI version defined by the Action release metadata.
- Run Arid once per Action invocation.
- Produce one supplemental report-v4 JSON file for metrics/action outputs.
- Expose core metrics and findings state as Action outputs.
- Support scan paths and dedicated focus input.
- Support optional job summary.
- Support optional SARIF upload when permissions permit.
- Skip SARIF upload for incomplete scans.
- Reject administrative CLI modes that do not produce the scan metrics promised by the Action.
- Do not invent Arid severity levels.

### Gate

The Action preserves whole-project detection, produces deterministic outputs from released Arid behavior, passes helper/unit tests, and the production release workflow is wired to verify the Action end-to-end against the exact published prerelease artifact. The first published v2 prerelease MUST pass that end-to-end verification as part of Phase 12.

---

## Phase 11 — Targeted v2 integration validation

**Goal:** prove the complete v2 contract together before broad real-world stabilization.

**Status:** Complete.

### Required validation

At minimum `validation/v2.sh` MUST cover:

- report-v4 contract and schema
- error-v1 contract and schema
- capabilities-v1 contract and schema
- report-v3 historical immutability
- finding fingerprint golden/invariance tests
- baseline-v1 fingerprint regression lock
- `hybrid` distribution across all formats
- structural `mixed` preservation
- focus whole-corpus semantics
- focus + baseline ordering
- focus + virtual source
- keep-going completeness and deterministic errors
- JSON fatal-error documents
- no-fail exit policy
- baseline status/prune safety
- exact config/no-config/project-root behavior
- show-config/list-files behavior
- virtual source add/replace/no-write behavior
- multi-output equivalence and failure handling
- public Rust API boundary
- Unicode and space path cases where relevant
- serial/numeric/auto finding equivalence

### Gate

All targeted v2 tests pass from source, inherited v1/v1.1/v1.2 targeted validation remains green where applicable, and no unresolved contract mismatch remains before alpha publication.

---

## Phase 12 — Alpha publication

**Goal:** publish the first externally installable v2 contract for integration testing.

**Status:** Complete.

### Entry criteria

- Phases 0–11 PASS.
- All committed v2 features are present in integrated form.
- Source formatting/tests/clippy gates pass.
- `release.sh` fully supports v2 prerelease metadata.
- Curated alpha release notes exist.
- Tagged artifacts build for all supported platforms.

### Alpha focus

Alpha is for discovering:

- machine-contract mistakes
- CLI interaction mistakes
- migration friction
- GitHub Action packaging/integration defects
- cross-platform packaging defects
- missing validation coverage

Alpha is **not** for adding unrelated scope after the contract has been published.

Breaking corrections discovered during alpha are allowed because the stable v2 contract has not yet shipped, but they require requirements/architecture/schema/docs updates and full affected validation.

### Gate

At least one alpha release is published successfully, installable from PyPI, accompanied by its curated GitHub Release notes, passes artifact smoke testing across the release matrix, and passes the official GitHub Action end-to-end verification against that exact published alpha artifact.

---

## Phase 13 — Real-world validation and beta stabilization

**Goal:** validate v2 behavior on representative repositories and stabilize the externally visible contract.

### Work

- Run the established Black, Django, mypy, Rich, and path-case campaign.
- Prove detector-level canonical group behavior remains consistent with the qualified v1.2 contract for equivalent settings.
- Exercise focus on representative files/directories.
- Exercise baseline enforcement with focus.
- Exercise a controlled malformed file with keep-going.
- Exercise multi-output on a large corpus.
- Exercise virtual-source workflows.
- Exercise the official Action end-to-end.
- Resolve alpha defects.
- Publish beta when the feature set and core machine contracts are considered frozen.

### Beta rule

After beta publication, incompatible machine-contract changes require a demonstrated defect and explicit contract revision. New product features are not added.

### Gate

Real-world validation passes, no known detector semantic regression exists, beta artifacts pass the release matrix, and the v2 feature/contract surface is frozen except for defect correction.

---

## Phase 14 — Performance and competitive verification

**Goal:** prove that the automation work did not compromise Arid's established performance profile.

### Work

- Run the reproducible pinned benchmark campaign.
- Compare against the qualified v1.2 baseline.
- Investigate meaningful unexplained regressions.
- Preserve serial as the benchmark regression baseline.
- Continue worker-mode validation.
- Run the documented comparison against the then-current stable Pylint release before publishing final v2 competitive claims.
- Identify exact versions and methodology in public performance claims.
- Do not introduce speculative optimization solely because v2 is a major release.

### Gate

Arid remains at least an order of magnitude faster than isolated Pylint duplicate detection on the established qualifying medium and large corpora, no unacceptable regression exists relative to v1.2, and any public competitive claim is supported by both reproducible and current-version evidence.

---

## Phase 15 — Migration, README, and release readiness

**Goal:** make v2 understandable and adoptable before RC freeze.

### Work

- Add `docs/arid-v2-migration-guide.md`.
- Include concrete report-v3 → report-v4 JSON before/after examples.
- Document `version` → `schema_version`.
- Document report metadata additions.
- Document `mixed` → `hybrid` for distribution only.
- Document finding fingerprint semantics.
- Document SARIF identity behavior.
- Document Rust API changes.
- Explicitly state that ordinary CLI users who do not consume machine contracts or Rust internals may require no migration work.
- Reconcile README with every normal-user v2 CLI addition.
- Document focused reporting, keep-going, baseline maintenance, config/root controls, introspection, virtual stdin, multi-output, no-fail, capabilities, and the GitHub Action.
- Ensure schema locations are documented.
- Update pre-commit documentation only where behavior or recommended revision changes.
- Prepare substantively complete stable `docs/releases/v2.0.0.md` before final RC qualification.
- Prepare curated RC release notes.

### Gate

A developer can understand whether v2 affects them and how to migrate without reading source or architecture documents; README reflects the actual CLI; stable release notes are already present before RC freeze.

---

## Phase 16 — Pre-RC release-system qualification

**Goal:** prove the release machinery and full artifact matrix before freezing the release candidate.

### Work

- Run source gates:
  - `cargo fmt --check`
  - `cargo test --locked`
  - clippy with warnings denied
  - `git diff --check`
- Run targeted v2 validation.
- Run full real-world validation.
- Run benchmark/performance qualification.
- Run release workflow as a non-publishing branch/manual dry run where supported.
- Verify all five platform build/smoke jobs.
- Verify report-v4/error-v1/capabilities-v1 schemas exist and validate.
- Verify root `action.yml` metadata and Action helper tests.
- Verify v2 release notes are present.
- Verify `release.sh --dry-run 2.0.0-rc.1` or the selected RC version prepares only the expected release-managed files.

### Gate

All pre-publication source, integration, real-world, performance, documentation, Action, and release-matrix checks pass with no known blocker to RC publication.

---

## Phase 17 — Release Candidate

**Goal:** freeze and fully qualify the `2.0.0` candidate using published artifacts.

### Rules

- No new product features after RC.
- Fixes are limited to qualification failures, regressions, packaging defects, documentation defects that affect correctness/adoption, or release-process defects.
- Any product/source change after RC requires affected validation and qualification gates to be rerun and normally requires a new RC.
- Curated stable release notes already exist and are not rewritten as part of stable promotion.

### Qualification requirements

The published RC qualification MUST include:

- production release workflow success
- all supported platform artifacts
- exact PyPI prerelease install
- standalone smoke testing
- Linux ARM64 native published verification
- report-v4 schema validation
- error-v1 schema validation
- capabilities-v1 schema validation
- finding-fingerprint validation
- baseline-v1 compatibility
- focus behavior
- keep-going behavior
- multi-output behavior
- virtual stdin behavior
- baseline status/prune behavior
- config/root/introspection behavior
- official published GitHub Action verification
- real-world validation
- benchmark qualification
- artifact equivalence where applicable
- migration guide and release-note presence

### Gate

The latest RC receives a complete qualification PASS with no unresolved product, packaging, schema, integration, or release-process defect.

---

## Phase 18 — Stable promotion

**Goal:** promote the fully qualified RC to `2.0.0` without changing product behavior.

### Rules

Stable promotion MUST be metadata-only.

The allowed RC-to-stable release-managed delta for v2 is expected to include only:

```text
Cargo.toml
Cargo.lock
pyproject.toml
README.md
action.yml
docs/arid-v2-release-roadmap.md
```

The exact allowlist is enforced by qualification and must match the implemented release tooling.

Stable promotion MUST NOT change:

- product source
- schemas
- migration guide
- release notes content
- GitHub Action implementation logic
- validation logic required to qualify the RC

### Gate

`2.0.0` is published to PyPI and GitHub, all production release jobs including published Action verification pass, the RC-to-stable delta is exactly the allowed metadata-only transition, and stable qualification passes.

---

## Phase 19 — Closeout

**Goal:** record the actual v2 release outcome and leave the repository in an internally consistent stable state.

### Work

- Set roadmap current phase to Stable through normal release metadata preparation.
- Record alpha/beta/RC/stable release evidence actually used.
- Record qualification result paths/run identifiers.
- Update README stable examples and integration revisions where required.
- Ensure official pre-commit and GitHub Action examples reference the intended stable v2 revision/version.
- Record final performance claims and compared tool versions.
- Record any deliberately deferred work explicitly rather than leaving ambiguous unfinished scope.
- Verify no unresolved v2 release-process state remains.

### Gate

Arid `2.0.0` is published, qualified, documented as stable, and the roadmap accurately reflects the shipped release and evidence.

---

## Cross-phase invariants

The following must remain true throughout v2 implementation.

### Detector invariant

There is one exact normalized duplicate detector.

No focus, baseline, error, output, GitHub Action, capability, or agent-oriented feature may change duplicate identity.

### Baseline invariant

Baseline schema v1 and existing baseline compatibility remain intact.

Baseline maintenance reduces stale acceptance only; it never silently accepts new debt.

### Focus invariant

Focus changes what is reported, not what is compared.

### Partial-analysis invariant

An incomplete scan is always visibly incomplete and exits `2`.

### Determinism invariant

Equivalent scans produce deterministic findings, metrics, identities, path ordering, structured errors, and machine output independent of worker scheduling.

### Performance invariant

New v2 features reuse the same scan/report model wherever possible and do not cause avoidable repeated parsing or duplicate detection.

### Product-scope invariant

Arid remains a focused exact duplicate-code checker, not a general code-quality platform.

## Completion criteria

V2 is complete only when:

- every committed v2 requirement is implemented
- report v4 is published and validated
- report v3 remains a preserved historical schema
- finding fingerprint v1 is stable and validated
- `hybrid` occurrence distribution is consistent across outputs
- structural `mixed` remains unchanged
- error-v1 and capabilities-v1 are published and validated
- focused reporting preserves whole-project detection
- keep-going partial results are useful but never falsely successful
- baseline-v1 remains compatible
- baseline status and safe pruning work as specified
- project/config controls and introspection are deterministic
- virtual stdin source behaves like normal source without disk mutation
- multiple outputs come from one scan
- no-fail-on-findings never masks operational errors
- the supported Rust API is intentionally narrow
- the official GitHub Action is validated against a published v2 artifact
- targeted and real-world validation pass
- performance qualification passes
- current-stable Pylint comparison is recorded for public v2 claims
- migration documentation and README are complete
- curated RC and stable release notes are present
- the latest RC receives a full qualification PASS
- stable promotion is metadata-only
- `2.0.0` stable qualification passes
- the roadmap is updated with actual release evidence and no unresolved release-process state remains