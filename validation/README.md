# Arid validation

Arid's validation suite exercises the current release build against unfamiliar real-world Python repositories and targeted filesystem edge cases.

It complements the automated test suite and benchmarks:

* tests verify known behavior with controlled fixtures
* validation looks for correctness and robustness defects on real repositories
* benchmarks measure performance on fixed benchmark corpora

Validation is intended primarily for prerelease stabilization and release qualification.

## Goals

The validation suite MUST:

* exercise Arid against real Python repositories outside the benchmark corpus
* include materially different repository structures and source characteristics
* validate normal `.py` and `.pyi` processing
* exercise intentionally invalid Python without silently skipping it
* verify ignore-aware file discovery
* verify deterministic output across worker counts on a large real repository
* exercise Unicode source and filesystem paths
* preserve exact corpus revisions for reproducible release qualification
* fail immediately when a validation invariant is violated

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

`run.sh` builds the current Arid repository in release mode, validates the selected corpora, and writes generated artifacts beneath:

```text
<project-root>/validation/results/
```

The validation repositories are:

* `black`
* `django`
* `mypy`
* `rich`

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

* formatter and parser-oriented Python
* unusual syntax
* deliberately malformed parser fixtures
* ordinary executable and declarative Python
* repeated source in tests and implementation code

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

* a large mature Python framework
* thousands of Python files
* broad application and test-code structure
* substantial same-file and cross-file duplication
* an intentionally invalid Python test fixture
* a large corpus suitable for real-world worker-determinism validation

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

* a large typing-heavy Python codebase
* extensive `.pyi` stub coverage
* compiler and type-checker implementation code
* intentionally malformed `.pyi` fixtures
* real ignore-rule interactions with tracked files

The raw corpus validation verifies that Arid rejects the known invalid stub:

```text
test-data/unit/lib-stub/blocker.pyi
```

The normal-source scan excludes:

```text
test-data/**
```

The validation output also records:

* tracked Python files outside `test-data`
* tracked `.pyi` files outside `test-data`
* findings touching `.pyi`
* differences between Git-tracked Python files and normal ignore-aware discovery

This last comparison is intentional.

A repository may contain a Git-tracked Python file that is nevertheless matched by an ignore rule. Arid follows normal ignore-aware directory discovery, so Git-tracked file count alone is not always the correct expected discovery count.

### Rich

Repository:

```text
https://github.com/Textualize/rich
```

Rich provides:

* normal production Python
* Unicode-heavy source
* generated and repetitive declarative data
* multiline strings
* same-file duplication
* cross-file duplication
* executable and declarative duplicate groups

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

* clones missing repositories
* fetches existing repositories
* verifies the expected upstream `origin`
* refuses to modify a dirty working tree
* resolves the requested revision to an exact commit
* leaves the corpus detached at that exact commit

Existing corpus repositories are therefore protected from accidental destruction of local changes.

## Running validation

Validate all repositories:

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

Before validation begins, `run.sh`:

1. verifies each selected corpus exists
2. verifies each selected corpus is a clean Git repository root
3. builds the current Arid repository with:

```bash
cargo build --release --locked
```

Validation therefore exercises:

```text
target/release/arid
```

from the current Arid source tree.

It does not install or execute a PyPI artifact.

Published-package installation is a separate release smoke check.

## Repository validation

For each selected repository, `run.sh` performs the repository-specific checks described above.

Normal-source scans require:

* Arid exit status `0` or `1`
* empty stderr
* valid JSON output
* Arid's discovered file count to match normal ignore-aware discovery
* `duplicate_groups` to equal the number of JSON findings
* exit status to agree with whether findings exist

An unexpected parse error, internal error, discovery mismatch, malformed report, or status inconsistency fails validation immediately.

### Invalid-source probes

Black, Django, and mypy contain known intentionally invalid Python fixtures.

For these repositories, validation first scans the unfiltered repository and requires:

* exit status `2`
* a diagnostic naming the expected invalid fixture
* an `invalid Python syntax` diagnostic

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

* finding content
* finding ordering
* canonical occurrence selection
* metrics
* source locations
* path representation
* JSON serialization

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

* exit status `1`
* empty stderr
* exactly two discovered files
* exactly one duplicate group
* a four-line finding
* correct physical source locations
* correct relative JSON paths
* preservation of spaces and non-ASCII filename characters

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

`metadata.txt` records:

* validation timestamp
* Arid Git commit
* Arid version
* global root
* corpus root
* selected repositories
* selected corpus Git commits
* selected corpus upstream URLs
* repository-specific exclusions
* Django determinism worker counts

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

## Result interpretation

A successful validation run demonstrates that the tested Arid revision behaved correctly against the tested corpus revisions and validation cases.

It does not prove that Arid is defect-free.

Validation findings SHOULD be interpreted according to Arid's existing v1 contract rather than used as a reason to expand product scope.

In particular:

* finding duplication in generated source is not itself a defect
* a tracked file excluded by normal ignore rules is not automatically a discovery defect
* intentionally malformed source should fail clearly rather than be silently skipped
* differences from another duplicate detector are not automatically Arid defects because detection semantics may differ
* finding something Arid does not detect is not evidence that Arid requires a new feature unless the behavior violates the existing product contract

If validation exposes suspicious behavior:

1. determine whether it violates the existing Arid contract
2. reduce genuine defects to the smallest useful reproduction
3. add an automated regression test when appropriate
4. fix the defect without expanding v1 scope unnecessarily
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
Published-artifact smoke test
    ↓
Release
```

The validation suite qualifies the current repository release binary.

The benchmark suite separately measures performance using the canonical benchmark corpora.

After publication, the distributed package or standalone artifact SHOULD receive a separate smoke test to confirm that installation and execution succeeded.

Correctness remains more important than validation convenience, benchmark performance, or release cadence.