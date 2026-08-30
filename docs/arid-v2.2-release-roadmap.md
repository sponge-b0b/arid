# Arid v2.2 Release Roadmap

**Product:** Arid  
**Stable target:** `2.2.0`  
**Current phase:** Stable

## Purpose

Arid v2.2 is a focused signal-extraction and presentation release built around one principle:

> **High-signal duplicate intelligence. Same focused detector.**

V2.2 deliberately preserves Arid 2.1 duplicate-detection semantics while making the information Arid already produces substantially easier to understand at a glance and easier for CI systems, tools, and coding agents to consume compactly.

The release is intended to deliver six outcomes:

1. **Rich human summary** — normal text scans end with compact overall metrics plus Context, Scope, and Distribution breakdowns.
2. **Objective hotspots** — the top files participating in reportable duplicate groups are visible without scrolling through every finding.
3. **Concise scan mode** — `--summary-only` suppresses detailed findings without changing analysis or exit policy.
4. **Compact machine contract** — `summary-v1` exposes deterministic high-level duplicate state without requiring consumers to parse every report-v4 finding.
5. **Better Action integration** — the official GitHub Action exposes occurrences, files with duplicates, and summary JSON from the same scan.
6. **Adaptive parallelism by default** — ordinary scans use the existing bounded automatic preparation-worker policy while explicit serial and numeric worker controls remain available.

The detector itself is not redesigned for v2.2.

## Governing documents

The v2.2 implementation contract is defined by:

- `docs/arid-v2.2-requirements-specification.md`
- `docs/arid-v2.2-technical-architecture-and-design.md`
- this roadmap

The requirements define externally observable behavior. The architecture defines technical boundaries and implementation order. This roadmap defines delivery phases, evidence, and release gates.

If these documents disagree, implementation MUST stop and the contract documents MUST be reconciled before proceeding.

## Scope boundary

### In scope

- rich normal-text Summary table;
- files-with-duplicates count;
- total occurrence count;
- Context group-count breakdown;
- Scope group-count breakdown;
- Distribution group-count breakdown;
- group-share percentages in human text;
- semantic summary color using existing `--color` policy;
- deterministic top-five Hotspots;
- `--summary-only` as a CLI-only normal-scan presentation modifier;
- `summary-v1` deterministic JSON;
- `schemas/summary-v1.schema.json`;
- summary-only JSON through `--summary-only --json` / `--format json`;
- preservation of existing supplemental full reports under `--summary-only`;
- GitHub Action outputs:
  - `occurrences`
  - `files-with-duplicates`
  - `summary-json`
- bounded adaptive parallelism as the implicit worker default;
- focused v2.2 validation;
- README/reference documentation;
- release-tooling support for 2.2;
- Rust/rustfmt reproducibility correction identified during v2.1 closeout.

### Explicitly out of scope

V2.2 does not add:

- semantic or fuzzy clone detection;
- identifier-renaming clone detection;
- structural similarity as duplicate identity;
- token-based clone metrics;
- another diagnostic beyond `DUP001`;
- severity levels;
- health scores;
- quality grades;
- percentage-based quality thresholds;
- automated refactoring advice;
- AI-generated remediation;
- Git blame/ownership analysis;
- changed-since Git integration;
- trend persistence;
- persistent caching;
- general analytics dashboards;
- per-file duplicate-line metrics without interval-union semantics;
- configurable hotspot scoring;
- configurable hotspot limits;
- a new report-v4 version merely for summary fields;
- a generic reporter framework;
- a generic table framework;
- MCP/server operation;
- daemon/RPC operation;
- LSP/editor integration;
- plugin architecture.

## Compatibility boundary

V2.2 is additive except for an intentional improvement to normal text presentation.

The following remain unchanged from 2.1:

