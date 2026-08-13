---
status: accepted
---

# Match normalized physical source lines

## Context

Arid needs a precise definition of duplicate code.

Possible representations include raw source text, lexical tokens, AST structures, or normalized physical source lines. The representation determines which differences are significant and therefore defines Arid's behavior.

Arid is intended to preserve the useful source-oriented semantics of Pylint `R0801` without becoming a structural or semantic clone detector.

## Decision

Detect exact duplication over normalized physical Python source lines.

Before matching, Arid may remove configured Python constructs such as comments, docstrings, imports, and function signatures using syntax-aware preprocessing. Leading and trailing insignificant whitespace and line-ending differences are normalized.

Internal source formatting and identifiers remain significant.

## Rationale

Normalized source lines preserve the behavior users expect from a Pylint-style duplicate-code checker while allowing Arid to improve the correctness of Python-aware filtering.

Token-based matching would make additional formatting differences insignificant and therefore change the meaning of duplication. AST matching would move Arid toward structural clone detection and overlap with a different class of tools.

This representation keeps Arid focused on repeated source code rather than code that is merely structurally or semantically similar.

## Consequences

Code such as:

```python
value = calculate()
and:
value=calculate()
```

is different in v1.

Likewise, renaming an identifier prevents an exact match.

Structural, renamed-variable, fuzzy, and semantic clone detection remain outside the v1 scope.