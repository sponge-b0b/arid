<p align="center">
  <img src="assets/arid-logo.png" alt="Arid banner" width="100%" />
</p>

<p align="center">
  <a href="#project-status"><img alt="Status: WIP" src="https://img.shields.io/badge/status-WIP%20%2F%20under%20active%20development-orange"></a>
  <a href="#what-is-arid"><img alt="Scope: Python-only" src="https://img.shields.io/badge/scope-Python--only-3776AB"></a>
  <a href="#what-is-arid"><img alt="Purpose: Duplicate-code detection" src="https://img.shields.io/badge/purpose-duplicate--code%20detection-D79A3B"></a>
  <a href="#planned-configuration"><img alt="Config: tool.arid" src="https://img.shields.io/badge/config-tool.arid-4B5563"></a>
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
  <a href="#planned-usage">Usage</a> ·
  <a href="#planned-configuration">Configuration</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#license">License</a>
</p>

---

## Project status

> [!IMPORTANT]
> **Arid is a work in progress.** The project is under active development and is not yet production-ready. Interfaces, behavior, defaults, and packaging details may change as the implementation matures.

Arid is currently being built as a **small, focused CLI** for one job:

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

Arid v1 is being designed to:

- detect duplicated Python source blocks across files
- detect duplicated blocks within the same file
- ignore comments, docstrings, imports, and function signatures when configured
- preserve accurate original source locations
- report concise `DUP001` diagnostics
- provide duplication metrics
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
- fuzzy AST similarity
- multi-language duplicate detection

If a feature belongs naturally in Ruff, it does not belong in Arid.

---

## Planned usage

Arid is intended to fit naturally into a Python quality workflow:

```bash
ruff check .
arid .
```

Example diagnostic:

```text
DUP001 8 duplicated lines

  src/foo.py:21-28
  src/bar.py:54-61

Found 1 duplicate group.
8 duplicate lines (1.7%).
```

---

## Planned configuration

Arid will use `pyproject.toml`:

```toml
[tool.arid]
min-lines = 4
ignore-comments = true
ignore-docstrings = true
ignore-imports = true
ignore-signatures = true
same-file = true
```

Command-line options will override project configuration.

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

Structural and semantic clone detection are outside the v1 scope.

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

---

## Installation

> [!WARNING]
> Arid is currently in early alpha. The CLI, configuration, and output format may change before 1.0.

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

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

---

## Contributing

Arid is in early development. Contribution guidelines will be added once the initial architecture and v1 behavior are established.