```text
exact duplicate semantics
DUP001
finding fingerprints
report-v4 schema and meaning
baseline-v1 schema and meaning
error-v1
suppression-status-v1
path-explanation-v1
normal suppression semantics
discovery semantics
focus semantics
baseline filtering semantics
normalization semantics
[tool.arid] behavior
explicit --workers 1 behavior
explicit numeric --workers N behavior
--workers auto semantics
implicit worker default changes from 1 to auto
ordinary-scan exit meanings 0 / 1 / 2
--no-fail-on-findings behavior
Markdown report semantics
SARIF semantics
supplemental --report semantics
Total time machine-output exclusion
supported platform set
```

Intentional normal-text change:

```text
old two-line finding totals
        ↓
rich Summary + Breakdown + Hotspots
```

New CLI control:

```text
--summary-only
```

New machine contract:

```text
summary-v1
```

## Development and release discipline

Implementation proceeds inside-out / bottom-up.

The summary counting model is made correct and deterministic before CLI/output integration.

No phase may introduce:

- another definition of duplication;
- another baseline/focus policy;
- another detector pass;
- renderer-specific summary arithmetic;
- naïve overlapping per-file line arithmetic;
- synthetic quality scoring;
- a generic reporting framework;
- MCP/server infrastructure.

The expected release path is:

```text
2.2.0-alpha.N
    ↓
2.2.0-beta.N
    ↓
[2.2.0-rc.N when beta qualification requires a corrected frozen build]
    ↓
2.2.0
```

Additional prereleases are created only when a distinct qualification risk, failed gate, or material fix requires them.

An RC is conditional rather than ceremonial. If the published beta satisfies the remaining stabilization gates without a material product/package correction, stable may proceed directly after stable-readiness qualification.

The final qualified-prerelease-to-stable promotion MUST remain metadata/documentation-only.

---

## Phase 0 — Contract

**Goal:** freeze the v2.2 product boundary, technical architecture, and release plan before implementation begins.

**Status:** Complete.

### Artifacts

- `docs/arid-v2.2-requirements-specification.md`
- `docs/arid-v2.2-technical-architecture-and-design.md`
- `docs/arid-v2.2-release-roadmap.md`

### Gate

- All three documents agree on the five v2.2 outcomes.
- Summary input is the final typed report after baseline and focus policy.
- Occurrences and files-with-duplicates counting semantics are explicit.
- Context/Scope/Distribution buckets and row order are explicit.
- Hotspot ranking and top-five limit are explicit.
- `--summary-only` is presentation-only.
- `summary-v1` is separate from report-v4.
- Supplemental reports remain full existing report formats.
- Timing remains absent from machine contracts.
- Action integration requires no second detector run.
- No implementation begins while a known contract contradiction remains.

**Gate result:** PASS.

---

## Phase 1 — Summary domain and counting foundation

**Status:** Complete.

**Goal:** establish one deterministic summary projection from the final typed `Report` without changing output yet.

### Work

- Add the small summary domain model.
- Add fixed Context/Scope/Distribution bucket structs.
- Derive overall counts from existing report metrics.
- Sum total finding occurrences.
- Count distinct reported paths for files with duplicates.
- Build per-path hotspot accumulation.
- Ensure one finding contributes at most one hotspot group count per path.
- Count every location toward hotspot occurrences.
- Sort hotspots by groups desc → occurrences desc → path asc.
- Truncate public/human hotspot results to five.
- Add `SummaryAnalysis` including effective ignore-file policy.
- Preserve report-v4 untouched.

### Required invariants

- No detector rerun.
- No filesystem reread.
- No baseline/focus logic inside summary aggregation.
- Context bucket sum equals duplicate groups.
- Scope bucket sum equals duplicate groups.
- Distribution bucket sum equals duplicate groups.
- `files_with_duplicates` is distinct path count.
- occurrence count is finding-occurrence membership sum.
- hotspot ordering is deterministic.

### Gate

Unit fixtures prove all counting semantics, hybrid same-path behavior, bucket invariants, hotspot ties, top-five truncation, zero findings, and incomplete report projection.

---

## Phase 2 — `summary-v1` machine contract

**Status:** Complete.

**Goal:** freeze and validate the compact deterministic machine/agent representation before human rendering depends on it.

