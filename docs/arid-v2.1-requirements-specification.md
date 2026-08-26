# **Arid v2.1 Requirements Specification**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Stable target:** `2.1.0`  
**Version scope:** This document defines backward-compatible functionality targeted for Arid **2.1.0**.

---

# **1. Purpose**

Arid 2.1 makes suppression maintenance and source discovery auditable without changing what Arid considers duplicate code.

Arid remains:

> **A fast, Python-specific duplicate-code checker written in Rust, designed to replace Pylint `R0801` and run alongside Ruff.**

The stable v1, v1.1, v1.2, and v2 requirements remain the base product contract.

This document defines only requirements added or changed for v2.1. Unless explicitly changed here, behavior specified by:

```text
docs/arid-v1-requirements-specification.md
docs/arid-v1.1-requirements-specification.md
docs/arid-v1.2-requirements-specification.md
docs/arid-v2-requirements-specification.md
```

remains in force.

Arid 2.1 addresses two related maintenance problems:

1. accepted suppression can outlive the duplicate code that justified it;
2. discovery decisions can be difficult to diagnose when a path is or is not selected for analysis.

The v2.1 theme is:

> **Auditable maintenance. Explainable discovery. Same focused detector.**

---

# **2. Product Principle**

Arid 2.1 MUST make maintenance and discovery state observable without creating alternate detector semantics.

A suppression or baseline entry is accepted maintenance state only while the condition that justifies it remains true.

Arid MUST make obsolete acceptance detectable so users can remove it rather than allow maintenance state to accumulate indefinitely.

Likewise, Arid MUST make an individual discovery decision explainable without introducing a general tracing subsystem or another discovery implementation.

V2.1 MUST preserve:

- Python-only scope
- exact duplication after documented normalization
- arbitrary contiguous source-block detection
- cross-file and same-file detection
- `DUP001`
- stable finding fingerprints
- deterministic findings, metrics, and ordering
- Python-aware normalization
- existing baseline-v1 compatibility
- existing report-v4 compatibility
- existing suppression effect during normal scans
- existing project configuration
- existing output formats
- existing serial and parallel execution semantics
- existing default exit-status semantics for ordinary scans

New v2.1 administrative features MUST reuse the same underlying parsing, normalization, detection, configuration, and discovery semantics used by normal Arid execution.

---

# **3. V2.1 Goals**

Arid 2.1 MUST add or complete:

1. a read-only suppression audit through `--suppression-status`
2. active and stale suppression-region classification
3. explicit, documented idempotent suppression-directive semantics
4. stale-state policy enforcement through `--fail-on-stale`
5. `--fail-on-stale` support for both suppression status and existing baseline status
6. targeted path discovery explanation through `--explain-path <PATH>`
7. an ignore-file traversal override through `--no-ignore-files`
8. a versioned deterministic suppression-status JSON contract
9. a versioned deterministic path-explanation JSON contract
10. semantic equivalence between supported stdout and file representations of the same administrative JSON document

These additions MUST remain compatible with the stable Arid 2.0 detector and report contracts.

---

# **4. Explicit V2.1 Non-Goals**

Arid 2.1 MUST NOT add:

- semantic clone detection
- fuzzy clone detection
- structural similarity as duplicate identity
- identifier-renaming clone detection
- another diagnostic beyond `DUP001`
- autofix or automated duplicate removal
- automated refactoring
- suppression expiration dates
- suppression owners
- suppression approval workflows
- suppression ticket metadata
- suppression rationale syntax
- nested or counted suppression levels
- a general waiver or policy database
- a new baseline schema
- automatic baseline expansion
- automatic suppression insertion
- automatic suppression removal
- a general filesystem tracing subsystem
- bulk path-explanation tracing as a replacement for `--list-files`
- a general `--no-excludes` or "scan everything" switch
- changes to hidden-file policy
- changes to Python source-extension policy
- changes to symlink policy
- changes to exact duplicate identity
- another detector implementation for suppression auditing
- a plugin framework
- a reporter framework
- persistent caching
- daemon/RPC operation
- MCP integration
- LSP/editor integration

V2.1 MUST remain an additive maintenance and discovery-auditability release.

---

# **5. Compatibility and Detection Contract**

