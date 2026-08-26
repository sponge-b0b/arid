# Arid v2.1 Release Roadmap

**Product:** Arid  
**Stable target:** `2.1.0`  
**Current phase:** Phase 2 — Suppression audit and machine contract

## Purpose

Arid v2.1 is a focused maintenance, discovery-auditability, and human-feedback release built around one principle:

> **Auditable maintenance. Explainable discovery. Same focused detector.**

V2.1 deliberately preserves Arid 2.0 duplicate-detection semantics while making two previously opaque forms of state observable and enforceable and making Arid's execution speed visible in normal human-facing scan output:

1. whether accepted source suppressions are still necessary;
2. why a specific path is or is not selected by Arid discovery; and
3. how long the completed duplicate scan took.

The release is intended to deliver five outcomes:

1. **Suppression lifecycle visibility** — `--suppression-status` classifies effective suppression regions as active or stale.
2. **Maintenance ratchets** — `--fail-on-stale` can fail suppression or baseline status when obsolete acceptance remains.
3. **Discovery explainability** — `--explain-path <PATH>` and `--no-ignore-files` make traversal decisions easier to diagnose without weakening Arid's own exclusion policy.
4. **Stable administrative machine contracts** — `suppression-status-v1` and `path-explanation-v1` provide deterministic JSON for automation.
5. **Visible execution speed** — completed normal text scans end with a concise human-readable `Total time:` footer.

The detector itself is not redesigned for v2.1.

## Governing documents

The v2.1 implementation contract is defined by:

- `docs/arid-v2.1-requirements-specification.md`
- `docs/arid-v2.1-technical-architecture-and-design.md`
- this roadmap

The requirements define externally observable behavior. The architecture defines technical boundaries and implementation order. This roadmap defines delivery phases, evidence, and release gates.

If these documents disagree, implementation MUST stop and the contract documents MUST be reconciled before proceeding.

## Scope boundary

### In scope

- `--suppression-status`.
- Active/stale classification for effective suppression regions.
- Formal idempotent suppression semantics.
- Valid `# arid: disable` through EOF.
- Repeated same-state suppression directives as no-ops.
- One suppression-disabled audit preparation view using the existing detector once.
- `--fail-on-stale` for `--suppression-status`.
- `--fail-on-stale` for existing `--baseline-status`.
- `--explain-path <PATH>`.
- `--no-ignore-files` for ignore-file-derived traversal filtering only.
- Preservation of Arid `exclude`, hidden-path, Python-source, and symlink policies.
- `schemas/suppression-status-v1.schema.json`.
- `schemas/path-explanation-v1.schema.json`.
- Deterministic administrative JSON.
- Supplemental administrative JSON file output with stdout/file semantic equivalence.
- An always-on human-readable `Total time:` footer for completed normal text scans.
- Adaptive human timing units in microseconds, milliseconds, or seconds.
- Reuse of existing `error-v1`, report-v4, baseline-v1, and Rust public API contracts.
- README/reference documentation, targeted validation, real-world regression validation, release qualification, and release-tooling support for 2.1.

### Explicitly out of scope

V2.1 does not add:

- semantic or fuzzy clone detection
- identifier-renaming clone detection
- structural similarity as duplicate identity
- another diagnostic beyond `DUP001`
- suppression nesting or balancing rules
- malformed-state errors for repeated directives or disable-through-EOF
- suppression owners, expiration dates, approvals, tickets, or rationale syntax
- automatic suppression insertion or removal
- automatic baseline growth
- a new baseline schema
- a generalized waiver/policy database
- a general filesystem tracing subsystem
- recursive path-explanation traces
- a general `--no-excludes` switch
- changes to hidden-file policy
- changes to `.py` / `.pyi` source selection
- changes to symlink policy
- a second detector
- a second discovery implementation
- a reporter/plugin framework
- detailed per-stage runtime timing in normal product output
- a profiling or telemetry subsystem for the timing footer
- timing fields in JSON, Markdown, SARIF, or versioned machine contracts
- a timing SLA or benchmark guarantee derived from one invocation
- persistent caching
- daemon/RPC/MCP operation
- LSP/editor integration