### Work

- Add `schemas/summary-v1.schema.json`.
- Add deterministic JSON serialization from the typed Summary.
- Include complete analysis metadata required by the requirements.
- Include structured errors and completeness.
- Include overall counts.
- Include fixed Context/Scope/Distribution count objects.
- Include deterministic top-five hotspots.
- Keep elapsed timing out of the type.
- Add representative complete, zero-finding, and incomplete schema fixtures.
- Verify worker determinism.

### Required invariants

- `schema_version` is `1`.
- report-v4 is unchanged.
- Bucket percentages are not redundantly serialized.
- Human formatting such as commas is not serialized.
- Timing is absent.
- Color is absent.
- Output is deterministic for logically identical scans.

### Gate

Representative documents validate against `summary-v1`; worker 1/numeric/auto outputs are logically and byte deterministic where the existing environment permits; existing report-v4 schema/fixtures remain compatible.

---

## Phase 3 — Rich human summary, breakdown, hotspots, and color

**Status:** Complete.

**Goal:** replace the minimal two-line normal text footer with a compact, information-dense human summary.

### Work

- Add the overall Summary table.
- Add thousands separators for human counts.
- Add files-with-duplicates percentage for text presentation.
- Add Breakdown table.
- Use fixed categorical row order:
  - Context: executable, declarative, mixed
  - Scope: function, module, class, mixed
  - Distribution: cross-file, same-file, hybrid
- Add group-share percentages.
- Add top-five Hotspots table.
- Reuse full project-relative paths from finding locations.
- Add a small private deterministic table renderer.
- Reuse semantic `TextStyles`.
- Keep borders/default totals neutral.
- Keep Context/Scope classification color.
- Keep Distribution color.
- Use existing group/problem/success emphasis where semantically appropriate.
- Make plain and colored visible output equivalent after stripping ANSI.
- Preserve Unicode path support.
- Keep `Total time:` after the summary sections.

### Required invariants

- No severity semantics from color.
- No green/red quality thresholds.
- No environment-dependent terminal-width layout.
- `--color never` contains no ANSI.
- Table arithmetic comes from the typed Summary only.
- Zero-finding output remains concise.
- Incomplete keep-going output is explicitly marked incomplete.

### Gate

Plain/color golden tests pass, Unicode hotspot paths remain legible, zero/incomplete outputs are clear, and detailed normal text ends with the agreed summary layout and existing timing footer.

---

## Phase 4 — `--summary-only` orchestration

**Status:** Complete.

**Goal:** provide a concise normal scan for developers, CI logs, tools, and agents without creating another analysis path.

### Work

- Add `--summary-only` to CLI parsing.
- Keep it CLI-only.
- Add terminal-mode conflicts.
- Add the primary output matrix:
  - text → summary text only
  - JSON → `summary-v1`
  - Markdown → reject
  - SARIF → reject
- Suppress individual `DUP001` blocks only for primary text.
- Preserve normal scan pipeline and exit policy.
- Preserve `--no-fail-on-findings`.
- Preserve baseline and focus behavior.
- Preserve keep-going behavior.
- Preserve supplemental `--report` full output.
- Preserve `Total time:` only for completed primary text scans.

### Required invariants

- Detailed and summary-only scans produce identical logical summary values.
- Detailed and summary-only scans return the same pre-`--no-fail-on-findings` scan status.
- `--summary-only --json` is timing-free.
- `--summary-only --report json=...` writes full report-v4 to the supplemental file.
- No second detector run occurs.

### Gate

Complete CLI composition matrix passes, exit statuses match detailed mode, baseline/focus fixtures match, supplemental report compatibility passes, and summary-only JSON validates against `summary-v1`.

---

## Phase 5 — GitHub Action integration

**Status:** Complete.

**Goal:** expose compact high-signal summary data from the official Action without changing its one-scan architecture.

### Work

- Add Action outputs:
  - `occurrences`
  - `files-with-duplicates`
  - `summary-json`
