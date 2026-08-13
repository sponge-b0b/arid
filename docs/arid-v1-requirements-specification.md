# **Arid v1 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust

## **1\. Purpose**

Arid is a fast, Python-specific duplicate-code checker written in Rust.

Its purpose is to replace the useful duplicate-code functionality of Pylint `R0801` / `symilar` while remaining intentionally complementary to Ruff.

Arid does one thing:

> Detect meaningful duplicated Python source code quickly and accurately.

It is not a general-purpose linter.

The core positioning becomes:

> **Arid is a fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

And technically:

ruff check .

arid .

That is a particularly clean pairing.

---

## **2\. Product Position**

The intended Python quality toolchain is:

Ruff  
├── linting  
├── formatting  
├── imports  
├── modernization  
└── general code-quality rules

Arid  
└── duplicate-code detection

Arid MUST NOT duplicate functionality that properly belongs in Ruff.

---

## **3\. V1 Goals**

Arid v1 MUST:

1. Detect duplicated Python source blocks across files.  
2. Detect duplicated source blocks within the same file.  
3. Preserve the useful behavior and configuration model of Pylint `R0801`.  
4. Improve Python-awareness where Pylint relies on textual heuristics.  
5. Be substantially faster than Pylint's duplication checker.  
6. Operate without starting or embedding a Python interpreter.  
7. Produce deterministic results.  
8. Provide precise source locations and useful duplication metrics.  
9. Describe duplicate findings using objective Python structural context and scope.  
10. Provide occurrence and file-distribution metadata that helps developers triage findings without assigning severity.  
11. Support both developer CLI usage and CI enforcement.  
12. Integrate naturally into Python projects using `pyproject.toml`.

---

## **4\. Explicit Non-Goals**

V1 MUST NOT implement:

* general lint rules  
* formatting  
* import sorting  
* unused-code detection  
* dead-code detection  
* complexity analysis  
* type checking  
* security scanning  
* code transformation  
* automatic duplicate removal  
* semantic clone detection  
* structural clone matching  
* identifier-renaming clone detection  
* fuzzy AST similarity  
* embedding-based similarity  
* multi-language support  
* JavaScript/TypeScript support  
* general copy/paste detection for arbitrary text  
* framework-specific duplicate classification or suppression heuristics

Arid MAY use Python syntax to describe an exact duplicate after detection. This MUST NOT be interpreted as structural clone detection: structurally similar regions that do not become identical after normalization MUST NOT be reported as duplicates.

Arid MUST remain Python-specific.

A feature that belongs naturally in Ruff SHOULD NOT be added to Arid.

---

# **5\. Detection Model**

## **5.1 Duplicate Definition**

V1 defines a duplicate as:

> Two or more sufficiently long contiguous Python source regions that become identical after Arid's configured preprocessing rules are applied.

V1 detects **exact duplication after normalization**.

Example:

def first():  
    \# explanation  
    value \= calculate\_value()  
    save\_value(value)

and:

def second():  
    \# different explanation  
    value \= calculate\_value()  
    save\_value(value)

may be considered duplicates when comments and function signatures are ignored.

Arid MUST NOT consider these equivalent:

value \= calculate\_value()  
save\_value(value)

result \= calculate\_value()  
save\_value(result)

Identifier normalization is outside v1 scope.

---

## **5.2 Arbitrary Source Blocks**

Detection MUST NOT be limited to:

* functions  
* methods  
* classes  
* modules  
* AST nodes

A duplicate may begin and end at arbitrary source-line boundaries.

This preserves an important property of Pylint `R0801` that function-oriented clone detectors do not provide.

---

## **5.3 Cross-File Detection**

Arid MUST detect duplicate blocks between different Python files.

Example:

src/accounts.py:40-55  
src/users.py:91-106

---

## **5.4 Same-File Detection**

Arid MUST also detect non-overlapping duplicate blocks within a single file.

This is an intentional improvement over Pylint `R0801`.

Same-file detection MUST be enabled by default.

