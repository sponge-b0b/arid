---
status: superseded
superseded_by: docs/arid-v2.2-adaptive-parallelism-amendment.md
---

# Default to Serial Execution

## Context

Arid's original architecture assumed that independent file preparation would use Rayon and execute in parallel.

Benchmarking of the serial implementation showed that Arid already performs exceptionally well without parallel execution. Making parallelism the default would therefore add scheduling overhead, machine-dependent behavior, and implementation complexity without demonstrated need for typical scans.

Parallel preparation remains a valid optimization for workloads that benefit from additional CPU concurrency.

## Decision

Arid will default to serial execution with one worker.

The CLI will provide:

`--workers <N>`

with the following behavior:

- `--workers 1` uses the existing serial preparation path.
- `--workers N`, where `N > 1`, enables parallel per-file preparation using at most `N` workers.
- `--workers 0` is invalid.
- Omitting `--workers` is equivalent to `--workers 1`.

`workers` is an execution-performance setting and will remain CLI-only in v1. It will not be added to `[tool.arid]`.

Parallelism is limited initially to independent file reading and preparation. Corpus construction, suffix-array construction, LCP construction, duplicate extraction, metrics, and reporting remain deterministic global stages.

Prepared files must be sorted before corpus construction so worker scheduling cannot affect results.

## Rationale

Serial execution is already fast enough to remain the simplest and most predictable default.

Making parallelism opt-in:

- preserves the lowest-overhead execution path for normal scans
- avoids consuming additional CPU resources unless requested
- prevents machine-specific worker counts from becoming project configuration
- gives users control when larger workloads benefit from concurrency
- preserves Arid's principle of adding complexity only when it provides measured value

Worker count may affect runtime but must never affect findings, grouping, canonical occurrence selection, metrics, structural metadata, or output ordering.

## Superseded

Arid v2.2 changes the implicit worker default to the existing bounded `auto` policy. The original serial path remains available through explicit `--workers 1`, and all determinism requirements from this ADR remain in force.

See `docs/arid-v2.2-adaptive-parallelism-amendment.md` for the current worker-selection contract and rationale.