- Preserve all existing outputs.
- Preserve existing failure policy.
- Preserve one Arid invocation per Action execution.
- Prefer deriving the new Action outputs from the existing supplemental report-v4 adapter unless consuming primary `summary-v1` can preserve current Action behavior more cleanly.
- If Action-side derivation is used, add parity fixtures against core `summary-v1` semantics.
- Include effective ignore-file policy in the Action summary contract.
- Keep output encoding safe for GitHub's output-file protocol.

### Required invariants

- No second Arid scan.
- `occurrences` matches core summary semantics.
- `files-with-duplicates` matches core summary semantics.
- `summary-json` matches the `summary-v1` logical contract.
- Existing Action users retain existing failure/output behavior.

### Gate

Action unit/integration tests pass, parity fixtures cover repeated/hybrid locations and hotspot ties, and a real workflow proves the new outputs can be consumed by a following step.

---

## Phase 6 — Integration, documentation, toolchain reproducibility, and pre-publication validation

**Status:** Complete.

**Goal:** prove the complete v2.2 product contract before publishing prereleases and repair the release-tooling reproducibility issue identified during v2.1 closeout.

### Product/documentation work

- Update README for the rich Summary.
- Document Breakdown semantics.
- Document Hotspots semantics and top-five limit.
- Document `--summary-only`.
- Document `summary-v1` and schema location.
- Document `--json` versus `--summary-only --json`.
- Document baseline interaction.
- Document focus interaction, including outside-focus retained occurrences.
- Document incomplete keep-going behavior.
- Document semantic color.
- Document new Action outputs.
- Update CLI help snapshots/integration checks where applicable.

### Toolchain/process work

- Define the known-good Rust/rustfmt policy for v2.2.
- Pin the toolchain in-repository if that is the smallest durable correction.
- Ensure `cargo fmt --check` is reproducible across v2.2 qualification stages.
- Avoid gratuitous reformat-only churn caused solely by an advanced local formatter.

### Validation work

- Add/update `validation/v2.2.sh`.
- Inherit relevant v2.1 validation.
- Run formatting, tests, Clippy, release build, diff checks, and clean-tree checks.
- Run summary-v1 schema validation.
- Run worker determinism checks.
- Run established real-world repository validation.
- Run v2.1 → v2.2 performance regression qualification.
- Run non-publishing release workflow validation across supported platforms.

### Required validation

At minimum:

```text
summary count fixtures
bucket invariants
hotspot ranking/truncation
plain/color parity
zero/incomplete text
summary-only mode matrix
summary-v1 complete/zero/incomplete schemas
summary-v1 worker determinism
baseline/focus summary parity
supplemental full-report preservation
report-v4 compatibility
Markdown compatibility
SARIF compatibility
Action output parity
Action one-scan invariant
real-world detector regression
performance regression
reproducible formatting gate
```

### Qualification evidence

Pre-publication qualification completed against the `v2.2` branch after all intended product work was integrated.

Repository/source qualification:

- Rust `1.97.1`, rustfmt `1.9.0-stable`, and Clippy `0.1.97` were pinned and verified.
- `cargo fmt --all -- --check` passed.
- the complete locked Rust test suite passed.
- Clippy passed across all targets/features with `-D warnings`.
- Action and release Python unit tests passed.
- the locked release build passed.
- `validation/v2.2.sh` passed.
- release metadata dry-run for `2.2.0-alpha.1` passed.
- branch-wide diff and clean-tree checks passed.

Real-world regression qualification:

- Black passed.
- Django passed, including deterministic output across workers `1`, `2`, `4`, and `8`.
- Mypy passed.
- Rich passed.
- Unicode, spaces, non-ASCII filename, relative-path, and source-location cases passed.
- no detector-behavior regression requiring a v2.2 product correction was identified.

Stable `2.1.0` → v2.2 candidate performance qualification used two Hyperfine passes per corpus with reversed command order, three warmups per pass, and ten measured runs per pass:

| Corpus | 2.1 implicit | v2.2 implicit | v2.2 `--workers 1` | v2.2 `--workers auto` | implicit change |
| --- | ---: | ---: | ---: | ---: | ---: |
| requests | 17.0 ms | 16.4 ms | 17.2 ms | 15.0 ms | -3.6% |
| pydantic | 251.2 ms | 134.9 ms | 244.9 ms | 132.2 ms | -46.3% |
| polaris | 521.4 ms | 303.2 ms | 518.8 ms | 305.9 ms | -41.8% |
| duplicate-heavy | 61.6 ms | 39.2 ms | 62.1 ms | 42.1 ms | -36.3% |

The explicit v2.2 serial path remained effectively at stable 2.1 performance while the new bounded automatic default materially improved medium, large, and duplicate-heavy workloads.

Release-workflow qualification:

- the non-publishing `Release` workflow passed from `v2.2`;
- release verification passed;
- Linux x86-64 passed;
- Linux aarch64 passed;
- macOS aarch64 passed;
- macOS x86-64 passed;
- Windows x86-64 passed;
- tag-gated PyPI and GitHub publication did not execute.

### Gate

All intended 2.2 behavior is code complete, documentation is sufficient for prerelease users, formatting is reproducible, pre-publication validation passes, and no out-of-scope product behavior has entered the release.

**Gate result:** PASS.

---

## Phase 7 — Alpha publication

**Status:** Complete.

**Goal:** publish the first feature-complete v2.2 prerelease and freeze the new machine/presentation contracts with real packaged artifacts.

### Rules

- Intended v2.2 functionality is code complete before alpha publication.
- `summary-v1` field names and vocabularies are frozen before publication.
- Hotspot ranking and top-five semantics are frozen.
- Summary-only JSON semantics are frozen.
- Product fixes are allowed after alpha; new scope is not.

### Qualification

Published alpha qualification MUST include:

- production release workflow success across supported targets;
- GitHub prerelease assets;
- exact PyPI installation;
- exact standalone installation;
- full `validation/v2.2.sh` against published artifacts;
- `summary-v1` schema validation from published executables;
- deterministic summary-only JSON;
- rich text summary smoke;
- color/no-color smoke;
- Action version pin smoke;
- report-v4 compatibility.

### Qualification evidence

`2.2.0-alpha.1` was published from the feature-complete v2.2 release commit and passed published-artifact qualification.

Publication and packaging:

- the tag-triggered production Release workflow completed successfully;
- the GitHub release was created as a prerelease;
- all five supported standalone assets were published:
  - Linux x86-64
  - Linux aarch64
  - macOS aarch64
  - macOS x86-64
  - Windows x86-64
- exact PyPI version `2.2.0a1` installed successfully;
- the published Linux x86-64 standalone archive passed its published SHA-256 digest check;
- both published Linux distribution paths reported `arid 2.2.0-alpha.1`.

Published-contract qualification:

- the complete `validation/v2.2.sh` suite passed against the published standalone binary;
- the complete `validation/v2.2.sh` suite passed against the exact PyPI-installed binary;
- inherited v2, v2.1, and v2.2 contracts passed from both artifacts;
- rich normal-text Summary, Breakdown, and Hotspots passed;
- summary-only text and JSON passed;
- complete, incomplete, and zero-finding `summary-v1` schema validation passed;
- baseline/focus summary behavior passed;
- supplemental report-v4 preservation passed;
- plain/color visible-output parity passed;
- adaptive worker determinism passed;
- standalone and PyPI `summary-v1` output was byte-identical;
- standalone and PyPI report-v4 output was byte-identical.

Published Action qualification:

- the tagged Action pins exact PyPI version `2.2.0a1`;
- the production Release workflow's published-Action verification job passed;
- the Action fixture and output verification passed from the tagged release source.

No alpha qualification defect requires a product or packaging correction before beta.

### Gate

A published alpha passes artifact/install validation and demonstrates the complete v2.2 summary contract from actual distributed binaries.

**Gate result:** PASS.

---

## Phase 8 — Real-world validation and beta stabilization

**Status:** Complete.