Configuration:

\[tool.arid\]  
same-file \= true

Users MAY disable it when closer Pylint behavior is desired.

---

## **5.5 Minimum Duplicate Length**

The primary detection threshold MUST be based on effective normalized lines.

Default:

min-lines \= 4

A candidate MUST contain at least `min-lines` effective normalized lines before being reported.

CLI override:

arid . \--min-lines 6

---

## **5.6 Structural Finding Metadata**

Structural context MUST be reporting metadata, not part of duplicate identity.

Arid MUST determine whether two regions are duplicates solely from the normalized source representation and detection rules. Structural metadata MUST NOT make non-identical normalized regions match, prevent identical normalized regions from matching, or alter suffix-array/LCP duplicate detection.

Structural metadata exists to help a developer interpret an already-detected duplicate.

The product principle is:

> Detection answers "is this duplicated?" Context helps answer "what kind of code is duplicated?" The developer decides whether it matters.

Arid MUST NOT assign severity, presumed safety, or framework-specific meaning from structural context.

---

# **6\. Python-Aware Preprocessing**

Arid MUST parse or tokenize Python source sufficiently to distinguish Python syntax from textual lookalikes.

Preprocessing MUST retain a mapping between normalized content and original source locations.

## **6.1 Comments**

Default:

ignore-comments \= true

Python comments MUST be ignored when enabled.

The implementation MUST correctly distinguish:

\# actual comment

from:

value \= "\# not a comment"

Arid MUST NOT reproduce Pylint's simple textual `#` splitting behavior where doing so would incorrectly interpret valid Python strings.

---

## **6.2 Docstrings**

Default:

ignore-docstrings \= true

Arid MUST recognize Python docstrings structurally.

This includes:

* module docstrings  
* class docstrings  
* function docstrings  
* async-function docstrings  
* multiline docstrings  
* single-line docstrings

Ordinary string expressions MUST NOT automatically be treated as docstrings.

---

## **6.3 Imports**

Default:

ignore-imports \= true

When enabled, Arid MUST ignore:

import foo

from foo import bar

and multiline imports such as:

from foo import (  
    first,  
    second,  
    third,  
)

The complete syntactic import statement MUST be excluded.

---

## **6.4 Function Signatures**

Default:

ignore-signatures \= true

When enabled, Arid MUST ignore the declaration portion of:

* functions  
* async functions  
* methods  
* nested functions  
* multiline signatures

Example:

def calculate(  
    first: int,  
    second: int,  
) \-\> int:

must be recognized as one function declaration regardless of physical line count.

Implementation MUST use Python syntax information rather than regex-based `def` matching.

---

## **6.5 Whitespace**

Leading and trailing insignificant whitespace MUST NOT affect duplicate detection.

Indentation required to represent Python block structure MUST be handled consistently.

Differences consisting solely of trailing whitespace MUST NOT prevent a match.

Blank lines MUST NOT count toward `min-lines`.

---

## **6.6 Source Mapping**

All preprocessing MUST preserve sufficient source mapping to report:

* original file  
* original starting line  
* original ending line

Normalized line numbers MUST never leak into user-facing diagnostics.

---

## **6.7 Structural Source Metadata**

Python-aware preprocessing MUST derive structural metadata without exposing parser-specific AST types to the duplicate-detection engine.

Each retained normalized line MUST carry:

* structural context  
* structural scope

Line-level structural context MUST use:

* `declarative` — direct declarations or definitions, including direct module/class assignments and definitions  
* `executable` — executable statements, control flow, or function-body logic  
* `mixed` — a retained normalized line contains both declarative and executable source

Line-level structural scope MUST use:

* `module`  
* `class`  
* `function`

Classification MUST consider only source bytes that remain after configured masking. Ignored comments, docstrings, imports, signatures, and suppressed source MUST NOT influence the classification of retained source.

When nested syntax regions overlap, classification SHOULD use the most specific enclosing statement for the retained source.

Structural metadata MUST NOT change normalized text, effective-line counting, segment boundaries, or duplicate matching.

