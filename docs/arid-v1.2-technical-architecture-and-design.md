# **Arid v1.2 Technical Architecture and Design**

**Status:** Draft  
**Product:** Arid  
**CLI:** `arid`  
**Cargo package:** `arid-cli`  
**PyPI package:** `arid`  
**Configuration:** `[tool.arid]`  
**Primary diagnostic:** `DUP001`  
**Implementation language:** Rust  
**Version scope:** This document defines the technical delta targeted for Arid **1.2.0**.

---

# **1. Purpose**

The v1 and v1.1 architecture documents remain the base implementation contract.

V1.2 changes only:

- Linux release portability
- opt-in `--workers auto`
- performance investigation/hardening
- published schemas for existing machine contracts
- release/qualification robustness

The duplicate detector remains semantically unchanged.

---

# **2. Architectural Principle**

The application pipeline remains:

```text
scan paths
    ↓
configuration
    ↓
discovery
    ↓
read + Python parse + normalization
    ↓
corpus
    ↓
suffix array + LCP
    ↓
maximal exact duplicate groups
    ↓
optional baseline enforcement
    ↓
Report
    ↓
text | JSON | Markdown | SARIF
```

V1.2 strengthens execution and delivery around this pipeline. It does not add another detection path.

---

# **3. Major Technical Decisions**

| Area | V1.2 decision |
| --- | --- |
| Detection model | unchanged from v1.1 |
| Default workers | `1` |
| Auto workers | opt-in `--workers auto` |
| Auto cap | `4` |
| Capacity source | `std::thread::available_parallelism()` |
| Worker config | CLI only; not `[tool.arid]` |
| Performance work | profile first; optimize measured bottlenecks only |
| Persistent cache | none |
| User-facing timing mode | none |
| Linux targets | x86_64 + aarch64 GNU/Linux |
| Linux compatibility | explicit manylinux; prefer 2014/2_17 if clean, else 2_28 |
| Linux standalone | exact binary extracted from built wheel |
| ARM64 verification | native post-publication GitHub Actions job |
| Report schema | publish existing v3 unchanged |
| Baseline schema | publish existing v1 unchanged |
| PyPI readiness | exact-version endpoint; retry 404 only; bounded |
| Breaking cleanup | deferred to 2.0 |

---

# **4. Repository Delta**

Expected additions:

```text
schemas/
├── baseline-v1.schema.json
└── report-v3.schema.json

qualification/
├── pypi_ready.py
└── run.sh

validation/
└── v1.2.sh
```

A benchmark-only profiling helper MAY be added under `benchmarks/` if permanent automation proves useful.

V1.2 MUST NOT add a worker manager, scheduler layer, packaging framework, schema registry, or profiling subsystem.

---

# **5. `--workers auto`**

## **5.1 Preserve the current CLI representation**

The existing field remains:

```rust
pub workers: usize
```

The current worker parser is extended so:

```text
"auto"           → resolved numeric capacity
positive integer → existing numeric behavior
0 / invalid      → CLI error
```

No new worker enum or sentinel value is required.

## **5.2 Resolution**

`auto` resolves at CLI parse time:

```text
available = std::thread::available_parallelism()
                 .map(get)
                 .unwrap_or(1)

workers = max(1, min(4, available))
```

The existing preparation path already caps the actual pool by discovered file count:

```text
worker_count = min(requested_workers, discovered_file_count)
```

Therefore effective auto behavior is:

```text
max(1, min(4, available_parallelism, discovered_file_count))
```

without another worker-resolution abstraction.

Failure to query available parallelism falls back to `1`.

## **5.3 Default and determinism**

The default remains numeric `1`; normal `arid .` does not query machine parallelism.

Serial, explicit numeric parallel, and auto modes MUST produce identical findings, metrics, deterministic non-colored output bytes, and exit status for the same input.

---

# **6. Performance Hardening**

V1.2 does not assume the suffix array is the bottleneck.

Investigation must first distinguish meaningful cost among:

```text
discovery
read / parse / normalize
corpus construction
suffix-array construction
LCP construction
duplicate extraction
report construction / rendering
```

