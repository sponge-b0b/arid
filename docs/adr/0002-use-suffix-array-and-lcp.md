---
status: accepted
---

# Use a suffix array and LCP for duplicate detection

## Context

Arid must find arbitrary repeated contiguous regions of normalized Python source efficiently, including cross-file, same-file, and multi-occurrence duplicates.

Pylint `R0801` uses hashes of fixed-size line windows and then merges adjacent matches. That approach can produce large candidate sets for highly repetitive input and requires additional match-extension and collision-handling logic.

## Decision

Represent the normalized source corpus as integer line tokens and detect repeated regions using a generalized suffix array and longest-common-prefix (LCP) array.

Use unique sentinel tokens between files and normalization segments so matches cannot cross source boundaries.

## Rationale

Exact duplicate detection is a repeated-substring problem. A suffix array directly indexes all suffixes of the normalized corpus, while the LCP array exposes repeated contiguous regions and naturally supports groups with more than two occurrences.

This approach provides deterministic exact matching without relying on hash equality and avoids the large matching-bucket behavior possible with rolling-window hashing.

## Considered Options

- Pylint-style rolling hashes
- Rabin-Karp window matching
- naive pairwise substring comparison
- suffix tree or suffix automaton

The suffix-array approach provides the required behavior with a relatively small implementation, predictable memory usage, and a simpler deterministic representation than tree-based alternatives.

## Consequences

Suffix-array construction is more algorithmically involved than rolling hashes, so Arid validates its optimized implementation against simple reference algorithms in tests.

The v1 implementation uses prefix doubling for suffix-array construction and Kasai's algorithm for LCP construction.