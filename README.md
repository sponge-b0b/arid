<p align="center">
  <img src="assets/arid-logo.png" alt="Arid banner" width="100%" />
</p>

<p align="center">
<!-- release-badge:start -->
  <a href="#project-status"><img alt="Status: Beta" src="https://img.shields.io/badge/status-beta-blue"></a>
<!-- release-badge:end -->
  <a href="#what-is-arid"><img alt="Scope: Python-only" src="https://img.shields.io/badge/scope-Python--only-3776AB"></a>
  <a href="#what-is-arid"><img alt="Purpose: Duplicate-code detection" src="https://img.shields.io/badge/purpose-duplicate--code%20detection-D79A3B"></a>
  <a href="#configuration"><img alt="Config: tool.arid" src="https://img.shields.io/badge/config-tool.arid-4B5563"></a>
  <a href="#license"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue"></a>
</p>

<h1 align="center">
  <img src="assets/arid-icon.png" alt="Arid icon" width="28" valign="middle" /> Arid
</h1>

<p align="center">
  <strong>Fast Python duplicate-code checker written in Rust. A focused replacement for Pylint <code>R0801</code> that complements Ruff.</strong>
</p>

<p align="center">
  <a href="#what-is-arid">What is Arid?</a> ·
  <a href="#project-status">Project status</a> ·
  <a href="#goals">Goals</a> ·
  <a href="#usage">Usage</a> ·
  <a href="#understanding-arids-output">Output</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#license">License</a>
</p>

---

## Project status

<!-- release-status:start -->
> [!IMPORTANT]
> **Arid is currently in beta.** The intended release feature set and core interfaces are frozen while prerelease validation and bug fixing continue toward stable release.
<!-- release-status:end -->

Arid is a **small, focused CLI** for one job:

> Detect duplicated Python source code quickly and accurately.

---

## What is Arid?

Arid is a Python-specific CLI for duplicate-code detection.

