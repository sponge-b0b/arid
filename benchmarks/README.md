# Arid benchmarks

Arid's v1 benchmarks compare duplicate-code detection performance against:

- Pylint `R0801` / `symilar`
- jscpd v5

The benchmark suite measures performance on fixed, publicly reproducible Python repositories using identical hardware.

## Goals

The v1 benchmark suite MUST:

- include representative small, medium, and large Python corpora
- use fixed repository revisions
- record tool versions and hardware metadata
- build Arid in release mode
- compare Arid with Pylint's duplicate-code checker in isolation
- compare Arid with jscpd using Python-only scanning
- use the same pinned repository revision and repository root for each comparison
- preserve the distinction between semantic compatibility and general tool performance

The benchmark runner is:

```bash
benchmarks/run.sh <corpus-path> [label]
```

## Canonical corpora

Canonical benchmark corpora MUST be publicly available repositories pinned to exact Git commits.

Each corpus records:

- repository URL
- commit SHA
- number of Python files
- physical Python source lines
- benchmark size classification: small, medium, or large

The size classifications describe the selected benchmark set rather than imposing product behavior based on repository size.

The canonical corpora exercise meaningfully different repository scales. They are real projects rather than artificial copies of the same source repeated to manufacture larger inputs.

Private repositories MAY be used as supplemental benchmarks but MUST NOT be required to reproduce the published v1 benchmark results.

### Small: Requests

Repository:

```text
https://github.com/psf/requests
```

The v1 benchmark revision is:

```text
6e83187b8feb273ed4c6cdab5efd8d54901dfab3
```

At this revision:

```text
Python files:                 37
  .py:                        37
  .pyi:                        0
Physical Python source lines: 11,997
```

Requests serves as the canonical small benchmark corpus.

### Medium: Pydantic

Repository:

```text
https://github.com/pydantic/pydantic
```

The v1 benchmark revision is:

```text
cf67d4b3193c3fe43ede18612ed62785eee11382
```

At this revision:

```text
Python files:                 404
  .py:                        403
  .pyi:                         1
Physical Python source lines: 164,878
```

Pydantic serves as the canonical medium benchmark corpus.

This corpus includes Python source beneath a hidden directory. Arid is therefore benchmarked with `--hidden`, which intentionally includes hidden files and directories while retaining normal ignore handling.

### Large: Polaris

Repository:

```text
https://github.com/sponge-b0b/Polaris
```

Polaris is a substantial, actively developed Python application and serves as Arid's canonical large benchmark corpus.

The v1 benchmark revision is:

```text
00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
```

At this revision:

```text
Python files:                  1,452
  .py:                         1,452
  .pyi:                            0
Physical Python source lines: 319,994
```

Polaris remains under active development, so benchmark results apply to this pinned revision rather than the repository's moving development state.

## Corpus integrity

Benchmark corpora MUST be clean Git repository roots.

Before benchmarking, the runner verifies:

- the corpus exists
- the corpus is a Git repository root
- the working tree is clean
- the repository contains tracked Python files
- Arid's discovered Python-file count with `--hidden` matches the number of Git-tracked `.py` and `.pyi` files

The last check prevents a benchmark from silently omitting tracked Python files because of hidden-path discovery behavior.

Benchmarks operate directly on the original repository layout. Files are not copied, flattened, renamed, or transformed before measurement.

## Comparison semantics

### Pylint

The Pylint comparison is intended to approximate Arid's Pylint-compatible detection configuration as closely as practical:

- minimum duplicate length: 4 lines
- comments ignored
- docstrings ignored
- imports ignored
- function signatures ignored
- same-file detection disabled in Arid
- hidden-path discovery enabled in Arid
- Arid run with one worker
- Pylint run with one worker
- unrelated Pylint checks disabled
- persistent Pylint state disabled
- reports and score disabled
- no project Pylint configuration loaded

This is the primary benchmark for Arid's v1 performance target.

The benchmark runner uses:

```text
Pylint 4.0.6
```

The Pylint comparison is serial so that serial Arid is compared directly against the isolated duplicate checker without multiprocessing affecting the result.

The Arid side is equivalent to:

```bash
arid <corpus> \
    --hidden \
    --no-same-file \
    --workers 1 \
    --json
```

The Pylint side is equivalent to:

```bash
pylint \
    --rcfile=/dev/null \
    --recursive=y \
    --disable=all \
    --enable=similarities \
    --min-similarity-lines=4 \
    --ignore-comments=y \
    --ignore-docstrings=y \
    --ignore-imports=y \
    --ignore-signatures=y \
    --jobs=1 \
    --persistent=n \
    --reports=n \
    --score=n \
    <corpus>
```

