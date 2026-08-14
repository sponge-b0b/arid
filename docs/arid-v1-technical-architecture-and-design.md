# **Arid v1 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust

---

# **1\. Purpose**

This document defines the technical architecture for Arid v1.

The v1 requirements specification is the product contract. This document determines how Arid will satisfy that contract while remaining:

* fast  
* deterministic  
* Python-specific  
* small in scope  
* easy to test  
* easy to maintain  
* independent of the Python runtime  
* complementary to Ruff

The core execution model is:

Python source  
    ↓  
file discovery  
    ↓  
Python parse \+ tokenize  
    ↓  
Python-aware filtering \+ structural classification  
    ↓  
normalized source lines \+ structural metadata  
    ↓  
global exact-duplicate index  
    ↓  
maximal repeated blocks  
    ↓  
DUP001 findings \+ metrics

Structural metadata describes detected duplicates but never participates in duplicate identity.

---

# **2\. Architectural Principles**

## **2.1 One purpose**

Every production component MUST support the single responsibility:

> Find duplicated Python source code.

Arid MUST NOT grow general linting infrastructure merely because it is convenient to do so.

---

## **2.2 Correctness before compatibility quirks**

Arid targets the intent of Pylint `R0801`, not bug-for-bug reproduction.

Where Pylint uses textual heuristics, Arid SHOULD use Python syntax information.

Pylint strips/filters lines and then hashes windows of `min-similarity-lines`, merging adjacent matching windows afterward. It also uses syntax information for import and function-signature filtering.

Arid preserves that behavioral intent but does not need to preserve that implementation.

---

## **2.3 Parser-specific knowledge stops at the frontend**

The duplicate-detection engine MUST NOT depend on:

* Python AST nodes  
* parser tokens  
* Ruff types  
* RustPython types  
* source syntax constructs

The Python frontend converts parser-owned syntax into Arid-owned masks and structural regions.

Normalization produces Arid-owned normalized lines containing source mapping, effective-line qualification, structural context, and structural scope.

Everything downstream operates on Arid-owned data structures.

---

## **2.4 Exact detection, not probabilistic detection**

V1 detects exact equality after configured normalization.

The core detector MUST NOT depend on hash equality as proof of duplication.

Hash maps MAY be used as implementation details where normal equality checks resolve collisions, but duplicate findings themselves MUST be exact.

Structural metadata MUST NOT influence equality.

---

## **2.5 Determinism is architectural**

Parallelism MUST NOT affect:

* findings  
* grouping  
* canonical occurrence selection  
* metrics  
* structural reporting metadata  
* JSON order  
* human-readable output order

Determinism MUST be enforced through explicit sorting and stable canonicalization, not assumed from thread scheduling.

---

## **2.6 No speculative framework**

V1 SHOULD use concrete modules rather than generic frameworks.

In particular:

* no plugin system  
* no language abstraction layer  
* no detector trait hierarchy  
* no reporter registry  
* no dependency-injection framework  
* no async runtime  
* no parser trait created merely in case the parser changes later

Parser-specific code is isolated by module boundaries, which is sufficient for v1.

---

## **2.7 Structural metadata is descriptive**

Structural context and scope help describe an already-detected duplicate.

Classification is syntax-based and framework-agnostic. It MUST NOT alter matching, grouping, metrics, or assign severity, presumed safety, or framework-specific meaning.

---

# **3\. Major Technical Decisions**

| Area | V1 decision |
| ----- | ----- |
| Python frontend | RustPython-packaged Ruff-derived parser |
| Parsing model | AST \+ lexical tokens |
| Internal matching unit | normalized physical source line |
| Ignored constructs | byte-range masks derived from tokens/AST |
| Structural metadata | syntax-derived context/scope on normalized lines; reporting only |
| Suppressions | segment barriers |
| Global representation | interned normalized-line IDs |
| Duplicate algorithm | generalized suffix array \+ LCP |
| Suffix construction | internal prefix-doubling implementation |
| LCP construction | Kasai algorithm |
| Duplicate grouping | LCP intervals / maximal repeats |
| Same-file clones | enabled, overlapping occurrences filtered |
| Parallelism | file read/parse/normalize only initially |
| File discovery | `ignore` crate |
| CLI | `clap` |
| Configuration | `pyproject.toml` via Serde \+ `toml` |
| JSON | Serde \+ `serde_json`, versioned schema |
| Runtime errors | typed errors with `thiserror` |
| Python runtime | none |
| Persistent cache | none in v1 |
| Git-diff mode | none in v1 |

---

# **4\. Parser Decision**

## **4.1 Selected frontend**

Arid uses the RustPython-packaged Ruff-derived parser crates from the RustPython Ruff repository, pinned to an explicit tag:

```toml
ruff_python_parser = { package = "rustpython-ruff_python_parser", git = "https://github.com/RustPython/ruff.git", tag = "0.15.19-rustpython" }
ruff_python_ast = { package = "rustpython-ruff_python_ast", git = "https://github.com/RustPython/ruff.git", tag = "0.15.19-rustpython" }
ruff_text_size = { package = "rustpython-ruff_text_size", git = "https://github.com/RustPython/ruff.git", tag = "0.15.19-rustpython" }
```

The Git tag fixes the parser source, and `Cargo.lock` pins the resolved dependency graph.

The parser exposes both source-ranged AST nodes and lexical tokens, including comment tokens with byte offsets, which provides the two views Arid needs.

---

## **4.2 Why not depend directly on Ruff's main repository?**

Arid SHOULD NOT depend on an unversioned Ruff workspace revision.

Using the RustPython-maintained fork at an explicit tag gives Arid a reproducible parser dependency while keeping parser-specific types isolated.

