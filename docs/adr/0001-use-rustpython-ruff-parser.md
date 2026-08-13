---
status: accepted
---

# Use the RustPython-packaged Ruff-derived Python parser

## Context

Arid must understand modern Python syntax accurately enough to identify comments, docstrings, imports, and function signatures without starting or embedding a Python interpreter.

Parser-specific details must remain isolated from the duplicate-detection engine.

## Decision

Use the RustPython-packaged Ruff-derived parser as Arid's Python frontend.

Use its AST and token stream from a single parse to derive source ranges for Python-aware normalization. Parser-specific types remain confined to the Python frontend.

## Rationale

The parser provides both syntax-aware AST nodes and lexical tokens with source ranges, which supports Arid's normalization requirements without executing Python.

Tree-sitter is better suited to permissive multi-language parsing but provides weaker guarantees for Arid's Python-specific correctness requirements. Invoking CPython would add runtime coupling and violate Arid's requirement to analyze source entirely in Rust.

Isolating the parser behind Arid-owned normalized representations prevents the parser dependency from propagating into detection, metrics, or reporting.