# Arid validation

Arid's validation suite exercises Arid against unfamiliar real-world Python repositories and targeted filesystem edge cases.

It complements the automated test suite and benchmarks:

- tests verify known behavior with controlled fixtures
- validation looks for correctness and robustness defects on real repositories
- benchmarks measure performance on fixed benchmark corpora

Validation is intended primarily for prerelease stabilization and release qualification.

## Goals

The validation suite MUST:

- exercise Arid against real Python repositories outside the benchmark corpus
- include materially different repository structures and source characteristics
- validate normal `.py` and `.pyi` processing
- exercise intentionally invalid Python without silently skipping it
- verify ignore-aware file discovery
- verify deterministic output across worker counts on a large real repository
- exercise Unicode source and filesystem paths
- support validation of both the current repository build and existing release artifacts
- preserve exact corpus revisions for reproducible release qualification
- fail immediately when a validation invariant is violated

Validation MUST NOT become a second unit-test framework.

When validation exposes a genuine Arid defect, the smallest useful reproduction SHOULD be added to the normal automated test suite before the defect is fixed.

## Tooling

The validation tooling consists of:

```bash
validation/build.sh <global-root> [options]
validation/run.sh <global-root> [options]
```

`build.sh` provisions the validation corpora beneath:

```text
<global-root>/validation/arid-corpora/
```

`run.sh` validates either:

- `target/release/arid` built from the current Arid repository, or
- an existing Arid executable supplied with `--arid-bin`

Generated validation artifacts are written beneath:

```text
<project-root>/validation/results/
```

The validation repositories are:

- `black`
- `django`
- `mypy`
- `rich`

All four are validated by default.

Use `--repos` to select a subset.

## Validation corpora

The repositories were selected for different validation characteristics rather than as performance benchmarks.

### Black

Repository:

```text
https://github.com/psf/black
```

Black provides:

- formatter and parser-oriented Python
- unusual syntax
- deliberately malformed parser fixtures
- ordinary executable and declarative Python
- repeated source in tests and implementation code

Black contains Python files that intentionally do not parse.

The raw corpus validation verifies that Arid detects the known invalid fixture and terminates with a parsing error rather than silently omitting it.

The normal-source scan excludes:

```text
tests/data/**
profiling/**
```

These paths contain Black-specific fixture or profiling material that is outside the ordinary-source validation corpus.

### Django

Repository:

```text
https://github.com/django/django
```

Django provides:

- a large mature Python framework
- thousands of Python files
- broad application and test-code structure
- substantial same-file and cross-file duplication
- an intentionally invalid Python test fixture
- a large corpus suitable for real-world worker-determinism validation

The raw corpus validation verifies that Arid rejects:

```text
tests/test_runner_apps/tagged/tests_syntax_error.py
```

The normal-source scan excludes only that known invalid fixture.

Django is also the canonical validation corpus for worker determinism.

Arid output is compared byte-for-byte at:

```text
workers=1
workers=2
workers=4
workers=8
```

All four JSON outputs MUST be identical.

### mypy

Repository:

```text
https://github.com/python/mypy
```

mypy provides:

- a large typing-heavy Python codebase
- extensive `.pyi` stub coverage
- compiler and type-checker implementation code
- intentionally malformed `.pyi` fixtures
- real ignore-rule interactions with tracked files

The raw corpus validation verifies that Arid rejects the known invalid stub:

```text
test-data/unit/lib-stub/blocker.pyi
```

The normal-source scan excludes:

```text
test-data/**
```

The validation output also records:

- tracked Python files outside `test-data`
- tracked `.pyi` files outside `test-data`
- findings touching `.pyi`
- differences between Git-tracked Python files and normal ignore-aware discovery

This last comparison is intentional.

A repository may contain a Git-tracked Python file that is nevertheless matched by an ignore rule. Arid follows normal ignore-aware directory discovery, so Git-tracked file count alone is not always the correct expected discovery count.

### Rich

Repository:

```text
https://github.com/Textualize/rich
```

Rich provides:

- normal production Python
- Unicode-heavy source
- generated and repetitive declarative data
- multiline strings
- same-file duplication
- cross-file duplication
- executable and declarative duplicate groups

No repository-specific exclusions are applied.

The validation output separately characterizes findings involving:

```text
rich/_unicode_data/
```

so large generated Unicode tables can be distinguished from ordinary-code findings without changing Arid's detection contract.

Generated Python remains valid Python. The validation suite does not automatically exclude generated source merely because it produces substantial duplication.

## Corpus provisioning

Validation corpora are provisioned with:

```bash
validation/build.sh <global-root> [options]
```

The script owns the local layout:

```text
<global-root>/
└── validation/
    └── arid-corpora/
        ├── black/
        ├── django/
        ├── mypy/
        └── rich/
```