---

## **4.3 Why not Tree-sitter?**

Tree-sitter is useful for permissive structural parsing, but Arid needs authoritative Python syntax interpretation for:

* docstrings  
* imports  
* function declarations  
* comments  
* source ranges

Arid therefore SHOULD NOT use Tree-sitter as its v1 Python frontend.

---

## **4.4 Frontend isolation**

Parser types MUST remain inside:

src/python.rs

or, if that module becomes too large:

src/python/  
    mod.rs  
    masks.rs  
    syntax.rs

The frontend converts parser-specific information into Arid-owned masks and structural regions.

No parser AST or token type may cross into normalization, detection, metrics, or reporting code.

---

# **5\. High-Level Execution Pipeline**

```text
                   ┌───────────────────┐
                   │ CLI arguments     │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ Configuration     │
                   │ resolution        │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ File discovery    │
                   └─────────┬─────────┘
                             │
             ┌───────────────▼───────────────┐
             │ Parallel per-file preparation │
             │                               │
             │ read / parse / tokenize       │
             │ suppression scan              │
             │ ignored ranges                │
             │ structural regions            │
             │ normalization                 │
             └───────────────┬───────────────┘
                             │
                   sorted PreparedFiles
                             │
                   ┌─────────▼─────────┐
                   │ Line interning    │
                   │ + corpus build    │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ Suffix array      │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ LCP array         │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ Repeat extraction │
                   │ + grouping        │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ Metrics           │
                   └─────────┬─────────┘
                             │
                   ┌─────────▼─────────┐
                   │ Report building   │
                   │ + Human / JSON    │
                   └───────────────────┘
```

Structural metadata remains attached to normalized lines for reporting. Corpus construction and duplicate detection ignore it.

---

# **6\. Repository and Crate Structure**

V1 SHOULD remain a **single Cargo package**, not a workspace of internal crates.

Recommended initial structure:

arid/  
├── Cargo.toml  
├── Cargo.lock  
├── pyproject.toml  
├── src/  
│   ├── main.rs  
│   ├── lib.rs  
│   ├── cli.rs  
│   ├── config.rs  
│   ├── files.rs  
│   ├── model.rs  
│   ├── python.rs  
│   ├── normalize.rs  
│   ├── corpus.rs  
│   ├── suffix.rs  
│   ├── detect.rs  
│   ├── metrics.rs  
│   └── report.rs  
├── tests/  
│   ├── fixtures/  
│   │   ├── comments/  
│   │   ├── docstrings/  
│   │   ├── imports/  
│   │   ├── signatures/  
│   │   ├── same\_file/  
│   │   ├── suppressions/  
│   │   └── pylint\_compat/  
│   └── cli.rs  
└── benches/  
    └── detection.rs

Modules SHOULD only be split into directories after their size or responsibilities justify it.

---

## **6.1 `main.rs`**

`main.rs` MUST remain thin.

Conceptually:

fn main() \-\> ExitCode {  
    arid::run()  
}

No detection logic belongs in the binary entry point.

---

## **6.2 `lib.rs`**

Owns orchestration:

resolve settings  
discover files  
prepare files  
build corpus  
detect duplicates  
build report \+ metrics  
render result

The core implementation remains callable without shelling out to the CLI, making integration and benchmarking straightforward.

---

# **7\. Core Domain Model**

Arid SHOULD use small owned internal types.

Conceptually:

```rust
type FileId = u32;
type LineId = u32;
type CorpusPos = u32;

enum StructuralContext {
    Declarative,
    Executable,
    Mixed,
}

enum StructuralScope {
    Module,
    Class,
    Function,
}

struct PreparedFile {
    path: PathBuf,
    source: String,
    normalized: String,
    lines: Vec<NormalizedLine>,
    segments: Vec<NormalizedSegment>,
}

struct NormalizedLine {
    text_range: Range<u32>,
    source_line: u32,
    effective: bool,
    context: StructuralContext,
    scope: StructuralScope,
}

struct NormalizedSegment {
    start: u32,
    end: u32,
}

struct Occurrence {
    file: FileId,
    normalized_start: u32,
    normalized_len: u32,
}

struct DuplicateGroup {
    effective_lines: u32,
    normalized_len: u32,
    occurrences: Vec<Occurrence>,
}
```

Line-level context supports `Mixed` for physical lines retaining more than one structural context. Line-level scope uses the most specific `Module`, `Class`, or `Function` scope; finding-level mixed scope is derived during reporting.

Structural metadata does not belong to `DuplicateGroup` and does not affect duplicate identity.

Internal line numbering SHOULD be zero-based.

User-visible line numbering MUST be one-based.

---

# **8\. Source Preparation**

Each source file is processed independently.

This is the primary parallel work unit.

Path  
 ↓  
read UTF-8 source  
 ↓  
parse \+ lex  
 ↓  
collect syntax masks \+ structural regions  
 ↓  
collect suppression boundaries  
 ↓  
construct normalized lines \+ metadata  
 ↓  
PreparedFile

A source file MUST be parsed only once.

Arid MUST NOT independently reparse source for:

* imports  
* signatures  
* docstrings  
* comments  
* structural context or scope

All required syntax information should come from one parser/lexer pass.

Parser-specific types are converted to Arid-owned masks and structural regions before normalization.

---

# **9\. Source Encoding**

V1 SHOULD support:

* UTF-8  
* UTF-8 with BOM

The BOM MUST NOT participate in duplicate detection.

A non-UTF-8 source file MUST produce a clear scan error rather than being silently ignored.

