# Arid Release Roadmap

**Product:** Arid  
**Stable v1 target:** `1.0.0`
<!-- release-phase:start -->
**Current phase:** Stable
<!-- release-phase:end -->

## Purpose

This document defines how Arid progresses from prerelease development to the stable `1.0.0` release.

The v1 requirements specification defines **what Arid v1 must do**. The v1 technical architecture defines **how that product is implemented**. This roadmap defines **when that implementation is ready to advance through alpha, beta, release candidate, and stable release stages**.

In this roadmap:

> **v1 is the product functionality targeted for Arid's stable `1.0.0` release.**

Prerelease versions may implement some or all of the v1 contract before `1.0.0` is released.

---

## Release Metadata

Release metadata is prepared from the repository root with:

    ./release.sh <version>

Use:

    ./release.sh --check

to verify that release metadata agrees with the current `Cargo.toml` version, or:

    ./release.sh --dry-run <version>

to validate the current state and preview the metadata for another release.

`release.sh` manages release metadata only. It does not commit, tag, push, or publish a release.

---

## Release Principles

### 1. V1 feature scope is frozen

Once the v1 requirements are code complete, prerelease work SHOULD focus on:

* correctness fixes
* compatibility fixes
* portability fixes
* packaging and installation fixes
* performance regressions
* documentation corrections
* release-process hardening

New functionality SHOULD NOT be added merely because time remains before `1.0.0`.

Features explicitly deferred beyond v1 remain post-1.0 candidates unless they become necessary to satisfy an existing v1 requirement.

### 2. Each stage has an explicit confidence level

The release stages mean:

**Alpha**  
The v1 implementation exists, but correctness, compatibility, packaging, or release behavior may still require stabilization.

**Beta**  
The v1 feature set is frozen and all known v1 blockers are resolved. The focus is real-world validation and bug fixing.

**Release Candidate**  
The build is believed ready to become `1.0.0` unchanged. No planned code changes remain.

**Stable**  
The release candidate has completed validation without unresolved release-blocking defects and is published as `1.0.0`.

### 3. Additional prereleases are defect-driven

Arid does not need a predetermined number of alpha, beta, or release-candidate versions.

Additional prereleases SHOULD be cut only when fixes justify them.

For example:

```text
0.1.0a3
    ↓
0.1.0b1
    ↓
1.0.0rc1
    ↓
1.0.0
```

is sufficient if each stage passes its release gate.

If defects are found:

```text
0.1.0a3
    ↓
0.1.0a4
    ↓
0.1.0b1
    ↓
0.1.0b2
    ↓
1.0.0rc1
    ↓
1.0.0rc2
    ↓
1.0.0
```

is equally valid.

---

# 1. Code-Complete Alpha

## Target

PyPI:

```text
0.1.0a3
```

GitHub release:

```text
v0.1.0-alpha.3
```

## Purpose

`0.1.0a3` is the first alpha intended to represent the complete v1 implementation.

At this point:

* all v1 MUST functionality is implemented
* the v1 architecture invariants are satisfied
* required correctness tests exist
* deterministic behavior is verified
* required benchmark infrastructure exists
* the product enters stabilization rather than feature development

This milestone means:

> **Arid v1 is code complete, but `1.0.0` has not yet been declared release ready.**

## Alpha Gate

Before publishing the code-complete alpha:

1. Run the complete Rust test suite.

   ```bash
   cargo test
   ```

2. Run Clippy with warnings denied.

   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

3. Verify formatting and repository cleanliness.

   ```bash
   cargo fmt --check
   git diff --check
   git status --short
   ```

4. Run Arid-only regression benchmarks against the established benchmark corpora.

   ```bash
    benchmarks/run.sh <global-root> \
        --tools arid \
        --no-worker-scaling
   ```

   Compare current Arid performance with the most recent established baseline and investigate any substantial unexplained regression.

5. A full Arid/Pylint/jscpd comparison is not required for every alpha release. Re-run the full cross-tool benchmark suite when:

   * performance-sensitive implementation changes have landed
   * entering beta
   * entering release candidate
   * establishing or changing a public performance claim

   Existing reproducible cross-tool results remain valid evidence between those gates when the comparison methodology and relevant implementation behavior have not materially changed.

6. Build all intended PyPI and GitHub release artifacts.

7. Install and test the generated artifacts rather than relying only on execution from the source checkout.

8. Smoke-test at minimum:

   ```text
   arid .
   arid --help
   arid --version
   arid . --json
   arid . --show-source
   ```

9. Verify process outcomes:

   ```text
   0  successful scan with no findings
   1  duplicate findings
   2  invocation, configuration, parsing, or internal error
   ```

10. Verify deterministic output across:

    ```text
    --workers 1
    --workers 2
    --workers 4
    --workers 8
    ```

