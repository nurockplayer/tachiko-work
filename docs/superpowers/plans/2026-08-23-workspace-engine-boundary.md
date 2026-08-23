# ADR-0016 Workspace Engine Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Evolve `tachiko-workflow` in place into the single portable `tachiko-workspace-engine` application boundary required by ADR-0016 while preserving all current semantic, storage, formula, and CLI behavior.

**Architecture:** Keep snapshot-style, document-local operations and move shared validation, calculation, diff, merge, mutation, proposal, and runtime-export orchestration behind workspace-engine. CLI remains the native host composition root for paths, storage, exclusive writes, arguments, and rendering; provider-free AI code remains an adapter over workspace-engine. Storage stays a sibling and the low-level engines retain their pure algorithms.

**Tech Stack:** Rust 2024, Cargo workspace, Bash/Node verification scripts, Rust 1.85 MSRV, `wasm32-unknown-unknown`.

**Spec:** GitHub Issue #72 (latest `agent-handoff:v1`) and `docs/decisions/ADR-0016-milestone-02-rust-crate-layering.md`, constrained by ADR-0015, ADR-0017, and ADR-0018.

## Global Constraints

- Start from `origin/main` `e953877f2dfd05ae5cebc5262656c2d877c2ed9c`.
- Preserve document-local authority and the host-supplied `IdGenerator` seam; do not introduce a semantic `Workspace` or `Project` aggregate.
- The exact local dependency DAG is the eight-crate graph accepted by ADR-0016; storage and workspace-engine remain siblings.
- The portable set remains capability-free and behaviorally equivalent on native and `wasm32-unknown-unknown`.
- Do not decide #10, #23, #26, #27/#28, or #41 and do not introduce a new semantic, storage, formula, host, or diagnostic contract.
- Preserve all existing CLI output, safe-write behavior, direct-ro compatibility, and four product smoke journeys.

---

### Task 1: Lock the Dependency Boundary

