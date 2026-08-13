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
9. Support both developer CLI usage and CI enforcement.  
10. Integrate naturally into Python projects using `pyproject.toml`.

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
* identifier-renaming clone detection  
* fuzzy AST similarity  
* embedding-based similarity  
* multi-language support  
* JavaScript/TypeScript support  
* general copy/paste detection for arbitrary text

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

The primary detection threshold MUST be source-line based.

Default:

min-lines \= 4

A candidate MUST contain at least `min-lines` effective normalized lines before being reported.

CLI override:

arid . \--min-lines 6

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

# **7\. Suppression**

Arid MUST provide native suppression directives.

Minimum v1 syntax:

\# arid: disable

and:

\# arid: enable

Lines within a disabled region MUST NOT participate in duplicate detection.

V1 SHOULD also recognize existing Pylint duplicate-code suppression directives where practical so repositories can migrate without immediately rewriting every suppression.

Suppression directives MUST be processed before comments are discarded.

---

# **8\. Match Construction**

## **8.1 Window Detection**

The implementation SHOULD identify candidate matches using hashes of consecutive normalized source lines.

The algorithm MUST avoid naive exhaustive comparison of every source region against every other source region.

---

## **8.2 Collision Verification**

Hash equality alone MUST NOT establish a duplicate.

Candidate matches MUST be verified against normalized source content before being reported.

Hash collisions MUST therefore produce no false duplicate findings.

---

## **8.3 Match Extension**

When adjacent matching windows belong to the same duplicated regions, Arid MUST extend them into the largest contiguous duplicate block.

For example, overlapping matches of lines:

10-13  
11-14  
12-15  
13-16

MUST produce one result:

10-16

rather than four findings.

---

## **8.4 Contained Matches**

A duplicate entirely contained within a larger reported duplicate MUST NOT normally be reported separately.

Arid SHOULD report maximal meaningful clone regions.

---

## **8.5 Multiple Occurrences**

If the same block appears in three or more locations, Arid SHOULD represent them as one duplicate group:

Duplicate block: 12 lines

src/a.py:10-21  
src/b.py:30-41  
src/c.py:75-86

rather than three independent pairwise findings.

---

## **8.6 Same-File Overlap**

Two same-file occurrences MUST NOT constitute a valid clone pair when their source regions overlap.

This prevents repetitive sequences from generating pathological self-matches.

---

# **9\. Diagnostics**

V1 defines one primary diagnostic:

DUP001 duplicate-code

Example:

DUP001 12 duplicated lines in 3 locations

  src/accounts.py:40-51  
  src/users.py:91-102  
  tests/helpers.py:18-29

12 similar lines

Diagnostics MUST contain:

* diagnostic code  
* duplicate length  
* number of occurrences  
* file path  
* starting line  
* ending line

---

# **10\. Default Human Output**

Default output SHOULD prioritize concise developer usability.

Example:

DUP001 8 duplicated lines

  src/foo.py:21-28  
  src/bar.py:54-61

DUP001 12 duplicated lines

  src/a.py:10-21  
  src/b.py:40-51  
  src/c.py:71-82

Found 2 duplicate groups.  
20 duplicate lines (3.4%).

An option SHOULD allow source snippets to be displayed when desired.

For example:

arid . \--show-source

---

# **11\. Machine-Readable Output**

V1 MUST provide JSON output.

Example:

arid . \--format json

Each finding MUST expose at minimum:

{  
  "code": "DUP001",  
  "lines": 8,  
  "locations": \[  
    {  
      "path": "src/foo.py",  
      "start\_line": 21,  
      "end\_line": 28  
    },  
    {  
      "path": "src/bar.py",  
      "start\_line": 54,  
      "end\_line": 61  
    }  
  \]  
}

JSON output MUST be stable enough for CI tooling to consume.

SARIF is explicitly deferred beyond v1 unless implementation proves trivial.

---

# **12\. Duplication Metrics**

Arid MUST report:

* analyzed files  
* analyzed source lines  
* duplicate groups  
* duplicate lines  
* duplication percentage

Duplicate-line accounting MUST avoid double-counting overlapping findings.

