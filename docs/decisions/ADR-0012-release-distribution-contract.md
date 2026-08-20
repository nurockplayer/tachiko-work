# ADR-0012: Tag-gated release distribution

## Status

Accepted

## Context

Tachiko Work now provides a complete first-user game-balance workflow and typed
semantic collaboration, but the repository cannot yet produce a trustworthy
external release. Workspace crates are not packageable because internal path
dependencies omit versions, legal files are absent, the declared Rust 1.85
minimum is not enforced in CI, and there is no reproducible binary artifact or
release procedure.

Shipping directly from a developer checkout would make installation,
provenance, rollback, and support dependent on undocumented local state. At the
same time, automatically publishing crates or a public GitHub release from an
ordinary branch build would grant CI more release authority than the product
owner intended.

## Alternatives considered

### Keep source installation as the only distribution

This has the smallest automation surface, but asks every first user to install
the Rust toolchain and build the entire workspace. It also leaves no checksummed
release unit for testers or downstream automation.

### Publish every merge to a rolling release channel

Rolling releases are convenient for internal dogfooding, but they weaken the
meaning of version `0.1.0`, complicate support, and turn branch pushes into
public distribution events.

### Publish crates and binaries from an explicit version tag

A human-created tag is a clear release authorization boundary. CI can validate
the version, build platform artifacts, and prepare a draft GitHub release while
leaving the final publication decision visible and reversible. This is the
selected approach for binary distribution. Crate archives are validated now,
but crates.io publication remains a separate manual decision until namespace
ownership and publication order are confirmed.

## Decision

Tachiko Work adopts a tag-gated distribution contract:

- Workspace version and minimum supported Rust version remain canonical in the
  root `Cargo.toml`.
- Every workspace crate includes complete package metadata and versioned path
  dependencies so `cargo package --workspace --no-verify` can build source
  archives without changing local development resolution.
- The repository includes the Apache-2.0 and MIT license texts, a changelog,
  security policy, contribution guide, and executable release checklist.
- Stable CI runs the full quality and product workflow. A separate exact Rust
  1.85 job proves the declared minimum against all targets with the lockfile.
- The complete local gate requires rustup, selects installed stable Rust for
  all ordinary and nested commands, and separately invokes exact Rust 1.85.0
  for the MSRV check. It does not depend on the caller's default toolchain.
- A release tag must exactly equal `v` plus the workspace version. Tag builds
  produce native archives for Linux x86-64, macOS arm64, macOS x86-64, and
  Windows x86-64.
- Each archive contains the `tachiko` executable, README, changelog, both
  Tachiko licenses, and a generated `THIRD_PARTY_LICENSES.md`, and has an
  adjacent SHA-256 checksum. The notice deterministically inventories the
  locked all-target normal `tachiko-cli` dependency closure, including
  platform-specific and proc-macro packages, and preserves every discovered
  vendored license/notice text. CI extracts and executes the native archive
  before accepting it.
- The tag workflow uses only an already-existing tag and creates a **draft**
  GitHub release. It never publishes the draft automatically and never invokes
  `cargo publish`.
- Ordinary pushes and pull requests cannot write repository contents or create
  releases.

The first external release is `0.1.0`. Future version changes must update the
changelog and pass the same contract. Signing, attestations, package-manager
distribution, and crates.io publication can extend this boundary without
changing the semantic product architecture.

## Consequences

Positive:

- first users can install a small, checksummed native archive;
- the declared MSRV becomes tested product behavior instead of documentation;
- source packages and binary artifacts are reproducible from one versioned
  repository state;
- dependency attribution is audited against Cargo's lockfile, dependency tree,
  and vendored source instead of relying on a hand-maintained legal summary;
- a tag authorizes release preparation while final publication remains a
  deliberate review step;
- CI credentials remain read-only outside the tag-gated release job.

Negative:

- four native runner jobs increase tagged-build time;
- the initial archives are not code-signed or notarized, so platform security
  prompts may still require an explicit user action;
- crates.io consumers must wait for a later namespace and publication decision;
- release publication still includes a manual draft review.