Full support for arbitrary PEP 263 codecs is deferred unless real-world compatibility testing demonstrates that it is necessary for v1.

This keeps the frontend independent of Python's codec registry.

---

# **10\. Ignored-Range Model**

Arid SHOULD NOT rewrite the AST or regenerate Python source.

Instead, preprocessing produces byte ranges to exclude from duplicate analysis.

Conceptually:

struct Mask {  
    start: u32,  
    end: u32,  
}

Masks are:

1. collected  
2. sorted  
3. merged when overlapping or adjacent  
4. applied during line normalization

Structural regions are maintained separately and classify only source bytes that remain after masking.

This gives preprocessing approximately:

O(source bytes \+ number of masks)

rather than repeatedly allocating rewritten source strings.

---

# **11\. Comment Handling**

The lexer identifies actual Python comment tokens.

When:

ignore-comments \= true

those token ranges are masked.

Therefore:

value \= "\# not a comment"

remains intact.

While:

value \= 42  \# actual comment

normalizes approximately to:

value \= 42

This intentionally improves on Pylint's current textual comment removal, which splits stripped lines at `#`.

---

# **12\. Docstring Handling**

Docstrings MUST be identified structurally.

For every suite belonging to:

* module  
* class  
* function  
* async function

the frontend examines the first statement.

If that statement is a valid Python string-expression docstring, its source range is masked.

This correctly distinguishes:

def foo():  
    """Actual docstring."""

from:

def foo():  
    x \= 1  
    """Ordinary string expression."""

The second string MUST NOT be treated as the function's docstring.

Implicitly concatenated string literals SHOULD follow the parser's AST interpretation rather than custom textual logic.

---

# **13\. Import Handling**

When:

ignore-imports \= true

all AST nodes representing:

import ...

and:

from ... import ...

are masked.

This applies at any scope:

* module  
* class  
* function  
* conditional  
* `TYPE_CHECKING`

Multiline imports are naturally covered by their syntax range.

Pylint similarly uses parsed node ranges to identify complete import statements rather than relying on textual prefixes.

---

# **14\. Function-Signature Handling**

When:

ignore-signatures \= true

Arid MUST remove the declaration portion of:

* `def`  
* `async def`  
* methods  
* nested functions  
* multiline functions

The AST identifies each function.

The token stream determines the end of the declaration.

Starting at the function declaration, Arid scans tokens while tracking delimiter nesting:

()  
\[\]  
{}

The first function-header colon at delimiter depth zero terminates the signature.

This handles constructs such as:

def foo(  
    value: dict\[str, tuple\[int, int\]\],  
    callback=lambda x: x,  
) \-\> list\[int\]:

without regex heuristics.

Decorators are not part of the ignored signature.

---

## **14.1 One-line functions**

Arid MUST correctly handle:

def foo(): return calculate()

With signature ignoring enabled, the remaining normalized line is effectively:

return calculate()

This is an intentional correctness improvement over line-range-based signature removal.

Pylint currently derives ignored signature lines from the function's declaration line through the first body statement's line.

---

# **15\. Statement-Separator Cleanup**

Removing an AST statement from:

import foo; calculate()

must not leave:

; calculate()

Likewise:

calculate(); import foo

must not leave a meaningless trailing separator.

When a complete statement is masked, Arid SHOULD extend the removal range to consume an adjacent same-line semicolon and surrounding insignificant whitespace when necessary.

This cleanup MUST be token-based.

---

# **16\. Suppression Architecture**

Suppression is semantically different from ordinary filtering.

Ignoring:

import foo

means code on either side may still be compared as adjacent normalized content.

Disabling Arid analysis means code on either side SHOULD NOT be joined into one artificial clone.

Therefore suppressions create **segment boundaries**.

Example:

first()  
second()

\# arid: disable  
generated\_code()  
generated\_code()  
\# arid: enable

third()  
fourth()

Arid treats this as:

segment A:  
    first()  
    second()

segment B:  
    third()  
    fourth()

It MUST NOT detect a duplicate region that starts in `segment A` and ends in `segment B`.

---

## **16.1 Native directives**

V1 recognizes:

\# arid: disable

and:

\# arid: enable

Directive matching SHOULD be:

* case-sensitive  
* token-based  
* whitespace tolerant around the directive text

The directive comment itself never participates in matching.

---

## **16.2 Pylint suppressions**

Compatibility with:

\# pylint: disable=duplicate-code

is desirable but SHOULD NOT be implemented by approximating Pylint's complex message-scope rules incorrectly.

Native Arid suppressions are mandatory for v1.

Pylint directive compatibility may be added once compatibility fixtures define the precise semantics Arid intends to support.

---

# **17\. Line Normalization**

After masks and structural regions are constructed, Arid walks physical source lines once.

For each line:

1. determine masked portions  
2. copy unmasked fragments into a small reusable scratch buffer  
3. remove leading/trailing insignificant whitespace  
4. discard the line if empty  
5. append remaining text into the file's `normalized` buffer  
6. record its original source line  
7. classify whether it counts toward the duplication threshold  
8. classify retained source context and scope

Internal whitespace is preserved.

Therefore:

value \= calculate()

and:

value=calculate()

are **not** identical in v1.

This matches the source-oriented nature of `R0801`.

---

## **17.1 Line-ending normalization**

The following MUST compare identically:

LF  
CRLF

Line-ending bytes are not part of normalized lines.

---

## **17.2 Structural classification**

Classification uses only retained source after masking.

The frontend supplies syntax-derived regions; normalization selects the most specific region covering each retained source fragment.

A line retaining both declarative and executable source is `mixed`.

For example:

```python
setting = 1; run()
```

may classify as mixed, while:

```python
import os; run()
```

with imports ignored classifies from the retained `run()` source.

Structural context and scope MUST NOT change normalized text, effective-line qualification, segment boundaries, line interning, or duplicate equality.

---

# **18\. Effective Lines**

Not every repeated physical line should contribute to `min-lines`.

For example:

)

or:

\]

may participate in an exact block but should not by itself make a clone meaningful.

Each `NormalizedLine` therefore carries:

effective: bool

A line counts as effective when its normalized text contains substantive source content.

The v1 implementation SHOULD use a simple deterministic criterion aligned with Pylint's intent:

> The normalized line contains at least one Unicode alphanumeric character or `_`.

Pylint currently uses a similar content test when accounting for meaningful common lines.

Effective-line qualification is independent of structural context and scope.

---

## **18.1 Detection versus qualification**

Punctuation-only lines remain part of the exact sequence.

They simply do not count toward:

min-lines

So:

normalized block length: 7  
effective lines:         4  
min-lines:               4

is reportable.

This is more accurate than deleting punctuation lines entirely.

---

# **19\. Normalized Storage**

Arid SHOULD avoid allocating one `String` per normalized line.

Instead each `PreparedFile` owns one normalized text buffer:

"value \= calculate()\\nsave(value)\\n..."

and each line stores a range into it:

text\_range: Range\<u32\>

Benefits:

* fewer allocations  
* good cache locality  
* cheap borrowing during interning  
* simple lifetime ownership  
* original source remains available for diagnostics

---

# **20\. Global Line Interning**

The detector operates on integer line identities rather than strings.

After all files are prepared and sorted by path, Arid interns every distinct normalized line.

Example:

"value \= calculate()"  → 17  
"save(value)"          → 42  
"return value"         → 91

Then:

value \= calculate()  
save(value)  
return value

becomes:

17 42 91

Two lines receive the same `LineId` **only if their normalized text is exactly equal**.

Structural context and scope are not part of line identity.

A normal Rust `HashMap` MAY be used for interning because hash collisions are resolved by string equality and therefore cannot create false duplicate findings.

---

## **20.1 Deterministic interning**

Prepared files MUST first be sorted by normalized output path.

Lines are then interned in deterministic:

path order  
    ↓  
line order

The numeric value of a `LineId` is therefore deterministic for identical input.

Detection correctness does not depend on what numeric ID a line receives.

---

# **21\. Corpus Construction**

The normalized segments are concatenated into one global integer sequence.

Example:

file A / segment 1:  
    10 20 30

file A / segment 2:  
    40 50

file B / segment 1:  
    10 20 30

becomes conceptually:

10 20 30 S1 40 50 S2 10 20 30 S3

where:

S1  
S2  
S3

are unique sentinel tokens.

Each segment receives a distinct sentinel.

---

## **21.1 Why unique sentinels?**

A match MUST never cross:

* a file boundary  
* a disabled-region boundary

Because every sentinel is unique and cannot equal a normalized source-line ID, no duplicate substring can cross those boundaries.

No special boundary logic is then required inside the suffix-array algorithm.

---

# **22\. Primary Detection Algorithm**

## **22.1 Decision**

Arid v1 SHOULD use a:

> **Generalized suffix array plus longest-common-prefix array over normalized line IDs.**

This intentionally differs from Pylint's rolling-window hash implementation.

---

## **22.2 Why this is a better fit**

Exact duplicate detection asks:

> Which contiguous subsequences of normalized lines occur more than once?

That is precisely a repeated-substring problem.

A suffix array directly indexes all suffixes of the normalized corpus.

The corresponding LCP array identifies how many leading lines adjacent suffixes share.

This gives Arid several properties naturally:

* arbitrary source blocks  
* cross-file clones  
* same-file clones  
* groups with 3+ occurrences  
* exact matching  
* no hash-collision verification stage  
* no pairwise comparison of every file  
* deterministic output  
* bounded worst-case behavior

---

# **23\. Why Not Use Pylint's Rolling-Hash Algorithm?**

Pylint hashes windows of `N` stripped lines, finds common hash buckets between files, and merges adjacent matches. Its current implementation even contains special handling to prevent a large Cartesian product when many identical windows fall into the same hash bucket.

That architecture is workable, but Arid does not need to inherit it.

A suffix-array design eliminates the central hot-bucket problem entirely.

---

# **24\. Why Implement the Suffix Array Internally?**

The detector operates on a sequence of `u32` line identities, not ordinary UTF-8 text.

Current Rust suffix-array packages present tradeoffs such as:

* old/prototype implementations  
* byte-oriented APIs  
* C bindings  
* pre-alpha/nightly-only implementations

For example, the current `sais-rs` project describes itself as pre-alpha, 8-bit-input-only, and requiring nightly Rust, while the older `suffix` project describes limitations around generalized suffix arrays.

For Arid's narrow use case, a small internal suffix-array implementation over `u32` is preferable to taking a fragile algorithm dependency.

---

# **25\. Suffix-Array Construction**

V1 SHOULD use **prefix doubling**.

Conceptually:

initial rank:  
    rank suffixes by first LineId

width \= 1

repeat:  
    rank each suffix by:

        (  
            rank\[position\],  
            rank\[position \+ width\]  
        )

    assign new compact ranks

    width \*= 2

until every suffix has a unique rank

A radix/counting sort over integer ranks SHOULD be used rather than general comparison sorting.

Expected complexity:

Time:   O(n log n)  
Memory: O(n)

where:

n \= number of normalized lines \+ segment sentinels

---

## **25.1 Integer width**

V1 SHOULD use `u32` corpus indices and line IDs.

This supports more than four billion normalized-line positions.

