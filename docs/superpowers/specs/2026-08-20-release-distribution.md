# Release Distribution Design

## Product outcome

A release owner can turn one reviewed Tachiko Work commit into installable,
checksummed native CLI archives through an explicit version tag. Contributors
can reproduce every release gate locally, and an ordinary branch build has no
authority to publish anything.

## User installation contract

The first release provides one archive per supported native target:

```text
tachiko-0.1.0-x86_64-unknown-linux-gnu.tar.gz
tachiko-0.1.0-aarch64-apple-darwin.tar.gz
tachiko-0.1.0-x86_64-apple-darwin.tar.gz
tachiko-0.1.0-x86_64-pc-windows-msvc.tar.gz
```

Each archive expands to a same-named directory containing the executable,
`README.md`, `CHANGELOG.md`, `LICENSE-APACHE`, and `LICENSE-MIT`. The adjacent
`.sha256` file records a portable SHA-256 digest using only the archive filename.
Running `tachiko --version` from an extracted native archive is the executable
acceptance test.

Source installation remains supported with `cargo install --path crates/cli
--locked`. Workspace crates must also produce valid Cargo source archives, but
this phase does not publish them to crates.io.

## Release authority and state machine

```text
branch/PR -> read-only CI -> reviewed commit
                              |
                              v
                     existing v0.1.0 tag
                              |
                              v
                validate -> build -> verify archives
                              |
                              v
                    draft GitHub release
                              |
                              v
                   human publish decision
```

The workflow accepts only tags whose name is exactly `v${workspace_version}`.
`gh release create --verify-tag --draft` must fail rather than create or publish
missing release state. The release job alone receives `contents: write`; build
jobs remain read-only. A repeated run must not silently replace an existing
release.

## Package metadata

The root workspace owns shared version, edition, license, authors, repository,
homepage, README, keywords, categories, and MSRV metadata. Each crate owns a
specific description. Every internal dependency declares both its existing
relative `path` and the compatible workspace `version`; Cargo continues to use
the path locally while packaged manifests remain resolvable.

The package gate uses `cargo package --workspace --locked --no-verify` because
workspace packages depend on sibling archives that have not been published to
a registry. Compilation is independently proved by stable and exact-MSRV
workspace builds. Each source archive must inherit the root README and retain
the SPDX license expression plus repository metadata. Cargo has a singular
`license-file` field that is used instead of the SPDX expression, so exact
copies of both license texts are required in the repository and binary
archives; the final crate-archive license layout is decided with the deferred
crates.io publication boundary.

## Automation

Stable CI runs formatting, warnings-as-errors Clippy, all tests, warning-free
documentation, source packaging, and both executable user journeys. The exact
Rust 1.85 job runs `cargo check --workspace --all-targets --locked`.

The tag workflow uses fixed native GitHub-hosted runner labels and current
official artifact actions. Each matrix job installs stable Rust, builds the
explicit target with the lockfile, calls the repository packaging script,
verifies the archive on that native runner, and uploads only the archive and
checksum. The final job downloads all artifacts and creates a draft release
with generated notes.

## Failure behavior

- A tag/version mismatch stops before any build or release write.
- A missing binary, legal file, checksum tool, or malformed archive fails its
  matrix job.
- A checksum mismatch or non-running native executable fails before upload.
- An existing release makes creation fail; assets are never overwritten by the
  workflow.
- No release is created when any target or quality gate fails.

## Deferred

- crates.io publication and namespace ownership
- signing, notarization, provenance attestations, and SBOM publication
- Homebrew, WinGet, cargo-binstall, or game-engine package registries
- automatic promotion from draft to published release
- updater behavior inside the CLI
