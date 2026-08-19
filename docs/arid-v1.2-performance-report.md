# Arid v1.2 Performance Report

**Status:** Complete — Phase 5  
**Product:** Arid  
**Release target:** `1.2.0`  
**Profiling baseline:** `cdf936dadcc122c09756890d83ea92265c58c082`  
**Benchmark baseline:** `3920f0cb392fa7a5441f8de921d1e1f8e40b7ac1`  
**Date:** 2026-08-19

## Purpose

This report records the evidence used to evaluate Arid v1.2 performance before accepting performance-sensitive implementation changes.

The v1.2 contract requires the investigation to distinguish the major pipeline regions, establish a reproducible benchmark baseline, exercise small, medium, large, and duplicate-heavy workloads, compare serial and parallel worker modes including `--workers auto`, continue the established Pylint and jscpd comparisons, and investigate meaningful unexplained regression from the qualified v1 baseline.

Phase 5 is complete. The evidence does not justify an additional product-code optimization for Phase 6.

## Executive conclusion

The performance investigation found:

1. `prepare_files` is the dominant broad execution region.
2. Filesystem I/O is not the dominant preparation cost.
3. `python::analyze` accounts for about 69% of preparation time.
4. Ruff `parse_module` accounts for about 85% of `python::analyze` and about 59% of preparation.
5. Arid's measured post-parse analysis work is only about 2.3% of preparation.
6. `normalize_source` is the largest remaining Arid-owned preparation cost at about 27% of preparation, but it is secondary rather than dominant.
7. `--workers auto` captures the useful preparation parallelism on representative medium, large, and duplicate-heavy workloads without the oversubscription cost observed at eight workers.
8. Serial Arid shows no meaningful unexplained regression from the established v1 benchmark baseline.
9. Arid remains far above the required 10x advantage over isolated Pylint duplicate detection on Pydantic and Polaris.

The dominant measured cost is therefore Python parsing through Ruff, not Arid's suffix-array detector, duplicate extraction, filesystem access, or post-parse analysis.

## Profiling target

The focused profiling campaign used:

```text
Arid commit:
cdf936dadcc122c09756890d83ea92265c58c082

Corpus:
Polaris

Python files:
1,452

Execution:
release build
--workers 1
--json
output redirected to /dev/null

Environment:
Linux under WSL2
kernel 5.15.167.4-microsoft-standard-WSL2
```

The profiling measurements are diagnostic evidence. The release benchmark campaign below uses the pinned canonical benchmark corpora and benchmark harness.

## Profiling methodology

The investigation narrowed the bottleneck sequentially:

```text
whole scan
    ↓
Callgrind broad attribution
    ↓
prepare_files identified as dominant region
    ↓
strace syscall profiling
    ↓
filesystem I/O ruled out as dominant explanation
    ↓
temporary direct preparation instrumentation
    ↓
python::analyze identified as dominant preparation stage
    ↓
temporary python::analyze instrumentation
    ↓
Ruff parse_module identified as dominant analysis stage
```

The direct timing instrumentation was temporary development-only code. It was removed after measurement and was never added as a user-facing timing mode or permanent profiling subsystem.

## Callgrind broad attribution

The serial Polaris Callgrind run recorded:

```text
Total instruction references: 3,392,193,229 Ir
prepare_files:                 2,308,223,089 Ir
prepare_files share:           68.05%
_int_malloc self cost:           187,420,926 Ir
_int_malloc share:               5.53%
```

The important result was:

> Approximately 68% of instrumented execution was attributed to file preparation.

This provided no evidence that suffix-array construction, LCP construction, duplicate extraction, or reporting was the dominant problem.

## Filesystem investigation

A serial `strace -c` run produced:

| Syscall | Calls | Traced syscall time |
| --- | ---: | ---: |
| `statx` | 3,749 | 47.132 ms |
| `openat` | 2,346 | 35.616 ms |
| `read` | 2,748 | 30.694 ms |
| `close` | 2,338 | 23.548 ms |
| `getdents64` | 902 | 12.445 ms |
| `fstat` | 878 | 10.807 ms |
| all syscalls | 13,288 | 163.196 ms |

The `strace -c` percentages apply only to time spent inside traced system calls and must not be interpreted as shares of Arid wall-clock time. Tracing also inflates syscall overhead.

Separate untraced serial samples observed approximately:

```text
wall time:   520–545 ms
user CPU:    488–521 ms
system CPU:   28–33 ms
```

The evidence therefore supports:

> File preparation is the dominant measured region, but filesystem I/O is not its dominant cost.

Filesystem-specific work such as parallel reads, `mmap`, alternative buffering, discovery redesign, or filesystem caching is not justified by the measurements.

## Preparation-stage instrumentation

Temporary direct instrumentation measured five serial Polaris runs after one discarded warmup.