If the corpus exceeds the representable limit, Arid MUST fail clearly rather than overflow or silently truncate.

Using `u32` halves several large working arrays relative to `usize` on 64-bit platforms.

---

# **26\. LCP Construction**

Once the suffix array is built, Arid constructs the **Longest Common Prefix** array using the Kasai algorithm.

Conceptually:

suffix array:  
    positions sorted by suffix content

LCP:  
    number of equal normalized lines  
    shared by each adjacent suffix pair

LCP construction is:

O(n)

The result allows repeated regions to be discovered without comparing complete source blocks repeatedly.

---

# **27\. Duplicate-Group Extraction**

Arid traverses the LCP array using a monotonic stack.

The traversal identifies **LCP intervals**.

Each interval represents:

one repeated normalized sequence  
\+  
all suffixes sharing that sequence

Conceptually:

Shared sequence:  
    17 42 91 23 88

Occurrences:  
    file A line 20  
    file B line 75  
    file C line 31

becomes one group rather than three pairwise reports.

---

# **28\. Maximal Repeat Rule**

Arid SHOULD report maximal meaningful repeated blocks.

A candidate repeat is reportable when:

1. its effective-line count is at least `min-lines`  
2. at least two valid occurrences remain  
3. the occurrences cannot all be extended left by the same normalized line  
4. the LCP interval establishes the corresponding right boundary

The left-extension check prevents reporting:

lines 11-18

when every occurrence is actually part of:

lines 10-18

---

## **28.1 Nested repeat groups**

Consider:

A and B share 12 lines  
A, B, and C share the first 8 of those lines

Arid MAY legitimately report:

12-line clone: A, B  
8-line clone:  A, B, C

because the occurrence sets differ.

The 8-line result is not merely duplicate reporting of the same clone; it identifies an additional occurrence.

---

# **29\. Same-File Detection**

Same-file detection requires no alternate algorithm.

Suffixes from the same source segment naturally coexist in the global suffix array.

When multiple occurrences from one file overlap, Arid MUST avoid reporting them as independent clone occurrences.

For equal-length clone intervals, overlapping occurrences within each file SHOULD be reduced deterministically using increasing start position.

Example:

candidate starts:

10  
11  
40

for a 10-line clone becomes:

10  
40

rather than treating heavily overlapping positions `10` and `11` as separate meaningful copies.

---

# **30\. Source Mapping**

Every normalized line retains:

original physical source line

A detected normalized range:

normalized lines 20..28

maps to:

source start \= source\_line(normalized\[20\])  
source end   \= source\_line(normalized\[27\])

Ignored constructs may therefore exist inside the reported physical source span.

That is intentional.

Example:

calculate()

\# ignored comment

save()

can form a two-effective-line normalized block while retaining the correct original source range.

---

# **31\. Canonical Occurrence**

Every duplicate group requires one deterministic canonical occurrence.

The canonical occurrence MUST be the first under:

normalized relative path  
    ↓  
source start line  
    ↓  
source end line

All other occurrences are considered redundant for duplication metrics.

This choice has no semantic meaning beyond deterministic accounting and display.

---

# **32\. Duplicate Metrics**

Arid SHOULD distinguish three concepts.

## **32.1 Physical source lines**

Total physical lines read from analyzed files.

source\_lines

---

## **32.2 Analyzed lines**

Effective normalized lines after:

* exclusions  
* suppressions  
* configured Python filtering

analyzed\_lines

---

## **32.3 Duplicate lines**

Effective analyzed lines belonging to redundant occurrences.

For a 10-line clone appearing twice:

duplicate\_lines \= 10

For the same clone appearing three times:

duplicate\_lines \= 20

The canonical copy is not redundant.

---

# **33\. Overlapping Metrics**

Different duplicate groups may overlap source regions.

Duplicate-line accounting MUST NOT count the same normalized source line twice.

Arid SHOULD maintain a per-file set of redundant normalized intervals and merge them before calculating totals.

Conceptually:

group A marks 10-20  
group B marks 15-25

becomes:

10-25

for metric purposes.

---

# **34\. Duplication Percentage**

Arid SHOULD define:

duplication\_percent \=  
    duplicate\_lines  
    /  
    analyzed\_lines  
    × 100

This is preferable to dividing normalized duplicate lines by raw physical lines because the numerator and denominator then represent the same analysis domain.

Human output MAY additionally report physical source lines for context.

---

# **35\. File Discovery**

Arid SHOULD use the `ignore` crate from the ripgrep ecosystem.

That crate provides directory walking with Git-ignore-aware filtering and also offers parallel walking when needed.

V1 SHOULD use discovery sequentially because discovery itself is unlikely to dominate runtime.

The expensive per-file preparation stage is parallelized separately.

---

## **35.1 Supported files**

Directory discovery includes:

\*.py  
\*.pyi

only.

---

## **35.2 Explicit files**

An explicitly supplied file SHOULD be analyzed even if normal directory discovery would ignore it.

Thus:

arid ignored/generated.py

means what it says.

Configured explicit `exclude` patterns MAY still reject the file if that behavior is documented consistently.

---

## **35.3 Symlinks**

Directory symlinks MUST NOT be followed by default.

This avoids:

* duplicate traversal  
* cycles  
* surprising scans outside the repository

---

# **36\. Configuration Resolution**

Configuration precedence is:

CLI  
 ↓  
\[tool.arid\]  
 ↓  
built-in defaults

Arid SHOULD search from the current working directory upward for the nearest:

pyproject.toml

containing:

\[tool.arid\]

No additional Arid-specific configuration file format is introduced in v1.

---

## **36.1 Configuration model**