---

# **7\. Suppression**

Arid MUST provide native suppression directives.

Minimum v1 syntax:

\# arid: disable

and:

\# arid: enable

Lines within a disabled region MUST NOT participate in duplicate detection.

Suppression regions MUST create matching segment barriers so a duplicate cannot span across disabled source.

V1 SHOULD also recognize existing Pylint duplicate-code suppression directives where practical so repositories can migrate without immediately rewriting every suppression.

Suppression directives MUST be processed before comments are discarded.

---

# **8\. Match Construction**

## **8.1 Corpus Construction**

Arid MUST construct one deterministic normalized corpus from all analyzed files and enabled source segments.

Normalized source lines MUST be interned to stable line identifiers for exact comparison.

Unique sentinels MUST separate files and suppression-created segments so duplicate matches cannot cross those boundaries.

Corpus construction MUST preserve sufficient mapping to recover the original file and normalized-line range for every corpus position.

---

## **8.2 Suffix Array**

Arid MUST identify repeated normalized line sequences using a generalized suffix array over the interned corpus.

The implementation MUST avoid naive exhaustive comparison of every possible source region against every other source region.

Suffix-array construction MUST be deterministic.

The v1 implementation SHOULD use an `O(n log n)` construction with `O(n)` auxiliary storage, where `n` is the normalized corpus length.

---

## **8.3 Longest Common Prefix**

Arid MUST compute longest-common-prefix information for adjacent suffixes so exact repeated source regions can be discovered without pairwise substring comparison.

The LCP construction SHOULD run in `O(n)` time after suffix-array construction.

Exact duplicate identity MUST ultimately depend on the interned normalized line sequence rather than an unverified probabilistic hash.

Hash collisions MUST therefore be incapable of producing false duplicate findings.

---

## **8.4 Maximal Repeats**

Arid MUST convert suffix-array/LCP repeat intervals into maximal meaningful duplicate regions.

Adjacent or overlapping evidence for the same repeated source regions MUST collapse into the largest valid contiguous duplicate block rather than producing sliding-window noise.

For example, overlapping matches of lines:

10-13  
11-14  
12-15  
13-16

MUST produce one result:

10-16

rather than four findings.

---

## **8.5 Contained and Nested Matches**

A smaller duplicate group whose occurrences are all contained within a larger reported group SHOULD be suppressed.

A shorter nested duplicate MAY remain when it adds at least one occurrence not represented by the larger group.

This preserves meaningful clone-family variation without reporting redundant contained findings.

---

## **8.6 Multiple Occurrences**

If the same block appears in three or more locations, Arid MUST represent them as one duplicate group:

Duplicate block: 12 lines

src/a.py:10-21  
src/b.py:30-41  
src/c.py:75-86

rather than independent pairwise findings.

---

## **8.7 Same-File Overlap**

Two same-file occurrences MUST NOT constitute a valid clone pair when their source regions overlap.

This prevents repetitive sequences from generating pathological self-matches.

---

## **8.8 Deterministic Canonical Ordering**

Occurrences within a duplicate group MUST be ordered deterministically by source location.

The canonical occurrence used for metric accounting MUST be the earliest occurrence under that deterministic source ordering.

---

# **9\. Diagnostics**

V1 defines one primary diagnostic:

`DUP001 duplicate-code`

Each finding MUST contain:

* diagnostic code  
* duplicate effective-line count  
* structural context  
* structural scope  
* number of occurrences  
* number of distinct files  
* occurrence distribution  
* original file path for every occurrence  
* original starting line for every occurrence  
* original ending line for every occurrence

A finding MAY include original source text when explicitly requested.

## **9.1 Finding Context**

Finding context MUST use one of:

* `declarative`  
* `executable`  
* `mixed`

A finding is `mixed` when its normalized lines contain more than one structural context or when different occurrences classify differently.

Context MUST be descriptive, not a severity. Arid MUST NOT imply that `declarative` findings are safe to ignore or that `executable` findings must be refactored.