| Run | Total | `python::analyze` | `normalize_source` | Remainder |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 276.790 ms | 191.009 ms | 75.659 ms | 10.121 ms |
| 2 | 274.945 ms | 189.624 ms | 75.474 ms | 9.846 ms |
| 3 | 273.485 ms | 188.833 ms | 74.980 ms | 9.671 ms |
| 4 | 274.977 ms | 189.956 ms | 74.975 ms | 10.045 ms |
| 5 | 277.568 ms | 191.116 ms | 76.517 ms | 9.934 ms |

Five-run mean:

| Preparation stage | Mean | Share |
| --- | ---: | ---: |
| total preparation | 275.553 ms | 100.00% |
| `python::analyze` | 190.108 ms | 68.99% |
| `normalize_source` | 75.521 ms | 27.41% |
| remainder | 9.923 ms | 3.60% |

The run-to-run spread was small enough to establish a stable split.

## Python-analysis instrumentation

A final temporary pass divided `python::analyze` into Ruff parsing, Arid comment/suppression scanning, Arid syntax collection, finalization, and remaining untimed analysis work.

Five-run mean:

| Stage | Mean | Share of preparation |
| --- | ---: | ---: |
| total preparation | 284.611 ms | 100.00% |
| `python::analyze` | 197.584 ms | 69.42% |
| Ruff `parse_module` | 168.129 ms | 59.07% |
| comments/suppressions | 0.882 ms | 0.31% |
| syntax collection | 4.974 ms | 1.75% |
| finalization | 0.717 ms | 0.25% |
| analyze other | 22.880 ms | 8.04% |
| `normalize_source` | 76.773 ms | 26.97% |
| preparation remainder | 10.253 ms | 3.60% |

The deeper instrumentation increased mean preparation time by about 3.3% because of the additional clocks and counters, but the stage proportions remained decisive.

Ruff `parse_module` represents about 85.09% of `python::analyze`. Arid's directly measured comments/suppressions, syntax collection, and finalization total only about 6.574 ms, or 2.31% of preparation.

The `analyze other` bucket was not further attributed because doing so was not necessary for the decision. Parser-owned AST/token lifetime and destruction are plausible contributors, along with small untimed setup and glue.

## Reproducible v1.2 benchmark campaign

The benchmark campaign used Hyperfine with:

```text
Warmup runs:   3
Measured runs: 10
```

The benchmark source baseline was:

```text
3920f0cb392fa7a5441f8de921d1e1f8e40b7ac1
```

The benchmark harness pins and validates:

```text
hyperfine 1.20.0
pylint 4.0.6
jscpd 5.0.12
```

The harness records the exact Arid binary SHA-256, Arid commit, corpus commit, CPU, logical CPU count, OS, Rust toolchain, and benchmark configuration in each result directory's `metadata.txt`.

### Canonical corpora

| Corpus | Role | Revision | Python files | Physical lines |
| --- | --- | --- | ---: | ---: |
| Requests | small | `6e83187b8feb273ed4c6cdab5efd8d54901dfab3` | 37 | 11,997 |
| Pydantic | medium | `cf67d4b3193c3fe43ede18612ed62785eee11382` | 404 | 164,878 |
| Polaris | large | `00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031` | 1,452 | 319,994 |
| duplicate-heavy | stress | generated from four copies of pinned Requests | 148 | 47,988 |

The deterministic duplicate-heavy corpus produced commit:

```text
815510e4ba755496ce3d2d4c4eea3d89afbc2ffc
```

It exists to stress duplicate-rich detection behavior without adding another moving third-party dependency to the benchmark suite.

## Worker-mode matrix

The Arid-only campaign compared:

```text
--workers 1
--workers 2
--workers 4
--workers 8
--workers auto
```

Results:

| Corpus | 1 worker | 2 workers | 4 workers | 8 workers | `auto` |
| --- | ---: | ---: | ---: | ---: | ---: |
| Requests | 15.7 ms | **12.4 ms** | 14.6 ms | 24.0 ms | 15.5 ms |
| Pydantic | 233.3 ms | 157.3 ms | **122.3 ms** | 167.9 ms | 124.6 ms |
| Polaris | 471.7 ms | 340.4 ms | 287.6 ms | 282.3 ms | **279.1 ms** |
| duplicate-heavy | 57.6 ms | 40.1 ms | 38.6 ms | 70.2 ms | **38.1 ms** |

### Worker interpretation

`auto` reduced wall-clock time relative to one worker by approximately:

```text
Requests:          1.3%
Pydantic:         46.6%
Polaris:          40.8%
duplicate-heavy:  33.9%
```

Requests is small enough that fixed parallelism overhead dominates and two workers happened to be fastest. The important result is that `auto` remains near serial rather than producing the severe oversubscription cost seen at eight workers.

On Pydantic, Polaris, and the duplicate-heavy corpus, `auto` is effectively tied with the best bounded explicit worker count. Eight workers provides no compelling benefit and is materially worse on Requests, Pydantic, and the duplicate-heavy corpus while also consuming more aggregate CPU.