Arid 2.1 MUST preserve the Arid 2.0 duplicate definition:

> Two or more sufficiently long contiguous Python source regions that become identical after Arid's configured preprocessing rules are applied.

Unless a separately documented correctness defect requires a compatible fix, v2.1 MUST NOT change:

- Python parsing semantics
- normalization rules
- comment handling
- docstring handling
- import handling
- signature handling
- exact normalized duplicate identity
- same-file overlap semantics
- maximal duplicate qualification
- deterministic group canonicalization
- finding fingerprints
- structural context
- structural scope
- occurrence distribution
- duplicate metrics
- baseline-v1 fingerprints
- baseline acceptance semantics
- normal suppression effect
- report-v4 meaning
- SARIF finding identity

Suppression auditing MUST evaluate the consequences of suppression around the existing detector.

It MUST NOT introduce a second definition of duplication.

Discovery explanation MUST evaluate the existing discovery policy.

It MUST NOT implement an independent approximation of discovery.

---

# **6. Suppression Directive Contract**

V2.1 MUST formally document the existing suppression directive semantics.

Recognized directives remain exactly:

```python
# arid: disable
# arid: enable
```

Arid suppression directives are state-setting operations rather than syntactically paired block delimiters.

The initial state of each Python source file is:

```text
enabled
```

The state transitions MUST be:

```text
Current state   Directive   Result
-------------   ---------   -----------------------------
enabled         disable     become disabled
disabled        disable     remain disabled; no-op
disabled        enable      become enabled
enabled         enable      remain enabled; no-op
disabled        EOF         valid suppression through EOF
enabled         EOF         normal completion
```

Suppression directives MUST therefore be idempotent.

Repeated `disable` directives while already disabled MUST be valid no-ops.

Repeated `enable` directives while already enabled MUST be valid no-ops.

A `disable` directive MUST NOT require a later `enable` directive to be valid.

Reaching EOF while disabled MUST be valid.

V2.1 MUST NOT reinterpret these valid cases as malformed suppression structure.

---

# **7. Suppression Region Boundaries**

Only effective suppression state transitions establish suppression-region boundaries.

An effective suppression region begins when:

```text
enabled
    ↓
# arid: disable
    ↓
disabled
```

and ends at either:

```text
# arid: enable
```

that transitions the state back to enabled, or:

```text
EOF
```

if no later effective `enable` occurs.

Therefore both forms are valid:

```python
# arid: disable

duplicated_code()

# arid: enable
```

and:

```python
# arid: disable

duplicated_code()

# EOF
```

A no-op directive MUST NOT:

- begin another suppression region
- terminate a suppression region
- split an existing suppression region
- create an independent suppression-status record
- be independently classified as active or stale

For example:

```python
# arid: disable

first()

# arid: disable

second()

# arid: enable

# arid: enable
```

contains one effective suppression region.

The second `disable` and second `enable` are valid no-ops.

Suppression directives MUST retain their existing physical-line semantics during normal analysis.

V2.1 MUST NOT change when a directive begins or ends suppression relative to source on the directive's own line.

---

# **8. Suppression Audit**

V2.1 MUST add the read-only administrative operation:

```text
--suppression-status
```

Example:

```bash
arid . --suppression-status
```

The operation MUST inspect effective suppression regions in the selected scan corpus.

It MUST NOT:

- modify source files
- insert directives
- remove directives
- rewrite configuration
- write or alter a baseline
- alter normal duplicate findings
- change suppression behavior for subsequent scans

The operation MUST classify each effective suppression region as exactly one of:

```text
active
stale
```

No third persistent suppression-maintenance state is required.

A project with no effective suppression regions is a valid successful audit.

---

# **9. Active and Stale Suppression Semantics**

An **active** suppression region is one whose suppression effect currently prevents source from participating in at least one otherwise reportable `DUP001` duplicate under the effective Arid analysis settings.

A **stale** suppression region is one whose suppression effect no longer prevents any otherwise reportable `DUP001` duplicate under those settings.

Conceptually:

```text
suppression-disabled audit view
            ↓
existing Arid normalization
            ↓
existing Arid detector
            ↓
otherwise-reportable DUP001 findings
            ↓
map findings to effective suppression regions
            ↓
active / stale
```