## Compatibility boundary

V2.1 is additive.

The following remain unchanged from 2.0:

```text
exact duplicate semantics
DUP001
report schema v4
finding fingerprint identity
baseline schema v1
baseline fingerprints and comparison arithmetic
error schema v1
normal suppression effect
normalization semantics
[tool.arid] configuration
normal CLI invocation
serial default
numeric --workers N
--workers auto
ordinary-scan exit meanings 0 / 1 / 2
official GitHub Action behavior unless documentation/version metadata requires update
supported platform set
```

New CLI controls are:

```text
--suppression-status
--fail-on-stale
--explain-path <PATH>
--no-ignore-files
```

In addition, completed normal text scans gain an always-on human-facing `Total time:` footer. This is an intentional text-presentation addition only; deterministic machine formats remain unchanged.

Formal documentation of idempotent directives and disable-through-EOF freezes existing valid suppression behavior rather than intentionally changing normal scans.

## Development and release discipline

Implementation proceeds inside-out / bottom-up.

Lower-layer domain behavior is made correct according to the v2.1 contract before application orchestration is adapted. Downstream callers are repaired afterward rather than preserving awkward compatibility shims.

No phase may introduce:

- another definition of duplicate identity
- one detector run per suppression region
- another discovery implementation
- a generic reporter framework
- mutation from a read-only administrative mode
- a runtime telemetry framework merely to print total elapsed time

The expected release sequence is:

```text
2.1.0-alpha.N
    ↓
2.1.0-beta.N
    ↓
2.1.0-rc.N
    ↓
2.1.0
```

Additional prereleases are created only when failed gates or material fixes require them. The roadmap does not require an arbitrary count of alpha, beta, or RC releases.

The final RC-to-stable promotion MUST remain metadata-only.

Curated release notes are required for every published v2.1 tag and MUST exist before publication. Stable `docs/releases/v2.1.0.md` MUST be substantively complete before the final RC is qualified.

---

## Phase 0 — Contract

**Goal:** freeze the v2.1 product boundary, technical architecture, and release plan before implementation begins.

**Status:** Complete.

### Artifacts

- `docs/arid-v2.1-requirements-specification.md`
- `docs/arid-v2.1-technical-architecture-and-design.md`
- `docs/arid-v2.1-release-roadmap.md`

### Gate

- All three documents agree on the six v2.1 product capabilities.
- Suppression semantics agree that directives are idempotent state setters.
- Disable-through-EOF is valid.
- Repeated same-state directives are valid no-ops.
- Suppression audit uses one audit preparation view and one existing detector pass.
- `--fail-on-stale` semantics agree for suppression and baseline status.
- Discovery explanation and ignore-file override use one discovery policy definition.
- New administrative schemas and output boundaries are explicit.
- Human execution timing is presentation-only and excluded from deterministic machine contracts.
- No implementation begins while a known contract contradiction remains.

**Gate result:** PASS.

---

## Phase 1 — Suppression state and audit foundation

**Goal:** establish the lower-level suppression model and audit preparation path without changing ordinary scan behavior.

**Status:** Complete.

### Work

- Derive effective suppression regions from existing ordered suppression events.
- Represent effective disable → enable and disable → EOF regions.
- Treat repeated `disable` while disabled as a no-op.
- Treat repeated `enable` while enabled as a no-op.
- Preserve directive-line timing exactly as in 2.0.
- Add the private suppression preparation mode described by the architecture.
- Preserve normal preparation as the default `Apply` behavior.
- Add `Audit` behavior that masks directive comments but does not remove disabled source or split matching segments.
- Retain effective suppression-region metadata only for audit callers.
- Add unit/golden tests for state transitions, EOF termination, no-op directives, and inline directive timing.

### Required invariants

- Ordinary prepared source remains behaviorally equivalent to 2.0.
- No new suppression balancing/nesting model appears.
- Audit preparation does not change parser, normalization, structural, or physical-line mapping semantics.
- Normal scans do not allocate or retain audit-only state unnecessarily.