The v1.2 cap of four workers is therefore supported by measurement. There is no evidence to increase the cap or make automatic parallelism the default.

## Cross-tool benchmark

The established comparison methodology was rerun against Pylint and jscpd on the three real-world canonical corpora.

### Arid versus Pylint R0801

| Corpus | Arid | Pylint | Result |
| --- | ---: | ---: | ---: |
| Requests | 15.7 ms | 3.003 s | **190.85x faster** |
| Pydantic | 219.4 ms | 47.141 s | **214.87x faster** |
| Polaris | 427.5 ms | 108.757 s | **254.41x faster** |

The v1.2 requirement is that Pydantic and Polaris remain at least 10x faster than isolated Pylint duplicate detection.

Result:

```text
Pydantic: PASS — 214.87x
Polaris:  PASS — 254.41x
```

Both exceed the release floor by a very large margin.

### Arid versus serial jscpd

| Corpus | Arid | serial jscpd | Result |
| --- | ---: | ---: | ---: |
| Requests | 15.5 ms | 68.2 ms | **Arid 4.39x faster** |
| Pydantic | 227.2 ms | 495.4 ms | **Arid 2.18x faster** |
| Polaris | 472.8 ms | 782.0 ms | **Arid 1.65x faster** |

Serial Arid remains faster than serial jscpd at every tested scale.

### Serial Arid versus auto-parallel jscpd

| Corpus | serial Arid | auto jscpd | Result |
| --- | ---: | ---: | ---: |
| Requests | 15.5 ms | 74.1 ms | **Arid 4.78x faster** |
| Pydantic | 230.2 ms | 252.5 ms | **Arid 1.10x faster** |
| Polaris | 476.0 ms | 404.1 ms | **jscpd 1.18x faster** |

The Polaris result is consistent with the existing architectural tradeoff: jscpd's automatic parallel execution can achieve lower wall-clock latency on the large corpus while using substantially more aggregate CPU. Arid intentionally remains serial by default and exposes bounded parallel preparation as an opt-in execution choice.

## Regression comparison with the v1 baseline

The closest comparable serial Arid measurements from the established v1 campaign were:

| Corpus/configuration | v1 | v1.2 Phase 5 | Change |
| --- | ---: | ---: | ---: |
| Requests, Pylint-compatible | 15.8 ms | 15.7 ms | -0.6% |
| Pydantic, Pylint-compatible | 251.5 ms | 219.4 ms | -12.8% |
| Polaris, Pylint-compatible | 441.9 ms | 427.5 ms | -3.3% |
| Requests, jscpd comparison | 15.5 ms | 15.5 ms | 0.0% |
| Pydantic, jscpd comparison | 237.9 ms | 227.2 ms | -4.5% |
| Polaris, jscpd comparison | 512.5 ms | 472.8 ms | -7.7% |

Negative values indicate faster v1.2 measurements.

There is no meaningful unexplained serial regression. The measured v1.2 values are flat or faster than the corresponding v1 measurements.

## Phase 5 gate

Phase 5 requires a benchmark report that identifies dominant cost centers and establishes a reproducible baseline before algorithmic optimization is accepted.

**Result: PASS.**

The campaign established:

- broad and narrow bottleneck attribution
- pinned small, medium, and large real-world corpora
- deterministic duplicate-heavy stress coverage
- serial, explicit multi-worker, and `auto` measurements
- Pylint and jscpd comparisons
- regression comparison with the qualified v1 baseline
- a measured basis for evaluating Phase 6

## Phase 6 decision

The completed evidence does not identify a worthwhile v1.2 product-code optimization beyond the already implemented opt-in worker selection.

Optimizing the dominant measured cost would require attacking Python parsing itself. Approaches such as replacing Ruff's parser, avoiding a complete Python parse, changing the syntax information Arid derives, or maintaining another parsing path would introduce disproportionate complexity and semantic risk for a backward-compatible minor release.

Arid's own measured post-parse analysis work is too small to justify optimization. Normalization is the largest remaining Arid-owned preparation cost, but the potential end-to-end savings are not large enough to justify speculative complexity without a more specific future performance requirement.

The following changes are therefore not justified for v1.2:

- parser replacement
- an additional parsing path
- suffix-array or LCP replacement
- duplicate-extraction redesign
- filesystem optimization
- parallel file I/O
- persistent caching
- permanent profiling infrastructure
- a user-facing timing mode

**Phase 6 outcome: no product-code change.**

This is an intentional evidence-driven result, not deferred unfinished work.

## Final performance conclusion

Arid v1.2 satisfies its performance-hardening goals without changing duplicate semantics or introducing speculative optimization machinery.

The release enters integration and validation with:

- no meaningful serial regression
- the Pydantic and Polaris Pylint performance requirements passing by large margins
- `--workers auto` validated across representative and duplicate-heavy workloads
- the four-worker cap supported by measurement
- Ruff parsing identified as the dominant remaining cost
- no additional product-code optimization justified for v1.2
