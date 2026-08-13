# ADR 0005: Derive structural context as reporting metadata

## Status

Accepted

## Context

Arid detects exact duplicated Python source after configurable normalization.

Real-world validation showed that exact duplicate findings can represent materially different kinds of Python code. A repeated block may be executable function logic, declarative class structure, module-level declarations, or a mixture of those contexts.

Detection alone answers:

> Is this source duplicated?

It does not tell the developer what kind of code was duplicated.

Arid needs enough objective structural information to make large duplicate reports easier to interpret without assigning severity, inferring framework semantics, or changing duplicate-detection behavior.

Python syntax is already available during preprocessing, making it possible to derive this information accurately without exposing parser-specific AST types to downstream detection.

The requirements therefore distinguish syntax-derived structural metadata from structural clone detection. Structural metadata describes exact normalized duplicates after they have been identified; it does not make structurally similar but non-identical source a duplicate.

## Decision

Arid will derive structural source metadata from Python syntax during preprocessing and carry that metadata with normalized lines.

Each normalized line records:

- structural context:
  - `declarative`
  - `executable`
  - `mixed`
- structural scope:
  - `module`
  - `class`
  - `function`

Structural classification considers only source retained after configured normalization and masking. Ignored comments, docstrings, imports, signatures, and suppressed source do not influence the classification of retained source.

Parser-specific AST and token types terminate at the Python-analysis boundary. Downstream layers consume Arid-owned structural metadata.

Structural metadata is explicitly **not part of duplicate identity**.

The detection pipeline continues to operate only on the normalized corpus:

```text
Python parsing / tokenization
        ↓
normalization + structural metadata
        ↓
normalized lines
        ↓
line interning + generalized corpus
        ↓
suffix array
        ↓
LCP
        ↓
duplicate groups
```

The duplicate detector does not compare, filter, rank, or group findings based on structural context or scope.

This preserves Arid's existing exact-normalized-source detection semantics and keeps structural interpretation separate from duplicate identity.

Finding-level metadata is derived in the reporting layer.

For each duplicate occurrence, reporting classifies the normalized lines covered by that occurrence. Reporting then aggregates structural metadata across all occurrences in the duplicate group.

Finding context uses:

- `declarative`
- `executable`
- `mixed`

Finding scope uses:

- `module`
- `class`
- `function`
- `mixed`

If all relevant source in all occurrences agrees on a classification, the finding retains that value.

If a duplicated region contains differing structural contexts, or different occurrences classify differently, the finding reports:

```text
Context: mixed
```

Likewise, if the duplicate spans or occurs under differing scopes, the finding reports:

```text
Scope: mixed
```

For example, identical normalized source appearing as declarative class code in one location and executable function code in another remains one exact duplicate group but is reported as:

```text
Context: mixed
Scope: mixed
```

This difference affects only how the finding is described. It does not affect whether the source is considered duplicated.

Structural metadata is descriptive only.

Arid will not use it to assign:

- severity
- remediation priority
- actionability
- framework-specific meaning
- presumed developer intent
- labels such as ORM boilerplate
- labels such as configuration noise
- labels such as safe duplication

The developer decides whether a duplicate warrants refactoring.

The intended responsibility boundary is:

> **Detection answers "is this duplicated?"**

> **Structural context helps answer "what kind of code is duplicated?"**

> **The developer decides whether it matters.**

## Classification Boundary

Structural classification belongs to Python-aware preprocessing rather than duplicate detection or reporting.

Conceptually:

```text
Python source
     ↓
parser / tokenizer
     ↓
structural regions
     ↓
normalization / masking
     ↓
retained normalized lines
+ structural context/scope
     ↓
duplicate detection
     ↓
duplicate groups
     ↓
finding-level aggregation
     ↓
DUP001
```

Parser-specific syntax information may be used while deriving structural regions, but parser-specific types must not escape the Python-analysis boundary.

The downstream normalized representation contains only Arid-owned values.

This keeps the duplicate detector independent of the chosen Python parser and prevents parser implementation details from leaking through the architecture.

