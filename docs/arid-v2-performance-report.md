# Arid v2 Performance Report

**Status:** Complete — Phase 14  
**Product:** Arid  
**Release target:** `2.0.0`  
**Candidate measured:** `2.0.0-beta.1`  
**Baseline:** `1.2.0`  
**Date:** 2026-08-22

## Purpose

This report records the Phase 14 evidence used to verify that Arid v2 preserves the established performance profile while adding the v2 machine-contract, automation, reporting, and integration surface.

The Phase 14 gate requires:

- reproducible measurements on the pinned canonical corpora
- comparison against the qualified Arid 1.2.0 baseline
- continued worker-mode validation
- investigation of meaningful unexplained regressions
- comparison with the then-current stable Pylint release
- at least a 10x advantage over isolated Pylint duplicate detection on the established medium and large corpora

## Executive conclusion

Phase 14 passes.

The evidence shows:

1. Arid v2 remains dramatically faster than isolated Pylint duplicate detection.
2. On the canonical Pydantic corpus, Arid v2 is **219.06x faster** than Pylint 4.0.6.
3. On the canonical Polaris corpus, Arid v2 is **249.68x faster** than Pylint 4.0.6.
4. The explicit 10x Pylint floor therefore passes by a very large margin.
5. The initial full-campaign serial comparison showed an apparent +8.5% Polaris regression, but that campaign measured v1.2 first and v2 after long Pylint/jscpd runs.
6. A paired Arid-only follow-up with reversed command ordering reduced the Polaris delta to +2.3% in the default configuration and +3.1% in the Pylint-compatible configuration.
7. Across all paired canonical measurements, v2 is between +0.3% and +3.1% relative to v1.2.
8. This low-single-digit difference is not an unacceptable regression and does not justify speculative product-code optimization.
9. Worker scaling remains healthy: bounded parallel preparation materially reduces wall-clock time on the medium, large, and duplicate-heavy corpora, while high worker counts remain counterproductive on smaller workloads.
10. Serial Arid remains faster than serial jscpd across the canonical real-world corpora; auto-parallel jscpd remains slightly faster than serial Arid on the large Polaris corpus, consistent with the established CPU/latency tradeoff.

No Phase 14 product-code optimization is warranted.

## Benchmark methodology

The canonical benchmark harness is:

```text
benchmarks/build.sh
benchmarks/run.sh
benchmarks/v2.sh
benchmarks/v2-paired.sh
```

Published qualification settings were:

```text
Hyperfine:      1.20.0
Warmup runs:    3
Measured runs:  10
Worker modes:   1,2,4,8,auto
Pylint:         4.0.6
jscpd:          5.0.12
```

Pylint 4.0.6 was the current stable PyPI release at the time of qualification.

The exact Arid versions compared were:

```text
Baseline:   arid 1.2.0
Candidate:  arid 2.0.0-beta.1
```

Both versions were benchmarked on the same machine and the same pinned corpora.

## Canonical corpora

| Corpus | Role | Revision | Python files | Physical lines |
| --- | --- | --- | ---: | ---: |
| Requests | small | `6e83187b8feb273ed4c6cdab5efd8d54901dfab3` | 37 | 11,997 |
| Pydantic | medium | `cf67d4b3193c3fe43ede18612ed62785eee11382` | 404 | 164,878 |
| Polaris | large | `00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031` | 1,452 | 319,994 |
| duplicate-heavy | stress | `815510e4ba755496ce3d2d4c4eea3d89afbc2ffc` | 148 | 47,988 |

The duplicate-heavy corpus is the deterministic four-copy Requests stress corpus established during v1.2 qualification.

## Full Phase 14 campaign

The first qualification pass ran the complete v1.2 Arid campaign followed by the v2 Arid/Pylint/jscpd campaign.

### Serial Arid comparison

| Corpus | v1.2 | v2 | Change |
| --- | ---: | ---: | ---: |
| Requests | 16.6 ms | 16.4 ms | -1.2% |
| Pydantic | 237.0 ms | 238.1 ms | +0.5% |
| Polaris | 474.4 ms | 514.7 ms | +8.5% |
| duplicate-heavy | 59.7 ms | 60.5 ms | +1.4% |

The isolated Polaris +8.5% delta was meaningfully different from the other corpora and therefore required investigation rather than immediate acceptance.

## Paired regression investigation

The first campaign measured all v1.2 Arid work before the long v2 cross-tool benchmark sequence. Pylint measurements on Pydantic and Polaris take tens to hundreds of seconds per measured run, so later v2 measurements were exposed to a materially different machine-load/thermal history.

`benchmarks/v2-paired.sh` was added to remove that methodological ambiguity.

For every corpus and configuration it performs two Hyperfine passes:

```text
Pass A: v1.2 then v2
Pass B: v2 then v1.2
```

The reported means average both command orders.

