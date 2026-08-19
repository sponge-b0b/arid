# Arid v1.2 Performance Report

**Status:** In progress — Phase 5  
**Product:** Arid  
**Release target:** `1.2.0`  
**Profiling baseline:** `cdf936dadcc122c09756890d83ea92265c58c082`  
**Date:** 2026-08-19

## Purpose

This report records the performance evidence gathered for Arid v1.2 before any performance-sensitive implementation change is accepted.

The v1.2 contract requires performance work to be evidence-driven. Investigation must distinguish meaningful cost among discovery, read/parse/normalize, corpus construction, suffix-array/LCP construction, duplicate extraction, and report construction/rendering. It must also establish a reproducible benchmark baseline across representative corpora and compare serial, explicit multi-worker, and `--workers auto` execution.

The bottleneck investigation recorded here is complete. The release benchmark matrix required to close Phase 5 remains pending.

## Profiling target

The focused profiling campaign used:

```text
Arid commit:
cdf936dadcc122c09756890d83ea92265c58c082

Corpus:
local Polaris checkout
/home/bobt/projects/polaris

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

The local Polaris checkout was used as a large real-world profiling target. These measurements are diagnostic evidence rather than the final release benchmark baseline. The release benchmark campaign must record its own pinned corpus revisions, hardware metadata, and tool versions before the Phase 5 gate is closed.

## Profiling methodology

The investigation deliberately narrowed the problem before changing implementation:

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

All direct timing instrumentation was temporary development-only code. It was removed after measurement and was not committed to the v1.2 branch. No user-facing timing option or permanent profiling subsystem was added.

## Callgrind: broad attribution

Callgrind was used first to identify the dominant application region without assuming that suffix-array construction or duplicate detection was the bottleneck.

The serial Polaris run recorded:

```text
Total instruction references:
3,392,193,229 Ir

prepare_files:
2,308,223,089 Ir
68.05%

_int_malloc self cost:
187,420,926 Ir
5.53%
```

The important result was:

> Approximately 68% of instrumented execution was attributed to file preparation.

At this point `prepare_files` consisted primarily of:

```text
prepare_files
└── prepare_path
    ├── fs::read_to_string
    └── prepare_file
        ├── Python analysis
        └── normalization
