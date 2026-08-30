# Arid release qualification

The qualification harness automates acceptance of published Arid release candidates and stable releases.

It composes the existing release, validation, and benchmark tooling rather than reimplementing their behavior.

```text
release.sh
    ↓
release workflow
    ↓
qualification/run.sh
    ├── validation/<release-series>.sh
    ├── validation/run.sh
    └── benchmarks/run.sh
```

Qualification operates on published release artifacts and exact Git tags.

## Goals

The qualification harness MUST:

- qualify the exact tagged release source rather than the current `main` source
- verify that the release tag belongs to `origin/main` history
- verify the production release workflow completed successfully
- verify the corresponding GitHub release
- verify the exact published PyPI package
- verify the published Linux x86-64 standalone artifact
- run release-series targeted integration validation against both published RC executables when that series provides a targeted suite
- run the real-world validation campaign against published RC artifacts
- compare standalone and PyPI validation JSON byte-for-byte
- benchmark the exact published standalone RC artifact
- enforce the release performance target
- preserve qualification evidence locally
- prevent stable promotion without a qualified RC
- prevent stable promotion when product code changed after the qualified RC

The harness does not publish releases.

Release creation and qualification remain separate operations.

## Usage

Qualify a published release candidate:

```bash
qualification/run.sh /home/bobt 1.0.1-rc.1
```

Qualify a published stable release:

```bash
qualification/run.sh /home/bobt 1.0.1
```

Supported version forms are:

```text
X.Y.Z-rc.N
X.Y.Z
```

Alpha and beta releases are not accepted by the qualification harness.

## Preconditions

Qualification MUST run from:

- Linux
- x86-64
- the Arid `main` branch
- a clean working tree
- a `main` branch synchronized with `origin/main`

The harness fetches:

```text
origin/main
tags
```

before qualification.

The release tag does not need to point at the current `main` HEAD.

Instead, the tag MUST:

- exist locally after fetch
- exist on `origin`
- resolve to the same commit locally and remotely
- be an ancestor of `origin/main`

This allows older releases to be qualified after subsequent development has moved `main` forward.

## Source identity

Qualification distinguishes between two source identities:

```text
harness_commit
tag_commit
```

`harness_commit` is the current synchronized `main` commit containing the qualification tooling.

`tag_commit` is the exact release source being qualified.

Release-source checks operate from a detached Git worktree at `tag_commit`.

Validation and benchmark orchestration use the current qualification harness.

This separation allows qualification tooling to evolve without changing the source identity of historical releases.

## Release-candidate qualification

Release candidates receive the complete qualification campaign.

For:

```bash
qualification/run.sh /home/bobt 1.0.1-rc.1
```

the harness performs:

```text
tag verification
    ↓
tagged release metadata verification
    ↓
tagged source gate
    ↓
production release workflow verification
    ↓
GitHub prerelease verification
    ↓
standalone artifact download
    ↓
standalone smoke test
    ↓
clean PyPI installation
    ↓
PyPI smoke test
    ↓
release-series targeted integration validation, when available
    ↓
standalone real-world validation
    ↓
PyPI real-world validation
    ↓
byte-for-byte JSON equivalence
    ↓
published-artifact benchmarks
    ↓
performance gates
    ↓
qualification PASS
```

### Tagged source gate

The release-candidate source gate runs against a detached worktree at the exact release tag.

It executes:

```bash
cargo fmt --check

cargo test --locked

cargo clippy \
    --locked \
    --all-targets \
    --all-features \
    -- \
    -D warnings

git diff --check
```

The tagged worktree MUST remain clean.

### Release metadata

The tagged release source MUST pass:

```bash
./release.sh --check
```

This verifies that the metadata stored in the release tag is internally consistent.

Qualification therefore does not rely on the release metadata currently present on `main`.

Release metadata files are selected by release series. The active roadmap mapping is:

```text
1.0.x  → docs/arid-v1-release-roadmap.md
1.1.x  → docs/arid-v1.1-release-roadmap.md
1.2.x  → docs/arid-v1.2-release-roadmap.md
2.0.x  → docs/arid-v2-release-roadmap.md
2.1.x  → docs/arid-v2.1-release-roadmap.md
2.2.x  → docs/arid-v2.2-release-roadmap.md
```

Only the roadmap for the release series being qualified participates in that RC-to-stable metadata transition.

### Production release workflow

The harness locates the GitHub Actions `release.yml` run whose:

```text
head_sha == tag_commit
```

The run MUST be:

```text
status=completed
conclusion=success
```

This proves that the production release workflow associated with the release tag succeeded.

The production workflow remains responsible for native build and package smoke tests across its platform matrix.