Repository names and upstream URLs are defined by the validation harness.

With no revision options, `build.sh` resolves each repository to the current upstream default-branch `HEAD`:

```bash
validation/build.sh /home/bobt
```

This is useful for exploratory validation against current upstream source.

Release-qualification runs SHOULD use explicit revisions so the validation inputs can be reproduced:

```bash
validation/build.sh /home/bobt \
    --black <revision> \
    --django <revision> \
    --mypy <revision> \
    --rich <revision>
```

Revision options accept Git commit SHAs, tags, or branches:

```text
--black <revision>
--django <revision>
--mypy <revision>
--rich <revision>
```

Any omitted repository defaults to its current upstream `HEAD`.

`build.sh`:

- clones missing repositories
- fetches existing repositories
- verifies the expected upstream `origin`
- refuses to modify a dirty working tree
- resolves the requested revision to an exact commit
- leaves the corpus detached at that exact commit

Existing corpus repositories are therefore protected from accidental destruction of local changes.

## Running validation

Validate all repositories using the current repository release build:

```bash
validation/run.sh /home/bobt
```

Validate a subset:

```bash
validation/run.sh /home/bobt --repos rich,mypy
```

Accepted repository names are:

```text
black,django,mypy,rich
```

Names are comma-separated and may not be repeated.

### Repository build mode

When `--arid-bin` is omitted, `run.sh` builds the current repository with:

```bash
cargo build --release --locked
```

and validates:

```text
target/release/arid
```

This is the default mode for development and source-tree qualification.

### External binary mode

Validate an existing Arid executable with:

```bash
validation/run.sh /home/bobt \
    --arid-bin /path/to/arid
```

When `--arid-bin` is supplied, `run.sh` does not build Arid.

The supplied executable MUST:

- exist
- be executable
- identify itself as Arid through `--version`

This mode is intended for validating previously built or published release artifacts against the same real-world validation campaign used for repository builds.

For example, a downloaded GitHub release binary can be validated directly without rebuilding Arid from source.

The validation behavior is otherwise unchanged: the same repositories, exclusions, malformed-source probes, discovery checks, determinism checks, and path cases are exercised.

## Repository preconditions

Before repository validation begins, `run.sh` verifies that each selected corpus:

1. exists
2. is a Git repository
3. is being validated from its Git repository root
4. has a clean working tree

A dirty corpus fails validation before Arid is executed.

This prevents local corpus modifications from silently changing release-qualification inputs.

## Repository validation

For each selected repository, `run.sh` performs the repository-specific checks described above.

Normal-source scans require:

- Arid exit status `0` or `1`
- empty stderr
- valid JSON output
- Arid's discovered file count to match normal ignore-aware discovery
- `duplicate_groups` to equal the number of JSON findings
- exit status to agree with whether findings exist

An unexpected parse error, internal error, discovery mismatch, malformed report, or status inconsistency fails validation immediately.

### Invalid-source probes

Black, Django, and mypy contain known intentionally invalid Python fixtures.

For these repositories, validation first scans the unfiltered repository and requires:

- exit status `2`
- a diagnostic naming the expected invalid fixture
- an `invalid Python syntax` diagnostic

This verifies that malformed Python is not silently omitted during normal directory discovery.

The known fixture is excluded only for the subsequent normal-source validation.

## Worker determinism

Django is validated at worker counts:

```text
1,2,4,8
```

The workers-1 scan is the baseline.

The workers-2, workers-4, and workers-8 JSON files MUST be byte-for-byte identical to the baseline.

The validation harness also prints SHA-256 hashes for all four outputs.

This checks deterministic:

- finding content
- finding ordering
- canonical occurrence selection
- metrics
- source locations
- path representation
- JSON serialization

across parallel file preparation on a large real-world repository.

## Filesystem path validation

Filesystem path handling is validated independently of the repository selection.

`run.sh` creates a temporary source tree equivalent to:

```text
ünicode project/
├── alpha file.py
└── sub dir/
    └── βeta.py
```

The two files contain a known four-line duplicate.

Validation requires:

- exit status `1`
- empty stderr
- exactly two discovered files
- exactly one duplicate group
- a four-line finding
- correct physical source locations
- correct relative JSON paths
- preservation of spaces and non-ASCII filename characters

The temporary source tree is removed after the check.

`path-cases` runs even when `--repos` selects only a subset of repositories.

## Results

Generated validation artifacts are written beneath:

```text
validation/results/
```

A full run produces:

```text
validation/results/
├── metadata.txt
├── black/
│   ├── arid.json
│   └── arid.stderr
├── django/
│   ├── arid.json
│   ├── arid.stderr
│   ├── arid-w2.json
│   ├── arid-w2.stderr
│   ├── arid-w4.json
│   ├── arid-w4.stderr
│   ├── arid-w8.json
│   └── arid-w8.stderr
├── mypy/
│   ├── arid.json
│   └── arid.stderr
├── rich/
│   ├── arid.json
│   └── arid.stderr
└── path-cases/
    ├── arid.json
    └── arid.stderr
```

