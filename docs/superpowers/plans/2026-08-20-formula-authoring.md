# Computational Formula Authoring Implementation Plan

> Execute task-by-task with test-first behavioral seams and reconcile live
> consumers before changing shared expression formatting or AI suggestions.

**Goal:** Ship deterministic, validated formula creation and editing for game
balance documents.

**Architecture:** Add a bounded parser and canonical formatter to the existing
formula engine, reuse them from workflow/diff rendering, add one immutable
workflow edit, keep CLI persistence thin, and extend the inert AI suggestion
contract to typed formulas.

**Tech stack:** Rust 2024, existing semantic/formula/diff/workflow/AI/CLI crates,
Clap, Thiserror, canonical `.ro`, Bash product smoke contracts.

## Task 1: Formula language

**Files:**

- Modify: `crates/formula-engine/src/lib.rs`
- Create: `crates/formula-engine/src/parser.rs`
- Create: `crates/formula-engine/tests/expression_parser.rs`

- [ ] Add failing tests for grammar, AST mapping, canonical round trips, stable
  diagnostics, and all three resource limits.
- [ ] Implement the minimal parser, formatter, error type, and limits.
- [ ] Run focused tests, formatting, warnings-as-errors Clippy, and exact Rust
  1.85 checking for the formula engine.

## Task 2: Validated workflow and shared rendering

**Files:**

- Modify: `crates/workflow/src/lib.rs`
- Modify: `crates/workflow/tests/explain_and_edit.rs`
- Modify: `crates/diff-engine/src/lib.rs`
- Modify: `crates/diff-engine/tests/semantic_diff.rs`

- [ ] Add failing tests for successful numeric/formula edits and every parse,
  target, validation, calculation, no-op, and immutability failure.
- [ ] Implement `set_formula` through the existing edit finalizer.
- [ ] Replace duplicate expression renderers with the formula engine's
  canonical formatter and update copy/paste output expectations.
- [ ] Run focused workflow/diff tests and consumer checks.

## Task 3: AI and CLI adapters

**Files:**

- Modify: `crates/ai-api/src/lib.rs`
- Modify: `crates/ai-api/tests/semantic_queries.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

- [ ] Add red AI tests for inert formula proposals and formula-to-scalar refusal.
- [ ] Allow typed formula suggestions only for numeric schema fields while
  preserving approval, candidate validation, calculation, and no-write behavior.
- [ ] Add red CLI tests for discoverability, success, semantic failures, and
  output preservation, then implement `tachiko formula set`.
- [ ] Run focused AI/CLI tests, formatting, and warnings-as-errors Clippy.

## Task 4: Computational-authoring journey

**Files:**

- Create: `scripts/formula-authoring-smoke.sh`
- Modify: `scripts/first-user-smoke.sh`
- Modify: `scripts/entity-lifecycle-smoke.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release-check.sh`
- Modify: `README.md`
- Modify: `examples/game-balance/README.md`
- Modify: `CHANGELOG.md`

- [ ] Update every displayed formula to canonical bracketed reference syntax.
- [ ] Document and smoke a formula edit that produces 45 DPS and deterministic
  repeated canonical output.
- [ ] Assert parse, missing-reference, and cycle failures write no output.
- [ ] Require the new journey in ordinary CI and the local release gate.
- [ ] Run all product smokes twice and inspect user-facing output.

## Task 5: Release verification and review

- [ ] Run the full release-equivalent gate from the reconciled branch.
- [ ] Independently review the fixed phase diff for grammar safety, semantic
  correctness, AI approval behavior, CLI persistence, docs, CI, and regressions.
- [ ] Fix every actionable P0-P2 issue with focused regression evidence.
- [ ] Record exact evidence in j-space and create a clean checkpoint without any
  tag, push, registry publication, draft release, or visibility change.
