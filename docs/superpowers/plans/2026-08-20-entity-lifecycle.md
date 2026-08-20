# Semantic Entity Lifecycle Implementation Plan

> Execute task-by-task with test-first behavioral seams and reconcile the live
> repository before each downstream integration step.

**Goal:** Ship safe duplicate, rename, and remove operations for the first
game-balance roster workflow.

**Architecture:** Expose the semantic identifier grammar from
`tachiko-semantic-core`, implement immutable lifecycle transformations in the
UI-independent workflow crate, then keep CLI persistence as a thin canonical,
exclusive-create adapter.

**Tech stack:** Rust 2024, existing workspace crates, Clap, Thiserror,
canonical `.ro` storage, Bash smoke contracts, GitHub Actions.

## Task 1: Workflow lifecycle contract

**Files:**

- Modify: `crates/semantic-core/src/validation.rs`
- Modify: `crates/semantic-core/src/lib.rs`
- Modify: `crates/semantic-core/tests/validation.rs`
- Modify: `crates/workflow/src/lib.rs`
- Create: `crates/workflow/tests/entity_lifecycle.rs`

- [x] Add failing tests for the public identifier predicate and all duplicate,
  rename, and remove rules.
- [x] Implement the minimal semantic transformations, recursive reference
  rewrite/scan, explicit errors, and shared validate/calculate/diff finalizer.
- [x] Run focused semantic-core and workflow tests, formatting, and Clippy.
- [x] Review the shared validation contract's direct consumers.

## Task 2: CLI authoring surface

**Files:**

- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

- [x] Add failing process tests for nested help, each successful operation,
  output protection, and dependency-error no-write behavior.
- [x] Add `tachiko entity duplicate|rename|remove` and one shared lifecycle
  persistence adapter.
- [x] Run focused CLI tests, formatting, and Clippy.

## Task 3: First-user roster journey

**Files:**

- Create: `scripts/entity-lifecycle-smoke.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release-check.sh`
- Modify: `README.md`
- Modify: `examples/game-balance/README.md`
- Modify: `CHANGELOG.md`

- [x] Document a copy-tune-rename-review workflow using the canonical starter.
- [x] Add a real smoke script covering successful lifecycle, safe referenced
  removal refusal, and successful unreferenced removal.
- [x] Require the smoke in ordinary CI and the local release gate.
- [x] Run all product smokes twice and inspect user-facing output.

## Task 4: Release verification and review

- [ ] Run `bash scripts/release-check.sh` from the reconciled branch.
- [ ] Independently review the complete phase against ADR-0013, repository
  rules, product ergonomics, safety, deterministic behavior, and regressions.
- [ ] Fix every actionable P0-P2 finding with regression coverage and rerun the
  relevant gate.
- [ ] Record exact evidence and remaining boundaries in j-space, then create a
  clean product checkpoint without tagging, pushing, or publishing.