Conceptually:

struct Settings {  
    min\_lines: u32,  
    ignore\_comments: bool,  
    ignore\_docstrings: bool,  
    ignore\_imports: bool,  
    ignore\_signatures: bool,  
    same\_file: bool,  
    excludes: Vec\<String\>,  
}

CLI parsing SHOULD produce optional overrides rather than immediately filling defaults.

Configuration is then merged once into one immutable `Settings`.

---

## **36.2 Unknown keys**

Arid SHOULD reject unknown `[tool.arid]` keys.

Example:

\[tool.arid\]  
ignore-comment \= true

should produce a configuration error rather than silently doing nothing.

Serde's:

\#\[serde(deny\_unknown\_fields)\]

is appropriate here.

---

# **37\. Parallelism**

V1 parallelism SHOULD be intentionally narrow.

The following work is independent per file:

read  
parse  
lex  
collect masks  
normalize

That stage SHOULD use Rayon.

Rayon provides data-parallel iterators and dynamically divides work among worker threads.

---

## **37.1 Deterministic reduction**

Parallel preparation produces:

Vec\<Result\<PreparedFile\>\>

The completed results are then sorted by path before any global indexing occurs.

Therefore worker completion order cannot affect output.

---

## **37.2 Global detection**

Suffix-array construction SHOULD initially remain single-process and deterministic.

It MAY internally parallelize later only if benchmarks demonstrate that it is worthwhile.

Do not introduce parallel suffix-array complexity before measurement proves it necessary.

---

## **37.3 No async runtime**

Arid has no network operations and primarily performs:

* filesystem reads  
* parsing  
* CPU analysis

Tokio or another async runtime is unnecessary.

---

# **38\. File I/O**

V1 SHOULD use normal whole-file reads.

Do not introduce memory mapping initially.

Python files are generally small enough individually that:

std::fs::read\_to\_string

or equivalent whole-file reads provide a simpler and portable implementation.

The original source SHOULD remain resident until reporting completes so `--show-source` does not require rereading files.

---

# **39\. Diagnostic Model**

Detection results are separate from report findings.

`DuplicateGroup` contains duplicate identity and occurrences. Report construction maps those occurrences to source and derives user-facing metadata.

Conceptually:

```rust
struct Finding {
    code: String,
    lines: u32,
    context: FindingContext,
    scope: FindingScope,
    occurrences: u32,
    files: u32,
    distribution: FindingDistribution,
    locations: Vec<Location>,
}

struct Location {
    path: String,
    start_line: u64,
    end_line: u64,
    source: Option<String>,
}
```

Finding context is `declarative`, `executable`, or `mixed`.

Finding scope is `module`, `class`, `function`, or `mixed`.

Finding distribution is:

* `same-file` — all occurrences are in one file  
* `cross-file` — multiple files, one occurrence per involved file  
* `mixed` — multiple files and at least one repeated occurrence within an involved file

Context and scope are aggregated across all normalized lines and all occurrences. Disagreement produces `mixed`.

`DUP001` is currently the only diagnostic.

---

# **40\. Output Ordering**

Findings MUST be sorted deterministically by:

1. canonical path  
2. canonical start line  
3. clone length descending  
4. remaining occurrence paths and lines

Occurrences inside each group MUST also be sorted.

No hash-map iteration order may leak into output.

---

# **41\. Human Reporter**

Default output SHOULD remain compact.

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

Report construction MAY perform source mapping and structural/distribution aggregation.

Rendering MUST NOT perform detection logic or assign severity, presumed safety, or framework-specific labels.

---

# **42\. JSON Reporter**

JSON SHOULD use a versioned top-level schema.

The current alpha schema is version `3`.

Example:

```json
{
  "version": 3,
  "files": 42,
  "source_lines": 12000,
  "analyzed_lines": 8400,
  "duplicate_groups": 18,
  "duplicate_lines": 286,
  "duplication_percent": 3.4048,
  "findings": [
    {
      "code": "DUP001",
      "lines": 8,
      "context": "executable",
      "scope": "function",
      "occurrences": 2,
      "files": 2,
      "distribution": "cross-file",
      "locations": [
        {
          "path": "src/foo.py",
          "start_line": 21,
          "end_line": 28
        },
        {
          "path": "src/bar.py",
          "start_line": 54,
          "end_line": 61
        }
      ]
    }
  ]
}
```

When source display is disabled, location objects SHOULD omit `source`.

An incompatible schema change MUST increment the top-level version.

---

# **43\. Error Model**

Arid SHOULD use typed internal errors with `thiserror`.

Top-level categories:

Configuration  
Discovery  
I/O  
Encoding  
Python parse  
Internal invariant

Parse errors MUST include:

* path  
* source line/column where available  
* parser error text

---

## **43.1 Partial scans**

If any selected Python file cannot be parsed or decoded, Arid SHOULD:

1. collect preparation errors  
2. sort them deterministically  
3. report them  
4. return exit code `2`  
5. not present partial duplication metrics as though the scan were complete

Silent skipping is prohibited.

---

# **44\. Exit Codes**

0  scan complete; no policy violation  
1  DUP001 finding(s)  
2  invocation/configuration/input/internal failure

An internal panic is a bug and MUST NOT be treated as a normal exit-code-2 path.

---

# **45\. Runtime Dependencies**

The initial runtime dependency set SHOULD remain approximately:

clap  
ignore  
rayon  
serde  
serde\_json  
toml  
thiserror

rustpython-ruff\_python\_parser  
rustpython-ruff\_python\_ast  
rustpython-ruff\_text\_size

Avoid adding dependencies for functionality easily expressed with the standard library.