The preferred approach is benchmark/development-only profiling of a release-equivalent binary using the established pinned corpora. No timing option is added to the user CLI.

If permanent automation is useful, a small `benchmarks/profile.sh` MAY wrap a system profiler and save generated output under the ignored benchmark-results area.

Optimization acceptance requires:

1. a measured bottleneck
2. a measurable representative gain
3. unchanged serial correctness and determinism
4. complexity proportionate to the gain
5. no meaningful regression in the full benchmark campaign

Allocation/local implementation improvements are preferred before replacing the existing suffix-array algorithm.

`--workers auto` is benchmarked as an additional mode. Serial remains the regression baseline.

---

# **7. Linux Release Architecture**

## **7.1 Matrix**

Add:

```text
linux-x86_64
    target: x86_64-unknown-linux-gnu

linux-aarch64
    target: aarch64-unknown-linux-gnu
```

Preserve:

```text
macos-aarch64
macos-x86_64
windows-x86_64
```

The ARM64 Linux job SHOULD use a native GitHub-hosted ARM64 runner so its artifacts can be executed without project-specific emulation.

## **7.2 Explicit manylinux policy**

Linux wheel jobs MUST pass an explicit manylinux value to maturin-action.

Implementation first proves:

```text
manylinux2014 / manylinux_2_17
```

for both architectures.

If that requires special-case build machinery for Arid's current toolchain/dependencies, both architectures fall back to:

```text
manylinux_2_28
```

The selected value becomes one v1.2 release-policy constant.

## **7.3 Pin release tooling**

Release builds stop using the floating:

```text
PyO3/maturin-action@v1
```

and use:

```text
PyO3/maturin-action@<exact commit SHA>  # release tag in comment
maturin-version: v<exact version>
```

The exact pins are proven during implementation and then frozen through the v1.2 RC/stable cycle.

`--compatibility pypi` remains enabled.

## **7.4 One Linux binary build**

Arid uses maturin `bindings = "bin"`, so the compiled executable is packaged in the wheel's scripts area.

For Linux, the standalone archive SHOULD use that exact binary:

```text
maturin manylinux build
    ↓
wheel
    ↓
locate exactly one .data/scripts/arid entry
    ↓
extract + chmod
    ↓
standalone smoke
    ↓
package arid-linux-<arch>.tar.gz
```

This ensures the wheel and standalone archive contain the same executable bits and therefore share the same glibc compatibility floor.

The step fails if the expected script is missing or ambiguous.

Linux MUST NOT build a second standalone executable on the host runner, because that could silently inherit the host's newer glibc floor.

macOS and Windows MAY retain their current separate wheel and Cargo builds.

---

# **8. Published Linux ARM64 Verification**

The local full qualification/benchmark harness remains Linux x86_64.

The production release workflow adds a final native ARM64 job, conceptually:

```text
Verify published Linux aarch64
```

It runs only after both PyPI and GitHub publication succeed and:

1. checks out the exact tag
2. waits for the exact PyPI release to become visible
3. installs `arid==<exact version>` in a clean environment
4. verifies `--version` and `--help`
5. downloads `arid-linux-aarch64.tar.gz` from the GitHub release
6. extracts it and verifies `--version` and `--help`

The production Release workflow is not successful until this job succeeds.

For v1.2 qualification, `qualification/run.sh` verifies both overall workflow success and the presence/success of this expected ARM64 verification job.

This avoids adding QEMU or cross-platform pip simulation to the local qualification harness.

---

# **9. Published JSON Schemas**

Add:

```text
schemas/report-v3.schema.json
schemas/baseline-v1.schema.json
```

Both SHOULD use JSON Schema Draft 2020-12 and include:

```json
"$schema": "https://json-schema.org/draft/2020-12/schema"
```

No network-dependent `$id` is required.

## **9.1 Report v3**

The schema mirrors the current serialized `Report`, `Finding`, and `Location` exactly, including:

```text
version = 3
context = declarative | executable | mixed
scope = module | class | function | mixed
distribution = same-file | cross-file | mixed
location.source = optional
```