Two serial configurations are measured:

```text
default:
--hidden --workers 1 --json

Pylint-compatible:
--hidden --no-same-file --workers 1 --json
```

### Paired results

| Corpus | Configuration | v1.2 | v2 | Change |
| --- | --- | ---: | ---: | ---: |
| Requests | default | 16.5 ms | 16.7 ms | +1.3% |
| Requests | Pylint-compatible | 16.3 ms | 16.4 ms | +0.6% |
| Pydantic | default | 234.6 ms | 235.3 ms | +0.3% |
| Pydantic | Pylint-compatible | 224.2 ms | 227.4 ms | +1.4% |
| Polaris | default | 483.5 ms | 494.5 ms | +2.3% |
| Polaris | Pylint-compatible | 438.0 ms | 451.6 ms | +3.1% |
| duplicate-heavy | default | 58.2 ms | 59.4 ms | +2.0% |
| duplicate-heavy | Pylint-compatible | 58.9 ms | 59.6 ms | +1.3% |

The paired results establish that the original +8.5% Polaris observation was primarily campaign-order/environmental bias rather than a persistent v2 regression.

The remaining v2 deltas are low-single-digit across all measured workloads. V2 performs additional contract/report work, including report-v4 metadata and stable finding identity, so exact zero-cost equivalence is not expected. The campaign does not separately attribute a +0.3% to +3.1% difference because the release gate is satisfied and deeper profiling would be speculative optimization rather than evidence-driven work.

## Current stable Pylint comparison

The established comparison isolates Pylint's similarities checker and aligns configuration with Arid as closely as practical:

```text
minimum similarity lines: 4
comments ignored
docstrings ignored
imports ignored
signatures ignored
same-file disabled in Arid
one worker in both tools
unrelated Pylint checks disabled
no persistent Pylint state
```

Results:

| Corpus | Result vs Pylint 4.0.6 |
| --- | ---: |
| Requests | **191.19x faster** |
| Pydantic | **219.06x faster** |
| Polaris | **249.68x faster** |

The release requirement applies to the medium and large corpora:

```text
Pydantic: PASS — 219.06x
Polaris:  PASS — 249.68x
```

Both exceed the required 10x floor by more than an order of magnitude beyond the requirement itself.

## jscpd comparison

Serial comparison:

| Corpus | Result |
| --- | ---: |
| Requests | Arid **4.32x faster** |
| Pydantic | Arid **2.19x faster** |
| Polaris | Arid **1.65x faster** |

Auto-parallel jscpd comparison:

| Corpus | Result |
| --- | ---: |
| Requests | Arid **4.55x faster** |
| Pydantic | Arid **1.14x faster** |
| Polaris | jscpd **1.16x faster** |

As in the qualified v1/v1.2 campaigns, jscpd's automatic parallel execution can beat serial Arid in wall-clock time on the largest corpus while consuming more aggregate CPU. This does not alter Arid's design decision to remain serial by default and provide bounded preparation parallelism as an opt-in latency control.

## V2 worker scaling

| Corpus | 1 worker | 2 workers | 4 workers | 8 workers | `auto` |
| --- | ---: | ---: | ---: | ---: | ---: |
| Requests | 16.3 ms | **12.7 ms** | 15.3 ms | 24.7 ms | 15.7 ms |
| Pydantic | 244.2 ms | 173.7 ms | 133.6 ms | 184.4 ms | **133.1 ms** |
| Polaris | 503.6 ms | 371.5 ms | **294.8 ms** | 296.9 ms | 300.1 ms |
| duplicate-heavy | 61.2 ms | 43.0 ms | **38.6 ms** | 74.2 ms | 39.7 ms |

The worker results preserve the established interpretation:

- small corpora do not justify aggressive parallelism
- medium and large corpora benefit materially from bounded parallel preparation
- `auto` remains close to the best bounded result on representative larger workloads
- eight workers adds overhead without a compelling general benefit
- serial remains the benchmark regression baseline and product default

## Phase 14 gate

Phase 14 requires that Arid:

- remain at least an order of magnitude faster than isolated Pylint duplicate detection on Pydantic and Polaris
- have no unacceptable regression relative to v1.2
- preserve worker-mode performance behavior
- support public competitive claims with reproducible and current-version evidence

**Result: PASS.**

The measured v2 candidate exceeds the Pylint floor by approximately 22x the required ratio on Pydantic and 25x on Polaris. The paired same-machine regression investigation finds only low-single-digit v2 differences relative to 1.2.0, with no evidence of a meaningful performance defect.

## Optimization decision

No product-code optimization is justified by Phase 14.

The project should not add parser alternatives, detector redesign, caching, extra parallel execution paths, or other complexity merely to chase a low-single-digit benchmark delta while the performance contract is exceeded by a very large margin.

Future optimization should require a concrete measured bottleneck or a materially different product requirement.
