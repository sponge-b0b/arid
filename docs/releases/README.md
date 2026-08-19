# Arid release notes

Every published Arid release must have curated, version-controlled release notes.

Release notes are stored as:

```text
docs/releases/<git-tag>.md
```

For example:

```text
docs/releases/v1.2.0.md
docs/releases/v1.3.0-rc.1.md
```

The production Release workflow requires the exact file for the tag being published and uses it as the GitHub Release body. GitHub-generated notes are not the canonical release notes.

## Release-note depth

### Stable major and minor releases

Include:

- release title, version, and date
- a short release theme and user-facing summary
- major features and enhancements
- meaningful fixes or release-hardening changes when relevant
- performance results when they are part of the release value
- compatibility or breaking-change guidance
- install or upgrade commands
- links to relevant documentation

### Stable patch releases

Keep the same structure, but focus on the fixes, affected users, compatibility, and upgrade guidance. Do not pad a patch release with empty sections.

### Release candidates

Use concise curated notes that identify what is frozen, the major changes being qualified, compatibility expectations, and what users should test.

### Alpha and beta releases

Short engineering-oriented notes are sufficient, but the notes must still be curated and explain the purpose of the prerelease and its meaningful changes.

## Stable-promotion rule

Arid stable promotion is metadata-only from the qualified release candidate. Therefore, the stable release-notes file must be present and substantively complete before the RC is tagged and qualified.

After RC qualification, stable release notes may receive only changes that are part of an otherwise permitted RC fix. Any source-tree change that affects the qualified RC lineage must follow the normal requalification rules.

The stable promotion itself must not introduce a new release-notes file or rewrite release content merely for publication.

## Style

Write for Arid users rather than for the implementation history:

- explain what changed and why it matters
- prefer commands and concrete examples over internal architecture details
- call out defaults and compatibility explicitly
- keep performance claims tied to qualified benchmark evidence
- do not list every commit
- do not invent a security section when there is no security-specific change
- use screenshots or terminal captures only when they communicate something better than copyable text

## Historical notes

Curated stable notes were backfilled for releases that predate this policy:

- `v1.0.0`
- `v1.1.0`
- `v1.2.0`

Those files document the shipped releases but were added to the repository after the corresponding immutable tags were published.