It SHOULD reject undeclared object properties.

## **9.2 Baseline v1**

The schema mirrors the existing baseline file exactly, including:

```text
version = 1
normalization snapshot
sha256:<64 lowercase hex> fingerprint
effective line count
project-relative path
positive per-path occurrence count
```

The schema describes structure only; baseline acceptance semantics remain implemented by Arid.

## **9.3 Validation**

Schema validation belongs to test/validation tooling, not the production binary.

Representative serialized Arid documents are validated against the committed schemas with a pinned development/validation-only standards validator.

No JSON Schema runtime dependency is added to Arid.

Released schema files are historical artifacts and are never rewritten to describe an incompatible future contract.

---

# **10. PyPI Readiness Helper**

Use one small helper:

```text
qualification/pypi_ready.py
```

It uses only the Python standard library and checks:

```text
https://pypi.org/pypi/arid/<version>/json
```

Behavior:

```text
200              → success immediately
404              → retry while inside bound
other HTTP error → fail immediately
network error    → fail immediately
```

Only the reproduced "upload succeeded but exact release is not visible yet" condition is retried.

The default bound SHOULD be approximately two minutes with a short fixed retry interval.

The helper is reused by:

- local release qualification
- published Linux ARM64 verification

Its tests are isolated from real PyPI and cover:

```text
200 immediately
404 then 200
404 until timeout
non-404 HTTP failure
network/protocol failure
```

Only minimal seams for fake request/clock/sleep behavior should be introduced.

---

# **11. Qualification Delta**

V1.2 RC qualification retains all existing gates and adds:

```text
exact PyPI readiness before pip install
expected Linux aarch64 GitHub release asset
successful native published-aarch64 workflow job
v1.2 targeted integration validation
published report/baseline schemas
selected Linux compatibility policy evidence
```

The full real-world validation and benchmark campaign remains x86_64 and uses the exact published x86_64 artifact.

Stable promotion remains metadata-only after a fully qualified RC.

---

# **12. Targeted V1.2 Validation**

Add:

```text
validation/v1.2.sh
```

It composes existing validation instead of duplicating the full campaign.

Targeted checks:

1. `--workers auto` parses and runs
2. default/serial, numeric parallel, and auto outputs agree
3. auto resolution stays within `1..=4`
4. report schema accepts representative report v3 output
5. baseline schema accepts a generated baseline v1 document
6. representative invalid schema documents are rejected
7. PyPI readiness helper unit tests pass

Linux architecture/compatibility is validated by the production release workflow and qualification path rather than simulated here.

---

# **13. Compatibility Boundary**

V1.2 does not change:

```text
DUP001
report schema v3
baseline schema v1
text / Markdown / SARIF meaning
default serial execution
numeric --workers N
[tool.arid]
exit statuses 0 / 1 / 2
```

The following remain 2.0 work:

```text
FindingDistribution::Mixed → Hybrid
"mixed" distribution → "hybrid"
Report.version → schema_version
report schema v4
public Rust API boundary cleanup
possible automatic-parallel default
first-class finding fingerprints
baseline pruning
```

V1.2 MUST NOT opportunistically start those migrations.

---

# **14. Implementation Order**

Proceed in this order:

```text
1. prove Linux compatibility floor + ARM64 build path
2. implement --workers auto + deterministic tests
3. add report/baseline schemas + validation
4. add PyPI readiness helper + tests
5. harden release workflow + native published ARM64 verification
6. extend qualification evidence
7. profile current performance and apply only justified optimizations
8. extend benchmark/targeted validation
9. reconcile documentation
10. release candidate qualification
```

The portability proof comes first because it can invalidate packaging assumptions without touching product code.

Optimization comes after profiling, never before it.

---

# **15. Completion Criterion**

V1.2 architecture is complete when the implementation can truthfully state:

> **The same deterministic exact-duplicate pipeline remains intact; serial execution is still the default; `--workers auto` resolves conservatively without changing findings; Linux x86_64 and aarch64 artifacts share an explicit compatibility policy; published machine contracts have formal schemas; and qualification distinguishes normal PyPI publication delay from a broken release.**
