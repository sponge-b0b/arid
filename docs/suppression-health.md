# Suppression health during normal scans

Arid source suppressions have two related views:

- aggregate suppression health during a normal duplicate-code scan;
- detailed suppression lifecycle administration.

These views use the same suppression audit semantics without changing normal duplicate identity or Arid's existing machine-report schemas.

## Aggregate suppression health

Use `--suppression-summary` when you want a normal text scan plus aggregate suppression maintenance state:

```bash
arid . --suppression-summary
```

The normal duplicate findings and Summary/Breakdown/Hotspots output remain intact. Arid additionally renders:

```text
Suppressions

┌────────┬───────┐
│ Status │ Count │
├────────┼───────┤
│ Total  │     4 │
│ Active │     3 │
│ Stale  │     1 │
└────────┴───────┘
```

`--suppression-summary` is informational. Stale suppressions do not change the process exit status unless `--fail-on-stale` is also requested.

Because active/stale classification requires an unsuppressed audit pass, ordinary `arid .` does not perform that work. The additional audit runs only when suppression health or stale-suppression enforcement is requested.

`--suppression-summary` is a human text-output feature. It does not add fields to `summary-v1`, report-v4, Markdown, SARIF, or supplemental normal-scan reports.

## Fail normal scans on stale suppressions

Use `--fail-on-stale` directly on a normal scan when stale suppressions should fail CI:

```bash
arid . --fail-on-stale
```

Arid performs the normal duplicate scan and a suppression audit. Exit status is then composed from both results:

- `0` — no enforced duplicate findings and no stale suppressions;
- `1` — duplicate findings, stale suppressions, or both;
- `2` — invocation, configuration, operational, or incomplete-scan failure.

For normal text output, `--fail-on-stale` also renders the aggregate Suppressions block automatically; a separate `--suppression-summary` flag is unnecessary.

For normal JSON, Markdown, or SARIF output, `--fail-on-stale` changes only exit policy. Existing machine-output shapes remain unchanged.

`--no-fail-on-findings` and `--fail-on-stale` remain independent policies. A complete scan may therefore report duplicate findings without failing while still failing if stale suppressions exist.

## Detailed suppression administration

Use `--suppression-status` when you need the complete maintenance view rather than a normal duplicate scan:

```bash
arid . --suppression-status
```

It reports aggregate counts plus each effective suppression region and its classification:

```text
Suppression status
Files: 8
Total suppressions: 2
Active suppressions: 1
Stale suppressions: 1
active: src/a.py:10-20 (enable)
stale: src/b.py:30-EOF (eof)
```

The deterministic JSON form remains `suppression-status-v1`:

```bash
arid . --suppression-status --json
```

The existing administrative enforcement form also remains valid:

```bash
arid . --suppression-status --fail-on-stale
```

## Current audit boundaries

Normal-scan suppression auditing requires disk-backed complete source state. `--suppression-summary` and normal-scan `--fail-on-stale` therefore do not combine with `--stdin-path` or `--keep-going`.

This restriction prevents Arid from reporting suppression health against source state different from the source state used by the normal scan.
