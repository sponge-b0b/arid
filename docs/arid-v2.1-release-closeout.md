# Arid v2.1 Release Closeout

**Product:** Arid  
**Stable release:** `2.1.0`  
**Stable tag:** `v2.1.0`  
**Released commit:** `5cafaa2ba1acc5738b7b3542e348fa7f3f4c8ad9`  
**Status:** Complete

## Outcome

Arid 2.1 shipped successfully and the release line is closed.

The final release preserves the Arid 2.0 detector while adding auditable suppression maintenance, stale-maintenance enforcement, targeted discovery explanation, ignore-file traversal override, deterministic administrative machine contracts, and a concise human-facing total-time footer.

No unresolved v2.1 product work remains.

## Actual release path

The release used the following prerelease path:

```text
v2.1.0-alpha.1
        ↓
v2.1.0-beta.1
        ↓
v2.1.0
```

No release candidate was published.

The published beta completed the full stabilization program without requiring any product, packaging, compatibility, deterministic-output, machine-contract, or detector-semantic correction. An unchanged RC would not have reduced a distinct remaining release risk, so the qualified beta proceeded directly to stable-readiness qualification and stable publication.

## Phase 11 — Stable promotion

**Status:** Complete.  
**Gate result:** PASS.

Stable preparation changed only expected release metadata/documentation state:

- `Cargo.toml`: `2.1.0-beta.1` → `2.1.0`
- `Cargo.lock`: `2.1.0-beta.1` → `2.1.0`
- `pyproject.toml`: Beta → Production/Stable
- `action.yml`: `2.1.0b1` → `2.1.0`
- `README.md`: Beta project status → Stable
- `docs/arid-v2.1-release-roadmap.md`: stable release state

No `src/`, schema, validation, benchmark, or detector-behavior change was introduced by stable promotion.

### Stable pre-tag qualification

The exact stable candidate passed:

- release metadata validation
- metadata-only promotion invariant checks
- `cargo test --locked`
- Clippy with `-D warnings`
- release build
- stable binary identity check
- full `validation/v2.1.sh`
- diff hygiene
- clean-tree verification

The exact candidate was then pushed and the non-publishing production release workflow passed across all supported build targets:

- Linux x86-64
- Linux aarch64
- macOS arm64
- macOS x86-64
- Windows x86-64

Publication jobs remained skipped during that qualification run.

### Stable publication

The annotated `v2.1.0` tag resolves to the exact qualified commit:

```text
5cafaa2ba1acc5738b7b3542e348fa7f3f4c8ad9
```

The tag-triggered production release workflow passed, including PyPI and GitHub release publication.

GitHub release `v2.1.0` was published as a stable release with all five standalone assets:

```text
arid-linux-aarch64.tar.gz
arid-linux-x86_64.tar.gz
arid-macos-aarch64.tar.gz
arid-macos-x86_64.tar.gz
arid-windows-x86_64.zip
```

Published asset SHA-256 digests recorded at closeout:

```text
linux-aarch64   c1aed9293e5ca0300277c343713b3d6f568453da3f0ad875450141d0b09d522b
linux-x86_64    b18f7286bdfed02d366b4fd422da3f219a62899383442c753a9fddc1a1c77da7
macos-aarch64   71f353f87b7cf2baabbd05968ed5d6a9abf340f40d30d31fccd67b7c50231054
macos-x86_64    7fd921a0e15ed7db3750e5a5006d5e8d5d710be3a5dc025f62309d21d8cd76f5
windows-x86_64  86085d23122c3b342f213ea2c5b3c20d84eecddcaefe4861519891b707ddae6f
```

### Published-artifact qualification

Both stable distribution paths passed the complete durable v2.1 contract suite:

- exact published GitHub Linux x86-64 standalone: `arid 2.1.0` — PASS
- fresh PyPI `arid==2.1.0` installation: `arid 2.1.0` — PASS
- `validation/v2.1.sh` against the standalone executable — PASS
- `validation/v2.1.sh` against the PyPI executable — PASS
- release metadata after publication — PASS
- repository cleanliness after qualification — PASS

