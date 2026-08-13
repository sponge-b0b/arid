---
status: accepted
---

# Count redundant effective lines for duplication metrics

## Context

Arid reports duplicate-line counts and a duplication percentage.

A repeated block has multiple occurrences, but counting every occurrence as duplicated would include the copy that must remain. Multiple reported duplicate groups may also overlap, which can cause the same source line to be counted more than once.

The metric therefore needs deterministic semantics that represent actual redundancy.

## Decision

For each duplicate group, select the earliest occurrence by deterministic source order as the canonical occurrence.

Treat all other occurrences as redundant.

Count only effective normalized lines, and union redundant lines across all groups so the same source line is counted at most once.

Calculate duplication percentage as:

`duplicate_lines / analyzed_lines * 100`

where `analyzed_lines` is the number of effective normalized lines participating in analysis.

## Rationale

This measures how much analyzed code is redundant beyond one retained copy rather than how much code participates in duplication.

Using effective analyzed lines for both numerator and denominator keeps the metric internally consistent. Unioning overlapping redundant regions prevents nested or overlapping findings from inflating the result.

## Consequences

A 10-line block appearing twice contributes 10 duplicate lines, not 20.

The same block appearing three times contributes 20 duplicate lines.

The canonical occurrence is chosen only for deterministic accounting; it does not imply that Arid recommends keeping that specific copy during refactoring.