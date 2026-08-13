# Pylint compatibility fixtures

These fixtures document the subset of Pylint `R0801` / `symilar` behavior that
Arid intends to preserve, together with intentional behavioral differences.

They are exercised by `tests/compatibility.rs`.

The suite records expected compatibility behavior rather than executing Pylint
during `cargo test`. This keeps Arid's test suite deterministic and avoids
requiring a Python runtime or a particular Pylint version to run Rust tests.

## Compatible intent

### `basic-cross-file`

Two identical four-line source regions in different files are reported as one
cross-file duplicate group.

### `ignored-comments`

Different Python comments do not prevent otherwise identical source regions
from matching when comment filtering is enabled.

### `ignored-docstrings`

Different structural Python docstrings do not prevent otherwise identical
function bodies from matching when docstring filtering is enabled.

### `ignored-imports`

Different import statements do not prevent otherwise identical retained source
from matching when import filtering is enabled.

### `ignored-signatures`

Different function signatures do not prevent otherwise identical function
bodies from matching when signature filtering is enabled.

## Intentional Arid deviations

### `hash-inside-string`

A `#` character inside a Python string is source content, not a comment.

Arid uses Python syntax information rather than textual `#` splitting, so the
string remains part of the normalized source.

### `same-file`

Arid detects non-overlapping duplicate regions within the same file by default.

This is an intentional extension beyond Pylint `R0801` behavior and can be
disabled with `--no-same-file` or `same-file = false`.