**Files:**
- Create: `scripts/workspace-dependency-check.mjs`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release-check.sh`

**Interfaces:**
- Consumes: `cargo metadata --no-deps --format-version 1 --locked`.
- Produces: an executable check requiring exactly the ADR-0016 local dependency sets and rejecting `tachiko-workflow`.

- [x] **Step 1: Write the failing graph check**

```js
const expected = new Map([
  ["tachiko-semantic-core", []],
  ["tachiko-formula-engine", ["tachiko-semantic-core"]],
  ["tachiko-diff-engine", ["tachiko-formula-engine", "tachiko-semantic-core"]],
  ["tachiko-merge-engine", ["tachiko-formula-engine", "tachiko-semantic-core"]],
  ["tachiko-storage", ["tachiko-semantic-core"]],
  ["tachiko-workspace-engine", ["tachiko-diff-engine", "tachiko-formula-engine", "tachiko-merge-engine", "tachiko-semantic-core"]],
  ["tachiko-ai-api", ["tachiko-workspace-engine"]],
  ["tachiko-cli", ["tachiko-storage", "tachiko-workspace-engine"]],
]);
```

- [x] **Step 2: Run the graph check and verify RED**

Run: `node scripts/workspace-dependency-check.mjs`

Expected: failure naming the missing workspace-engine package and current workflow/AI/CLI edge mismatches.

- [x] **Step 3: Wire the check into CI and the release-equivalent gate**

Run: `bash scripts/docs-consistency-check.sh`

Expected: PASS; the new executable graph check is invoked separately by CI/release checks.

### Task 2: Evolve Workflow In Place into Workspace Engine

**Files:**
- Rename: `crates/workflow/` to `crates/workspace-engine/`
- Modify: `crates/workspace-engine/Cargo.toml`
- Modify: `crates/workspace-engine/src/lib.rs`
- Rename/update: `crates/workspace-engine/tests/*.rs`
- Create: `crates/workspace-engine/tests/application_boundary.rs`

**Interfaces:**
- Consumes: semantic-core, formula-engine, diff-engine, and merge-engine only among workspace crates.
- Produces: `WorkspaceError`, existing starter/query/mutation functions, and these snapshot-style operations:

```rust
pub fn validate(document: &Document) -> Result<(), WorkspaceError>;
pub fn calculate_fields(document: &Document) -> Result<Vec<CalculatedField>, WorkspaceError>;
pub fn compare_documents(before: &Document, after: &Document) -> Result<SemanticDiff, WorkspaceError>;
pub fn merge_documents(base: &Document, ours: &Document, theirs: &Document) -> Result<WorkspaceMergeOutcome, WorkspaceError>;
pub fn analyze_formula(document: &Document, field: &FieldRef) -> Result<FormulaAnalysis, WorkspaceError>;
pub fn validate_field_value_suggestion(document: &Document, field: FieldRef, value: Value) -> Result<ValidatedFieldValue, WorkspaceError>;
pub fn runtime_export(document: &Document) -> Result<RuntimeExport, WorkspaceError>;
```

- [x] **Step 1: Write application-boundary tests**

Cover validation failure, sorted calculated human addresses, semantic impact, successful/conflicted merge, inert typed proposal validation, formula analysis, and runtime-export projection. Each test asserts observable values and source-document immutability.

- [x] **Step 2: Run the focused tests and verify RED**

Run: `cargo test -p tachiko-workspace-engine --test application_boundary --locked`

Expected: failure because the renamed package and application operations do not exist.

- [x] **Step 3: Rename the crate and implement the minimal shared operations**

Keep the existing ID generator trait unchanged in behavior. Refactor scalar/formula/typed proposal mutation through one candidate builder, while retaining the current operation-specific error precedence and formula-to-scalar protection.

- [x] **Step 4: Remove the workspace-engine test-only storage edge**

Replace the storage-loaded starter comparison with literal authoring-boundary expectations so the package graph has no workspace-engine-to-storage edge in any dependency kind.

- [x] **Step 5: Run focused engine tests and verify GREEN**

Run: `cargo test -p tachiko-workspace-engine --all-targets --locked`

Expected: every migrated and new engine test passes.

### Task 3: Rebase Provider-Free AI Behavior

**Files:**
- Modify: `crates/ai-api/Cargo.toml`
- Modify: `crates/ai-api/src/lib.rs`
- Modify: `crates/ai-api/tests/semantic_queries.rs`

**Interfaces:**
- Consumes: workspace-engine only among workspace crates.
- Produces: unchanged AI-facing description, explanation, impact, and approval-required suggestion DTO behavior.

- [x] **Step 1: Add delegation regressions before changing AI production code**

Update test imports to workspace-engine-owned/re-exported boundary types and add a candidate case whose validation/calculation outcome is supplied by `validate_field_value_suggestion`.

- [x] **Step 2: Run AI tests and verify RED**

Run: `cargo test -p tachiko-ai-api --all-targets --locked`

Expected: compile failure until AI no longer imports low-level workspace crates.

- [x] **Step 3: Delegate formula analysis, impact, and typed proposal validation**

Retain `requires_approval: true` in the AI adapter. Remove candidate cloning, type matching, expression checking, semantic validation, calculation, and diff/formula-engine imports from AI code.

- [x] **Step 4: Run AI tests and verify GREEN**

Run: `cargo test -p tachiko-ai-api --all-targets --locked`

Expected: existing AI semantics and error classes pass.

### Task 4: Thin the CLI Adapter

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

**Interfaces:**
- Consumes: workspace-engine and storage only among workspace crates.
- Retains: Clap parsing, OS paths, storage load/canonical encode, exclusive-create writes, UUIDv7 host generation, and text/JSON rendering.

- [x] **Step 1: Update CLI tests to consume only the workspace-engine boundary**

Replace formula projection helpers with `explain_field` and import semantic test fixtures through workspace-engine. Keep every existing process assertion unchanged.

- [x] **Step 2: Run CLI tests and verify RED**

Run: `cargo test -p tachiko-cli --test cli --locked`

Expected: compile failure until command handlers stop importing low-level engines/core.

- [x] **Step 3: Delegate semantic command behavior**

Use `validate`, `calculate_fields`, `compare_documents`, `merge_documents`, existing mutation/query operations, and `runtime_export`. Delete CLI-owned calculation/diff/merge/export semantic policy while retaining rendering and write timing.

- [x] **Step 4: Run CLI tests and all four product smokes**

Run: `cargo test -p tachiko-cli --all-targets --locked`

Run: `bash scripts/first-user-smoke.sh && bash scripts/collaboration-smoke.sh && bash scripts/entity-lifecycle-smoke.sh && bash scripts/formula-authoring-smoke.sh`

Expected: all process and journey behavior passes unchanged.

### Task 5: Enforce Portability and Reconcile Documentation

**Files:**
- Modify: `scripts/portable-conformance-check.sh`
- Modify: `scripts/portable-conformance-check.rs`
- Modify: `docs/architecture/rust-crate-architecture.md`
- Modify: `docs/specs/ai-agent-api.md`
- Modify: `README.md`
- Modify: `docs/governance/canonical-reconciliation-register.md`
- Modify: `.jspace/WORKSPACE.md`

**Interfaces:**
- Consumes: the same production workspace-engine operation corpus on native and WASM.
- Produces: exact native/WASM records plus current before/after ownership documentation and explicit #10/#23/#26 deferrals.

- [x] **Step 1: Add workspace-engine/AI portable records and package checks**

The conformance corpus must exercise at least one calculated workspace query and one provider-free AI semantic query on both targets. The build must include semantic-core, formula, diff, merge, workspace-engine, and AI API without host capabilities.

- [x] **Step 2: Run portable conformance**

Run: `bash scripts/portable-conformance-check.sh`

Expected: exact native/WASM output match and no unexpected record class.

- [x] **Step 3: Reconcile current architecture documentation and ledger**

Record the implemented graph, single application ownership, storage sibling boundary, portable set, verification evidence, and deferred #10/#23/#26 decisions. Preserve historical plan/ADR wording where it is explicitly historical.

- [x] **Step 4: Run documentation and dependency checks**

Run: `bash scripts/docs-consistency-check.sh && node scripts/workspace-dependency-check.mjs`

Expected: both pass.

### Task 6: Final Verification, Review, and Pull Request

**Files:**
- Verify all changed files from Tasks 1-5.

**Interfaces:**
- Produces: one clean focused branch and one unmerged PR closing #72.

- [x] **Step 1: Run fast quality gates**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --workspace --all-targets --locked -- -D warnings`

Run: `cargo test --workspace --all-targets --locked`

- [x] **Step 2: Run warning-free docs and exact MSRV**

Run: `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked`

Run: `rustup run 1.85.0 cargo check --workspace --all-targets --locked`

- [x] **Step 3: Commit and run the clean-tree release-equivalent gate**

Run: `bash scripts/release-check.sh`

Expected: formatting, dependency graph, Clippy, all tests, native/WASM conformance, Rustdoc, MSRV, packages, four smokes, and native archive checks pass.

- [ ] **Step 4: Independently review `origin/main...HEAD`**

Review both repository standards and Issue #72/ADR-0016 alignment. Explicitly inspect semantic drift, duplicated application policy, forbidden edges, host leakage, native/WASM divergence, and accidental #10/#23/#26 decisions. Fix every P0/P1/P2 and rerun affected plus full gates.

- [ ] **Step 5: Push and open the focused PR**

The PR body must include before/after dependency ownership, exact verification evidence, explicit deferred #10/#23/#26 items, and `Closes #72`. Leave it unmerged.