### jscpd

The jscpd comparison is a general duplicate-detector performance comparison, not a claim of equivalent detection semantics.

The benchmark uses:

- Python-only scanning
- `.py` and `.pyi` mapped to the Python format
- minimum duplicate length: 4 lines
- weak mode
- jscpd's native 50-token minimum
- normal `.gitignore` handling
- a sufficiently high file-size limit so corpus files are not excluded because of the benchmark configuration

The benchmark runner uses:

```text
jscpd 5.0.12
```

Two jscpd measurements are recorded:

1. one worker, for a serial comparison
2. default automatic workers, for comparison with jscpd's normal parallel execution

Arid retains its default same-file detection, runs with `--hidden`, and uses `--workers 1` for both cross-tool comparisons.

Because jscpd is token-based and uses different normalization and clone semantics, its finding counts are not expected to match Arid's.

## Measurement

Benchmarks use Hyperfine.

The benchmark runner uses:

```text
hyperfine 1.20.0
```

Warmup and measured-run counts are configurable through environment variables.

Defaults:

```text
WARMUP=1
RUNS=5
```

Published v1 measurements use:

```bash
WARMUP=3 RUNS=10 benchmarks/run.sh <corpus-path> <label>
```

Benchmark output records:

- mean execution time
- run variance
- user CPU time
- system CPU time
- tool versions
- Rust toolchain version
- operating system
- CPU model
- logical CPU count
- Arid Git commit
- corpus Git commit
- corpus repository URL
- Python file counts
- physical source lines
- analyzed effective lines
- Arid worker counts used for worker-scaling measurements

Performance conclusions are based on repeated measurements rather than a single run.

Benchmark commands discard normal report output so terminal rendering does not dominate duplicate-detection timing.

Duplicate findings legitimately produce nonzero exit codes:

```text
Arid:   0 or 1
Pylint: 0 or 8
```

The runner explicitly accepts those finding statuses inside the benchmark command.

Unexpected execution statuses remain benchmark failures rather than being globally ignored by Hyperfine.

## Final v1 benchmark results

The following cross-tool results use:

```text
Warmup runs:   3
Measured runs: 10
```

The cross-tool results were produced using the same benchmark methodology.

### Summary

| Corpus | Python files | Arid vs Pylint | Arid vs serial jscpd | Arid vs auto jscpd |
| --- | ---: | ---: | ---: | ---: |
| Requests 2.34.2 | 37 | **196.79x faster** | **4.97x faster** | **4.57x faster** |
| Pydantic 2.13.4 | 404 | **192.88x faster** | **2.13x faster** | **1.13x faster** |
| Polaris `00e208e` | 1,452 | **264.45x faster** | **1.68x faster** | jscpd **1.02x faster** |

### Requests 2.34.2

Corpus:

```text
Commit:                       6e83187b8feb273ed4c6cdab5efd8d54901dfab3
Python files:                 37
Physical Python source lines: 11,997
```

#### Arid vs Pylint

```text
Arid:    15.8 ms ± 0.4 ms
Pylint:   3.115 s ± 0.021 s
```

Result:

```text
Arid was 196.79 ± 4.77 times faster than Pylint R0801.
```

#### Arid vs serial jscpd

```text
Arid:   15.5 ms ± 0.3 ms
jscpd:  77.2 ms ± 3.7 ms
```

Result:

```text
Arid was 4.97 ± 0.26 times faster than serial jscpd.
```

#### Arid vs auto-parallel jscpd

```text
Arid:   16.7 ms ± 0.6 ms
jscpd:  76.6 ms ± 4.1 ms
```

Result:

```text
Arid was 4.57 ± 0.30 times faster than auto-parallel jscpd.
```

Automatic workers provide no wall-clock advantage for jscpd on this small corpus.

### Pydantic 2.13.4

Corpus:

```text
Commit:                       cf67d4b3193c3fe43ede18612ed62785eee11382
Python files:                 404
Physical Python source lines: 164,878
```

#### Arid vs Pylint

```text
Arid:    251.5 ms ± 25.3 ms
Pylint:   48.503 s ± 0.259 s
```

Result:

```text
Arid was 192.88 ± 19.45 times faster than Pylint R0801.
```

#### Arid vs serial jscpd

```text
Arid:   237.9 ms ± 10.7 ms
jscpd:  506.0 ms ± 10.0 ms
```

Result:

```text
Arid was 2.13 ± 0.10 times faster than serial jscpd.
```

#### Arid vs auto-parallel jscpd

```text
Arid:   232.4 ms ± 10.1 ms
jscpd:  262.8 ms ± 8.5 ms
```

Result:

```text
Arid was 1.13 ± 0.06 times faster than auto-parallel jscpd.
```

Parallel execution substantially narrows jscpd's wall-clock disadvantage on this medium corpus, but serial Arid remains faster.

### Polaris `00e208e`

Corpus:

```text
Commit:                       00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
Python files:                 1,452
Physical Python source lines: 319,994
```

#### Arid vs Pylint

```text
Arid:      441.9 ms ± 9.0 ms
Pylint:    116.854 s ± 2.380 s
```

Result:

```text
Arid was 264.45 ± 7.64 times faster than Pylint R0801.
```

#### Arid vs serial jscpd

```text
Arid:   512.5 ms ± 9.7 ms
jscpd:  858.9 ms ± 23.7 ms
```

Result:

```text
Arid was 1.68 ± 0.06 times faster than serial jscpd.
```

#### Arid vs auto-parallel jscpd

```text
Arid:   509.7 ms ± 4.5 ms
jscpd:  498.8 ms ± 21.7 ms
```

Result:

```text
jscpd was 1.02 ± 0.05 times faster in wall-clock time.
```

CPU usage differed substantially:

```text
Arid user CPU:     478.7 ms
Arid system CPU:    28.7 ms

jscpd user CPU:    901.4 ms
jscpd system CPU:  439.1 ms
```

Parallel jscpd therefore reached essentially the same wall-clock time as serial Arid while consuming substantially more aggregate CPU.

## Performance target

Arid's v1 performance requirement targets at least an order-of-magnitude advantage over Pylint's isolated duplicate-code checker on representative medium and large Python repositories.

The measured results are:

```text
Requests:  196.79x faster
Pydantic:  192.88x faster
Polaris:   264.45x faster
```

The Pydantic and Polaris measurements exceed the v1 performance target by a wide margin.

The small Requests corpus demonstrates that the advantage is not limited to large repositories, but the medium and large corpora are the primary evidence for satisfying the v1 requirement.

## Parallelism decision

The finalized cross-tool benchmarks did not justify making parallel execution the default. Serial Arid already:

- exceeds the Pylint performance target by a large margin at every tested scale
- outperforms serial jscpd at every tested scale
- outperforms auto-parallel jscpd on the small and medium corpora
- remains effectively tied with auto-parallel jscpd on the large corpus
- uses substantially less aggregate CPU than auto-parallel jscpd on Polaris

Arid therefore defaults to one worker and provides optional parallel per-file preparation through `--workers <N>`.

### Worker scaling

A follow-up worker-scaling benchmark measured optional parallel preparation on the same pinned Polaris corpus:

```text
Corpus commit:  00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
Arid commit:    502c639d213deb888a2c2a1dc364be4eec0b5eef
Warmup runs:    3
Measured runs:  10
Logical CPUs:   8
CPU:            Intel(R) Core(TM) i7-9700 CPU @ 3.00GHz
```

The benchmark used repository-root native discovery with:

```bash
arid <corpus> \
    --hidden \
    --workers <N> \
    --json
```

Results:

| Workers | Mean time | User CPU | System CPU | Speedup vs 1 worker |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 468.9 ms ± 7.6 ms | 446.5 ms | 28.3 ms | 1.00x |
| 2 | 334.1 ms ± 1.9 ms | 464.6 ms | 25.5 ms | 1.40x |
| 4 | **266.4 ms ± 4.2 ms** | 483.2 ms | 17.4 ms | **1.76x** |
| 8 | 269.0 ms ± 10.3 ms | 495.1 ms | 93.1 ms | 1.74x |

On this corpus and benchmark host:

- two workers reduced wall-clock time by approximately 28.7%
- four workers reduced wall-clock time by approximately 43.2%
- eight workers provided no meaningful wall-clock improvement over four and consumed substantially more system CPU

Four workers therefore provided the best measured latency/CPU tradeoff for Polaris on the tested hardware.

This is not a universal worker-count recommendation. The useful level of parallelism depends on repository shape, preparation cost, available CPUs, and the proportion of total execution time spent in the serial global stages.

Serial execution remains the default because it is already fast, minimizes CPU consumption, and satisfies Arid's v1 performance target. `--workers` is an opt-in latency optimization rather than an analysis setting or a reason to parallelize the global detection pipeline.

The worker-scaling measurement was produced from a newer Arid commit than the published cross-tool Polaris measurements. The measured values are recorded here rather than being merged into the historical cross-tool result set.

Generated benchmark artifacts are written beneath `benchmarks/results/`. That directory is intentionally ignored by Git and contains local reproducibility artifacts rather than version-controlled benchmark data.

## Setting up benchmark corpora