### Evidence

- `978e3f67` — `feat(suppression): add effective suppression regions`
- `6e9417c6` — `feat(suppression): add audit preparation mode`
- `67a9290f` — `fix(suppression): remove unreachable audit wrapper`
- Local gate: `cargo fmt --check`, `cargo test --locked`, clippy with `-D warnings`, scoped `git diff --check`, and clean status — PASS.

### Gate

Suppression state tests pass, representative normal-scan fixtures remain unchanged, and audit preparation can expose all effective suppression regions without running duplicate detection itself.

**Gate result:** PASS.

---

## Phase 2 — Suppression audit and machine contract

**Goal:** implement complete active/stale suppression auditing and publish its stable machine contract.

### Work

- Add the suppression audit domain model.
- Prepare the complete source set in audit mode.
- Build one corpus from the audit view.
- Run the existing detector exactly once.
- Map otherwise-reportable duplicate occurrences back to effective suppression regions.
- Cover both source overlap and boundary-only duplicate prevention.
- Ensure two suppressed regions that duplicate one another can both become active.
- Classify every effective region as exactly `active` or `stale`.
- Add deterministic region ordering and derived summary counts.
- Add deterministic human-readable text rendering.
- Add deterministic JSON rendering.
- Add `schemas/suppression-status-v1.schema.json` with the architecture-defined v1 shape.
- Validate representative documents against the schema.

### Required invariants

- Baseline acceptance does not make a suppression stale.
- Focus/report filtering does not make a suppression stale.
- No-op directives do not create audit records.
- EOF-terminated regions are normal audit records, not errors.
- A partial or failed audit never produces a successful status document.

### Gate

Known active, stale, suppressed-vs-suppressed, boundary-only, and empty-audit fixtures classify correctly; JSON validates against `suppression-status-v1`; output is deterministic across worker modes; detector semantics remain unchanged.

---

## Phase 3 — Generic stale-policy enforcement

**Goal:** make stale maintenance state enforceable without changing the underlying audit/comparison models.

### Work

- Add `--fail-on-stale` to CLI parsing and semantic validation.
- Require `--suppression-status` or `--baseline-status` when it is used.
- Add the generic stale-policy helper in `exit_policy.rs`.
- Apply stale policy to suppression status.
- Apply stale policy to existing baseline status after the existing active/new-debt result is calculated.
- Preserve baseline comparison, pruning, serialization, and fingerprints unchanged.
- Add the complete exit-status matrix for both administrative modes.
- Verify that `--fail-on-stale` never mutates source or baseline state.

### Required invariants

- Suppression status without the flag remains informational and exits `0` on active/stale results.
- Suppression status with stale state and the flag exits `1`.
- Existing baseline-status behavior remains unchanged without the flag.
- Baseline status with active/new debt, stale debt, or both exits `1` when policy requires it.
- Operational failure remains exit `2`.
- Ordinary scans never call stale-policy logic.

### Gate

The complete suppression/baseline exit matrix passes, baseline-v1 compatibility tests remain unchanged, and invalid standalone `--fail-on-stale` usage is rejected clearly.

---

## Phase 4 — Shared discovery policy and ignore-file override

**Goal:** make discovery policy explicit and add `--no-ignore-files` without weakening unrelated source-selection rules.

### Work

- Introduce the small private discovery policy value defined by the architecture.
- Centralize `WalkBuilder` construction used by normal directory traversal.
- Preserve link following and hidden-path configuration.
- Add `--no-ignore-files`.
- When active, disable only ignore-file-derived controls:
  - parent ignore files
  - `.ignore`
  - `.gitignore`
  - global Git ignore rules
  - `.git/info/exclude`
- Do not use `standard_filters(false)`.
- Preserve Arid configured/CLI `exclude` behavior.
- Preserve hidden-file policy.
- Preserve `.py` / `.pyi` source filtering.
- Preserve explicit-file behavior.
- Preserve existing symlink behavior.
- Route every discovery-consuming mode through the same effective policy.

### Required invariants