No new v1 feature work SHOULD begin after this gate unless the work is necessary to correct a failure of the existing v1 contract.

---

# 2. Alpha Stabilization

## Purpose

The code-complete alpha is exercised against real Python repositories before the beta feature freeze is declared.

This phase is primarily for discovering defects that unit and integration tests did not expose.

## Validation

Arid SHOULD be exercised against:

* repositories of different sizes
* `.py` and `.pyi` source
* repositories with extensive `.gitignore` rules
* hidden files and directories
* Unicode source
* multiline and unusual Python syntax
* malformed or unsupported Python source
* same-file duplication
* cross-file duplication
* large repetitive files
* projects with `[tool.arid]` configuration
* projects without Arid configuration
* serial and parallel worker counts

Representative benchmark corpora SHOULD continue to include the established Arid benchmark repositories.

For suspicious or unexpected findings:

1. inspect the normalized behavior
2. compare against the v1 duplicate definition
3. compare against expected Pylint behavior when compatibility is relevant
4. classify the result as:
   * expected Arid behavior
   * intentional compatibility improvement
   * Arid defect

## Alpha Exit Criteria

Arid may advance to beta when:

* no known v1 correctness blocker remains
* no known crash or silent partial-scan defect remains
* deterministic behavior remains verified
* source locations remain correct
* configuration and CLI behavior remain stable
* release artifacts install and execute successfully
* benchmark results remain within the intended performance envelope
* no unresolved issue requires changing the planned v1 feature set

If an alpha-blocking defect is found, fix it and publish another alpha.

Do not add another alpha merely to increase the prerelease count.

---

# 3. Beta

## Initial Target

PyPI:

```text
0.1.0b1
```

GitHub release:

```text
v0.1.0-beta.1
```

## Meaning

Beta marks the formal v1 feature freeze.

At beta:

> **The intended `1.0.0` functionality is complete and no planned feature development remains.**

From `b1` onward, accepted changes SHOULD be limited to:

* correctness fixes
* regression fixes
* compatibility fixes
* platform and packaging fixes
* performance regressions
* documentation corrections
* release-process fixes
* changes required to satisfy an already-defined v1 requirement

Changes that expand product scope SHOULD normally be deferred until after `1.0.0`.

## Stability Expectations

The following interfaces SHOULD be treated as effectively frozen during beta:

* CLI command shape
* configuration keys and precedence
* exit-code semantics
* `DUP001` meaning
* human diagnostic semantics
* JSON schema v3
* duplicate-definition semantics
* suppression semantics
* metrics interpretation

An incompatible change during beta requires explicit justification.

An incompatible JSON change MUST increment the JSON schema version as required by the v1 specification.

## Beta Validation

Beta SHOULD be the first release actively presented to external developers as a substantially complete preview of Arid 1.0.

Validation SHOULD emphasize:

* real repositories not used during implementation
* installation from PyPI
* standalone GitHub binaries
* Linux, macOS, and Windows release artifacts where supported
* clean-environment installation
* CI usage
* configuration precedence
* deterministic structured output
* malformed-input diagnostics
* large-repository behavior
* compatibility expectations documented by the Pylint fixture suite

## Beta Exit Criteria

Arid may advance to release candidate when:

* no known release-blocking correctness defect exists
* no known data-dependent crash exists
* no known silent scan corruption exists
* no known deterministic-output defect exists
* no known source-location defect exists
* no known packaging blocker exists for required distribution channels
* the complete test and lint suite is green
* release artifacts install successfully
* required benchmarks remain reproducible
* documented performance claims remain supported by benchmark results
* README and installation instructions match actual released behavior
* no planned v1 code change remains

Additional beta releases SHOULD be published only when fixes require them.

---

# 4. Release Candidate

## Initial Target

PyPI:

```text
1.0.0rc1
```

GitHub release:

```text
v1.0.0-rc.1
```

## Meaning

A release candidate is not another development milestone.

It means:

> **This build is believed ready to ship as `1.0.0` without code changes.**

The only expected difference between the final successful release candidate and `1.0.0` is release versioning and associated release metadata.

No feature work is permitted during the release-candidate phase.

## Release-Candidate Validation

RC validation SHOULD begin from a clean repository checkout and exercise the complete release process.

At minimum:

1. Run:

   ```bash
   cargo fmt --check
   cargo test
   cargo clippy --all-targets --all-features -- -D warnings
   git diff --check
   ```

2. Build release binaries through the production release workflow.

3. Build PyPI wheels through the production release workflow.

4. Install the produced PyPI package in a clean environment.

5. Install or execute each produced standalone binary.

6. Verify:

   ```text
   --help
   --version
   human output
   JSON output
   --show-source
   configuration
   exclusions
   hidden-file behavior
   worker-count behavior
   exit codes
   parse-error diagnostics
   ```