## **9.2 Finding Scope**

Finding scope MUST use one of:

* `module`  
* `class`  
* `function`  
* `mixed`

A finding is `mixed` when the duplicated region spans more than one structural scope or when different occurrences classify under different scopes.

## **9.3 Finding Distribution**

Finding distribution MUST use one of:

* `same-file` — all occurrences are in one file  
* `cross-file` — multiple files are involved and each involved file contains one occurrence  
* `mixed` — multiple files are involved and at least one involved file contains multiple occurrences

Distribution describes where the duplicate appears. It MUST NOT imply severity or actionability.

## **9.4 Source Locations**

Reported locations MUST use original physical source line numbers, never normalized internal line numbers.

Ignored constructs MAY therefore appear inside a reported physical source range even though they did not participate in matching.

---

# **10\. Default Human Output**

Default output SHOULD prioritize concise developer usability while exposing enough objective metadata to triage findings.

Example:

```text
DUP001 4 duplicated lines
Context: declarative
Scope: class
Occurrences: 2 across 2 files (cross-file)

  src/models/user.py:12-15
  src/models/account.py:20-23

Found 1 duplicate group.
4 duplicate lines (2.31%).
```

The human output MUST NOT assign severity, remediation priority, or framework-specific labels.

An option SHOULD allow source snippets to be displayed when desired.

For example:

`arid . --show-source`

---

# **11\. Machine-Readable Output**

V1 MUST provide JSON output.

Invocation:

`arid . --json`

The JSON document MUST contain a top-level schema version and stable scan metrics.

Each finding MUST expose at minimum:

```json
{
  "code": "DUP001",
  "lines": 4,
  "context": "declarative",
  "scope": "class",
  "occurrences": 2,
  "files": 2,
  "distribution": "cross-file",
  "locations": [
    {
      "path": "src/models/user.py",
      "start_line": 12,
      "end_line": 15
    },
    {
      "path": "src/models/account.py",
      "start_line": 20,
      "end_line": 23
    }
  ]
}
```

When source display is not requested, location objects SHOULD omit the `source` field rather than serializing it as `null`.

JSON output MUST be deterministic and stable enough for CI tooling to consume.

An incompatible change to the JSON structure MUST increment the schema version.

SARIF is explicitly deferred beyond v1 unless implementation proves trivial.

---

# **12\. Duplication Metrics**

Arid MUST report:

* analyzed files  
* physical source lines  
* analyzed effective lines  
* duplicate groups  
* duplicate effective lines  
* duplication percentage

An effective normalized line is a retained normalized line containing substantive identifier/alphanumeric content. Blank lines and punctuation-only lines MUST NOT increase the effective-line count.

Duplicate-line accounting MUST avoid double-counting overlapping redundant findings.

The metric interpretation MUST be deterministic and documented:

> Duplicate lines are effective lines redundant beyond one canonical occurrence of each duplicate group.

For a 10-line block appearing twice, the block contributes 10 duplicate lines rather than 20.

For the same 10-line block appearing three times, it contributes 20 duplicate lines.

When redundant regions overlap, the union of redundant effective lines MUST be counted so the same physical redundancy is not counted repeatedly.

Duplication percentage MUST be:

`duplicate effective lines / analyzed effective lines * 100`

---

# **13\. File Discovery**

Running:

arid .

MUST recursively discover Python source files.

V1 MUST support:

.py  
.pyi

Arid SHOULD honor standard VCS ignore files, particularly `.gitignore`.

Hidden files and directories MUST be skipped by default during directory discovery.

Users MUST be able to include hidden files and directories through:

hidden \= true

or:

arid . \--hidden

Including hidden paths MUST NOT disable standard VCS ignore handling or configured `exclude` patterns.

Explicit paths MUST be supported:

arid src/foo.py src/bar.py

and:

arid src tests

An explicitly named Python file MUST be considered for analysis regardless of whether the file itself is hidden. Configured `exclude` patterns MUST still apply.