- Without `--no-ignore-files`, discovery remains behaviorally equivalent to 2.0.
- With it, only ignore-file-derived filtering changes.
- A file excluded by Arid remains excluded.
- Hidden files do not become visible merely because ignore files are disabled.
- Explicit files retain their existing bypass of traversal ignore rules.

### Gate

Hermetic discovery fixtures prove normal and override behavior for `.ignore`, `.gitignore`, parent ignore sources, Arid excludes, hidden paths, Python extensions, and symlink cases. `--list-files` demonstrates the expected corpus under both policies.

---

## Phase 5 — Targeted path explanation and machine contract

**Goal:** explain one discovery decision using the same policy as actual traversal.

### Work

- Add `--explain-path <PATH>`.
- Resolve target context relative to effective scan roots.
- Distinguish explicit file, explicit directory root, traversal descendant, and outside-scan-roots cases.
- Reuse Arid's existing exclude matcher.
- Use the configured `ignore` matching machinery for targeted ignore-file evaluation rather than reimplement ignore precedence.
- Apply hidden, source-type, and symlink policy consistently with actual discovery.
- Add the architecture-defined stable decision/reason model.
- Freeze symlink reason vocabulary before the first published prerelease; collapse to one `symlink-policy` reason if cross-platform evidence cannot support the finer distinction reliably.
- Add deterministic text rendering.
- Add deterministic JSON rendering.
- Add `schemas/path-explanation-v1.schema.json`.
- Validate representative documents against the schema.

### Required invariants

- `--explain-path` does not parse Python source.
- It does not run duplicate detection.
- It does not recursively enumerate descendants.
- Its decision agrees with real discovery for the same invocation context.
- The JSON reason vocabulary is stable and never inferred from prose.

### Gate

Representative path decisions agree with actual `--list-files` membership under normal traversal and `--no-ignore-files`; schema validation passes; ordering is deterministic; path cases with spaces and Unicode pass.

---

## Phase 6 — Output and application integration

**Goal:** integrate the new administrative modes cleanly, provide equivalent direct JSON files, and add the human-facing total-time footer without changing machine contracts.

### Work

- Extend invocation-mode resolution for suppression status and path explanation.
- Add final CLI conflicts/requirements from the architecture.
- Keep `--no-ignore-files` as discovery policy, not a terminal mode.
- Reuse existing output selection for text/JSON administrative stdout.
- Reuse `--report json=PATH` for supplemental files from the two new administrative modes only.
- Reject text/Markdown/SARIF supplemental targets for those modes.
- Extract only the minimal atomic string-write primitive needed from existing output code.
- Render stdout and supplemental JSON from the same typed administrative model and renderer.
- Reuse existing `OperationalError` / `error-v1` semantics.
- Leave `capabilities-v1` unchanged as an immutable published contract.
- Add one monotonic elapsed timer for normal scan execution.
- Append `Total time: <duration>` to completed primary text scan output after normal scan/report work succeeds.
- Format elapsed time adaptively as microseconds below 1 ms, milliseconds below 1 s, and seconds at or above 1 s.
- Keep timing out of JSON, Markdown, SARIF, supplemental report files, report-v4, and every versioned machine contract.
- Do not print the timing footer for exit-2 failures or incomplete scans.
- Do not add per-stage timers, a timing registry, telemetry, or profiling infrastructure.
- Confirm ordinary scan `--report` behavior remains unchanged apart from the primary text footer.
- Confirm the normal scan path does not execute audit detection or targeted explanation work.

### Required invariants

- Administrative JSON file output is semantically equivalent to JSON stdout for the same model.
- Existing scan machine output is unaffected.
- The timing footer is human-facing presentation only and is intentionally excluded from deterministic-output guarantees.
- A completed normal text scan with exit `0` or `1` contains exactly one `Total time:` footer.
- Exit `2` does not gain timing output.
- Supplemental report files do not gain timing output.
- Existing administrative modes do not accidentally gain new supplemental output behavior.
- No reporter registry, timing framework, or generalized output framework is introduced.
- `error-v1`, report-v4, baseline-v1, and capabilities-v1 schemas remain unchanged.

