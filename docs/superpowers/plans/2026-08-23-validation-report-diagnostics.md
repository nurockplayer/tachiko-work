# ADR-0019 Validation Report and Semantic Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Implement Issue #89 as the first authoritative first-party semantic ValidationReport over the ADR-0018 full formula failure oracle, while preserving ADR-0016 layering and keeping operation-specific projection/output gates distinct from semantic validity.

**Architecture:** semantic-core owns only generic diagnostic, location, stable-subject, fact, severity, and opaque provider primitives plus its existing core validation rules. formula-engine owns the #90-implemented deterministic node-keyed calculation outcome and retains calculate only as a compatibility projection of that outcome. workspace-engine composes core semantic diagnostics and formula failures into the single authoritative ValidationReport, while finalizers apply any additional projection/output preflight as explicitly separate gates. Adapters render or transport the report without becoming semantic authorities.

**Tech Stack:** Rust 2024, Cargo workspace, Bash/Node verification scripts, Rust 1.85 MSRV, wasm32-unknown-unknown.

**Spec:** GitHub Issue #89; Accepted ADR-0015 through ADR-0019; docs/specs/validation-engine.md, diagnostics-contract.md, formula-engine-spec.md, and schema-system.md; docs/governance/knowledge-authority.md and canonical-reconciliation-register.md.

## Global constraints

- Start from origin/main 342f69f2fc252554c240650d1438cc0d6cd82e2f on branch codex/issue-89-validation-report; reconcile later mainline changes before handoff. The branch consumed #97's #90-owned ADR-0018 oracle at 6ad364755566bc604e69800c8656868dab60a365 and was ultimately rebased onto current origin/main 77821143e9847f62e129e553522556743c5032c1 after the independent presentation-projection documentation merge.
- Keep semantic-core diagnostic primitives generic. Provider identity is opaque and internal; semantic-core must not encode formula-engine or higher-layer taxonomies.
- Make the ADR-0018 full formula outcome authoritative. Compatibility calculate behavior must be derived from it.
- Stable observations are diagnostic meaning, classification/severity, stable semantic subjects and related subjects/facts, provider identity, and deterministic ordering.
- Human paths, messages, spans, human keys, and cycle witnesses are presentation only.
- Preserve all-or-nothing Calculation publication and native/WASM equality.
- Keep shared semantic validation separate from additional authoring projection, finalization, export, and output gates.
- Preserve ADR-0016 crate layering and storage/numeric behavior.
- Do not stabilize #10 contracts, design #26 transport/runtime, solve #13 progressive typing, design #17 plugins, start #41 roproj, add a common diagnostics crate, or introduce a constraint DSL.

## Initial audited ownership before migration

- semantic-core owns the current fail-accumulating document validator, but its Diagnostic identity is effectively path-first and lacks severity, stable subjects, facts, and provider provenance.
- On the initial base, formula-engine owned parsing, binding, dependency extraction, and evaluation but calculate was fail-first. #97 landed #90's complete node-keyed oracle during implementation, so the final #89 diff consumes that upstream authority without changing formula-engine.
- workspace-engine repeats validate-then-calculate sequencing across first-party operations and separately applies formula projection preflight in authoring/finalization paths.
- merge-engine, AI, CLI, storage, and the portable harness consume legacy validation or calculation surfaces. Storage representation checks remain out of scope; adapters must consume the workspace report without taking ownership.
- The clean base passes 219 workspace tests across 29 suites.

---

### Task 1: Add generic semantic diagnostic primitives

**Files:**
- Create: crates/semantic-core/src/diagnostic.rs
- Modify: crates/semantic-core/src/lib.rs
- Modify: crates/semantic-core/src/validation.rs
- Create: crates/semantic-core/tests/diagnostic_contract.rs
- Modify: crates/semantic-core/tests/validation.rs

- [x] Write compile-failing tests for severity, opaque provider identity, stable semantic subjects, related subjects/facts, presentation-independent stable observations, and deterministic diagnostic ordering.
- [x] Run the focused semantic-core tests and capture RED.
- [x] Add minimal generic primitives without any formula or workspace dependency/taxonomy.
- [x] Refactor core diagnostics to stable codes and subjects while preserving human path/message rendering.
- [x] Expose a core semantic-validation pass suitable for workspace composition while retaining the legacy validate_document compatibility behavior needed by current storage consumers.
- [x] Run semantic-core tests and capture GREEN.

### Task 2: Consume the authoritative ADR-0018 full formula outcome

**Authority read without final #89 changes:**
- crates/formula-engine/src/lib.rs
- crates/formula-engine/tests/complete_oracle.rs
- crates/formula-engine/tests/calculation.rs