The precise metric MUST be deterministic and documented.

The preferred interpretation is:

> Lines redundant beyond one canonical occurrence of each duplicate group.

For a 10-line block appearing twice, the block contributes 10 duplicate lines rather than 20\.

For the same 10-line block appearing three times, it contributes 20 duplicate lines.

---

# **13\. File Discovery**

Running:

arid .

MUST recursively discover Python source files.

V1 MUST support:

.py  
.pyi

Arid SHOULD honor standard VCS ignore files, particularly `.gitignore`.

Explicit paths MUST be supported:

arid src/foo.py src/bar.py

and:

arid src tests

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

Additional v1 configuration MAY include:

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

Arid SHOULD NOT introduce a second proprietary configuration format in v1.

---

# **15\. CLI**

The primary invocation MUST remain simple:

arid .

V1 SHOULD avoid unnecessary subcommands.

Required options:

\--min-lines  
\--ignore-comments  
\--ignore-docstrings  
\--ignore-imports  
\--ignore-signatures  
\--same-file  
\--format  
\--show-source  
\--exclude  
\--version  
\--help

Boolean configuration MUST have a CLI mechanism for both enabling and disabling behavior.

Examples:

arid . \--ignore-comments

arid . \--no-ignore-comments

---

# **16\. Exit Codes**

V1 MUST use predictable CI-friendly exit codes.

0  scan succeeded and duplication policy passed  
1  duplicate-code findings caused policy failure  
2  configuration, invocation, parsing, or internal error

A successful scan and an unsuccessful scan MUST be distinguishable from a codebase that merely contains duplication.

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

Independent file reading and preprocessing SHOULD execute in parallel when beneficial.

Detection MAY also use parallel processing where doing so preserves deterministic results.

---

## **18.3 Algorithmic Constraint**

The implementation MUST NOT perform naive exhaustive pairwise substring comparison.

Candidate generation SHOULD have approximately linear or near-linear behavior with respect to normalized source size under normal workloads.

---

## **18.4 Performance Target**

The v1 benchmark goal is:

> At least an order-of-magnitude faster than Pylint's isolated duplicate-code checker on representative medium and large Python repositories when measured on identical hardware.

This is a performance target rather than permission to sacrifice correctness.

---

## **18.5 Determinism**

Parallel execution MUST NOT change:

* which duplicates are detected  
* duplicate grouping  
* canonical occurrence selection  
* metric calculations  
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

Python source MUST never be executed during analysis.

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
* hashing  
* collision verification  
* match extension  
* match grouping  
* overlap handling  
* metrics  
* configuration  
* suppression directives

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

The same source tree MUST produce byte-for-byte equivalent structured findings regardless of available thread count.

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

cargo install arid

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
source preprocessing  
        ↓  
normalized line representation  
        ↓  
candidate hashing  
        ↓  
match verification \+ extension  
        ↓  
clone grouping  
        ↓  
metrics  
        ↓  
reporting

Each stage SHOULD have a narrow responsibility.

The parser implementation MUST NOT dictate the duplicate-detection algorithm.

The duplicate-detection engine SHOULD operate on a normalized source representation independent of CLI concerns.

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
9. `min-lines` behaves predictably.  
10. Adjacent candidate matches collapse into maximal duplicate regions.  
11. Contained and overlapping noise is suppressed.  
12. Duplicate groups support more than two occurrences.  
13. Human-readable diagnostics are useful.  
14. JSON diagnostics are machine-readable.  
15. Duplication metrics are deterministic.  
16. `[tool.arid]` configuration works.  
17. Exit codes work reliably in CI.  
18. Invalid Python cannot silently corrupt scan results.  
19. Results are deterministic under parallel execution.  
20. Benchmarks demonstrate a substantial performance advantage over Pylint `R0801`.  
21. No general linting functionality overlaps with Ruff.  
22. No Python runtime is required to analyze Python source.

---

# **26\. V1 Product Principle**

When evaluating any proposed feature, ask:

> **Does this make Arid better at finding duplicated Python code?**

If no, the feature does not belong in Arid.

The intended product remains:

Ruff \+ Arid

not:

Ruff vs. Arid