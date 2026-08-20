# Contributing to Tachiko Work

Tachiko Work is a semantic-first, computational, Git-native workspace. Keep
changes focused on a concrete user or architecture need and follow existing
crate boundaries before introducing new abstractions.

## Public pre-alpha contribution status

The repository is intentionally public, but the project is still pre-alpha and
the long-term licensing/contributor model is being decided in GitHub issue #15.

External implementation/code pull requests are temporarily not accepted while
that decision remains open. Issue reports, architecture/product discussion,
review findings, and documentation feedback are welcome.

This avoids accidentally constraining future licensing choices before the
project decides its CLA/DCO/copyright model. Revisions already published under
`Apache-2.0 OR MIT` remain available under those historical grants. See
[`docs/governance/licensing-posture.md`](docs/governance/licensing-posture.md)
for the current provisional direction.

## Set up the toolchains

Install stable Rust for development and the exact minimum version for
compatibility checks:

```sh
rustup toolchain install stable --profile minimal
rustup component add --toolchain stable rustfmt clippy
rustup toolchain install 1.85.0 --profile minimal
rustup run 1.85.0 cargo check --workspace --all-targets --locked
```

Use the checked-in `Cargo.lock`. The project does not require a global install;
run the CLI with `cargo run -p tachiko-cli -- <command>` while developing.

## Work in focused loops

Run the smallest affected crate or test while iterating:

```sh
cargo test -p tachiko-semantic-core --locked
cargo clippy -p tachiko-semantic-core --all-targets --locked -- -D warnings
```

Replace the package name with the crate you changed. Add regression coverage
for behavior changes and prove the failure before the implementation when
practical. Deterministic serialization, formula results, semantic diffs,
conflict order, and diagnostics need exact assertions.

Before requesting review, run the fast workspace gate:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

Then make a clean local commit and run the complete release-equivalent gate on
a supported native target (the commit does not need to be pushed):

```sh
bash scripts/release-check.sh
```

The clean commit is required because Cargo source packaging rejects dirty
package inputs. The full gate selects stable for bare and nested Rust commands,
regardless of an inherited `RUSTUP_TOOLCHAIN`, and separately checks exact Rust
1.85 compatibility. It also checks documentation, deterministic audited
dependency notices, Cargo packages, executable product journeys, and a native
release archive. Do not claim a change is ready when a relevant gate is
skipped; state the exact limitation in the pull request.

## Preserve the product contract

- **Semantic first:** schemas, entities, typed values, references, and formulas
  are the source of truth. Views and integrations are projections or adapters,
  not competing document models.
- **Computational and deterministic:** equivalent inputs must produce stable
  canonical data, calculation, diff, merge, and export results.
- **Git native:** make semantic changes reviewable in canonical files. Do not
  silently configure Git, hide conflicts, or treat raw text merge as semantic
  reconciliation.
- **No overwrite:** document-changing CLI operations create a distinct output
  and fail closed if it exists. Preserve this boundary in new workflows.
- **AI requires approval:** AI-facing APIs may read, explain, and propose
  validated operations. They must not mutate user data or bypass explicit human
  approval.
- **Game development first:** strengthen the real game-balance workflow without
  drifting into an Office compatibility clone.

Before changing a shared type, wire format, CLI contract, or release policy,
scan its direct consumers and tests and verify downstream compatibility.

## Tests, decisions, and changelog

- Put unit behavior beside the owning crate and process-level workflows in CLI
  integration tests or executable smoke scripts.
- Add or update an ADR for a durable architecture, data-model, compatibility,
  security, or release-authority decision. Small implementation choices do not
  need an ADR.
- Update specifications and examples when their promised behavior changes.
- Update `CHANGELOG.md` for user-visible behavior, migrations, compatibility,
  security, or distribution changes. A behavior-preserving internal refactor
  does not need a changelog entry.
- Regenerate `THIRD_PARTY_LICENSES.md` after any `Cargo.lock` or runtime
  dependency change with `bash scripts/generate-third-party-licenses.sh > THIRD_PARTY_LICENSES.md`;
  the full gate rejects stale output.

## Pull requests

Keep each pull request independently reviewable. Lead with the user-visible or
architectural outcome, list locked decisions and risks, and report the exact
commands and results used for verification. Include migration or compatibility
notes when applicable. Avoid unrelated cleanup, generated noise, and narrative
that duplicates an issue, ADR, or discoverable repository state.

Security reports do not belong in ordinary pull requests or public issues. See
[`SECURITY.md`](SECURITY.md) before sharing vulnerability details.