Symlinked directories SHOULD NOT be followed by default.

---

# **14\. Configuration**

Primary project configuration MUST live in:

\[tool.arid\]

within `pyproject.toml`.

Default configuration:

\[tool.arid\]  
min-lines \= 4  
ignore-comments \= true  
ignore-docstrings \= true  
ignore-imports \= true  
ignore-signatures \= true  
same-file \= true  
hidden \= false  
exclude \= \[\]

`exclude` MUST accept project-relative path patterns:

exclude \= \[  
    "generated/\*\*",  
    "vendor/\*\*",  
\]

Configuration precedence MUST be:

CLI arguments  
    ↓  
pyproject.toml  
    ↓  
built-in defaults

A CLI setting MUST replace the corresponding project setting when explicitly provided.

For `exclude`, one or more CLI `--exclude` values MUST replace the configured `exclude` list for that scan rather than append to it.

Arid SHOULD NOT introduce a second proprietary configuration format in v1.

---

# **15\. CLI**

The primary invocation MUST remain simple:

arid .

V1 SHOULD avoid unnecessary subcommands.

Required options:

\--min-lines  
\--ignore-comments  
\--no-ignore-comments  
\--ignore-docstrings  
\--no-ignore-docstrings  
\--ignore-imports  
\--no-ignore-imports  
\--ignore-signatures  
\--no-ignore-signatures  
\--same-file  
\--no-same-file  
\--hidden  
\--no-hidden  
\--json  
\--show-source  
\--exclude  
\--version  
\--help

Boolean configuration MUST provide both enabling and disabling CLI forms.

The positive and negative forms for the same setting MUST be mutually exclusive.

Examples:

arid . \--ignore-comments

arid . \--no-ignore-comments

---

# **16\. Exit Codes**

V1 MUST use predictable CI-friendly exit codes.

0  scan succeeded and no failing duplicate findings were reported  
1  duplicate-code findings were reported  
2  configuration, invocation, parsing, or internal error

A duplicate finding MUST be distinguishable from an Arid execution failure.

---

# **17\. Python Compatibility**

V1 SHOULD support modern Python 3 syntax through Python 3.14.

Arid MUST NOT require the corresponding Python interpreter to be installed.

Source analysis MUST be performed entirely by the Rust implementation.

Unsupported or invalid Python syntax MUST produce a clear diagnostic rather than silently skipping the file.

---

# **18\. Performance Requirements**

Performance is a primary product requirement, not merely an implementation optimization.

## **18.1 No Python Runtime**

Normal execution MUST NOT:

* launch Python  
* import the analyzed project  
* execute project code  
* initialize Pylint

---

## **18.2 Parallelism**

Independent file reading and preprocessing SHOULD execute in parallel when benchmarks show a meaningful performance benefit.

Detection MAY also use parallel processing where doing so preserves deterministic results.

Parallelism MUST NOT be introduced solely for architectural completeness. Performance requirements SHOULD be met using the simplest implementation that satisfies the measured target.

---

## **18.3 Algorithmic Constraint**

The implementation MUST NOT perform naive exhaustive pairwise substring comparison.

The normalized corpus MUST be analyzed using a generalized suffix array plus longest-common-prefix construction.

Suffix-array construction SHOULD be `O(n log n)` with `O(n)` auxiliary storage, and LCP construction SHOULD be `O(n)`, where `n` is normalized corpus length.

Duplicate extraction SHOULD operate from suffix-array/LCP intervals rather than enumerating every possible source-region pair.

---

## **18.4 Performance Target**

V1 MUST demonstrate a substantial performance advantage over Pylint's isolated duplicate-code checker on representative medium and large Python repositories when measured on identical hardware.

The target is:

> At least an order-of-magnitude faster than Pylint `R0801` / `symilar`.

Failure to meet the target SHOULD trigger profiling and targeted optimization before introducing additional architectural complexity such as parallel execution.

Correctness MUST NOT be sacrificed to meet the performance target.

---