**Goal:** freeze v2.2 after real-world use and resolve prerelease defects without scope expansion.

### Rules

- No new product features after beta begins.
- Fixes are limited to correctness defects, compatibility regressions, deterministic-output defects, platform inconsistencies, documentation defects, or release-process defects.

### Work

- Run full established real-world detector validation against the beta artifact.
- Inspect rich summaries on small, medium, and large repositories.
- Validate hotspot usefulness on real repositories.
- Validate summary-only output in normal developer and CI use.
- Validate `summary-v1` with tool/agent-style consumption.
- Validate Action outputs in a real workflow.
- Verify Unicode paths and supported terminals/platforms.
- Verify deterministic summaries across worker modes.
- Run published-to-published performance comparison against stable 2.1.
- Reconcile README/reference docs with actual beta behavior.
- Prepare substantive stable release notes before stable-readiness qualification.

### Qualification evidence

Published beta boundary:

- `v2.2.0-beta.1` was published from commit `af35957c8169284792b4c4f9bce9fe1fce65b1d2`;
- the production Release workflow completed successfully;
- all five supported standalone assets were published;
- exact PyPI `arid==2.2.0b1` installed successfully;
- the published Linux x86-64 standalone digest matched the GitHub release digest;
- both standalone and PyPI artifacts passed `validation/v2.2.sh`;
- standalone/PyPI `summary-v1` output was byte-identical;
- standalone/PyPI report-v4 output was byte-identical.

Real-world detector campaign:

- Black: PASS;
- Django: PASS;
- mypy: PASS;
- Rich: PASS;
- Unicode/space/non-ASCII path cases: PASS;
- detector behavior remained consistent with the established 2.1 expectations.

Human presentation:

- Summary, Breakdown, and Hotspots remained legible from small through large repositories;
- objective hotspot ranking produced useful application-code results;
- Rich demonstrated that generated/data-heavy paths can objectively dominate hotspots and that ordinary Arid exclusion policy produces the intended application-code view without a new heuristic;
- `--summary-only` developer use passed;
- semantic color was visually reviewed as restrained, readable, and semantically sensible;
- stripping ANSI from forced-color output produced the same visible content as plain output after excluding the intentionally volatile timing footer.

Machine contract and determinism:

- real Django `summary-v1` validated against `schemas/summary-v1.schema.json`;
- default, `auto`, and explicit workers 1, 2, 4, and 8 produced byte-identical `summary-v1`;
- aggregate counts, Context, Scope, Distribution, and Hotspots were independently reproduced from report-v4;
- timing, worker count, and color state remained excluded from `summary-v1`;
- compact tool/agent-style consumption passed.

GitHub Action:

- the published beta Action executed successfully in the production release workflow;
- beta review identified a release-process coverage gap: the workflow did not assert the new `occurrences`, `files-with-duplicates`, and `summary-json` outputs;
- the production gate was strengthened after beta without changing Action or detector behavior;
- non-publishing Release workflow run `33296962210` passed with all three new outputs asserted, including logical `summary-json` contents;
- the qualification-visible job identity `Verify published GitHub Action` was preserved.

Published-to-published performance:

| Corpus | 2.1 implicit | 2.2 implicit | 2.2 `--workers 1` | 2.2 `--workers auto` | implicit change |
| --- | ---: | ---: | ---: | ---: | ---: |
| requests | 16.9 ms | 15.4 ms | 16.3 ms | 15.6 ms | -9.1% |
| pydantic | 239.7 ms | 129.6 ms | 237.2 ms | 126.6 ms | -45.9% |
| polaris | 501.5 ms | 293.9 ms | 497.6 ms | 295.7 ms | -41.4% |
| duplicate-heavy | 61.2 ms | 40.6 ms | 59.3 ms | 38.4 ms | -33.8% |

No material unexplained performance regression was observed.

Documentation and release state:

- README behavior documentation was reconciled with beta behavior and measured performance;
- release qualification reference documentation was reconciled with the current multi-series harness;
- substantive stable `2.2.0` release notes were prepared before stable-readiness qualification;
- no v2.2 product, packaging, detector, or machine-contract correction was required after beta;
- the only post-beta executable-repository change was the release-workflow qualification correction.

### Gate

The beta is feature-frozen, real-world output is useful and legible, machine contracts remain stable, detector behavior matches 2.1 expectations, Action integration is reliable, and no material unexplained performance regression remains.

**Gate result:** PASS.

---

## Phase 9 — Release Candidate

**Goal:** qualify one corrected build believed ready for stable `2.2.0` without further product-code changes.

**Status:** Required.

### Decision record

The beta product itself satisfied every stabilization gate without a product, packaging, compatibility, deterministic-output, machine-contract, Action-implementation, or platform correction.

An RC is nevertheless required because beta qualification identified and corrected a production release-workflow assertion gap after `v2.2.0-beta.1` was published. The corrected workflow now verifies all three v2.2 Action summary outputs and preserves the job identity expected by release qualification.

The release qualification harness accepts RC and stable releases, requires a qualified RC before stable promotion, and requires the RC-to-stable repository delta to be the exact managed metadata transition. Because `.github/workflows/release.yml` changed after the beta tag, direct beta-to-stable promotion would not satisfy that invariant.

The RC therefore freezes the already-qualified beta product together with the corrected production release gate. No additional product scope or product-code change is planned.

### Qualification

The RC qualification MUST include:

- supported platform release workflow;
- exact PyPI/standalone smoke;
- `summary-v1` validation;
- summary-only JSON determinism;
- rich text/color validation;
- Action outputs;
- report-v4 compatibility;
- baseline/focus compatibility;
- real-world detector regression;
- release benchmark qualification;
- release-note/metadata checks.

### Gate

The RC passes complete qualification and is acceptable for stable promotion with no product-code changes.

---

## Phase 10 — Stable promotion

**Goal:** promote the final qualified prerelease to `2.2.0`.

### Rules

- Stable promotion is metadata/documentation-only while qualified prerelease behavior remains unchanged.
- Stable tag points to the exact promoted commit.
- GitHub and PyPI publication are verified.
- Qualified-prerelease-to-stable repository delta contains only expected release metadata/documentation changes.
- Published stable artifacts receive final contract qualification.

### Gate

`2.2.0` is published and installable on supported targets, GitHub assets are complete, PyPI is correct, `summary-v1` and human summary smoke checks pass from published artifacts, Action stable pin works, and stable qualification passes.

---

## Phase 11 — Closeout

**Goal:** record the actual v2.2 release outcome and leave the repository in a fully closed stable state.

### Work

- Mark completed roadmap phases and record final evidence.
- Record actual alpha/beta/RC/stable tags used.
- Record published-artifact qualification.
- Record performance evidence.
- Record Action qualification.
- Record toolchain reproducibility outcome.
- Confirm stable release notes reflect shipped behavior.
- Confirm README project status is stable.
- Confirm `main` contains final stable metadata/history.
- Confirm no committed v2.2 product work remains unresolved.
- Leave deferred ideas as future product decisions rather than unfinished v2.2 scope.

### Gate

The roadmap accurately describes the shipped release, stable qualification is recorded, and the following statement is true:

> **Arid 2.2 preserves the focused exact detector while turning its existing duplicate evidence into a concise, deterministic summary that developers can understand at a glance and tools or coding agents can consume without parsing every finding.**

---

## Release completion criterion

Arid 2.2 is complete when:

```text
2.1 duplicate semantics remain unchanged
        +
one typed summary is derived from final reportable findings
        +
overall duplicate state is visible at a glance
        +
Context / Scope / Distribution group shares are visible
        +
objective top-file hotspots are visible
        +
--summary-only preserves analysis and exit semantics
        +
summary-v1 is stable and deterministic
        +
existing full machine reports remain compatible
        +
GitHub Action exposes compact summary data without a second scan
        +
published artifacts pass established release qualification
```

No additional feature is required merely because it might be useful in a future 2.x release.