This confirms the actually distributed stable executables, not merely a local build.

## Phase 12 — Closeout

**Status:** Complete.  
**Gate result:** PASS.

### Repository state

After published-artifact qualification, `main` was fast-forwarded directly to the released v2.1 history with no merge commit.

At integration time the following refs all resolved to the released commit:

```text
main
origin/main
v2.1
origin/v2.1
v2.1.0
        ↓
5cafaa2ba1acc5738b7b3542e348fa7f3f4c8ad9
```

The fast-forward preserved the exact qualified release history.

The stable tag remains immutable at the shipped commit. This closeout document is intentionally a post-release documentation commit on `main` and does not alter `v2.1.0`.

### Documentation state

- `README.md` documents the complete v2.1 product surface and reports Stable project status.
- `docs/releases/v2.1.0.md` contains the curated stable release notes.
- `docs/arid-v2.1-requirements-specification.md` remains the frozen requirements contract.
- `docs/arid-v2.1-technical-architecture-and-design.md` remains the frozen architecture contract.
- `docs/arid-v2.1-release-roadmap.md` records the phased delivery and qualification history.
- This document records the final post-publication outcome and repository closeout evidence.

## Final beta stabilization evidence

The qualified published beta established the final product evidence used for stable readiness:

- exact published standalone and PyPI beta executables passed `validation/v2.1.sh`
- Black, Django, mypy, Rich, malformed-source, path, and worker-determinism real-world validation passed
- a real Polaris duplicate wrapped by suppression classified `active`
- a deliberately unnecessary disable-through-EOF suppression classified `stale`
- suppression JSON was byte-deterministic across workers 1 and 4
- `--fail-on-stale` enforced stale maintenance correctly
- an ignored Python path explained as `ignore-file`
- `--no-ignore-files` made that path includable
- `--explain-path` agreed with actual `--list-files` traversal
- normal text produced exactly one valid `Total time:` footer
- machine JSON remained timing-free

Published-to-published Linux x86-64 performance versus `2.0.0` showed no measurable regression:

```text
Requests   17.08 ms → 16.57 ms   -3.03%
Pydantic  239.69 ms → 238.80 ms  -0.37%
Polaris   503.95 ms → 503.59 ms  -0.07%
```

## Release-process follow-up

Stable qualification exposed one tooling-reproducibility issue unrelated to Arid 2.1 product behavior.

The local stable environment had:

```text
rustc   1.97.1
cargo   1.97.1
rustfmt 1.9.0-stable
```

`cargo fmt --check` under that newer formatter proposed formatting-only changes to source that was byte-for-byte unchanged from the already qualified published beta. Because applying those changes would have introduced an unnecessary post-beta source delta, stable qualification correctly preserved the qualified source and inherited the earlier formatting evidence.

Future release/process work should pin the Rust/rustfmt toolchain used for formatting checks so `cargo fmt --check` remains reproducible across release stages instead of drifting when the local `stable` toolchain advances.

This is a release-process reproducibility improvement, not an Arid 2.1 product defect and not unfinished v2.1 scope.

## Final release statement

> **Arid 2.1 preserves the 2.0 detector while making suppression maintenance auditable and enforceable, making individual discovery decisions explainable, allowing ignore-file traversal to be bypassed without bypassing Arid policy, exposing the new administrative state through deterministic versioned machine contracts, and making normal human-facing scans visibly fast through a concise total-time footer.**

The v2.1 release completion criterion is satisfied:

```text
normal duplicate semantics remain unchanged
        +
effective suppressions are deterministically active or stale
        +
stale suppression/baseline maintenance can fail CI when requested
        +
one path can be explained against actual discovery policy
        +
ignore-file traversal can be disabled without disabling Arid policy
        +
suppression-status-v1 and path-explanation-v1 are stable and deterministic
        +
completed normal text scans display a concise Total time footer
        +
machine-readable outputs remain free of volatile timing data
        +
published artifacts pass the established release qualification path
```

**Arid 2.1 is complete.**