## Retained-Source Classification

Structural classification must describe the source that actually participates in Arid's normalized representation.

Configured masking therefore happens before final line-level structural classification is determined.

Ignored source must not change the reported context of retained source.

This includes:

- comments
- docstrings
- imports
- function signatures
- suppressed source

For example:

```python
import os
run()
```

with imports ignored must classify the retained source based on:

```python
run()
```

rather than allowing the masked import to make the resulting line or region declarative.

Likewise, a physical line may contain both masked and retained statements:

```python
import os; run()
```

The retained source may therefore have a different structural classification from the physical line as originally written.

A retained normalized line that contains both declarative and executable source is represented as:

```text
mixed
```

This line-level `mixed` classification is distinct from finding-level `mixed`, which may also result from aggregation across multiple lines or occurrences.

## Structural Context

Structural context describes the kind of Python statement represented by retained source.

### Declarative

`declarative` represents direct declarations or definitions.

This includes structural constructs such as:

- function definitions
- async function definitions
- class definitions
- direct module-level assignments
- direct class-level assignments
- direct module-level imports when imports are retained
- direct class-level imports when imports are retained

The classification is syntax-based.

It does not attempt to determine whether a declaration represents:

- an ORM field
- framework configuration
- application configuration
- metadata
- schema definition
- dependency injection
- another framework-specific concept

### Executable

`executable` represents executable Python statements and control-flow logic.

This includes source such as:

- function-body statements
- method-body statements
- calls
- returns
- conditional logic
- loops
- exception handling
- executable nested statements

### Mixed

`mixed` represents retained source containing more than one structural context.

This can occur within one physical line, for example:

```python
setting = 1; run()
```

It can also be produced at the finding level when different normalized lines or different occurrences disagree on context.

## Structural Scope

Structural scope describes where retained source occurs.

Line-level scope uses the most specific applicable scope:

```text
function > class > module
```

The supported line-level scopes are:

- `module`
- `class`
- `function`

A method is represented as `function` scope rather than introducing a separate method category.

Nested functions are also represented as `function`.

Finding-level scope additionally supports:

- `mixed`

Finding-level `mixed` is produced when the duplicate region spans or occurs across differing scopes.

For example, identical normalized source may appear once at module level and once inside a function.

Arid still detects one exact duplicate group, but the finding reports:

```text
Scope: mixed
```

## Finding Aggregation

Structural metadata belongs to normalized source lines, while `DUP001` represents an entire duplicate group.

Reporting therefore aggregates line-level metadata into finding-level metadata.

Aggregation must consider every occurrence in the duplicate group rather than only the canonical occurrence.

This is necessary because identical normalized source may legitimately appear under different structural contexts or scopes.

For example:

```python
class First:
    value = build()
```

and:

```python
def second():
    value = build()
```

may contain identical normalized source for the assignment while occurring under different structural scopes.

Using only the canonical occurrence would incorrectly describe the entire duplicate group using whichever location happened to sort first.

Instead, disagreement across occurrences produces `mixed`.

This keeps finding metadata representative of the complete duplicate group.

## Detection Independence

Structural metadata must never participate in:

- suffix-array construction
- LCP construction
- duplicate equality
- minimum-line qualification
- maximal-repeat extraction
- overlap filtering
- nested-group handling
- occurrence grouping
- canonical occurrence selection
- duplication metrics

The detector operates on normalized line identity and segment structure only.

Two regions with identical normalized content remain duplicates even if their context or scope differs.

Conversely, two regions with similar syntax or identical structural classifications are not duplicates unless their normalized content matches according to Arid's normal detection rules.

This preserves the product boundary between:

```text
exact duplicate detection
```

and:

```text
structural clone detection
```

## Framework Independence

Structural classification must remain framework-agnostic.

Arid must not introduce classifications such as:

```text
orm
sqlalchemy
django
pydantic
configuration
boilerplate
generated-looking
safe
low-value
```

based on library conventions or inferred intent.

Such classifications would require semantic assumptions and would make Arid responsible for deciding whether particular forms of duplication matter.

