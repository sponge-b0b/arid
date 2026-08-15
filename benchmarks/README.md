# Arid benchmarks

Arid's v1 benchmarks compare duplicate-code detection performance against:

- Pylint `R0801` / `symilar`
- jscpd v5

The benchmark suite measures performance on fixed, publicly reproducible Python repositories using identical hardware.

It supports benchmarking either:

- `target/release/arid` built from the current repository, or
- an existing Arid executable supplied with `--arid-bin`

The second mode allows release qualification to benchmark the exact published artifact rather than another local build.

## Goals

The v1 benchmark suite MUST:

- include representative small, medium, and large Python corpora
- use fixed repository revisions for published measurements
- record tool versions and hardware metadata
- compare Arid with Pylint's duplicate-code checker in isolation
- compare Arid with jscpd using Python-only scanning
- use the same pinned repository revision and repository root for each comparison
- preserve the distinction between semantic compatibility and general tool performance
- support benchmarking exact previously built or published Arid executables
- fail when corpus integrity or benchmark preconditions are violated

Correctness remains more important than benchmark performance.

## Tooling

The benchmark tooling consists of:

```bash
benchmarks/build.sh <global-root> [options]
benchmarks/run.sh <global-root> [options]
```

`build.sh` provisions benchmark corpora beneath:

```text
<global-root>/benchmarks/arid-corpora/
```

`run.sh` executes benchmarks and writes generated artifacts beneath:

```text
<project-root>/benchmarks/results/
```

The canonical repositories are:

- `polaris`
- `pydantic`
- `requests`

All three are benchmarked by default.

Use `--repos` to select a subset.

### Repository build mode

When `--arid-bin` is omitted, `run.sh` builds:

```text
target/release/arid
```

from the current Arid repository with:

```bash
cargo build --release --locked
```

This is the default mode for development and source-tree performance qualification.

### External binary mode

Benchmark an existing Arid executable with:

```bash
benchmarks/run.sh /home/bobt \
    --arid-bin /path/to/arid
```

When `--arid-bin` is supplied, `run.sh` does not build Arid.

The executable MUST:

- exist
- be executable
- identify itself as Arid through `--version`

This mode is intended for benchmarking previously built or published release artifacts.

For example:

```bash
benchmarks/run.sh /home/bobt \
    --arid-bin /path/to/published/arid \
    --label 1.0.1-rc.1
```

The benchmark methodology is otherwise unchanged.

## Canonical corpora

Canonical benchmark corpora MUST be publicly available repositories pinned to exact Git commits for published measurements.

Each corpus records:

- repository URL
- commit SHA
- number of Python files
- physical Python source lines
- benchmark size classification

The size classifications describe the selected benchmark set rather than imposing product behavior based on repository size.

The canonical corpora exercise meaningfully different repository scales. They are real projects rather than artificial copies of the same source repeated to manufacture larger inputs.

Private repositories MAY be used as supplemental benchmarks but MUST NOT be required to reproduce published benchmark results.

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

Polaris is a substantial Python application and serves as Arid's canonical large benchmark corpus.

Benchmark results apply to the pinned revision rather than the repository's moving development state.

## Corpus integrity

Benchmark corpora MUST be clean Git repository roots.

Before benchmarking, the runner verifies:

- the corpus exists
- the corpus is a Git repository root
- the working tree is clean
- the repository contains tracked Python files
- Arid's discovered Python-file count with `--hidden` matches the number of Git-tracked `.py` and `.pyi` files

The last check prevents a benchmark from silently omitting tracked Python files because of hidden-path discovery behavior.

Benchmarks operate directly on the original repository layout.

Files are not copied, flattened, renamed, or transformed before measurement.

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

The benchmark runner requires:

```text
Pylint 4.0.6
```

The Pylint comparison is serial so serial Arid is compared directly against the isolated duplicate checker without multiprocessing affecting the result.

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

The benchmark runner requires:

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

The benchmark runner requires:

```text
hyperfine 1.20.0
```

Warmup and measured-run counts are configurable through environment variables or CLI options.

Defaults:

```text
WARMUP=1
RUNS=5
```

CLI options take precedence over environment variables.

Published v1 measurements use:

```text
Warmup runs:   3
Measured runs: 10
```

Equivalent runner options are:

```bash
benchmarks/run.sh /home/bobt \
    --warmup 3 \
    --runs 10
```

or:

```bash
WARMUP=3 RUNS=10 benchmarks/run.sh /home/bobt
```

Benchmark output records:

- mean execution time
- run variance
- user CPU time
- system CPU time
- tool versions
- Rust toolchain version when Arid is built by the runner
- operating system
- CPU model
- logical CPU count
- validation harness Git commit
- Arid source mode
- Arid executable path
- Arid executable SHA-256
- Arid reported version
- Arid Git commit when built from the current repository
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

## Running benchmarks

Run the complete benchmark suite:

```bash
benchmarks/run.sh /home/bobt
```

Benchmark selected repositories:

```bash
benchmarks/run.sh /home/bobt \
    --repos requests,pydantic
```

Run an Arid-only regression benchmark:

```bash
benchmarks/run.sh /home/bobt \
    --repos pydantic \
    --tools arid \
    --no-worker-scaling
```

Run a cross-tool comparison without worker scaling:

```bash
benchmarks/run.sh /home/bobt \
    --repos requests,polaris \
    --tools arid,pylint,jscpd \
    --no-worker-scaling
```

Control Hyperfine measurement counts:

```bash
benchmarks/run.sh /home/bobt \
    --warmup 3 \
    --runs 10
```

