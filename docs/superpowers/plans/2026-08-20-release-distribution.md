# Release Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Tachiko Work produce validated source packages and checksummed native CLI archives through a safe, tag-gated draft-release workflow.

**Architecture:** Cargo workspace metadata defines the source distribution contract. Repository shell scripts own deterministic native archive creation and verification. Stable/MSRV CI proves the code and user workflows; an exact version tag authorizes a separate matrix workflow to prepare, but not publish, a GitHub release.

**Tech Stack:** Rust 2024/MSRV 1.85, Cargo packaging, portable Bash, GitHub Actions, GitHub CLI.

**Spec:** `docs/superpowers/specs/2026-08-20-release-distribution.md`

## Global constraints

- Do not push tags, publish crates, create a GitHub release, or broaden ordinary
  CI write permissions.
- Preserve local path development while adding registry-compatible internal
  dependency versions.
- Release archives must be native-tested and include both license texts.
- Use explicit supported runner labels rather than moving `-latest` aliases.
- Use official GitHub artifact actions and `gh release create --verify-tag`.
- All new executable behavior follows a demonstrated red/green test cycle.

---

### Task 1: Package and legal contract

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/*/Cargo.toml`
- Create: `LICENSE-APACHE`
- Create: `LICENSE-MIT`
- Create: `CHANGELOG.md`

- [x] Record the current failing `cargo package --workspace --allow-dirty
  --locked --no-verify` output for missing dependency versions.
- [x] Add shared workspace package metadata and a specific description for each
  crate.
- [x] Add `version = "0.1.0"` beside every internal `path` dependency,
  including development dependencies.
- [x] Add canonical Apache-2.0 and MIT texts plus a `0.1.0` changelog based only
  on implemented product behavior.
- [x] Run source packaging and inspect every archive for the inherited README,
  SPDX license expression, and repository metadata; both canonical license
  texts remain mandatory in the repository and native binary archives.
- [x] Commit as `build: make workspace crates packageable`.

### Task 2: Reproducible native archive scripts

**Files:**
- Create: `scripts/package-binary.sh`
- Create: `scripts/verify-release-archive.sh`
- Create: `scripts/release-check.sh`

- [x] Invoke the absent archive script and record the expected red failure.
- [x] Implement deterministic naming, required payload checks, isolated staging,
  `tar.gz` creation, and portable SHA-256 generation.
- [x] Implement archive verification: checksum, exact member payload, native
  `tachiko --version`, and version/name agreement.
- [x] Add one local release-check entry point for formatting, clippy, tests,
  docs, exact MSRV, Cargo packages, both product smokes, and native archive
  smoke.
- [x] Run focused scripts on the current native host and commit as
  `build: add reproducible release artifacts`.

### Task 3: CI and tag-gated draft release

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

- [x] Update official checkout/artifact action majors from current upstream
  documentation.
- [x] Add exact Rust 1.85 and source-package gates to normal CI while preserving
  read-only permissions.
- [x] Add tag validation, four native build/archive/verification jobs, artifact
  aggregation, and draft release creation with `--verify-tag`.
- [x] Validate workflow syntax and statically assert that only the release job
  has `contents: write`, no workflow invokes `cargo publish`, and draft creation
  is mandatory.
- [x] Commit as `ci: prepare tag-gated draft releases`.

### Task 4: External-user and release-owner documentation

**Files:**
- Modify: `README.md`
- Rewrite: `docs/governance/release-process.md`
- Create: `CONTRIBUTING.md`
- Create: `SECURITY.md`
- Modify: `.jspace/WORKSPACE.md`

- [x] Document binary checksum/install steps, source installation, supported
  targets, and the current unsigned-binary limitation.
- [x] Replace the generic release outline with the exact local validation,
  version/tag, draft review, publication, and rollback procedure.
- [x] Add concise contribution quality gates and a private vulnerability
  reporting policy that does not invent an unavailable contact address.
- [x] Record completed phase decisions and verification in j-space.
- [x] Commit as `docs: document release and support workflow`.

### Task 5: Release verification and independent review

**Files:**
- Modify only files needed to address verified findings.

- [ ] Run `bash scripts/release-check.sh` from a clean branch checkout.
- [ ] Independently review the implementation against ADR-0012, the design,
  security boundaries, Cargo packaging, and first-user install ergonomics.
- [ ] Fix every actionable finding and rerun the smallest relevant gate followed
  by the full release check.
- [ ] Update this plan and j-space with exact outcome evidence.
- [ ] Commit the verified checkpoint as
  `build: ship release distribution contract`.