7. Run representative real-world corpora.

8. Re-run the reproducible Arid/Pylint/jscpd benchmark suite.

9. Confirm deterministic output across supported worker counts.

10. Verify README installation and usage commands exactly as documented.

11. Verify released artifacts report the intended RC version.

12. Verify the release contains no unintended generated files, benchmark outputs, development artifacts, or local configuration.

## RC Failure

If any release-blocking defect is found:

1. fix only the defect
2. run the complete validation gate again
3. publish the next release candidate

For example:

```text
1.0.0rc1
    ↓
fix
    ↓
1.0.0rc2
```

A failed RC MUST NOT be promoted to stable merely to preserve a release schedule.

---

# 5. Stable V1 Release

## Target

PyPI:

```text
1.0.0
```

GitHub release:

```text
v1.0.0
```

## Stable Release Gate

`1.0.0` may be released when:

* the latest release candidate passed the complete release-candidate validation
* no known release-blocking defect remains
* no planned v1 code change remains
* the release artifacts are reproducible and installable
* documented behavior matches actual behavior
* required performance claims remain benchmark-supported
* the v1 requirements acceptance criteria remain satisfied

The stable release SHOULD use the same source state as the successful release candidate except for changes strictly necessary to finalize release metadata or versioning.

Any code change after RC validation SHOULD invalidate that validation and require another RC unless the change is demonstrably release-metadata-only.

---

# 6. `1.0.0` Release Tasks

The stable release SHOULD include:

* PyPI release
* GitHub release
* standalone supported-platform binaries
* release notes
* installation instructions
* documented intentional differences from Pylint `R0801`
* benchmark results supporting public performance claims

The v1 requirements and architecture documents SHOULD be changed from `Draft` to their final status when they accurately describe the shipped `1.0.0` product contract.

Rust-native publication through crates.io remains a SHOULD-level distribution goal and does not by itself block `1.0.0` unless it is separately promoted to a stable-release requirement.

---

# 7. Release-Blocking Defects

A defect SHOULD block advancement to the next release stage when it materially violates the v1 product contract.

Examples include:

* incorrect duplicate detection
* false duplicate findings caused by violation of exact normalized equality
* missed duplicate findings required by the v1 contract
* incorrect same-file overlap handling
* incorrect source locations
* nondeterministic findings or structured output
* worker-count-dependent semantics
* silent file skipping
* partial successful scans after parse failure
* source execution during analysis
* crashes on valid supported Python
* broken required CLI or configuration behavior
* incorrect exit status
* corrupt or incompatible JSON output
* incorrect duplication metrics
* packaging that prevents required installation
* a substantial unexplained performance regression

The following normally SHOULD NOT block advancement:

* deferred post-v1 features
* optional convenience integrations
* speculative performance improvements
* cosmetic CLI changes
* Criterion microbenchmarks when existing required benchmark coverage remains reproducible
* crates.io publication when dependency constraints prevent it and required distribution channels remain functional

---

# 8. Scope Control Before `1.0.0`

Every proposed prerelease change SHOULD be evaluated with two questions:

> **Does this fix a defect in the defined v1 contract?**

or:

> **Is this required to safely release that contract?**

If both answers are no, the change SHOULD normally wait until after `1.0.0`.

Examples of intentionally deferred work include:

* persistent scan caching
* Git-diff-only analysis
* baseline files
* SARIF
* GitHub annotations
* editor/LSP integration
* pre-commit-specific integration
* configurable duplication budgets
* historical duplication tracking
* structural clone detection
* renamed-variable clone detection
* semantic clone detection
* autofix or refactoring suggestions

The stabilization period is not an opportunity to expand v1.

---

# 9. Planned Progression

The expected path from the code-complete implementation to stable v1 is:

```text
0.1.0a3
Code-complete alpha
        │
        │ real-world validation
        │ correctness / packaging fixes only
        ▼
0.1.0b1
V1 feature freeze
        │
        │ public stabilization
        │ regression fixes only
        ▼
1.0.0rc1
Believed release-ready
        │
        │ final release validation
        ▼
1.0.0
Stable Arid v1
```

Additional prerelease versions are inserted only when necessary:

```text
a3 → a4 → ...
b1 → b2 → ...
rc1 → rc2 → ...
```

There is no required minimum number or duration of prerelease stages.

Advancement is based on satisfying the release gate for each stage, not on elapsed time.

---

# 10. After `1.0.0`

The stable `1.0.0` release closes the Arid v1 product scope.

Post-1.0 development SHOULD begin from measured user needs, real defects, and demonstrated workflow gaps rather than automatically implementing the deferred-feature list.

Future work should continue to preserve Arid's core product principle:

> **Does this make Arid better at finding duplicated Python code?**

If not, it probably does not belong in Arid.