Each repository directory contains the normal-source validation result.

Django additionally contains the worker-determinism outputs.

## Validation metadata

`metadata.txt` records the identity of both the validation harness and the Arid executable being exercised.

Common metadata includes:

```text
date_utc
harness_commit
arid_source
arid_binary
arid_sha256
arid_version
global_root
corpus_root
selected_repositories
path_cases
```

`harness_commit` identifies the Arid repository commit containing the validation harness.

`arid_source` is:

```text
repository
```

when `run.sh` built `target/release/arid` from the current repository, or:

```text
external
```

when the executable was supplied with `--arid-bin`.

`arid_binary` records the absolute executable path.

`arid_sha256` records the SHA-256 digest of the exact executable used for validation.

`arid_version` records the executable's `--version` output.

When validating a repository-built executable, metadata also records:

```text
arid_commit
```

which identifies the source commit used to build Arid.

An external executable does not receive an `arid_commit` value because the validation harness cannot infer source provenance from an arbitrary executable. External artifacts are instead identified by their:

- absolute path
- reported version
- SHA-256 digest

Metadata also records, for each selected validation repository:

- corpus Git commit
- upstream remote URL
- repository-specific exclusions
- Django determinism worker counts where applicable

This allows a validation result to be tied to both the exact Arid executable and the exact real-world corpus revisions used.

## Subset runs

A subset run refreshes only the result directories for the selected repositories.

For example:

```bash
validation/run.sh /home/bobt --repos rich,mypy
```

refreshes:

```text
validation/results/rich/
validation/results/mypy/
validation/results/path-cases/
validation/results/metadata.txt
```

Existing Black and Django result directories are left untouched.

`path-cases` and campaign metadata are refreshed on every invocation.

This permits focused validation during development without destroying unrelated previous results.

The same behavior applies when `--arid-bin` is used.

For example:

```bash
validation/run.sh /home/bobt \
    --repos rich \
    --arid-bin /path/to/arid
```

refreshes only:

```text
validation/results/rich/
validation/results/path-cases/
validation/results/metadata.txt
```

## Release-artifact validation

External binary mode allows the validation suite to qualify the exact executable distributed to users.

A release candidate can therefore be validated in two distinct layers:

```text
Repository source
    ↓
cargo build --release --locked
    ↓
validation/run.sh
```

and:

```text
Published release artifact
    ↓
extract executable
    ↓
validation/run.sh --arid-bin <artifact>
```

The second form is especially useful during release-candidate qualification because it verifies the exact published bytes rather than another locally rebuilt executable.

The SHA-256 recorded in `metadata.txt` provides a stable identity for that artifact.

Platform-specific release workflows SHOULD still smoke-test their native artifacts on their native CI runners. The real-world validation harness complements those platform smoke tests by exercising the release executable against large and varied Python corpora.

## Result interpretation

A successful validation run demonstrates that the tested Arid executable behaved correctly against the tested corpus revisions and validation cases.

It does not prove that Arid is defect-free.

Validation findings SHOULD be interpreted according to Arid's existing product contract rather than used as a reason to expand product scope.

In particular:

- finding duplication in generated source is not itself a defect
- a tracked file excluded by normal ignore rules is not automatically a discovery defect
- intentionally malformed source should fail clearly rather than be silently skipped
- differences from another duplicate detector are not automatically Arid defects because detection semantics may differ
- finding something Arid does not detect is not evidence that Arid requires a new feature unless the behavior violates the existing product contract

If validation exposes suspicious behavior:

1. determine whether it violates the existing Arid contract
2. reduce genuine defects to the smallest useful reproduction
3. add an automated regression test when appropriate
4. fix the defect without expanding release scope unnecessarily
5. rerun the affected validation target

Validation SHOULD stop once the release criteria are satisfied rather than expanding indefinitely into additional repositories or synthetic cases without a specific unresolved risk.

## Release qualification

Validation is one part of the release process.

A typical prerelease qualification flow is:

```text
Automated tests
    ↓
Real-world validation
    ↓
Performance benchmarks
    ↓
Native release-artifact smoke tests
    ↓
Published-artifact validation
    ↓
Release
```

During development, the validation suite normally qualifies the current repository release binary.

During release-candidate qualification, `--arid-bin` can additionally qualify the exact published executable.

The benchmark suite separately measures performance using the canonical benchmark corpora.

Published Python packages and standalone binaries SHOULD also receive platform-appropriate installation or execution smoke tests as part of the release workflow.

Correctness remains more important than validation convenience, benchmark performance, or release cadence.