The suppression audit MUST use the same effective settings that determine duplicate identity during the corresponding normal scan, including where applicable:

- `min-lines`
- comment handling
- docstring handling
- import handling
- signature handling
- same-file behavior
- project configuration
- CLI overrides
- source corpus selection

The audit MUST NOT use fuzzy similarity or a different duplicate threshold solely for suppression analysis.

A region MUST be considered active when removing its suppression effect reveals duplicate participation attributable to that region, including when the suppression region acts as a boundary preventing an otherwise valid duplicate occurrence.

Suppression regions MUST be evaluated against an audit view in which effective suppressions do not hide candidate duplicate source from the existing detector.

This is necessary so multiple suppressed regions that duplicate one another can still be recognized as active.

The technical architecture MUST define the exact deterministic mapping from audit findings to suppression regions without changing duplicate identity.

---

# **10. Suppression Audit and Existing Filtering**

Suppression maintenance status describes the underlying duplicate condition, not merely final report visibility.

Therefore baseline acceptance MUST NOT cause an otherwise active suppression to be classified as stale.

Likewise, reporting-only filtering MUST NOT redefine suppression activity.

Conceptually, suppression activity is determined before acceptance or presentation filters such as:

```text
baseline enforcement
focus filtering
output rendering
```

A duplicate may therefore be accepted by a baseline or omitted by a reporting filter and still prove that a suppression region is active.

This preserves the distinction between:

```text
duplicate identity
suppression maintenance
baseline acceptance
report presentation
```

These concerns MUST remain separate.

---

# **11. Suppression Status Output**

`--suppression-status` MUST support:

```text
text
json
```

Markdown and SARIF output are not required for this administrative operation.

Human-readable text MUST make at least the following clear:

- audited corpus context
- each effective suppression region
- source path
- source location sufficient to find the region
- active or stale status
- active suppression count
- stale suppression count

EOF-terminated suppression MUST be representable clearly without treating EOF termination as an error.

No-op directives MUST NOT appear as independent suppression regions.

Human-readable wording is not a stable machine parsing contract.

Machine consumers MUST use the versioned JSON contract defined by this specification.

Output ordering MUST be deterministic.

---

# **12. Generic Stale-Policy Enforcement**

V2.1 MUST add:

```text
--fail-on-stale
```

`--fail-on-stale` is an administrative policy control.

It MUST be valid with:

```text
--suppression-status
--baseline-status <PATH>
```

It MUST NOT silently alter ordinary duplicate-scan exit behavior.

Using `--fail-on-stale` without an administrative operation that defines stale state MUST be invalid CLI usage.

`--fail-on-stale` MUST NOT modify the audited source, suppression directives, or baseline.

Its only purpose is to make already-determined stale maintenance state enforceable through the process exit status.

---

# **13. Suppression-Status Exit Semantics**

Without `--fail-on-stale`:

```text
0  suppression audit completed successfully
2  usage, configuration, discovery, source-processing,
   output, or internal error
```

The presence of active or stale suppression regions alone MUST NOT produce exit status `1`.

Therefore:

```text
no suppressions          → 0
active only              → 0
stale only               → 0
active + stale           → 0
Arid failure             → 2
```

With:

```text
--suppression-status --fail-on-stale
```

the exit status MUST be:

```text
0  suppression audit completed and no stale regions exist
1  suppression audit completed and one or more stale regions exist
2  usage, configuration, discovery, source-processing,
   output, or internal error
```

Active suppression is accepted maintenance state and MUST NOT itself cause exit status `1`.

---

# **14. Baseline-Status Stale Enforcement**

The existing operation remains:

```text
--baseline-status <PATH>
```

Its existing classification semantics remain unchanged:

- accepted/current baseline debt
- active/new debt not fully accepted
- stale baseline acceptance

Its existing default exit-status behavior MUST remain backward compatible.

Without `--fail-on-stale`:

```text
0  status completed and no active/new debt exists
1  status completed and active/new debt exists
2  baseline or scan error
```

Stale baseline acceptance alone remains informational under the default v2 behavior.

With:

```text
--baseline-status <PATH> --fail-on-stale
```

the exit status MUST be:

```text
0  status completed with no active/new debt and no stale acceptance
1  status completed with active/new debt, stale acceptance, or both
2  baseline or scan error
```

In matrix form:

```text
Active/new debt   Stale debt   --fail-on-stale   Exit
---------------   ----------   ---------------   ----
no                no           no                0
no                yes          no                0
yes               no           no                1
yes               yes          no                1

no                no           yes               0
no                yes          yes               1
yes               no           yes               1
yes               yes          yes               1
```

V2.1 MUST NOT change baseline-v1 serialization or baseline identity semantics.

`--fail-on-stale` MUST NOT prune the baseline.

Existing:

```text
--prune-baseline <PATH>
```

remains the explicit operation for removing stale baseline acceptance.

This produces the intended maintenance ratchet:

```text
remediation
    ↓
baseline acceptance becomes stale
    ↓
--fail-on-stale detects maintenance debt
    ↓
explicit prune
    ↓
baseline shrinks
```

V2.1 MUST NOT provide any automatic mechanism that grows accepted baseline debt.

---

# **15. Targeted Discovery Explanation**

V2.1 MUST add:

```text
--explain-path <PATH>
```

Example:

```bash
arid . --explain-path src/generated/example.py
```

This is a read-only administrative diagnostic answering:

> **What would Arid do with this path under the effective discovery policy, and why?**

`--explain-path` MUST resolve the same:

- project root
- project configuration
- CLI overrides
- scan roots
- hidden-path policy
- ignore-file policy
- Arid `exclude` rules
- Python source-type rules
- symlink rules

used by normal discovery.

It MUST NOT parse Python source or run duplicate detection merely to explain discovery.

It MUST NOT mutate any project state.

---

# **16. Path Explanation Scope**

`--explain-path` is targeted explanation.

It MUST NOT become an alias for:

```text
--list-files
```

and MUST NOT emit a trace for every path encountered during a filesystem walk.

The operation MUST explain only the requested target path.

The supplied target MAY be a file or directory.

For a file, the result MUST communicate whether the file would be selected for source preparation under the effective invocation and why.

For a directory, the result MUST communicate whether normal discovery would traverse that directory under the effective invocation and why.

Explaining a directory MUST NOT implicitly produce explanations for all descendants.

A missing target MUST fail clearly rather than silently inventing a discovery result.

The technical architecture MUST define deterministic path normalization and path-identity rules consistent with the existing project-root contract.

---

# **17. Path Explanation Reasons**

Path explanation MUST provide a stable machine-readable decision and reason representation.

At minimum, the explanation model MUST be able to distinguish decisions arising from:

- path eligibility under the supplied scan roots
- Arid configured or CLI `exclude` rules
- normal ignore-file traversal rules
- hidden-path policy
- supported Python `.py` / `.pyi` source selection
- symlink policy
- explicit-file behavior
- successful inclusion or traversal eligibility

The exact stable reason vocabulary MUST be frozen in the technical architecture before implementation of `path-explanation-v1` is considered complete.

A path affected by more than one policy MUST produce a deterministic explanation.

The implementation MUST NOT depend on parsing human-readable diagnostics to derive the JSON reason.

Human-readable text MAY present a concise primary explanation while machine-readable output retains the stable information required by the schema.

---

# **18. Explicit-File Discovery Semantics**

V2.1 MUST preserve the established distinction between directory discovery and explicit file selection.

An explicit Python file supplied directly to Arid continues to bypass normal ignore-file traversal filtering because directly naming the file is an explicit request to analyze it.

Configured Arid `exclude` rules continue to apply.

`--explain-path` MUST represent this distinction accurately rather than reporting that a directly supplied file would necessarily be skipped merely because the same file would be ignored during directory traversal.

The path-explanation model MUST therefore have enough invocation context to explain the decision Arid would actually make.

---

# **19. Ignore-File Traversal Override**

V2.1 MUST add:

```text
--no-ignore-files
```

Example:

```bash
arid . --no-ignore-files
```

This option disables ignore-file-derived filtering used during directory traversal.

It MUST NOT disable unrelated Arid discovery policy.

Specifically, `--no-ignore-files` MUST preserve:

- `[tool.arid].exclude`
- CLI `--exclude`
- effective hidden-file policy
- Python-only `.py` / `.pyi` source selection
- existing symlink policy
- explicit project-root semantics
- deterministic discovery ordering

Conceptually:

```text
filesystem candidate
        ↓
ignore-file traversal policy     ← bypassed by --no-ignore-files
        ↓
Arid exclude policy              ← preserved
        ↓
hidden-path policy               ← preserved
        ↓
Python source selection          ← preserved
        ↓
symlink policy                   ← preserved
        ↓
selected source
```

The option MUST NOT mean:

```text
ignore every exclusion mechanism
```

and MUST NOT evolve into a general `--no-excludes` switch.

---

# **20. Ignore-File Scope**

`--no-ignore-files` applies only where normal Arid behavior would otherwise consult ignore-file-derived traversal rules.

Explicit file arguments already bypass those traversal rules and MUST retain their existing semantics.

The option MUST compose consistently with every v2.1 operation that performs normal directory discovery, including where applicable:

```text
normal duplicate scans
--list-files
--explain-path
--suppression-status
--baseline-status
--prune-baseline
```

The same effective discovery policy MUST be used by all of these operations.

An administrative mode MUST NOT silently discover a different corpus from an equivalent normal scan.

The exact set of ignore-file sources disabled by the option MUST correspond to the normal ignore-file-derived rules actually honored by Arid's directory traversal implementation.

The technical architecture MUST document those underlying traversal controls explicitly.

`--no-ignore-files` MUST remain invocation-specific in v2.1.

It MUST NOT add a `[tool.arid]` configuration key.

---

# **21. Suppression Status JSON Contract**

V2.1 MUST publish:

```text
schemas/suppression-status-v1.schema.json
```

The schema MUST use JSON Schema Draft 2020-12, consistent with Arid's existing published JSON schemas.

A suppression-status-v1 document MUST identify at minimum:

```text
schema_version
tool_version
suppression regions
region source identity/location
region status
active count
stale count
```

The initial schema version MUST be:

```json
"schema_version": 1
```

Each region MUST identify enough physical source information for a machine consumer to locate the effective suppression region.

Each effective region status MUST be one of:

```text
active
stale
```

No-op directives MUST NOT create independent region records.

EOF-terminated regions MUST be representable as valid regions.

The schema MUST NOT encode EOF termination as malformed state.

The precise JSON object shape, field names beyond those required here, and canonical location representation MUST be frozen in the technical architecture before implementation is complete.

Once `suppression-status-v1` is released as stable, incompatible changes MUST require a new schema version rather than silently redefining v1.

---

# **22. Path Explanation JSON Contract**

V2.1 MUST publish:

```text
schemas/path-explanation-v1.schema.json
```

The schema MUST use JSON Schema Draft 2020-12.

A path-explanation-v1 document MUST identify at minimum:

```text
schema_version
tool_version
target path
target kind where determinable
final discovery decision
stable machine-readable explanation reason or reasons
```

The initial schema version MUST be:

```json
"schema_version": 1
```

The contract MUST be sufficient for automation to determine why the supplied path would or would not participate in discovery without parsing human-readable prose.

The schema MUST distinguish Arid exclusion from ignore-file-derived exclusion because:

```text
--no-ignore-files
```

changes only the latter.

The schema MUST also represent explicit-file behavior correctly.

The precise reason vocabulary and serialized object shape MUST be frozen in the technical architecture before implementation is complete.

Once `path-explanation-v1` is released as stable, incompatible changes MUST require a new schema version rather than silently redefining v1.

---

# **23. Deterministic Administrative JSON**

Both:

```text
suppression-status-v1
path-explanation-v1
```

are public machine-readable contracts.

They MUST NOT be incidental serialization of internal Rust structs.

For identical:

- Arid version
- source state
- configuration
- CLI options
- project root
- scan roots

the logical administrative output MUST be deterministic.

Determinism MUST include:

- deterministic record ordering
- deterministic path representation
- deterministic status classification
- deterministic reason ordering where multiple reasons are represented
- deterministic summary counts
- absence of volatile timestamps
- absence of process identifiers
- absence of temporary-directory identity
- absence of host-specific values unrelated to the result
- independence from hash-map iteration
- independence from worker scheduling
- independence from filesystem traversal arrival order

