# First Usable Product Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the developer MVP into a complete first-user game-balance workflow from creation through safe change, explanation, review, validation, and export.

**Architecture:** Add `tachiko-workflow` as a UI-independent product layer over the existing semantic, formula, diff, and AI crates. Extend the CLI as a thin filesystem/rendering adapter and make the checked-in example the canonical starter and CI smoke contract.

**Tech Stack:** Rust 2024, existing workspace crates, `clap`, `serde`, `serde_json`, `thiserror`, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-20-first-usable-product.md` and `docs/decisions/ADR-0010-first-usable-product-workflow.md`.

## Global Constraints

- Preserve accepted ADR-0001 through ADR-0010 and the semantic model as source of truth.
- Do not add spreadsheet UI, Office compatibility, realtime collaboration, cloud infrastructure, engine plugins, or `.roproj` behavior.
- Every edit produces a new output document and refuses overwrite or input/output path equality.
- Formula fields remain computed and cannot be replaced through scalar `set`.
- New behavior follows a demonstrated red/green test cycle.
- Commit only after the release gates and independent review are clean.

---

### Task 1: Product workflow starter and overview

**Files:**
- Create: `crates/workflow/Cargo.toml`
- Create: `crates/workflow/src/lib.rs`, `crates/workflow/src/template.rs`, `crates/workflow/src/overview.rs`
- Test: `crates/workflow/tests/starter_and_overview.rs`

**Interfaces:**
- Produces: `StarterTemplate::{GameBalance, Empty}`, `create_document(template, id, title) -> Document`, and `overview(&Document) -> Result<DocumentOverview, WorkflowError>`.
- `DocumentOverview` contains sorted `EntityOverview` and `FieldOverview` values with explicit input/reference/formula kinds and human-readable results.

- [x] **Step 1:** Write tests proving the default starter has character, weapon, item, and economy schemas; typed references; three formulas; deterministic overview ordering; and calculated DPS `40`.
- [x] **Step 2:** Run `rtk cargo test -p tachiko-workflow --test starter_and_overview`; verify failure because the workflow API is absent.
- [x] **Step 3:** Implement the template and structured overview using `BTreeMap` iteration and the existing formula engine.
- [x] **Step 4:** Re-run focused and workflow crate tests; verify pass.

### Task 2: Explanation and typed edit preview

**Files:**
- Create: `crates/workflow/src/explain.rs`, `crates/workflow/src/edit.rs`
- Modify: `crates/workflow/src/lib.rs`
- Test: `crates/workflow/tests/explain_and_edit.rs`

**Interfaces:**
- Produces: `explain_field(&Document, &FieldRef) -> Result<FieldExplanation, WorkflowError>` and `set_scalar(&Document, &FieldRef, &str) -> Result<EditPreview, WorkflowError>`.
- `EditPreview` owns the validated cloned `Document` and `SemanticDiff`; it never mutates the input.

- [x] **Step 1:** Write tests for formula dependency explanation, input dependent-formula explanation, number/text/boolean/reference parsing, formula-target refusal, missing paths, invalid values, broken references, calculation failure, and no-op refusal.
- [x] **Step 2:** Run `rtk cargo test -p tachiko-workflow --test explain_and_edit`; verify failure because explanation/edit APIs are absent.
- [x] **Step 3:** Implement schema-directed parsing, cloned mutation, semantic validation, full calculation, and semantic diff generation.
- [x] **Step 4:** Re-run focused and workflow crate tests; verify pass.

### Task 3: Discoverable CLI product workflow

**Files:**
- Modify: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/cli/src/commands.rs`
- Test: `crates/cli/tests/cli.rs`

**Interfaces:**
- Adds: `init --template <game-balance|empty>`, `show`, `explain`, and `set --output`.
- Existing commands gain concise help text. `init` defaults to `game-balance` and prints next steps.

- [x] **Step 1:** Add process tests for default starter creation, empty opt-out, informative help, overview output, input/formula explanation, successful typed edit with semantic impact, and every safe-write failure.
- [x] **Step 2:** Run `rtk cargo test -p tachiko-cli --test cli`; verify new command tests fail against the existing CLI.
- [x] **Step 3:** Implement clap arguments and thin handlers over `tachiko-workflow`; use storage exclusive-create for output documents.
- [x] **Step 4:** Re-run CLI and workspace tests; verify pass.

### Task 4: Onboarding, canonical starter, and CI demo contract

**Files:**
- Modify: `README.md`, `examples/game-balance/README.md`, `.jspace/WORKSPACE.md`
- Create: `scripts/first-user-smoke.sh`, `.github/workflows/ci.yml`
- Test: `crates/workflow/tests/starter_and_overview.rs`

**Interfaces:**
- The starter-template test compares canonical serialization with `examples/game-balance/game-balance.ro`.
- The smoke script executes init, show, explain, set, diff, validate, calculate, and export in a temporary directory and asserts semantic highlights.

- [x] **Step 1:** Extend the starter test with byte-for-byte example equality; run it and observe failure until the template/example converge.
- [x] **Step 2:** Update onboarding around the seven-command first-user journey and create the executable smoke contract.
- [x] **Step 3:** Add CI formatting, clippy, workspace tests, and smoke steps using current supported action versions.
- [x] **Step 4:** Run the smoke script twice, compare deterministic artifacts, and update j-space with exact evidence.

### Task 5: Verification and independent review

**Files:**
- Modify only as findings require: files from Tasks 1-4.
- Modify: `docs/superpowers/plans/2026-08-20-first-usable-product.md`, `.jspace/WORKSPACE.md`

**Interfaces:**
- Completion requires formatting, warnings-as-errors clippy, all tests, warning-free docs, first-user smoke, and standards/spec review.

- [x] **Step 1:** Run `rtk cargo fmt --all -- --check`, `rtk cargo clippy --workspace --all-targets -- -D warnings`, `rtk cargo test --workspace --all-targets`, and warning-free docs.
- [x] **Step 2:** Run `scripts/first-user-smoke.sh` from a clean temporary workspace and inspect every user-facing output.
- [x] **Step 3:** Review the entire live diff against ADR-0010, repository standards, and first-user success criteria; fix actionable findings with regression tests.
- [x] **Step 4:** Repeat all gates, mark the plan and j-space outcome complete, and leave the branch ready for the user's commit decision.
