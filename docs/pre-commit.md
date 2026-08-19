# Arid pre-commit integration

Arid provides an official pre-commit hook that performs a whole-project duplicate-code scan.

## Requirements

The hook requires:

- Arid installed and available as `arid` on `PATH`
- pre-commit 4.4.0 or newer

Verify Arid before installing the hook:

```bash
arid --version
```

The hook deliberately uses pre-commit's `unsupported` language so pre-commit runs the Arid executable selected by the developer or CI environment. It does not install Arid, build Arid from Rust source, or introduce another package-management layer.

## Configuration

Add Arid to `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/sponge-b0b/arid
    rev: v1.2.0
    hooks:
      - id: arid
```

Then install the Git hook:

```bash
pre-commit install
```

The official hook is equivalent to:

```bash
arid .
```

with staged filenames disabled. Duplicate detection is repository-global: a changed file can duplicate unchanged source elsewhere in the project, so scanning only staged filenames would be incorrect.

The hook also uses `always_run: true`, ensuring the whole-project check remains eligible even when the staged files themselves would not otherwise select the hook.

## Project configuration and baselines

The hook uses normal Arid configuration from `[tool.arid]` in `pyproject.toml`.

For example, baseline enforcement works without any hook-specific arguments:

```toml
[tool.arid]
baseline = "arid-baseline.json"
```

With that configuration, the hook runs the same effective command:

```bash
arid .
```

and Arid applies the configured baseline normally.

## Exit behavior

Arid keeps its normal exit-status contract when invoked by the hook:

| Exit code | Meaning |
| --- | --- |
| `0` | Scan completed successfully with no active duplicate findings. |
| `1` | Active duplicate findings were reported. |
| `2` | Invocation, configuration, parsing, I/O, baseline, or other scan error. |

pre-commit treats any nonzero hook exit as a failed hook. Arid's output distinguishes duplicate findings from execution errors.