Control worker-scaling measurements:

```bash
benchmarks/run.sh /home/bobt \
    --workers 1,2,4,8
```

Label result directories:

```bash
benchmarks/run.sh /home/bobt \
    --label rc1
```

This produces directories such as:

```text
benchmarks/results/
├── polaris-rc1/
├── pydantic-rc1/
└── requests-rc1/
```

### Published-artifact benchmarking

Benchmark an exact previously built or published Arid executable:

```bash
benchmarks/run.sh /home/bobt \
    --arid-bin /path/to/arid \
    --label 1.0.1-rc.1
```

External-binary mode records:

```text
arid_source=external
arid_binary=<absolute path>
arid_sha256=<sha256>
arid_version=<reported version>
```

and records:

```text
rustc=not-used
cargo=not-used
```

because the benchmark runner did not build that executable.

When the runner builds Arid itself, metadata instead records:

```text
arid_source=repository
arid_commit=<repository commit>
```

This distinction allows performance results to identify the exact executable under test.

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

The finalized cross-tool benchmarks did not justify making parallel execution the default.

Serial Arid already:

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

This is not a universal worker-count recommendation.

The useful level of parallelism depends on repository shape, preparation cost, available CPUs, and the proportion of total execution time spent in the serial global stages.

Serial execution remains the default because it is already fast, minimizes CPU consumption, and satisfies Arid's v1 performance target.

`--workers` is an opt-in latency optimization rather than an analysis setting or a reason to parallelize the global detection pipeline.

The worker-scaling measurement was produced from a newer Arid commit than the published cross-tool Polaris measurements. The measured values are recorded here rather than being merged into the historical cross-tool result set.

## Setting up benchmark corpora

Keep benchmark corpora outside the Arid repository so benchmark inputs remain independent of Arid's working tree.

The benchmark tooling owns this layout:

```text
<global-root>/
└── benchmarks/
    └── arid-corpora/
        ├── polaris/
        ├── pydantic/
        └── requests/
```

Provision all corpora at their current upstream default-branch `HEAD`:

```bash
benchmarks/build.sh /home/bobt
```

For reproducible measurements, pin the exact revisions:

```bash
benchmarks/build.sh /home/bobt \
    --polaris 00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031 \
    --pydantic cf67d4b3193c3fe43ede18612ed62785eee11382 \
    --requests 6e83187b8feb273ed4c6cdab5efd8d54901dfab3
```

Revision values may be commit SHAs, tags, or branches.

When a revision is omitted, `build.sh` resolves that repository to the current upstream default-branch `HEAD`.

`build.sh`:

- clones missing repositories
- fetches existing repositories
- verifies the expected `origin`
- refuses to modify dirty working trees
- resolves requested revisions to exact commits
- leaves each corpus detached at the resolved commit

Published or release-qualification measurements SHOULD use explicit pinned revisions.

## Reproducing the v1 benchmark

Provision the exact canonical revisions:

```bash
benchmarks/build.sh /home/bobt \
    --polaris 00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031 \
    --pydantic cf67d4b3193c3fe43ede18612ed62785eee11382 \
    --requests 6e83187b8feb273ed4c6cdab5efd8d54901dfab3
```

Then run:

```bash
benchmarks/run.sh /home/bobt \
    --label v1 \
    --warmup 3 \
    --runs 10
```

The runner benchmarks:

```text
polaris
pydantic
requests
```

with:

```text
Arid
Pylint
jscpd
```

and performs Arid worker-scaling measurements at:

```text
1,2,4,8
```

The resulting directories are:

```text
benchmarks/results/
├── polaris-v1/
├── pydantic-v1/
└── requests-v1/
```

## Release-candidate benchmarking

Release qualification SHOULD benchmark the exact published standalone executable rather than another local build.

For example:

```bash
benchmarks/run.sh /home/bobt \
    --arid-bin /path/to/arid \
    --label 1.0.1-rc.1 \
    --warmup 3 \
    --runs 10
```

This keeps the canonical benchmark corpora and cross-tool methodology unchanged while tying the Arid measurements to the exact release artifact identified by:

```text
arid_binary
arid_sha256
arid_version
```

in each result directory's metadata.

## Result layout

Generated benchmark artifacts are written beneath:

```text
benchmarks/results/
```

Without a label:

```text
benchmarks/results/<repository>/
```

With:

```bash
--label rc1
```

the result directory becomes:

```text
benchmarks/results/<repository>-rc1/
```

A complete result directory contains:

```text
metadata.txt
arid-baseline.json
arid.json
arid.md
pylint.json
pylint.md
jscpd-serial.json
jscpd-serial.md
jscpd-auto.json
jscpd-auto.md
arid-workers.json
arid-workers.md
```

Files are present only for the tools and benchmark modes selected for that run.

`metadata.txt` records the corpus, environment, tool versions, measurement configuration, worker-count matrix, and exact Arid executable identity.

`benchmarks/results/` is intentionally ignored by Git.

These files support local inspection, release qualification, and reproduction but are not committed benchmark artifacts.

Published performance claims SHOULD identify the relevant:

- corpus revision
- Arid revision or artifact SHA-256
- hardware
- benchmark configuration
- comparison tool versions

## Benchmark interpretation

Performance results describe the tested:

- tool versions
- corpus revisions
- hardware
- Arid executable
- benchmark configuration

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
- an external executable should be identified by its SHA-256 when results are used for release qualification.
- correctness remains more important than benchmark performance.

Arid MUST NOT sacrifice duplicate-detection correctness, deterministic output, or source-location accuracy solely to improve benchmark results.