In particular, v1 does not require:

* Tokio  
* regex  
* database crates  
* cache crates  
* mmap crates  
* logging frameworks  
* HTTP clients  
* embedding libraries  
* parser generators

---

# **46\. Development Dependencies**

Useful development-only dependencies MAY include:

proptest  
criterion  
tempfile

`proptest` is especially valuable for validating suffix-array correctness against a simple reference implementation.

---

# **47\. Detection Correctness Testing**

The suffix-array detector MUST have a deliberately slow reference implementation available under tests.

For small generated sequences:

optimized detector

must produce the same repeated regions as:

brute-force detector

Property tests SHOULD generate:

* random line-ID sequences  
* repetitive sequences  
* identical files  
* periodic sequences  
* same-file overlaps  
* many identical lines  
* segment barriers  
* empty files  
* one-line files

The optimized algorithm must be proven against the simple implementation rather than trusted because the implementation looks correct.

---

# **48\. Python Frontend Testing**

Fixtures MUST cover at least:

## **Comments**

\# comment  
value \= "\# string"  
value \= 1  \# comment

## **Docstrings**

"""module"""

class A:  
    """class"""

    def foo(self):  
        """function"""

## **Ordinary strings**

def foo():  
    value \= 1  
    """not a docstring"""

## **Imports**

import foo

from foo import (  
    one,  
    two,  
)

## **Signatures**

async def foo(  
    value: dict\[str, tuple\[int, int\]\],  
) \-\> list\[int\]:  
    return \[\]

## **One-line functions**

def foo(): return calculate()

## **Decorators**

@decorator  
def foo():  
    return calculate()


## **Structural metadata**

Fixtures MUST also cover:

* declarative module/class source  
* executable function source  
* nested scopes  
* mixed-context same-line statements  
* retained-source classification after masking  
* finding-level mixed context/scope aggregation  
* same-file / cross-file / mixed distribution

---

# **49\. Pylint Compatibility Fixtures**

A dedicated fixture set SHOULD execute equivalent source examples through:

Pylint R0801  
Arid

Results are classified as:

MATCH  
INTENTIONAL IMPROVEMENT  
ARID BUG

Intentional deviations require a fixture and comment explaining why Arid differs.

This prevents accidental compatibility drift.

---

# **50\. Determinism Testing**

Tests MUST execute detection under differing Rayon thread counts.

For example:

1  
2  
4  
8

and verify identical serialized findings, including structural and distribution metadata.

At minimum, structured JSON output SHOULD be byte-identical after removing intentionally environment-dependent metadata, if any.

V1 SHOULD avoid environment-dependent metadata entirely.

---

# **51\. Performance Testing**

Arid needs two benchmark layers.

## **51.1 Microbenchmarks**

Criterion benchmarks SHOULD measure:

* normalization  
* line interning  
* suffix-array construction  
* LCP construction  
* repeat extraction

These identify implementation regressions.

---

## **51.2 End-to-end benchmarks**

External benchmark scripts SHOULD compare:

arid  
pylint R0801 / symilar  
jscpd v5

against identical repositories on identical hardware.

Performance documentation MUST clearly distinguish:

cold full scan

from any future cached/incremental scan.

V1 has no cache, so its benchmark story remains simple.

---

# **52\. Memory Model**

For:

N normalized lines

the primary global arrays are approximately:

corpus tokens  
suffix array  
rank array  
temporary rank/sort array  
LCP array  
position mapping

Each SHOULD use compact integer storage where practical.

Structural context/scope remain per-line metadata and are not copied into suffix-array/LCP working arrays.

Memory is expected to remain:

O(N)

The design MUST avoid constructing:

all file pairs  
all matching substring pairs  
all possible source windows

simultaneously.

---

# **53\. Packaging**

Cargo metadata:

\[package\]  
name \= "arid-cli"

The executable remains:

\[\[bin\]\]  
name \= "arid"  
path \= "src/main.rs"

A library target MAY use:

\[lib\]  
name \= "arid"  
path \= "src/lib.rs"

This gives:

cargo install arid-cli

arid .

---

# **54\. PyPI Distribution**

PyPI distribution SHOULD use Maturin's binary-binding mode.

Maturin explicitly supports distributing Rust binary applications as Python packages, placing the executable on the installed environment's `PATH`.

This enables:

uv tool install arid

and:

pip install arid

without requiring a Python extension module.

Running Arid still invokes only the Rust executable.

---

# **55\. Release Artifacts**

Eventually supported release forms SHOULD be:

PyPI wheels        → arid  
crates.io package  → arid-cli  
GitHub binaries    → arid

Publishing infrastructure SHOULD remain outside the detection architecture.

---

# **56\. Explicitly Rejected V1 Architectures**

## **Generic multi-language frontend**

Rejected.

Arid is Python-specific.

---

## **AST clone detector**

Rejected.

Arid detects exact source duplication after normalization, not structural similarity.

---

## **Token-stream clone detector**

Rejected as the primary model.

Token comparison would alter `R0801` semantics by making formatting differences disappear.

---

## **Rolling hashes as primary detector**

Rejected.

Useful for candidate search, but unnecessary when a suffix/LCP model directly solves the exact repeated-subsequence problem.

---

## **Tree-sitter frontend**

Rejected for v1.

The parser's role requires stronger Python-specific correctness.

---

## **Persistent database/cache**

Rejected for v1.

First prove that a cold scan is sufficiently fast.

---

## **Git-aware incremental detector**

Rejected for v1.

Useful later, but not necessary to establish the product.

---

## **Memory-mapped source**

Rejected initially.

Measure normal I/O first.

---

## **Multi-crate workspace**