For 1.2 and all v2 releases, qualification additionally requires the production workflow's published Linux aarch64 verification job to pass. For v2 releases it also requires the published GitHub Action verification job to pass.

Qualification does not duplicate those platform-specific jobs locally.

### GitHub release

For an RC, the GitHub release MUST:

- exist
- use the expected tag
- not be a draft
- be marked as a prerelease
- contain `arid-linux-x86_64.tar.gz`

### Standalone artifact

The Linux standalone archive is downloaded directly from the GitHub release:

```text
arid-linux-x86_64.tar.gz
```

The extracted executable MUST:

- exist
- be executable
- report the requested version through `--version`
- execute `--help` successfully

The harness records SHA-256 values for both:

```text
standalone archive
standalone executable
```

### PyPI package

The exact release is installed into a new temporary virtual environment directly from PyPI.

For:

```text
1.0.1-rc.1
```

the PyPI version is:

```text
1.0.1rc1
```

The installed executable MUST:

- exist
- be executable
- report the requested Arid version
- execute `--help` successfully

No existing local Python environment is reused.

### Release-series targeted integration validation

When a release series provides a targeted integration suite, both published executables must pass that exact series suite before the larger real-world campaign.

Current mappings are:

```text
1.1.x  → validation/v1.1.sh
1.2.x  → validation/v1.2.sh
2.0.x  → validation/v2.sh
2.1.x  → validation/v2.1.sh
2.2.x  → validation/v2.2.sh
```

The targeted suite runs first against the downloaded standalone executable and then against the exact PyPI-installed executable.

For v2.2 this includes the inherited v2/v2.1 contracts plus rich Summary/Breakdown/Hotspots behavior, `--summary-only`, `summary-v1`, complete/incomplete behavior, baseline/focus interactions, supplemental report preservation, color/plain parity, timing exclusions, and adaptive-worker determinism.

This gate runs before the larger real-world validation and benchmark campaign so a broken release-series integration surface fails qualification early.

### Real-world validation

The exact published standalone executable is passed to:

```bash
validation/run.sh \
    <global-root> \
    --arid-bin <standalone-executable>
```

The resulting validation tree is preserved.

The exact PyPI-installed executable is then passed through the same validation campaign.

The resulting validation tree is also preserved.

The validation harness remains responsible for:

- canonical validation corpora
- exclusions
- path cases
- expected finding statuses
- Django worker determinism
- validation metadata

Qualification does not duplicate those rules.

### Artifact equivalence

After both validation campaigns complete, qualification discovers every:

```text
*.json
```

file beneath the standalone and PyPI validation result trees.

The two result trees MUST contain the same relative JSON file set.

Every corresponding JSON file MUST then be byte-identical.

This includes the primary real-world corpus results and Django worker-determinism outputs produced by the validation harness.

The comparison is discovery-based rather than a hard-coded list, so newly added validation JSON outputs are automatically included.

### Published-artifact benchmarks

Release-candidate benchmarks use the exact downloaded standalone executable:

```bash
benchmarks/run.sh \
    <global-root> \
    --arid-bin <standalone-executable> \
    --label <release-version> \
    --warmup 3 \
    --runs 10
```

The benchmark harness provisions the pinned canonical revisions:

```text
Polaris:
00e208e7f5dcb3329c3d8d1ee5f13aec7fbe1031

Pydantic:
cf67d4b3193c3fe43ede18612ed62785eee11382

Requests:
6e83187b8feb273ed4c6cdab5efd8d54901dfab3
```

The benchmark harness remains responsible for the detailed cross-tool methodology and result generation.

Qualification does not duplicate those rules.

### Performance gate

The release qualification target requires Arid to remain at least:

```text
10x faster
```

than isolated Pylint duplicate detection on both:

```text
Pydantic
Polaris
```

The speedup is calculated from the Hyperfine mean execution times stored in each `pylint.json` result.

Failure to meet either target fails qualification.

Requests remains part of the full benchmark suite but is not used as the v1 medium/large performance gate.

## Stable qualification

Stable releases do not repeat the complete RC campaign when the stable release is only the approved metadata transition from a fully qualified RC.

For:

```bash
qualification/run.sh /home/bobt 1.0.1
```

the harness:

1. finds the highest `v1.0.1-rc.N` tag
2. requires a local PASS qualification record for that RC
3. verifies the qualification record still identifies the current RC tag commit
4. verifies that no non-release-metadata files changed from the RC to stable
5. independently reproduces the stable metadata transition with `release.sh`
6. compares the reproduced metadata byte-for-byte with the stable tag
7. verifies the production stable release workflow
8. verifies the GitHub stable release
9. smoke-tests the published standalone artifact
10. installs and smoke-tests the exact stable PyPI package