### Gate

CLI mode-composition tests pass, stdout/file equivalence tests pass, atomic-write failures map to exit `2`, timing-format/footer tests pass, machine formats remain timing-free and deterministic, existing output regression suites remain green, and normal-scan performance shows no unexplained v2.1 overhead beyond negligible clock measurement/formatting.

---

## Phase 7 — Integration, documentation, and pre-publication validation

**Goal:** prove the complete v2.1 product contract before publishing prereleases.

### Work

- Add/update focused v2.1 validation tooling, preferably `validation/v2.1.sh` or the smallest equivalent extension.
- Add release-tooling support so `2.1.*` maps to `docs/arid-v2.1-release-roadmap.md`.
- Preserve prior release-family checks in `release.sh --check`.
- Update README for the four new CLI controls, suppression semantics, and the normal text `Total time:` footer.
- Document active/stale lifecycle examples.
- Document disable-through-EOF and repeated-directive no-op behavior.
- Document ignore-file override versus Arid `exclude`.
- Document both new JSON schema locations and machine-contract expectations.
- Document that elapsed timing is human presentation and is intentionally absent from deterministic machine formats.
- Add/update CLI help snapshots or integration checks where used.
- Run source formatting, tests, lint, diff checks, and targeted integration validation.
- Run the established real-world repository validation campaign.
- Run benchmark regression qualification to verify ordinary 2.0 behavior remains performance-stable.
- Perform pre-publication release workflow validation with publication jobs skipped where supported.

### Required validation

At minimum:

```text
suppression state transition tests
suppression active/stale fixtures
suppressed-vs-suppressed audit fixture
boundary-only audit fixture
suppression-status-v1 schema validation
stale exit-policy matrices
discovery-policy fixtures
--no-ignore-files composition
path explanation vs --list-files consistency
path-explanation-v1 schema validation
stdout/file administrative JSON equivalence
normal text scan Total time footer on exit 0
normal text scan Total time footer on exit 1
adaptive timing unit formatting
timing absent from exit-2 output
timing absent from JSON/Markdown/SARIF and supplemental reports
report-v4 compatibility
baseline-v1 compatibility
error-v1 compatibility
ordinary scan finding equivalence
serial/numeric/auto equivalence
real-world detector regression validation
performance regression validation
```

### Gate

All intended 2.1 behavior is code complete, documentation is sufficient for prerelease users, all pre-publication validation passes, and no out-of-scope product behavior has entered the release.

---

## Phase 8 — Alpha publication

**Goal:** publish the first feature-complete v2.1 prerelease and validate the packaged machine contracts.

### Rules

- The intended v2.1 functionality is code complete before alpha publication.
- `suppression-status-v1` and `path-explanation-v1` shapes and vocabularies are frozen for the release line before publication.
- Any incompatible machine-contract correction discovered after publication requires an explicitly versioned contract decision rather than silent schema mutation.
- Product fixes are allowed; new scope is not.

### Evidence to capture

- Release workflow result for `v2.1.0-alpha.N`.
- All supported platform artifacts published successfully.
- PyPI installation smoke.
- Standalone archive smoke.
- New CLI controls execute from published artifacts.
- A normal published text scan displays the `Total time:` footer.
- Published JSON/Markdown/SARIF output remains free of elapsed timing data.
- Both new JSON documents validate against the published schema files from the release.
- Direct JSON files are equivalent to stdout behavior from the same published executable.

### Gate

A published alpha passes artifact/install smoke, normal text timing is visible, machine contracts remain timing-free, and the new administrative machine contracts work from the actual released artifacts on supported release paths.

---

## Phase 9 — Real-world validation and beta stabilization

**Goal:** freeze the v2.1 feature set after real-world use and resolve prerelease defects without scope expansion.

### Rules

- No new product features after beta begins.
- Fixes are limited to correctness defects, compatibility regressions, deterministic-output defects, platform inconsistencies, documentation defects, or release-process defects.
- Any behavior change must remain inside the approved requirements/architecture boundary.

### Work