Execution strategy MUST NOT leak into the public administrative contract.

---

# **24. Stdout and File Semantic Equivalence**

When the same administrative representation can be emitted to stdout or written to a file through Arid's supported output routing, both destinations MUST represent the same logical document.

For the same administrative operation and format:

```text
stdout representation
        ≡
file representation
```

Semantic equivalence requires the same:

- schema version
- tool version
- records
- classifications
- paths
- reasons
- counts
- ordering semantics

Presentation-only differences such as a trailing newline MUST NOT create a semantic difference.

Arid MUST NOT maintain separate "short" and "full" JSON models for stdout and file output.

Both destinations MUST render from the same logical administrative result.

The technical architecture MUST select the smallest output-routing mechanism that satisfies this requirement without creating an unnecessary new reporter framework.

---

# **25. Administrative Output Compatibility**

The existing normal report domain remains separate from the new administrative domains.

Conceptually:

```text
report-v4
    duplicate scan results

suppression-status-v1
    suppression maintenance state

path-explanation-v1
    discovery decision explanation
```

V2.1 MUST NOT overload report-v4 with suppression-status or path-explanation records.

Likewise, suppression-status-v1 and path-explanation-v1 MUST NOT pretend to be duplicate scan reports.

Text output is intended for humans.

Versioned JSON output is the stable machine contract.

Markdown and SARIF are not required for the new administrative domains.

---

# **26. CLI Composition and Validation**

The new administrative modes MUST compose with project/configuration selection where meaningful:

```text
--config <PATH>
--no-config
--project-root <PATH>
```

They MUST honor applicable normalization and discovery CLI overrides required to evaluate their result correctly.

`--suppression-status` MUST conflict with administrative operations that represent a different terminal operation, including at minimum:

```text
--show-config
--list-files
--baseline-status
--prune-baseline
--write-baseline
--explain-path
```

`--explain-path` MUST likewise behave as a terminal administrative operation rather than initiating a duplicate scan.

`--fail-on-stale` MUST require one of:

```text
--suppression-status
--baseline-status
```

Invalid combinations MUST fail as CLI usage errors rather than being silently ignored.

`--no-ignore-files` is not itself an administrative mode and MAY be used with normal scans or compatible administrative operations.

The technical architecture MUST enumerate the final `clap` conflict/requirement relationships before implementation.

---

# **27. Configuration Compatibility**

`[tool.arid]` remains Arid's project configuration namespace.

Existing valid 2.0 configuration MUST remain valid in 2.1.

Configuration precedence remains:

```text
CLI arguments
    ↓
pyproject.toml
    ↓
built-in defaults
```

V2.1 MUST NOT rename existing configuration keys.

The following new controls remain invocation-specific:

```text
--suppression-status
--fail-on-stale
--explain-path
--no-ignore-files
```

V2.1 MUST NOT add project configuration merely to mirror every new CLI option.

In particular:

```text
--fail-on-stale
```

is CI/invocation policy rather than persistent duplicate-detection policy.

---

# **28. Error and Completeness Semantics**

The new administrative operations MUST preserve Arid's established distinction between policy results and operational failure.

Exit status:

```text
1
```

MUST mean an explicitly defined successful policy result.

Exit status:

```text
2
```

MUST remain reserved for usage, configuration, discovery, source-processing, baseline, output, or internal failure as applicable.

A failed or incomplete suppression audit MUST NOT return exit `0` or `1` as though its stale classification were complete.

A failed path explanation MUST NOT fabricate an included or excluded result.

Machine-readable administrative output MUST never silently represent incomplete information as a complete successful audit.

The technical architecture MUST define how existing `error-v1` behavior composes with the new JSON administrative modes without creating another unrelated operational-error taxonomy.

---

# **29. Performance and Implementation Constraints**

V2.1 MUST preserve serial execution as the default.

Existing numeric and automatic worker behavior MUST remain unchanged.

Normal scans with no new v2.1 options MUST NOT incur a material suppression-audit or path-explanation cost.

In particular:

```bash
arid .
```

MUST NOT perform a shadow suppression audit merely because v2.1 supports one.

Suppression auditing MAY require additional analysis because its purpose is to determine what would be reported without effective suppression.

