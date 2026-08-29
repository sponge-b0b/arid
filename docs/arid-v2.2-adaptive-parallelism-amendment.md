# Arid v2.2 Adaptive Parallelism Contract Amendment

**Status:** Approved contract amendment  
**Product:** Arid  
**Stable target:** `2.2.0`  
**Branch:** `v2.2`  
**Amends:**

- `docs/arid-v2.2-requirements-specification.md`
- `docs/arid-v2.2-technical-architecture-and-design.md`
- `docs/arid-v2.2-release-roadmap.md`

---

## 1. Decision

Arid 2.2 changes the implicit worker default from serial execution to bounded automatic parallelism.

The product contract becomes:

```text
arid .
```

is execution-equivalent to:

```text
arid . --workers auto
```

for worker selection.

The existing explicit forms remain supported:

```text
arid . --workers 1
arid . --workers N
arid . --workers auto
```

This change affects execution policy only. It MUST NOT change duplicate identity, findings, ordering, metrics, fingerprints, exit semantics, or deterministic machine contracts.

The v2.2 release theme remains:

> **High-signal duplicate intelligence. Same focused detector.**

Adaptive parallelism is a developer-experience and default-performance improvement around that same detector.

---

## 2. Motivation and Observed Evidence

The change is motivated by direct current-tree observation on Polaris using ordinary user-facing invocations:

```text
jscpd .                  441.6 ms
arid .                   640.9 ms
arid . --workers auto    299.4 ms
arid . --workers 4       306.5 ms
```

These samples are motivating evidence, not a release benchmark or performance guarantee.

They demonstrate that the apparent default-performance disadvantage is primarily caused by Arid selecting one worker implicitly while its existing bounded `auto` mode already provides substantially lower wall-clock latency on a large real-world repository.

Existing Arid performance evidence also established that:

- source preparation, especially Python parsing, is the dominant broad cost on large repositories;
- preparation is safely parallelizable under the existing worker model;
- `auto` captures useful wall-clock parallelism on medium, large, and duplicate-heavy corpora;
- automatic selection is intentionally capped rather than consuming every logical CPU by default.

The v2.2 decision therefore changes the default selection policy instead of adding a new concurrency implementation.

---

## 3. Requirements Amendment

The v2.2 requirements specification is amended by the requirements in this section.

### 3.1 New v2.2 goal

Arid 2.2 MUST make bounded automatic worker selection the implicit default.

The v2.2 goal set therefore gains:

> **Adaptive parallelism by default:** when the user does not provide `--workers`, Arid MUST resolve the effective worker count using the same bounded automatic-selection policy as explicit `--workers auto`.

### 3.2 Default worker contract

When `--workers` is omitted, Arid MUST behave for worker selection exactly as if the user supplied:

```text
--workers auto
```

The automatic worker count MUST continue to use the existing policy:

```text
effective workers = clamp(available parallelism, 1, 4)
```

If available parallelism cannot be determined, automatic selection MUST fall back to:

```text
1 worker
```

The automatic cap remains:

```text
4 workers
```

V2.2 MUST NOT silently raise that cap without separate benchmark evidence and an explicit contract amendment.

### 3.3 Explicit worker behavior

Explicit numeric worker selection remains authoritative.

Examples:

```text
--workers 1    -> exactly 1 worker
--workers 2    -> exactly 2 workers
--workers 4    -> exactly 4 workers
--workers 8    -> exactly 8 workers
```

The automatic cap applies only to automatic selection.

V2.2 MUST NOT clamp an explicitly requested positive numeric worker count to 4.

`--workers 0` remains invalid.

### 3.4 Execution scope

The worker model remains the existing Arid worker model.

V2.2 MUST NOT introduce parallel duplicate detection merely because automatic preparation parallelism becomes the default.

Workers continue to apply only where the current implementation already uses worker-count-controlled parallel execution, principally source preparation.

### 3.5 Semantic equivalence

For the same logical scan inputs and configuration, changing worker selection MUST NOT change:

- discovered source identity;
- normalized source semantics;
- duplicate groups;
- occurrence membership;
- group canonicalization;
- finding order;
- fingerprints;
- structural Context;
- structural Scope;
- Distribution;
- duplicate metrics;
- baseline behavior;
- focus behavior;
- suppression behavior;
- exit status;
- report-v4 logical content;
- `summary-v1` logical content;
- deterministic machine-output bytes where the existing contract requires byte determinism.

### 3.6 Machine-contract exclusion

Effective worker count MUST remain execution policy rather than finding semantics.

V2.2 MUST NOT add effective worker count to `summary-v1` merely because automatic selection becomes the default.

This preserves the requirement that logically identical scans remain machine-equivalent across worker modes.

Report-v4 remains unchanged.

### 3.7 CLI and help behavior

CLI help and documentation MUST no longer imply that the implicit default is one worker.

The documented default MUST be `auto` or equivalent language that clearly describes bounded automatic selection.

Explicit `--workers 1` MUST remain the documented mechanism for users who require serial execution or intentionally minimize concurrent CPU use.

### 3.8 Configuration behavior

V2.2 MUST NOT add a new `[tool.arid]` worker setting solely for this change.

The change is to the existing CLI default-selection policy.

### 3.9 GitHub Action behavior

The official GitHub Action requires no new worker input for v2.2.

When the Action invokes a normal Arid scan without an explicit `--workers` argument, it SHOULD naturally receive the new bounded automatic default.

Existing Action users MAY continue to pass an explicit worker value through supported additional arguments when they need to override the default.

No second scan or Action-side concurrency layer is permitted.

---

## 4. Architecture Amendment

The v2.2 technical architecture is amended by this section.

### 4.1 Reuse the existing automatic resolver

V2.2 MUST reuse the existing automatic worker-count resolution semantics rather than adding a second adaptive-parallelism implementation.

Conceptually:

```rust
fn auto_worker_count(available: Option<usize>) -> usize {
    available.unwrap_or(1).clamp(1, MAX_AUTO_WORKERS)
}
```

The implementation detail MAY differ, but there MUST be one authoritative automatic worker-count policy.

### 4.2 CLI default resolution

The current implicit numeric default of `1` MUST be replaced by an implicit `auto` resolution.

Conceptually, CLI parsing should behave as though the default textual value were:

```text
auto
```

and route that value through the same parser/resolver used by explicit:

```text
--workers auto
```

The implementation SHOULD avoid separate code paths for implicit-auto and explicit-auto selection.

### 4.3 No concurrency redesign

The architecture MUST NOT add:

- a second thread pool;
- asynchronous runtime infrastructure;
- parallel suffix-array construction solely for this feature;
- parallel report rendering;
- a work-stealing framework beyond the existing Rayon-based preparation model;
- unbounded automatic CPU use;
- persistent worker pools or daemon state.

This is a default-policy change around an already-qualified bounded parallel path.

### 4.4 Deterministic collection remains mandatory

Parallel completion order MUST remain hidden behind deterministic result collection.

Existing source-order and error-order guarantees remain in force.

Automatic parallelism MUST NOT weaken deterministic behavior merely because it becomes the ordinary execution path.

### 4.5 Failure semantics

Worker-pool creation and worker execution failures retain existing operational-error behavior.

The new default MUST NOT convert internal worker failures into partial success or silently retry with different semantics.

### 4.6 Resource policy

The automatic cap of four workers is a deliberate product resource policy.

Its purpose is to obtain most useful preparation parallelism without making ordinary `arid .` aggressively consume every logical CPU on large developer or CI machines.

Explicit numeric worker selection remains the escape hatch for users who intentionally want a different resource profile.

---

## 5. Roadmap Amendment

The v2.2 roadmap is amended by this section.

### 5.1 Scope boundary

Add to v2.2 in-scope behavior:

```text
bounded adaptive parallelism as the implicit worker default
```

Remove `serial default` from the list of behaviors preserved unchanged from 2.1.

Replace it with:

```text
explicit --workers 1 behavior unchanged
explicit numeric --workers N behavior unchanged
--workers auto semantics unchanged
implicit worker default changes from 1 to auto
```

### 5.2 Phase 0 contract gate

Phase 0 MUST additionally verify:

- implicit worker selection is `auto`;
- automatic selection remains capped at 4;
- inability to determine available parallelism falls back to 1;
- explicit numeric worker selection is unchanged;
- worker mode remains execution-only and absent from deterministic summary semantics;
- no new concurrency subsystem is introduced.

### 5.3 Implementation placement

The implementation MAY be completed alongside the v2.2 CLI orchestration work because it is a small change to existing worker parsing/default behavior.

It MUST be independently unit-tested rather than relying only on end-to-end timing observations.

Required tests include at least:

```text
implicit default resolves identically to explicit auto
auto with 1 available CPU resolves to 1
auto with 2 available CPUs resolves to 2
auto with 4 available CPUs resolves to 4
auto with more than 4 available CPUs resolves to 4
auto with unavailable CPU count resolves to 1
explicit 1 remains 1
explicit 4 remains 4
explicit value above 4 remains explicit
worker 0 remains invalid
```

### 5.4 Integration and performance qualification

The v2.2 pre-publication performance campaign MUST compare at least:

```text
stable 2.1 implicit default
v2.2 implicit default
v2.2 --workers 1
v2.2 --workers auto
```

on the established representative corpus classes:

```text
small
medium
large
duplicate-heavy
```

Where practical, the campaign SHOULD continue the established jscpd comparison as external context, but Arid's release gate is regression against its own qualified stable baseline rather than beating another tool on every corpus.

### 5.5 Performance acceptance

The new default MUST demonstrate that:

- medium and large representative workloads obtain a meaningful wall-clock benefit when hardware parallelism is available;
- duplicate-heavy workloads do not show a material unexplained regression;
- small workloads do not incur a material unexplained regression from automatic-selection overhead;
- explicit serial execution remains available and semantically identical;
- findings and deterministic machine output remain equivalent across worker modes.

No fixed millisecond SLA or competitor-relative speed guarantee is introduced.

### 5.6 Published-artifact qualification

Published prerelease and stable qualification MUST include at least one check proving that an invocation with no `--workers` argument exercises the same worker-selection policy as explicit `--workers auto` on the qualification environment.

Documentation and `--help` output from published artifacts MUST show the new default accurately.

---

## 6. Non-Goals

Adaptive parallelism by default does NOT authorize:

- changing duplicate semantics;
- replacing Ruff parsing;
- changing normalization to gain speed;
- changing suffix-array/LCP semantics;
- parallelizing the detector without separate evidence;
- unbounded use of all available logical CPUs;
- changing explicit numeric worker semantics;
- embedding worker counts in deterministic result identity;
- adding a configurable concurrency subsystem;
- adding persistent caching;
- adding a daemon solely to amortize startup or parsing cost.

---

## 7. Acceptance Criteria Amendment

Arid 2.2 is not functionally complete until all original v2.2 acceptance criteria remain true and the following are also true:

1. `arid .` resolves worker selection using the same policy as `arid . --workers auto`;
2. automatic selection is bounded to `1..=4` workers;
3. unavailable CPU parallelism falls back to one worker;
4. explicit positive numeric worker values retain their exact requested value;
5. explicit `--workers 1` retains a supported serial path;
6. worker selection does not change findings, metrics, fingerprints, ordering, exit semantics, report-v4, or `summary-v1` semantics;
7. deterministic machine-output requirements pass across worker modes;
8. small, medium, large, and duplicate-heavy performance qualification passes;
9. published help/documentation accurately describes the default as automatic;
10. no new concurrency framework or detector path was introduced.

The amended release-completion statement is:

> **Arid 2.2 preserves the focused exact detector, uses bounded automatic preparation parallelism by default, and turns existing duplicate evidence into a concise deterministic summary that developers can understand at a glance and tools or coding agents can consume without parsing every finding.**

---

## 8. Contract Precedence

This document is a normative v2.2 contract amendment approved during Phase 0.

Where one of the three original v2.2 planning documents still states that the implicit worker default remains serial, this amendment supersedes that statement.

All original v2.2 requirements, architecture decisions, roadmap phases, non-goals, and gates remain in force except where explicitly amended above.