```

This provided no evidence that suffix-array construction, LCP construction, duplicate extraction, or reporting was the dominant problem. The `_int_malloc` result indicated meaningful allocation activity but did not identify whether it originated primarily from file reads, parser structures, normalization, or another preparation operation.

## strace: filesystem investigation

The next measurement separated filesystem activity from CPU-side preparation.

The serial Polaris run produced:

| Syscall | Calls | Time |
| --- | ---: | ---: |
| `statx` | 3,749 | 47.132 ms |
| `openat` | 2,346 | 35.616 ms |
| `read` | 2,748 | 30.694 ms |
| `close` | 2,338 | 23.548 ms |
| `getdents64` | 902 | 12.445 ms |
| `fstat` | 878 | 10.807 ms |
| all syscalls | 13,288 | 163.196 ms |

The percentages reported by `strace -c` apply only to time spent inside traced system calls, not to Arid's total wall-clock execution. `strace` also increases syscall overhead, so the traced syscall total must not be interpreted as normal execution cost.

Separate untraced serial samples observed approximately:

```text
wall time:   520–545 ms
user CPU:    488–521 ms
system CPU:   28–33 ms
```

This indicates that kernel/system work represents only a small part of normal scan execution. The filesystem performs many operations because Arid must recursively discover and read the corpus, but filesystem I/O does not explain the dominant `prepare_files` cost.

### Conclusion

> File preparation is the dominant measured region, but filesystem I/O is not its dominant cost.

The evidence does not justify filesystem-specific optimization such as parallel file reads, `mmap`, alternative buffering, discovery redesign, or filesystem caching.

## Direct preparation instrumentation

Temporary timing instrumentation was added around:

```text
prepare_files total
python::analyze
normalize_source
preparation remainder
```

The remainder includes file reads and the small amount of preparation work outside the two directly measured CPU stages. One warmup run was discarded, followed by five measured serial Polaris runs.

### Raw results

Times are microseconds.

| Run | Total | `python::analyze` | `normalize_source` | Remainder |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 276,790 | 191,009 | 75,659 | 10,121 |
| 2 | 274,945 | 189,624 | 75,474 | 9,846 |
| 3 | 273,485 | 188,833 | 74,980 | 9,671 |
| 4 | 274,977 | 189,956 | 74,975 | 10,045 |
| 5 | 277,568 | 191,116 | 76,517 | 9,934 |

### Five-run mean

| Preparation stage | Mean | Share |
| --- | ---: | ---: |
| total preparation | 275.553 ms | 100.00% |
| `python::analyze` | 190.108 ms | 68.99% |
| `normalize_source` | 75.521 ms | 27.41% |
| remainder | 9.923 ms | 3.60% |

The five runs were highly consistent.

### Conclusion

> Approximately 69% of preparation time is spent in Python analysis, approximately 27% in Arid normalization, and only approximately 4% in all remaining preparation work.

The measured bottleneck therefore narrowed from file preparation to `python::analyze`.

## Python analysis instrumentation

A final temporary instrumentation pass divided `python::analyze` into:

```text
parse_module
comment / suppression scan
syntax collection
finalization
other analysis time
```

The measured stages correspond to Ruff `parse_module`, Arid's token scan for comments and suppression directives, Arid's syntax/structural-region traversal, mask merging/result sorting, and remaining untimed work inside the enclosing analysis call.

One warmup run was discarded, followed by five measured serial Polaris runs.

### Raw results

Times are microseconds.

| Run | Total | Analyze | Parse | Comments | Syntax | Finalize | Analyze other | Normalize | Remainder |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 282,921 | 196,477 | 167,282 | 864 | 4,911 | 695 | 22,723 | 76,307 | 10,136 |
| 2 | 286,822 | 199,786 | 170,025 | 871 | 5,004 | 779 | 23,105 | 76,766 | 10,269 |
| 3 | 282,611 | 195,670 | 166,194 | 899 | 4,982 | 701 | 22,893 | 76,906 | 10,034 |
| 4 | 282,417 | 195,715 | 166,806 | 851 | 4,908 | 691 | 22,456 | 76,487 | 10,214 |
| 5 | 288,283 | 200,274 | 170,337 | 926 | 5,065 | 721 | 23,223 | 77,397 | 10,611 |

### Five-run mean

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

The deeper instrumentation increased mean preparation time by approximately 3.3% relative to the simpler instrumentation pass. This is expected from the additional clocks and atomic counters. The stage proportions remained stable enough that the added measurement overhead does not affect the conclusion.

## Dominant cost center

Ruff `parse_module` averaged:

```text
168.129 ms
```

which represents:

```text
85.09% of python::analyze
59.07% of measured preparation
```

Arid's directly measured post-parse analysis work was:

```text
comments/suppressions
+ syntax collection
+ finalization
= 6.574 ms
```

or approximately:

```text
3.33% of python::analyze
2.31% of preparation
```

The `analyze other` bucket was not independently attributed. It includes work inside the enclosing analysis call that falls outside the explicitly timed regions. Parser-owned AST/token lifetime and destruction are plausible contributors, along with small amounts of untimed setup and glue, but no further attribution is required for the Phase 5 decision.

The important result does not depend on interpreting that bucket:

> Ruff parsing is overwhelmingly the largest directly measured component of Python analysis, while Arid's own post-parse analysis is inexpensive.

## Interpretation

The profiling chain supports the following conclusions:

1. **File preparation is the dominant broad region.** Callgrind attributed approximately 68% of instrumented execution to `prepare_files`.
2. **Filesystem I/O is not the reason.** Normal system CPU usage is small relative to wall time, and direct preparation instrumentation left only approximately 4% of preparation outside analysis and normalization.
3. **Python analysis dominates preparation.** `python::analyze` accounts for approximately 69% of preparation.
4. **Ruff parsing dominates Python analysis.** `parse_module` accounts for approximately 85% of `python::analyze`.
5. **Arid's post-parse analysis is already inexpensive.** The measured comment scan, syntax traversal, and finalization together consume only approximately 6.6 ms on the 1,452-file Polaris profiling corpus.
6. **Normalization is the secondary Arid-owned cost.** `normalize_source` consumes approximately 75–77 ms, or about 27% of preparation. It is material but is not the dominant measured bottleneck.

## Phase 6 implication

The profiling evidence does not justify a substantial parser or detector redesign for v1.2.

Optimizing the dominant measured cost would require attacking Python parsing itself. Potential approaches such as replacing Ruff's parser, avoiding a complete Python parse, changing the syntax information Arid derives, or maintaining another parsing path would introduce significant implementation and semantic risk for a backward-compatible minor release.

Arid's own post-parse analysis is too small to justify optimization work. Normalization is the largest remaining Arid-owned preparation cost, but the profiling evidence alone does not establish that optimizing it would produce a material enough end-to-end improvement to justify added complexity.

Therefore the current performance decision is:

> **Do not make a product-code performance change based on the profiling campaign alone.**

Phase 6 is explicitly allowed to produce no product-code change if the completed benchmark campaign does not identify another worthwhile optimization or meaningful regression.

## Changes not justified by current evidence

The profiling campaign provides no basis for undertaking:

- suffix-array replacement
- LCP algorithm replacement
- duplicate-extraction redesign
- filesystem optimization
- parallel file I/O
- persistent caching
- parser replacement
- an additional parsing path
- a profiling subsystem
- a user-facing timing mode

Any future optimization in these areas requires new evidence.

## Phase 5 work remaining

The bottleneck investigation is complete, but the Phase 5 gate is not yet closed.

The release benchmark campaign still must establish the reproducible v1.2 baseline required by the roadmap and requirements. At minimum, the remaining campaign must:

- use the established pinned benchmark corpora
- record exact corpus revisions
- record hardware and tool metadata
- measure the established small, medium, and large workloads
- include the required duplicate-heavy workload
- compare `--workers 1`
- compare an explicit multi-worker mode
- compare `--workers auto`
- continue the established Pylint comparison
- continue the established jscpd comparison
- investigate any meaningful unexplained serial regression from the qualified v1.1 baseline
- confirm Pydantic and Polaris remain at least 10x faster than isolated Pylint duplicate detection
- record enough evidence for the benchmark results to be reproduced

The established pinned primary corpora remain:

```text
Requests
Pydantic
Polaris
```

The benchmark campaign, not this diagnostic profiling run, will provide the release baseline used for Phase 5 qualification.

## Phase 5 completion criterion

Phase 5 may be closed when this report contains both:

1. the completed bottleneck investigation recorded above
2. the completed reproducible benchmark matrix across the required corpora and worker modes

At that point the report must identify:

- dominant cost centers
- serial baseline performance
- explicit multi-worker performance
- `--workers auto` performance
- comparison with the qualified v1.1 baseline
- whether Phase 6 has any justified product-code optimization

Until that matrix is recorded, this report remains **In progress — Phase 5**.