That additional work MUST occur only when the administrative operation requires it.

The audit MUST reuse the existing parsing, normalization, corpus, and detector concepts where practical.

It MUST NOT create an independently maintained duplicate detector.

`--explain-path` SHOULD avoid parsing Python source and MUST avoid duplicate detection because neither is necessary to answer a discovery question.

`--no-ignore-files` MUST alter traversal policy only and MUST NOT introduce a second discovery implementation.

V2.1 MUST preserve deterministic behavior across worker modes.

Meaningful unexplained performance regression in ordinary scans MUST be investigated before release.

---

# **30. Validation Requirements**

V2.1 validation MUST cover the new maintenance and discovery contracts and prove that existing detector behavior remains unchanged.

At minimum targeted validation MUST verify:

```text
suppression directives remain recognized exactly
disable transitions enabled → disabled
repeated disable while disabled is a no-op
enable transitions disabled → enabled
repeated enable while enabled is a no-op
disable through EOF is valid
EOF termination is not reported as malformed
no-op directives do not create audit regions
effective disable/enable creates one suppression region
effective disable/EOF creates one suppression region

--suppression-status is read-only
active suppression classification
stale suppression classification
multiple suppressed duplicate regions classify correctly
suppression audit uses existing min-lines semantics
suppression audit uses existing normalization semantics
suppression audit respects same-file setting
baseline acceptance does not incorrectly make suppression stale
focus/report filtering does not redefine suppression activity
suppression-status ordering is deterministic
suppression-status text output is deterministic
suppression-status-v1 validates against its JSON Schema
suppression-status-v1 output is deterministic
EOF-terminated suppression serializes correctly
no-op directives do not serialize as independent regions

--suppression-status without --fail-on-stale exits 0 for active-only state
--suppression-status without --fail-on-stale exits 0 for stale state
--suppression-status --fail-on-stale exits 0 with no stale regions
--suppression-status --fail-on-stale exits 1 with stale regions
suppression audit operational failure exits 2

existing --baseline-status default exit behavior remains unchanged
--baseline-status --fail-on-stale exits 0 for clean status
--baseline-status --fail-on-stale exits 1 for stale-only status
--baseline-status --fail-on-stale exits 1 for active/new-only status
--baseline-status --fail-on-stale exits 1 for combined active/new and stale status
baseline-status operational failure exits 2
--fail-on-stale alone is rejected
--fail-on-stale with an unrelated normal scan is rejected

--explain-path is read-only
--explain-path does not parse source
--explain-path does not run duplicate detection
included Python file explanation
Arid-excluded file explanation
ignore-file-excluded path explanation
hidden-path explanation
unsupported-extension explanation
symlink-policy explanation
explicit-file behavior explanation
directory traversal explanation
missing target fails clearly
path explanation ordering is deterministic
path-explanation-v1 validates against its JSON Schema
path-explanation-v1 output is deterministic

--no-ignore-files bypasses normal ignore-file-derived traversal filtering
--no-ignore-files does not bypass Arid exclude patterns
--no-ignore-files does not enable hidden files by itself
--no-ignore-files does not change .py/.pyi filtering
--no-ignore-files does not change symlink policy
--no-ignore-files does not change explicit-file semantics
--no-ignore-files composes consistently with --list-files
--no-ignore-files composes consistently with --explain-path
--no-ignore-files composes consistently with --suppression-status
--no-ignore-files composes consistently with --baseline-status

administrative JSON stdout/file semantic equivalence
normal report-v4 remains unchanged
baseline-v1 remains unchanged
ordinary suppression behavior remains unchanged
ordinary scan findings remain unchanged
serial/numeric/auto finding equivalence remains unchanged
```

Tests MUST include paths with:

- spaces
- Unicode
- nested ignored directories
- nested Arid excludes
- hidden directories
- `.py`
- `.pyi`

where relevant.

Representative generated JSON documents MUST validate against the published schemas using pinned validation tooling.

The established real-world validation campaign remains required before stable release.

---

# **31. Documentation Requirements**

Before stable 2.1.0, user-facing documentation MUST explain:

1. `--suppression-status`
2. active versus stale suppression
3. `--fail-on-stale`
4. use of `--fail-on-stale` with baseline status
5. valid suppression through EOF
6. idempotent repeated suppression directives
7. `--explain-path <PATH>`
8. `--no-ignore-files`
9. the distinction between ignore-file rules and Arid `exclude`
10. `suppression-status-v1`
11. `path-explanation-v1`
12. administrative exit-status semantics

Documentation MUST make clear that:

```python
# arid: disable
```

does not require a matching:

```python
# arid: enable
```

before EOF.

Documentation MUST also make clear that repeated state-setting directives are valid no-ops.

The README SHOULD include concise examples for the new common CLI workflows without becoming a complete duplication of the reference documentation.

V2.1 does not require a migration guide because the release is additive and existing valid 2.0 usage requires no migration.

Curated release notes MUST explicitly state that normal duplicate-detection semantics remain unchanged.

---

# **32. Release and Qualification Requirements**

Before stable `2.1.0`:

- all committed v2.1 requirements MUST be implemented
- source formatting, tests, and lint gates MUST pass
- targeted v2.1 integration validation MUST pass
- suppression directive state-machine validation MUST pass
- suppression active/stale validation MUST pass
- suppression-status-v1 schema validation MUST pass
- path-explanation-v1 schema validation MUST pass
- `--fail-on-stale` suppression-status validation MUST pass
- `--fail-on-stale` baseline-status validation MUST pass
- `--explain-path` discovery-policy validation MUST pass
- `--no-ignore-files` traversal validation MUST pass
- stdout/file administrative semantic-equivalence validation MUST pass where supported
- report-v4 compatibility validation MUST pass
- baseline-v1 compatibility validation MUST pass
- ordinary suppression-behavior regression validation MUST pass
- full real-world validation MUST pass for the release candidate
- full release benchmark qualification MUST pass for the release candidate
- README/reference documentation MUST cover normal-user v2.1 additions
- curated RC release notes MUST exist
- curated stable `docs/releases/v2.1.0.md` MUST be substantively complete before final RC qualification
- the latest release candidate MUST receive a complete qualification PASS
- stable promotion MUST remain metadata-only from the qualified RC
- published stable artifacts MUST pass the established post-publication qualification path

Qualification MUST exercise the published artifact for the new administrative machine contracts rather than relying solely on source-checkout execution.

---

# **33. Backward-Compatibility Boundary**

Arid 2.1 is additive.

The following MUST remain compatible with 2.0:

```text
exact duplicate semantics
DUP001
normal CLI invocation
existing CLI option names
[tool.arid] configuration
report schema v4
error schema v1
baseline schema v1
existing baseline files
finding fingerprint identity
normalization behavior
normal suppression behavior
text/Markdown/SARIF scan semantics
serial default
numeric workers
--workers auto
default ordinary-scan exit statuses 0 / 1 / 2
pre-commit integration
official GitHub Actions integration
published platform set
```

The following are additive:

```text
--suppression-status
--fail-on-stale
--explain-path <PATH>
--no-ignore-files
suppression-status-v1
path-explanation-v1
```

Formal documentation that suppression directives are idempotent and may remain disabled through EOF describes and freezes valid suppression behavior; it MUST NOT intentionally break existing normal scan behavior.

No other incompatibility should be introduced in v2.1.

---

# **34. V2.1 Completion Criterion**

Arid 2.1 is complete when the following statement is true:

> **Arid still detects the same exact normalized Python duplicates as 2.0, but can audit whether source suppressions remain necessary, fail CI when suppression or baseline maintenance state becomes stale, explain why an individual path is or is not selected by discovery, deliberately bypass ignore-file traversal without bypassing Arid's own discovery policy, and expose those administrative decisions through deterministic versioned machine contracts.**

The intended maintenance invariant is:

```text
intentional duplication
    ↓
active suppression is acceptable

duplication removed
    ↓
suppression becomes stale
    ↓
maintenance check fails when requested
    ↓
obsolete suppression is removed
```

and, for migration baselines:

```text
historical duplicate debt
    ↓
baseline acceptance

debt removed
    ↓
baseline entry becomes stale
    ↓
maintenance check fails when requested
    ↓
baseline is explicitly pruned
```

V2.1 MUST strengthen maintenance and discovery auditability without broadening Arid's detector.