- [x] Verify #90's `calculate_complete()` exposes node-keyed failures, complete SCC membership, direct failed dependencies, static dependency sets, precedence, and no partial `Calculation`.
- [x] Remove the superseded branch-local formula implementation and duplicate oracle suite during the rebase onto #97.
- [x] Adapt workspace diagnostics to `CalculationFailure`, `CalculationFailures`, and `ReferenceFailure` without reinterpreting the formula authority.
- [x] Preserve formula-engine source and tests exactly as current `origin/main`.
- [x] Run the upstream formula tests as part of the workspace gate.

### Task 3: Compose one authoritative workspace ValidationReport

**Files:**
- Modify: crates/workspace-engine/src/lib.rs
- Create: crates/workspace-engine/tests/validation_report.rs
- Modify: crates/workspace-engine/tests/application_boundary.rs
- Modify: crates/workspace-engine/tests/workspace.rs

- [x] Write failing tests for independent accumulation, prerequisite/cascade suppression, stable subjects across human-key rename, multi-subject duplicate/cycle/dependency diagnostics, deterministic ordering, and ValidationReport stable observations.
- [x] Write failing reconciliation tests proving shared semantic failures agree across validate and finalization while a semantically valid but unprojectable document fails an explicitly operation-specific gate.
- [x] Run focused workspace tests and capture RED.
- [x] Add ValidationReport and a single internal semantic validation orchestration that composes core diagnostics with the full formula outcome.
- [x] Map formula failures into generic diagnostics with opaque formula-validator provenance owned by workspace composition, stable primary/related subjects and machine facts, and presentation-only messages/paths.
- [x] Apply deterministic semantic ordering and explicit cascade suppression.
- [x] Make first-party semantic operations consume the shared outcome and keep projection/output preflight visibly separate in authoring/finalization paths.
- [x] Update WorkspaceError to carry the authoritative report without duplicating diagnostic authority.
- [x] Run all workspace-engine tests GREEN.

### Task 4: Reconcile direct consumers and adapter boundaries

**Files:**
- Modify as required: crates/merge-engine/src/lib.rs and tests
- Modify: crates/ai-api/src/lib.rs and tests
- Modify: crates/cli/src/main.rs and tests
- Modify as required: crates/diff-engine tests

- [x] Add failing adapter tests that assert typed report-derived behavior rather than presentation paths or cycle witnesses.
- [x] Remove only duplicate semantic orchestration whose ADR-0016 ownership is unambiguous; retain pure merge/diff/formula algorithms in their owning crates.
- [x] Keep storage representation validation unchanged and preserve CLI/AI presentation boundaries.
- [x] Run focused AI, CLI, merge, and diff tests GREEN.

### Task 5: Extend portable native/WASM conformance

**Files:**
- Modify: scripts/portable-conformance-check.rs
- Modify as required: scripts/portable-conformance-check.sh

- [x] Add stable observation records for independent diagnostics, rename-invariant subjects, complete SCC membership, direct failed dependencies, precedence, and all-or-nothing failure.
- [x] Exclude compatibility cycle witnesses and human paths/messages from the new stable ValidationReport fingerprints.
- [x] Run bash scripts/portable-conformance-check.sh and require exact native/WASM equality.

### Task 6: Reconcile implementation-state documentation

**Files:**
- Modify: .jspace/WORKSPACE.md
- Modify: docs/specs/validation-engine.md
- Modify: docs/specs/diagnostics-contract.md
- Modify: docs/specs/formula-engine-spec.md
- Modify: docs/governance/canonical-reconciliation-register.md
- Modify only if required by existing policy: CHANGELOG.md

- [x] Record before/after ownership, the implemented stable observation surface, formula oracle completion, validation/finalization separation, conformance evidence, and explicit deferrals.
- [x] Keep Accepted ADR text authoritative and avoid public wire/API commitments.
- [x] Run docs consistency checks.

### Task 7: Verify, independently review, and hand off

- [x] Run formatting and focused lint/tests.
- [ ] Run the complete bash scripts/release-check.sh gate from a clean committed tree after the #97 reconciliation.
- [ ] Dispatch two exact-head independent read-only reviews: one against ADR-0018 and one against ADR-0019 plus Issue #89.
- [ ] Fix every P0/P1/P2 finding and rerun affected focused tests plus the release-equivalent gate.
- [ ] Commit and force-push the reconciled `codex/issue-89-validation-report` branch with lease.
- [ ] Refresh focused unmerged PR #99 with before/after ownership, stable observations, formula-oracle evidence, semantic/finalization reconciliation, native/WASM and release-gate results, explicit #10/#13/#17/#26/#41 deferrals, and `Closes #89`.