A successful stable qualification therefore reuses the complete evidence from the latest qualified RC while independently proving that stable promotion did not change product behavior.

## Stable metadata transition

The common files permitted to change between the qualified RC and stable release are:

```text
Cargo.toml
Cargo.lock
pyproject.toml
README.md
```

The release-series roadmap is also permitted:

```text
1.0.x  → docs/arid-v1-release-roadmap.md
1.1.x  → docs/arid-v1.1-release-roadmap.md
1.2.x  → docs/arid-v1.2-release-roadmap.md
2.0.x  → docs/arid-v2-release-roadmap.md
2.1.x  → docs/arid-v2.1-release-roadmap.md
2.2.x  → docs/arid-v2.2-release-roadmap.md
```

For v2 releases, `action.yml` is also managed release metadata and may change only as part of the exact `release.sh` transition.

Allowing a file name alone is not sufficient.

Qualification creates a detached worktree at the qualified RC and executes:

```bash
./release.sh <stable-version>
```

It then compares every managed release metadata file against the actual stable tag byte-for-byte.

The stable release MUST therefore be exactly the transition that `release.sh` would have produced from the qualified RC.

If product code or any other file changed after the RC, stable qualification fails and another release candidate is required.

## Qualification evidence

Generated evidence is written beneath:

```text
qualification/results/
```

For a release candidate:

```text
qualification/results/
└── v1.0.1-rc.1/
    ├── qualification.txt
    ├── artifacts/
    │   ├── arid-linux-x86_64.tar.gz
    │   ├── sha256.txt
    │   └── linux-x86_64/
    │       └── arid
    ├── standalone/
    │   └── validation/
    └── pypi/
        └── validation/
```

Benchmark output remains owned by:

```text
benchmarks/results/
```

with the qualification version used as the result label.

For example:

```text
benchmarks/results/
├── polaris-1.0.1-rc.1/
├── pydantic-1.0.1-rc.1/
└── requests-1.0.1-rc.1/
```

### Qualification record

A successful RC record contains information including:

```text
qualification=PASS
version
tag
tag_commit
harness_commit
release workflow identity
GitHub release identity
standalone artifact SHA-256
standalone executable SHA-256
PyPI executable SHA-256
validation result paths
validation JSON count
artifact-equivalence result
benchmark configuration
Pydantic/Pylint speedup
Polaris/Pylint speedup
```

For release series with targeted integration validation, the record additionally includes series-specific standalone and PyPI validation PASS fields. V2 qualification records also include:

```text
published_action_verification=PASS
```

A successful stable record additionally identifies:

```text
base_rc
base_rc_qualification
metadata_delta
```

Stable qualification uses the RC record as evidence that the selected RC actually completed the full qualification campaign.

## Local evidence

`qualification/results/` is intentionally ignored by Git.

Qualification artifacts can be large and include:

- published release archives
- extracted executables
- complete real-world validation results

They are operational release evidence rather than source files.

The authoritative published artifacts remain on GitHub Releases and PyPI.

The local qualification record connects those published artifacts to the exact qualification campaign that was executed.

Because stable qualification currently depends on the local RC PASS record, preserve the relevant qualification result directory until stable promotion is complete.

## Exit behavior

Successful qualification exits with status `0` and prints:

```text
QUALIFICATION:               PASS
```

Any failed prerequisite, release check, validation, comparison, benchmark, or promotion rule terminates qualification with a nonzero status.

A failure MUST be investigated rather than bypassed manually.

In particular, do not manually mark an RC qualified or manually promote stable after a qualification failure.

## Typical release flow

Prepare release metadata:

```bash
./release.sh 1.0.1-rc.1
```

Commit, push, tag, and allow the production release workflow to publish the release.

Then qualify the published RC:

```bash
qualification/run.sh /home/bobt 1.0.1-rc.1
```

If qualification passes and no product changes are required, prepare stable:

```bash
./release.sh 1.0.1
```

After committing, tagging, and publishing stable:

```bash
qualification/run.sh /home/bobt 1.0.1
```

If source changes are required after the qualified RC, create and fully qualify another RC instead of promoting the changed source directly to stable.

For example:

```text
v1.0.1-rc.1
    ↓
qualification PASS
    ↓
product change required
    ↓
v1.0.1-rc.2
    ↓
qualification PASS
    ↓
metadata-only stable transition
    ↓
v1.0.1
```

## First proven qualification

The qualification harness was initially proven retrospectively against Arid's `1.0.0` release sequence.

The already-published:

```text
v1.0.0-rc.2
```

successfully completed the automated full RC qualification campaign.

The already-published:

```text
v1.0.0
```

then successfully completed automated stable qualification using that RC2 PASS record.

This confirmed both qualification paths against the same release sequence that had previously been qualified manually.