Rejected initially.

One package is enough.

---

# **57\. Implementation Sequence**

Implementation SHOULD proceed from the core domain outward.

## **Phase 1 — Internal source model**

Implement:

PreparedFile  
NormalizedLine  
NormalizedSegment  
StructuralContext  
StructuralScope  
Occurrence  
DuplicateGroup

No CLI behavior beyond what tests require.

---

## **Phase 2 — Python normalization**

Implement:

parser integration  
comment ranges  
docstrings  
imports  
function signatures  
source mapping  
suppression segments  
structural regions  
retained-source context/scope classification

Exit criterion:

> Given Python source and settings, Arid produces the expected normalized representation and structural metadata.

---

## **Phase 3 — Corpus construction**

Implement:

line interning  
LineId assignment  
segment sentinels  
corpus position mapping

Exit criterion:

> Multiple normalized files become one exact integer corpus without losing source locations.

---

## **Phase 4 — Suffix array**

Implement:

prefix-doubling suffix array  
radix rank sorting

Test against a naive suffix sort.

Exit criterion:

> Optimized and reference suffix arrays agree for generated corpora.

---

## **Phase 5 — LCP**

Implement Kasai LCP construction.

Test against naive prefix comparison.

---

## **Phase 6 — Duplicate extraction**

Implement:

LCP intervals  
minimum effective lines  
left-maximal filtering  
occurrence grouping  
same-file overlap filtering  
source mapping

Exit criterion:

> Core detection returns correct `DuplicateGroup`s independently of CLI/reporting.

---

## **Phase 7 — Metrics**

Implement:

canonical occurrence  
redundant interval union  
duplicate lines  
duplication percentage

---

## **Phase 8 — File discovery and configuration**

Add:

ignore walker  
.py / .pyi filtering  
pyproject.toml  
higher-precedence settings override merging

---

## **Phase 9 — Reporting and outcome model**

Add:

DUP001 human reporter  
versioned JSON schema  
structural context/scope aggregation  
occurrence/file distribution metadata  
source display support  
success / findings / error outcome model

Exit criterion:

> Detection results can be converted into deterministic human or JSON reports with a stable outcome independent of the CLI.

---

## **Phase 10 — CLI and end-to-end execution**

Add:

complete `clap` argument parsing  
positive and negative boolean overrides  
CLI `exclude` replacement semantics  
default current-directory scan  
configuration override wiring  
end-to-end pipeline orchestration  
human / JSON output selection  
`--show-source` wiring  
`--help` and `--version`  
process exit 0 / 1 / 2

Exit criterion:

> `arid` can execute a complete scan from command-line arguments through deterministic reporting and return the required process exit status.

---

## **Phase 11 — Parallel preparation**

Replace sequential per-file preparation with Rayon.

Verify deterministic output before and after.

---

## **Phase 12 — Packaging**

Add:

arid-cli crate publishing  
Maturin bin wheel  
PyPI alpha  
GitHub release binaries

---

## **Phase 13 — Benchmarks**

Benchmark:

Arid  
Pylint R0801  
jscpd v5

Only optimize measured bottlenecks.

---

# **58\. First Publishable Vertical Slice**

The first legitimate alpha does not require every v1 feature.

A useful:

0.1.0a1

can contain:

arid .  
    ↓  
discover .py files  
    ↓  
parse Python  
    ↓  
ignore comments/docstrings/imports/signatures  
    ↓  
normalize lines  
    ↓  
suffix/LCP detection  
    ↓  
DUP001 output

It does not need:

* JSON  
* metrics  
* source snippets  
* every exclusion option  
* parallelism  
* PyLint directive compatibility  
* advanced reporting

This is sufficient to make Arid a real functioning tool and claim the PyPI project legitimately.

---

# **59\. Architecture Invariants**

The implementation MUST preserve these invariants.

### **A1**

Two lines share a `LineId` if and only if their normalized text is exactly equal.

### **A2**

No duplicate match can cross a file boundary.

### **A3**

No duplicate match can cross an Arid-disabled region.

### **A4**

Every reported occurrence maps entirely to real source lines.

### **A5**

Every `DUP001` group has at least two valid occurrences.

### **A6**

Every reported group contains at least `min-lines` effective lines.

### **A7**

Two overlapping same-file occurrences are not independently counted as duplicate copies.

### **A8**

Hash collisions cannot create duplicate findings.

### **A9**

Parallel scheduling cannot change the result, including structural and distribution metadata.

### **A10**

Python source is never executed.

### **A11**

Parser-specific types never leave the Python-analysis boundary.

### **A12**

A parse failure cannot silently become a partial successful scan.

### **A13**

Duplicate-line metrics never double-count the same redundant normalized line.

### **A14**

No v1 feature performs general linting that belongs in Ruff.


### **A15**

Structural context and scope never participate in line interning, suffix-array/LCP matching, duplicate equality, grouping, or metrics.

### **A16**

Structural classification describes only retained source after configured masking and suppression.

### **A17**

Finding-level context and scope aggregate across all occurrences and never imply severity or framework semantics.

---

# **60\. Architectural North Star**

The complete v1 architecture should remain understandable as:

discover  
   ↓  
parse  
   ↓  
normalize  
   ↓  
intern lines  
   ↓  
suffix array  
   ↓  
LCP  
   ↓  
maximal repeats  
   ↓  
DUP001

Structural metadata travels alongside normalized lines and is consumed only when findings are built for reporting; it is not another detection stage.

If the implementation begins requiring substantially more conceptual machinery than that, the design should be challenged before adding it.

Arid's advantage should come from:

> **a better representation and a better algorithm, not a larger system.**