That responsibility remains with the developer.

Framework-specific behavior may still emerge naturally from syntax-derived metadata.

For example, a project containing many repeated class declarations may produce many findings with:

```text
Context: declarative
Scope: class
```

That is useful objective information without Arid needing to know whether the classes belong to SQLAlchemy, Django, Pydantic, or ordinary Python code.

## Alternatives Considered

### Include structural metadata in duplicate identity

Rejected.

Two source regions that normalize to identical content are duplicates under Arid's detection model even when they occur in different Python contexts.

Including context or scope in duplicate identity would suppress valid exact duplicates.

It would also couple Python syntax classification to the detection algorithm and weaken the separation between normalization and detection.

### Filter findings based on structural context

Rejected.

Arid should not automatically discard findings because they are declarative, class-scoped, module-scoped, or otherwise structurally categorized.

Real-world validation showed that repeated declarative source can represent genuine duplication.

Whether such duplication is worth changing is project-specific.

Structural metadata exists to improve interpretation, not to silently alter detection.

### Assign severity from structural context

Rejected.

Executable duplication is not inherently more severe than declarative duplication, and declarative duplication is not inherently harmless.

Severity would encode product opinion that cannot be determined reliably from syntax alone.

### Infer structural context only during reporting

Rejected.

Re-parsing or reconstructing syntax in the reporting layer would duplicate Python-analysis responsibilities and make reporting dependent on parser-specific implementation details.

Deriving metadata once during preprocessing keeps syntax interpretation at the Python boundary and allows downstream layers to consume Arid-owned data.

### Store parser AST nodes with normalized lines

Rejected.

This would leak parser-specific representations into the core model and couple corpus/detection/reporting layers to the selected Python parser.

Arid instead converts syntax information into its own small structural vocabulary before leaving the Python-analysis boundary.

### Add framework-specific classifications

Rejected.

Categories such as SQLAlchemy models, Django declarations, configuration boilerplate, or similar framework concepts require semantic assumptions outside Arid's scope.

Arid remains framework-agnostic.

### Perform structural clone detection

Rejected for v1.

Syntax-derived metadata does not make structurally similar source equivalent.

AST structural similarity, renamed-variable clones, fuzzy matching, and semantic clone detection remain separate deferred features.

Structural metadata describes an exact duplicate **after** normal duplicate detection. It does not create duplicate relationships.

## Consequences

### Positive

- Developers receive objective information about what kind of Python code is duplicated.
- Large reports become easier to triage without introducing subjective severity.
- Detection semantics remain unchanged.
- The duplicate detector remains independent of parser and reporting concerns.
- Framework-specific heuristics are avoided.
- Parser-specific AST types remain isolated to the Python frontend.
- Structural reporting remains deterministic.
- Identical source appearing in different structural contexts remains correctly grouped.
- Reporting can distinguish repeated application logic from repeated declarations without judging either one.
- Future output formats can expose the same metadata without modifying detection.

### Negative

- Normalized lines carry additional metadata.
- Python preprocessing must derive structural regions accurately.
- Masking and structural classification must interact correctly.
- Multiple statements on one physical line require retained-byte-aware classification.
- Reporting must aggregate context and scope across every occurrence.
- `mixed` must be supported both as a line-level context and as a finding-level aggregate value.
- Human and machine-readable output gain additional fields that become part of the reporting contract.
- Structural classification adds test surface even though it does not change duplicate detection.

## Result

Arid retains a narrow duplicate-detection model:

```text
exact normalized source equality
```

Python syntax provides additional information about detected duplicates, but it does not redefine equality.

The architecture therefore remains intentionally separated:

```text
Python syntax
     ↓
structural metadata
     │
     │ carried with normalized source
     ↓
normalized corpus ──────→ duplicate detection
                              │
                              ↓
                       duplicate groups
                              │
                 structural metadata
                              ↓
                          reporting
```

The detector answers:

> **Is this duplicated?**

Reporting adds:

> **What kind of Python code is duplicated?**

The developer decides:

> **Does this duplication matter here?**