Keep benchmark corpora outside the Arid repository so benchmark inputs remain independent of Arid's working tree.

The canonical local layout is:

```text
~/benchmarks/
└── arid-corpora/
    ├── polaris/
    ├── pydantic/
    └── requests/
```

Create the corpus directory and clone the three canonical repositories:

```bash
mkdir -p ~/benchmarks/arid-corpora
cd ~/benchmarks/arid-corpora

git clone https://github.com/psf/requests.git requests
git clone https://github.com/pydantic/pydantic.git pydantic
git clone https://github.com/sponge-b0b/Polaris.git polaris
```

Pin each corpus to the revision documented by this benchmark suite:

```bash
git -C requests checkout 6e83187b8feb273ed4c6cdab5efd8d54901dfab3
git -C pydantic checkout cf67d4b3193c3fe43ede18612ed62785eee11382
git -C polaris checkout 00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
```

The resulting layout is:

```text
~/benchmarks/arid-corpora/
├── polaris/
├── pydantic/
└── requests/
```

These repositories are benchmark fixtures. Keep them pinned and clean rather than using active development working trees for published measurements.

Before benchmarking, verify the pinned revisions and clean working trees:

```bash
git -C ~/benchmarks/arid-corpora/requests rev-parse HEAD
git -C ~/benchmarks/arid-corpora/requests status --short

git -C ~/benchmarks/arid-corpora/pydantic rev-parse HEAD
git -C ~/benchmarks/arid-corpora/pydantic status --short

git -C ~/benchmarks/arid-corpora/polaris rev-parse HEAD
git -C ~/benchmarks/arid-corpora/polaris status --short
```

The expected revisions are:

```text
requests: 6e83187b8feb273ed4c6cdab5efd8d54901dfab3
pydantic: cf67d4b3193c3fe43ede18612ed62785eee11382
polaris:  00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031
```

Each `status --short` command should produce no output.

## Reproducing a benchmark

Set up the desired canonical corpus as described above, then run the benchmark from the Arid repository.

Published measurements use:

```bash
WARMUP=3 RUNS=10 \
  benchmarks/run.sh \
  <corpus-path> \
  <label>
```

For Requests:

```bash
WARMUP=3 RUNS=10 \
  benchmarks/run.sh \
  ~/benchmarks/arid-corpora/requests \
  requests-2.34.2
```

For Pydantic:

```bash
WARMUP=3 RUNS=10 \
  benchmarks/run.sh \
  ~/benchmarks/arid-corpora/pydantic \
  pydantic-2.13.4
```

For Polaris:

```bash
WARMUP=3 RUNS=10 \
  benchmarks/run.sh \
  ~/benchmarks/arid-corpora/polaris \
  polaris-00e208e
```

The runner records the cross-tool comparisons and Arid worker scaling for worker counts `1`, `2`, `4`, and `8`.

Results are generated beneath:

```text
benchmarks/results/<label>/
```

Each result directory contains:

```text
metadata.txt
arid-baseline.json
pylint.json
pylint.md
jscpd-serial.json
jscpd-serial.md
jscpd-auto.json
jscpd-auto.md
arid-workers.json
arid-workers.md
```

`metadata.txt` records the worker-count matrix in addition to the corpus, environment, tool-version, and Arid revision metadata.

`benchmarks/results/` is intentionally ignored by Git. These files support local inspection and reproduction but are not committed benchmark artifacts. Published performance claims are recorded in this README and MUST identify the relevant corpus revision, Arid revision, hardware, and benchmark configuration.

## Benchmark interpretation

Performance results describe the tested tool versions, corpus revisions, hardware, Arid revisions, and benchmark configuration.

They MUST NOT be generalized into claims about all repositories or all duplicate-detection workloads without supporting measurements.

In particular:

- Pylint is the primary performance target because Arid is designed as a focused replacement for `R0801`.
- The Pylint comparison is configured for the closest practical behavioral comparison, including disabling Arid's same-file detection.
- jscpd provides a useful comparison with another high-performance duplicate detector but does not have equivalent detection semantics.
- Arid's same-file detection remains enabled for the jscpd comparisons.
- cross-tool Arid measurements use one worker so optional parallel preparation does not change the meaning of the existing comparisons.
- worker-scaling measurements evaluate Arid separately using identical analysis semantics across worker counts.
- wall-clock time and aggregate CPU consumption provide different information and SHOULD both be considered when evaluating parallel implementations.
- small-corpus results are particularly sensitive to process startup and fixed overhead.
- correctness remains more important than benchmark performance.

Arid MUST NOT sacrifice duplicate-detection correctness, deterministic output, or source-location accuracy solely to improve benchmark results.