- Run full real-world detector validation against the beta artifact.
- Exercise suppression status on repositories with representative suppressions.
- Exercise ignore-file override and path explanation across representative repository layouts.
- Verify active/stale and path-explanation determinism across worker/platform modes where applicable.
- Verify the timing footer remains a single concise line across representative normal text scans and timing stays absent from machine formats.
- Run performance comparison against the qualified 2.0 baseline.
- Reconcile README/reference docs with actual beta behavior.
- Prepare substantive stable release notes early enough that final RC-to-stable promotion remains metadata-only.

### Gate

The beta is feature-frozen, real-world validation passes, machine contracts remain stable, ordinary detector behavior matches 2.0 expectations, the timing footer behaves as specified, and no material unexplained performance regression remains.

---

## Phase 10 — Release Candidate

**Goal:** qualify a build believed ready for stable `2.1.0` without product-code changes.

### Rules

- No new product features after RC.
- Fixes are limited to qualification failures, regressions, packaging defects, documentation defects required for correctness, or release-process defects.
- Any source-code change after RC requires affected validation gates to be rerun and generally requires another RC.
- Stable `docs/releases/v2.1.0.md` is substantively complete before final RC qualification.

### Qualification

The RC qualification MUST include:

- release workflow success across all supported platforms
- PyPI and standalone installation smoke
- published schema presence
- `suppression-status-v1` validation against published output
- `path-explanation-v1` validation against published output
- stale-policy exit matrix on the published executable
- direct JSON file equivalence on the published executable
- normal text `Total time:` footer on the published executable
- absence of timing data from JSON/Markdown/SARIF machine-oriented output
- baseline-v1 compatibility
- report-v4 compatibility
- ordinary detector real-world validation
- release benchmark qualification
- release-note/metadata checks

### Gate

The RC passes the complete qualification suite and is acceptable for stable promotion with no product-code changes.

---

## Phase 11 — Stable promotion

**Goal:** promote the qualified RC to `2.1.0`.

### Rules

- Stable promotion is metadata-only when the RC remains qualified.
- The stable tag must point to the exact promoted commit.
- GitHub and PyPI publication must be verified.
- RC-to-stable repository delta must contain only expected release metadata/documentation state changes.
- Stable qualification verifies published artifacts and the metadata-only promotion path without unnecessarily repeating unrelated development work.

### Gate

`2.1.0` is published and installable on supported targets, GitHub release assets are complete, published machine-contract smoke checks pass, and stable qualification passes.

---

## Phase 12 — Closeout

**Goal:** record the actual v2.1 release outcome and leave the repository in a fully closed stable state.

### Work

- Mark completed roadmap phases and record final evidence.
- Record published alpha/beta/RC/stable tags actually used.
- Record qualification results and any release-process corrections.
- Confirm curated stable release notes reflect the shipped behavior.
- Confirm README project status is stable.
- Confirm `main` contains the final stable metadata and roadmap state.
- Confirm no committed v2.1 product work remains unresolved.
- Leave explicitly deferred ideas as future product decisions rather than unfinished v2.1 work.

### Gate

The roadmap accurately describes the shipped release, stable qualification is recorded, and the following statement is true:

> **Arid 2.1 preserves the 2.0 detector while making suppression maintenance auditable and enforceable, making individual discovery decisions explainable, allowing ignore-file traversal to be bypassed without bypassing Arid policy, exposing the new administrative state through deterministic versioned machine contracts, and making normal human-facing scans visibly fast through a concise total-time footer.**

---

## Release completion criterion

Arid 2.1 is complete when:

```text
normal duplicate semantics remain unchanged
        +
effective suppressions are deterministically active or stale
        +
stale suppression/baseline maintenance can fail CI when requested
        +
one path can be explained against actual discovery policy
        +
ignore-file traversal can be disabled without disabling Arid policy
        +
suppression-status-v1 and path-explanation-v1 are stable and deterministic
        +
completed normal text scans display a concise Total time footer
        +
machine-readable outputs remain free of volatile timing data
        +
published artifacts pass the established release qualification path
```

No additional feature is required merely because it might be useful in a future 2.x release.