It is designed to replace the duplicate-code functionality of **Pylint `R0801` / `symilar`** without turning into another general-purpose linter. Arid is intentionally narrow in scope and is meant to run alongside [Ruff](https://github.com/astral-sh/ruff), not compete with it.

```text
Ruff
├── linting
├── formatting
├── imports
├── modernization
└── general code quality

Arid
└── duplicate-code detection
```

> **Why Arid?** Because duplicated code isn't DRY.

---

## Why not just use Pylint?

Pylint's `R0801` checker provides useful Python-aware duplicate-code detection, but duplicate analysis can become very slow on larger codebases.

Arid aims to preserve the useful behavior of `R0801` while using a Rust-native architecture designed specifically for duplicate detection.

The goal is **not** bug-for-bug compatibility. Where Pylint relies on textual heuristics, Arid prefers correct Python syntax interpretation.

---

## Why not just use jscpd?

[jscpd](https://github.com/kucherenko/jscpd) is a capable multi-language copy/paste detector, and its current implementation is also written in Rust.

Arid occupies a narrower niche:

- Python only
- focused on Pylint-style duplicate-code semantics
- Python-aware filtering for comments, docstrings, imports, and signatures
- designed to fit naturally into modern Python workflows
- intentionally minimal in scope

Arid is **not** intended to replace jscpd for multi-language repositories.

---

## Goals

Arid v1 is designed to:

- detect duplicated Python source blocks across files
- detect duplicated blocks within the same file
- ignore comments, docstrings, imports, and function signatures when configured
- preserve accurate original source locations
- report concise `DUP001` diagnostics
- describe duplicate findings using Python structural context
- provide deterministic duplication metrics
- support `pyproject.toml` configuration via `[tool.arid]`
- provide machine-readable JSON output
- run substantially faster than Pylint's duplicate-code checker
- require no Python runtime to analyze Python source

---

## Non-goals

Arid is intentionally **not** a general-purpose linter.

It does not aim to provide:

- formatting
- import sorting
- type checking
- dead-code detection
- complexity analysis
- security scanning
- semantic clone detection
- structural clone matching
- fuzzy AST similarity
- multi-language duplicate detection

If a feature belongs naturally in Ruff, it does not belong in Arid.

---

## Usage

Arid is intended to fit naturally into a Python quality workflow:

```bash
ruff check .
arid .
```

Scan specific files or directories:

```bash
arid src tests
```

Require a larger duplicate before reporting it:

```bash
arid . --min-lines 8
```

Override normalization behavior for a single scan:

```bash
arid . --no-ignore-docstrings
```

Configurable boolean options support both positive and negative forms:

```text
--ignore-comments       --no-ignore-comments
--ignore-docstrings     --no-ignore-docstrings
--ignore-imports        --no-ignore-imports
--ignore-signatures     --no-ignore-signatures
--same-file             --no-same-file
--hidden                --no-hidden
```

This allows command-line arguments to explicitly override either value from `pyproject.toml`.

Hidden files and directories are skipped by default during directory discovery. Include them when needed with:

```bash
arid . --hidden
```

This allows Arid to scan Python files under hidden directories such as `.github/` while still honoring `.gitignore` and configured `exclude` patterns.

Exclude matching paths:

```bash
arid . --exclude 'generated/**'
```

`--exclude` may be repeated:

```bash
arid . \
    --exclude 'generated/**' \
    --exclude 'vendor/**'
```

Include the original source in each finding:

```bash
arid . --show-source
```

Emit machine-readable JSON:

```bash
arid . --json
```

Example diagnostic:

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

---

## Understanding Arid's output

Arid separates two questions:

> **Detection answers: "Is this code duplicated?"**  
> **Context helps answer: "What kind of code is duplicated?"**

Arid deliberately does **not** assign a severity or decide whether duplication should be removed. Duplicate code can be intentional, harmless, framework-driven, or worth refactoring.

The structural metadata exists to help you make that decision.

Consider:

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

### `DUP001 4 duplicated lines`

`DUP001` is Arid's duplicate-code diagnostic.

`4 duplicated lines` means the matching region contains four **effective normalized lines** that satisfy the configured duplicate threshold.

Arid compares source after its configured Python-aware normalization. Depending on configuration, this can remove constructs such as:

- comments
- docstrings
- imports
- function signatures

Blank lines do not count toward `min-lines`, and lines containing only non-substantive punctuation do not increase the effective-line count.

Because of that, a finding reported as four duplicated lines may span more than four physical source lines.

### `Context`

`Context` describes the structural kind of Python code involved in the duplicate.

Possible values are:

| Context | Meaning |
| --- | --- |
| `declarative` | The duplicate consists of declarations or definitions, such as direct module/class assignments or definitions. |
| `executable` | The duplicate consists of executable statements, control flow, or function-body logic. |
| `mixed` | The duplicate contains or occurs across more than one structural context. |

For example:

```text
Context: declarative
```

often appears for repeated class or module definitions.

```text
Context: executable
```

often appears for repeated application logic inside functions.

> [!NOTE]
> Context is **descriptive, not a severity**. `declarative` does not mean "safe to ignore," and `executable` does not mean "must refactor."

Arid describes Python structure without attempting to infer framework semantics or developer intent.

It therefore does not label findings as "ORM boilerplate," "configuration noise," "safe duplication," or similar framework-specific categories.

### `Scope`

`Scope` describes where the duplicated code occurs structurally.

Possible values are:

| Scope | Meaning |
| --- | --- |
| `module` | Module-level code. |
| `class` | Code structurally associated with a class. |
| `function` | Code structurally associated with a function or method. |
| `mixed` | The duplicate spans or occurs across more than one scope. |

For example:

```text
Context: executable
Scope: function
```

indicates repeated executable logic within functions or methods.

By contrast:

```text
Context: declarative
Scope: class
```

indicates repeated declarative code associated with classes.

Again, scope describes **where** the duplicate exists, not whether it is a problem.

### `Occurrences`

The occurrence line tells you how widely the duplicate appears.

```text
Occurrences: 2 across 2 files (cross-file)
```

contains three pieces of information:

- the number of duplicate occurrences
- the number of distinct files containing them
- how those occurrences are distributed

Distribution values are:

| Distribution | Meaning |
| --- | --- |
| `same-file` | All occurrences are contained in one file. |
| `cross-file` | Occurrences are spread across multiple files, with one occurrence in each involved file. |
| `mixed` | Multiple files are involved and at least one file contains multiple occurrences. |

Examples:

```text
Occurrences: 2 across 1 file (same-file)
```

means the same block appears twice in one file.

```text
Occurrences: 3 across 3 files (cross-file)
```

means one occurrence appears in each of three files.

```text
Occurrences: 4 across 3 files (mixed)
```

means the duplicate spans multiple files and at least one of those files contains more than one occurrence.

### Source locations

Locations such as:

```text
src/models/user.py:12-15
```

always refer to the **original physical Python source**, not Arid's internal normalized representation.

This remains true even when ignored comments, imports, signatures, docstrings, or blank lines appear within the physical range.

Use:

```bash
arid . --show-source
```

to include the original source text alongside each location.

### Duplicate groups

When the same block appears more than twice, Arid reports it as one duplicate group rather than generating every possible pair.

For example, a block appearing in:

```text
a.py
b.py
c.py
```

is one finding with three occurrences, not three separate pairwise findings.

### Duplicate lines and duplication percentage

The final summary:

```text
4 duplicate lines (2.31%).
```

measures **redundant effective lines**, not every line participating in a duplicate.

One occurrence of each duplicate group is treated as canonical. Only redundant copies beyond that canonical occurrence contribute duplicate lines.

For example:

```text
10-line block × 2 occurrences
```

contributes:

```text
10 duplicate lines
```

not 20.

A 10-line block appearing three times contributes:

```text
20 duplicate lines
```

because two of the three copies are redundant.

Overlapping redundant regions are not counted repeatedly.

The duplication percentage is:

```text
duplicate effective lines
───────────────────────── × 100
 analyzed effective lines
```

This makes the metric represent how much analyzed code is redundant rather than how much code merely participates in a duplicated region.

### How to interpret findings

There is no universal rule for which duplicate should be refactored first, but Arid's metadata can help you triage a large report.

A practical review order is often:

1. Look at **larger duplicate regions** before very short ones.
2. Review `executable` / `function` findings for repeated application logic.
3. Look at findings with many occurrences to identify patterns repeated broadly through the codebase.
4. Use `same-file`, `cross-file`, and `mixed` to distinguish localized repetition from code repeated across modules.
5. Review `declarative` findings in context. Repeated declarations may be intentional, generated by a common coding pattern, or candidates for consolidation depending on the project.

Arid intentionally stops short of saying:

```text
high severity
low value
safe to ignore
must refactor
```

Those are project-specific judgments.

Its job is to provide accurate duplicate detection and enough objective structural information for the developer to make them.

---

## Configuration

Arid uses `[tool.arid]` in `pyproject.toml`:

```toml
[tool.arid]
min-lines = 4
ignore-comments = true
ignore-docstrings = true
ignore-imports = true
ignore-signatures = true
same-file = true
hidden = false
exclude = [
    "generated/**",
    "vendor/**",
]
```

Current defaults are:

| Option | Default | Meaning |
| --- | --- | --- |
| `min-lines` | `4` | Minimum effective normalized lines required for a duplicate. |
| `ignore-comments` | `true` | Ignore Python comments during matching. |
| `ignore-docstrings` | `true` | Ignore structural Python docstrings. |
| `ignore-imports` | `true` | Ignore import statements. |
| `ignore-signatures` | `true` | Ignore function and method declaration signatures. |
| `same-file` | `true` | Detect non-overlapping duplicate regions within the same file. |
| `hidden` | `false` | Include hidden files and directories during directory discovery. |
| `exclude` | `[]` | Path patterns excluded from discovery. |

Configuration precedence is:

```text
CLI arguments
    ↓
pyproject.toml
    ↓
built-in defaults
```

For example:

```toml
[tool.arid]
min-lines = 6
ignore-docstrings = true
same-file = true
hidden = false
```

can be overridden for one scan with:

```bash
arid . \
    --min-lines 10 \
    --no-ignore-docstrings \
    --no-same-file \
    --hidden
```

Each configurable boolean has both an enabling and disabling CLI form. This matters when the project configuration differs from the built-in default. For example, if the project contains:

```toml
[tool.arid]
ignore-comments = false
```

then:

```bash
arid . --ignore-comments
```

explicitly enables comment filtering for that scan.

Likewise:

```bash
arid . --no-ignore-comments
```

explicitly disables it.

Supplying one or more `--exclude` options on the command line overrides the configured `exclude` list for that scan:

```bash
arid . \
    --exclude 'build/**' \
    --exclude 'generated/**'
```

`--json` and `--show-source` control report output and are CLI-only options.

---

## Detection model

Arid is focused on **exact duplicate source blocks after configurable Python-aware normalization**.

For example, with comments and function signatures ignored:

```python
def first():
    # explanation
    value = calculate_value()
    save_value(value)
```

and:

```python
def second():
    # different explanation
    value = calculate_value()
    save_value(value)
```

can be considered duplicates.

Arid v1 does **not** normalize identifiers, so these are intentionally different:

```python
value = calculate_value()
save_value(value)
```

```python
result = calculate_value()
save_value(result)
```

Arid can attach structural context such as `declarative`, `executable`, `class`, or `function` to a duplicate that it has already detected.

That does **not** make Arid a structural clone detector. Two pieces of code that are merely structurally similar but do not become identical after normalization are not considered duplicates.

Semantic clone detection, identifier-renaming clone detection, and fuzzy AST similarity remain outside the v1 scope.

---

## Suppressing intentional duplication

Arid supports source-level suppression regions:

```python
# arid: disable

# intentionally duplicated code

# arid: enable
```

Code inside a disabled region does not participate in duplicate detection.

Suppression regions also create matching boundaries, so Arid does not construct a duplicate across disabled source.

Use suppression for duplication that is intentionally accepted by the project rather than expecting Arid to infer whether a particular framework pattern or coding convention should be ignored.

---

## Architecture

The v1 architecture is intentionally small:

```text
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
```

Arid analyzes Python source entirely in Rust and never imports or executes the project being scanned.

Duplicate detection operates on Arid's normalized source representation. Structural context is derived from Python syntax and attached as reporting metadata; it does not alter whether two normalized regions match.

---

## Installation

> [!NOTE]
> See [Project status](#project-status) for the current release stage and stability expectations.

### uv

Install Arid as an isolated command-line tool:

```bash
uv tool install arid
```

### pip

Install Arid from PyPI:

```bash
python -m pip install --pre arid
```

Verify the installation:

```bash
arid --version
```

Scan the current project:

```bash
arid .
```

---

## Exit codes

Arid uses predictable exit codes for CLI and CI usage:

| Exit code | Meaning |
| --- | --- |
| `0` | Scan completed successfully and no duplicate findings failed the scan. |
| `1` | Duplicate-code findings were reported. |
| `2` | Invocation, configuration, parsing, or internal error. |

A finding exit status is therefore distinct from an Arid execution failure.

---

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

---

## Development

Arid includes dedicated tooling and documentation for performance benchmarking and real-world release validation:

- [Benchmarks](benchmarks/README.md) — reproducible performance comparisons against Pylint `R0801` and jscpd, including corpus provisioning and benchmark execution.
- [Validation](validation/README.md) — real-world correctness and robustness validation across Black, Django, mypy, Rich, determinism checks, malformed-source handling, and filesystem edge cases.
- [Release roadmap](docs/arid-v1-release-roadmap.md) — release stages, qualification gates, and release metadata preparation with `./release.sh`.

---

## Contributing

Contributions should preserve Arid's focused scope and existing product contract. Bug fixes, compatibility improvements, tests, documentation, and performance work are welcome; scope-expanding features should be discussed before implementation.