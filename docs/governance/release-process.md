# Release Process

This is the release-owner runbook for Tachiko Work. An exact annotated version
tag authorizes CI to prepare a **draft** GitHub release. Publishing that draft is
a separate manual decision. Ordinary pushes and pull requests have no release
authority.

Crates.io publication is deferred. This runbook validates Cargo source archives
but must not run `cargo publish` until namespace ownership, package order, and
the crate-archive license layout have an explicit decision.

## 1. Prepare one reviewed release commit

Start from the intended release commit after review and required CI have passed.
The checkout must be clean, and the commit must contain every file that will be
released:

```sh
git status --short
git rev-parse HEAD
```

`git status --short` must print nothing. Record the commit ID for draft review.
Do not release from a checkout with local fixes, generated assets, or an
unreviewed merge.

For a new version, update all of the following in one reviewed change:

- `[workspace.package].version` in `Cargo.toml`;
- every explicit version on an internal path dependency in `crates/*/Cargo.toml`;
- `Cargo.lock`;
- `CHANGELOG.md`, including the release date, user-visible changes, migrations
  when needed, and the final version link;
- `THIRD_PARTY_LICENSES.md` whenever `Cargo.lock` or the CLI runtime dependency
  closure changes.

After editing manifests, refresh the lockfile with a normal Cargo check, inspect
the diff, commit the version and changelog change, and confirm the checkout is
clean again:

```sh
cargo check --workspace --all-targets
bash scripts/generate-third-party-licenses.sh > THIRD_PARTY_LICENSES.md
git diff --check
git status --short
```

## 2. Run the complete local release gate

Install stable Rust with the formatter and linter plus the exact minimum
toolchain, then run the repository-owned gate from the repository root:

```sh
rustup toolchain install stable --profile minimal
rustup component add --toolchain stable rustfmt clippy
rustup toolchain install 1.85.0 --profile minimal
bash scripts/release-check.sh
```

The command must report the selected stable `rustc` and finish with `release
check passed`. It process-locally selects stable for every bare Cargo/Rust
command and nested smoke/build script, even if `RUSTUP_TOOLCHAIN` named another
installed toolchain on entry. It does not modify the caller's persistent rustup
override. The gate checks formatting, warnings-as-errors Clippy, all tests,
warning-free documentation, exact Rust 1.85 compatibility, deterministic
dependency-license regeneration, all Cargo source packages, the first-user and
collaboration journeys, and a checksummed native archive that is extracted and
executed.

If the gate changes tracked files or exposes a problem, fix and review the
release commit, rerun the complete gate, and start this runbook again from the
new commit. Never tag a merely local success that differs from the reviewed
commit.

## 3. Create and deliberately push the annotated tag

Derive the canonical version using the same helper as CI, then confirm the tag
does not already exist locally or on the remote:

```sh
set -euo pipefail
source scripts/release-lib.sh
version="$(tachiko_workspace_version "$PWD")"
tag="v${version}"
release_commit="$(git rev-parse HEAD)"
local_tag="$(git tag --list "$tag")"
remote_tag="$(git ls-remote --tags origin "refs/tags/$tag")"

test -z "$local_tag"
test -z "$remote_tag"
git tag --annotate "$tag" --message "Tachiko Work ${version}" "$release_commit"
git show --no-patch "$tag"
```

Verify that the tag name is exactly `v${version}` and that it points to the
recorded reviewed commit. Creating the local tag does not authorize remote
distribution.

Only after the release owner deliberately authorizes release preparation, push
that one tag:

```sh
git push origin "refs/tags/$tag"
```

The tag push is the authorization boundary. Do not push it as an experiment or
as a way to test the workflow.

## 4. Review the draft and all eight assets

The tag starts the `Draft release` workflow. Wait for its validation job and
all four native build jobs to pass. The workflow must create an unpublished
draft release for the exact tag with these eight assets:

```text
tachiko-VERSION-x86_64-unknown-linux-gnu.tar.gz
tachiko-VERSION-x86_64-unknown-linux-gnu.tar.gz.sha256
tachiko-VERSION-aarch64-apple-darwin.tar.gz
tachiko-VERSION-aarch64-apple-darwin.tar.gz.sha256
tachiko-VERSION-x86_64-apple-darwin.tar.gz
tachiko-VERSION-x86_64-apple-darwin.tar.gz.sha256
tachiko-VERSION-x86_64-pc-windows-msvc.tar.gz
tachiko-VERSION-x86_64-pc-windows-msvc.tar.gz.sha256
```

Before publication, confirm:

- the draft tag and title match the workspace version and reviewed commit;
- exactly four archives and four adjacent checksums exist, with no extra asset;
- every checksum validates against its archive;
- generated notes accurately reflect `CHANGELOG.md`, including migrations or
  known limitations;
- each archive contains only the executable, README, changelog, both Tachiko
  license texts, and `THIRD_PARTY_LICENSES.md`;
- the checked-in dependency notice matches a fresh locked all-target Cargo
  inventory and retains every vendored license, copying, notice, unlicense, or
  copyright file;
- the release remains a draft and no crates.io package was published.

The initial binaries are unsigned and macOS binaries are not notarized. Keep
that limitation visible in the draft notes.

## 5. Smoke-test clean native machines

Download each archive and checksum from the draft onto a clean machine of its
matching architecture: Linux x86-64, macOS arm64, macOS x86-64, and Windows
x86-64. Do not reuse the repository checkout or its `target` directory.

Follow the checksum and extraction commands in the
[`README`](../../README.md#binary-archives-after-the-first-release). The first
command from every extracted archive must report exactly:

```text
tachiko VERSION
```

Then exercise a minimal user journey with the extracted executable in a fresh
temporary directory:

```sh
tachiko_path="./tachiko-VERSION-TARGET/tachiko" # substitute reviewed values
"$tachiko_path" init balance.ro --title "Release smoke"
"$tachiko_path" show balance.ro
"$tachiko_path" validate balance.ro
"$tachiko_path" calculate balance.ro
```

Use the equivalent PowerShell invocation with `tachiko.exe` on Windows. Confirm
that no Rust toolchain or repository file is needed at runtime. Record the four
target results with the draft review.

## 6. Publish manually and verify externally

After all eight assets, notes, and four clean-machine results are approved, use
GitHub's release interface to publish the existing draft. Do not recreate the
release, replace assets, move the tag, or run `cargo publish`.

The repository is intentionally public during pre-alpha development. Publishing
a source repository and publishing a versioned binary release are separate
authority boundaries: source visibility does not authorize a tag or release.
After publication, verify the release from a logged-out browser or other
unauthenticated client as well as from a normal authenticated GitHub session.

Verify the release page, tag, release notes, all eight downloadable assets, and
checksums. Repeat one README installation on a clean native machine and confirm
the exact `tachiko VERSION` output plus the minimal user journey. Check that the
changelog's version link resolves to this release.

## 7. Fail forward; never replace released identity

A pushed version tag is immutable release identity, even if the draft is not
yet public. Do not delete, move, or reuse that tag. Do not overwrite an archive
or checksum under an existing version, and do not delete and recreate a release
to evade the workflow's existing-release check.

If any check fails before publication, leave the draft unpublished, fix the
problem in a new reviewed commit, increment the patch version, and repeat this
runbook with a new tag. If a defect is found after publication, retain the
original tag and assets, document the defect, and ship a new patch release.
Mark the affected release as not recommended when appropriate, but preserve its
artifacts so published checksums and provenance remain truthful.