## **18.5 Determinism**

Parallel execution MUST NOT change:

* which duplicates are detected  
* duplicate grouping  
* canonical occurrence selection  
* metric calculations  
* structural context and scope  
* occurrence/file-distribution metadata  
* output ordering

Given identical source and configuration, output MUST be deterministic.

---

# **19\. Correctness Requirements**

Arid MUST correctly handle at minimum:

* normal functions  
* async functions  
* methods  
* nested functions  
* decorators  
* multiline signatures  
* type annotations  
* `type` alias statements
* module docstrings  
* class docstrings  
* function docstrings  
* multiline strings  
* `#` characters inside strings  
* ordinary comments  
* multiline imports  
* relative imports  
* blank lines  
* mixed indentation  
* Unicode source  
* `.pyi` files  
* multiple statements on one physical line  
* masked and retained statements sharing one physical line  
* nested class/function/control-flow scopes for structural metadata

Python source MUST never be executed during analysis.

Structural classification MUST describe retained normalized source correctly without changing duplicate identity.

---

# **20\. Pylint Compatibility Philosophy**

Arid targets:

> **Pylint-compatible intent, not bug-for-bug compatibility.**

When Pylint behavior exists because of textual parsing limitations, Arid SHOULD prefer correct Python syntax interpretation.

Examples include:

* distinguishing `#` inside strings from comments  
* structurally recognizing docstrings  
* structurally recognizing imports  
* structurally recognizing function declarations

Intentional behavioral differences MUST be documented.

---

# **21\. Testing Requirements**

## **21.1 Unit Tests**

Tests MUST cover:

* normalization  
* comments  
* docstrings  
* imports  
* signatures  
* source-line mapping  
* suppression directives and segment barriers  
* corpus construction and sentinels  
* line interning  
* suffix-array construction  
* LCP construction  
* maximal-repeat extraction  
* match grouping  
* contained/nested match handling  
* same-file overlap handling  
* structural context classification  
* structural scope classification  
* masked-source structural classification  
* finding-level mixed-context/scope aggregation  
* same-file/cross-file/mixed distribution reporting  
* duplication metrics  
* configuration precedence  
* positive and negative CLI boolean overrides
* conflicting positive/negative CLI flags
* CLI `exclude` replacement semantics
* hidden-path discovery defaults and opt-in behavior
* hidden configuration and CLI override precedence
* human diagnostics  
* JSON schema and serialization

---

## **21.2 Compatibility Fixtures**

A dedicated fixture suite MUST compare Arid behavior against expected `R0801` behavior for features intended to remain compatible.

Fixtures MUST also document intentional deviations.

---

## **21.3 False-Positive Tests**

Tests MUST verify that Arid does not falsely classify:

"\# text"

as a comment.

They MUST distinguish docstrings from ordinary strings.

They MUST distinguish similar code from exact normalized duplication.

Renaming identifiers alone MUST NOT make two blocks duplicates.

---

## **21.4 Determinism Tests**

Deterministic output MUST be covered by tests.

When parallel execution is enabled, the same source tree MUST produce byte-for-byte equivalent structured findings regardless of worker-thread count.

---

## **21.5 Performance Benchmarks**

The repository MUST contain reproducible benchmarks comparing at least:

* Arid  
* Pylint `R0801` / `symilar`  
* jscpd v5

Benchmarks SHOULD include small, medium, and large Python corpora.

Performance claims in project documentation MUST be derived from reproducible benchmarks.

---

# **22\. Distribution**

Arid SHOULD ultimately be easy to install from the Python ecosystem despite being implemented in Rust.

Preferred installation experience:

uv tool install arid

or:

pip install arid

Rust-native installation SHOULD also be supported:

cargo install arid-cli

Prebuilt binaries SHOULD be provided for major Linux, macOS, and Windows targets.

Installing Arid MUST NOT require compiling or installing Python dependencies at runtime.

---

# **23\. V1 Architecture Boundary**

The implementation SHOULD consist conceptually of:

CLI / configuration  
        ↓  
file discovery  
        ↓  
Python parsing / tokenization  
        ↓  
normalization + structural source metadata  
        ↓  
normalized line representation  
        ↓  
line interning + generalized corpus  
        ↓  
suffix array  
        ↓  
LCP  
        ↓  
maximal repeat extraction + grouping  
        ↓  
metrics  
        ↓  
reporting + structural finding aggregation

Each stage SHOULD have a narrow responsibility.

The Python frontend MAY use parser-specific AST/token types internally, but those types MUST terminate at the Python-analysis boundary. Downstream layers SHOULD consume Arid-owned representations.

Structural source metadata MUST be attached during Python-aware preprocessing/normalization and carried with normalized lines.

The duplicate-detection engine MUST ignore structural metadata and operate only on the normalized corpus representation.

The reporting layer MUST derive finding-level context and scope from normalized-line metadata and aggregate across all occurrences.

The parser implementation MUST NOT dictate the duplicate-detection algorithm.

The duplicate-detection engine SHOULD remain independent of CLI and reporting concerns.

---

# **24\. Deferred Features**

The following SHOULD be considered after v1 rather than included automatically:

* persistent scan cache  
* Git-diff-only analysis  
* baseline files  
* SARIF  
* GitHub annotations  
* editor/LSP integration  
* pre-commit-specific integration  
* configurable duplication budgets  
* historical duplication tracking  
* AST structural similarity  
* renamed-variable clones  
* semantic clone detection  
* autofix/refactoring suggestions

`AST structural similarity` here means using structural similarity to create duplicate matches. It does not prohibit syntax-derived structural metadata attached to exact normalized duplicates after detection.

These features require separate justification.

---

# **25\. V1 Acceptance Criteria**

Arid v1 is complete when all of the following are true:

1. `arid .` scans a Python repository successfully.  
2. Arbitrary cross-file duplicate blocks are detected.  
3. Same-file duplicates are detected.  
4. Comments can be ignored correctly.  
5. Docstrings can be ignored correctly.  
6. Imports can be ignored correctly.  
7. Function signatures can be ignored correctly.  
8. All ignored constructs preserve correct original source locations.  
9. `min-lines` behaves predictably using effective normalized lines.  
10. The generalized suffix-array/LCP pipeline detects exact normalized repeats deterministically.  
11. Adjacent evidence collapses into maximal duplicate regions.  
12. Contained and overlapping noise is suppressed while meaningful nested groups that add occurrences may remain.  
13. Duplicate groups support more than two occurrences.  
14. Structural context is reported as `declarative`, `executable`, or `mixed` without affecting duplicate identity.  
15. Structural scope is reported as `module`, `class`, `function`, or `mixed`.  
16. Structural classification considers only retained normalized source and is framework-agnostic.  
17. Findings report occurrence count, distinct file count, and `same-file` / `cross-file` / `mixed` distribution correctly.  
18. Human-readable diagnostics are concise and documented.  
19. JSON diagnostics are versioned, deterministic, and machine-readable.  
20. Duplication metrics are deterministic and do not double-count overlapping redundant lines.  
21. `[tool.arid]` configuration works with CLI > project config > built-in precedence.  
22. Hidden files and directories are skipped by default, can be enabled through configuration or CLI override, and remain subject to normal ignore and `exclude` rules.  
23. Exit codes work reliably in CI.  
24. Invalid Python cannot silently corrupt scan results.  
25. Results are deterministic, and when parallel execution is enabled they remain deterministic across worker-thread counts.  
26. Benchmarks demonstrate a substantial performance advantage over Pylint `R0801`.  
27. No general linting functionality overlaps with Ruff.  
28. No Python runtime is required to analyze Python source.

---

# **26\. V1 Product Principle**

When evaluating any proposed feature, ask:

> **Does this make Arid better at finding duplicated Python code?**

If no, the feature does not belong in Arid.

The intended product remains:

Ruff \+ Arid

not:

Ruff vs. Arid