# Semantic Three-Way Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic, typed, validated three-way merge for concurrent game-balance changes and expose it through a safe CLI workflow.

**Architecture:** A new `tachiko-merge-engine` crate recursively reconciles the common ancestor, ours, and theirs at semantic boundaries and returns either a complete document or ordered typed conflicts. The CLI remains a thin adapter that loads canonical inputs, renders the outcome, and exclusively creates only validated successful output.

**Tech Stack:** Rust 2024, existing workspace crates, `clap`, `thiserror`, canonical `.ro` storage, GitHub Actions shell smoke tests.

**Spec:** `docs/superpowers/specs/2026-08-20-semantic-merge.md`

## Global Constraints

- Preserve ADR-0001 through ADR-0011 and the semantic model as source of truth.
- Do not implement `.roproj`, a graphical editor, raw textual merge, or Git configuration in this slice.
- Merge conflicts are structured expected outcomes and never produce an output document.
- A successful candidate must pass both `validate_document` and complete formula calculation.
- The CLI never overwrites an input or existing output.
- New behavior follows a demonstrated red/green test cycle.

---

### Task 1: Merge engine compatible-change contract

**Files:**
- Create: `crates/merge-engine/Cargo.toml`
- Create: `crates/merge-engine/src/lib.rs`
- Create: `crates/merge-engine/tests/three_way_merge.rs`

**Interfaces:**
- Consumes: `tachiko_semantic_core::{Document, Schema, Entity, FieldDefinition, Value}` and `tachiko_formula_engine::calculate`.
- Produces: `merge(&Document, &Document, &Document) -> Result<MergeOutcome, MergeError>`, `MergeOutcome::{Merged, Conflicted}`, `MergeConflict`, and typed `MergeValue`.

- [x] **Step 1: Write the failing independent-change tests**

```rust
#[test]
fn independent_fields_on_the_same_entity_merge() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.9);
    let theirs = balance_document(36.0, 0.8);

    let MergeOutcome::Merged(merged) = merge(&base, &ours, &theirs).unwrap() else {
        panic!("independent edits should merge");
    };
    assert_eq!(merged.entities["iron_sword"].fields["damage"], Value::Number(45.0));
    assert_eq!(merged.entities["iron_sword"].fields["attack_interval"], Value::Number(0.8));
    assert_eq!(calculate(&merged).unwrap().value(&FieldRef::new("iron_sword", "dps")), Some(56.25));
}

#[test]
fn identical_two_sided_change_is_not_a_conflict() {
    let base = balance_document(36.0, 0.9);
    let ours = balance_document(45.0, 0.9);
    let theirs = ours.clone();
    assert!(matches!(merge(&base, &ours, &theirs).unwrap(), MergeOutcome::Merged(_)));
}
```

- [x] **Step 2: Run the focused test and verify RED**

Run: `cargo test -p tachiko-merge-engine --test three_way_merge`

Expected: compilation fails because `tachiko-merge-engine` and its API do not exist.

- [x] **Step 3: Implement minimal recursive compatible merge**

Implement the standard scalar rule:

```rust
fn choose<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> Option<T> {
    if ours == theirs { Some(ours.clone()) }
    else if ours == base { Some(theirs.clone()) }
    else if theirs == base { Some(ours.clone()) }
    else { None }
}
```

Use `BTreeSet` key unions for schemas, fields, entities, and entity fields.
Recurse into entries present in all three inputs; apply add/delete rules for
optional entries. Construct a complete candidate only when no conflicts exist.

- [x] **Step 4: Run the focused test and verify GREEN**

Run: `cargo test -p tachiko-merge-engine --test three_way_merge`

Expected: compatible-change tests pass with no warnings.

- [x] **Step 5: Commit the compatible merge engine**

Run: `rtk git add crates/merge-engine Cargo.toml Cargo.lock`

Run: `rtk git commit -m "feat: add semantic merge engine"`

### Task 2: Typed conflicts and candidate safety

**Files:**
- Modify: `crates/merge-engine/src/lib.rs`
- Modify: `crates/merge-engine/tests/three_way_merge.rs`

**Interfaces:**
- `MergeConflict { path: String, base: Option<MergeValue>, ours: Option<MergeValue>, theirs: Option<MergeValue> }` is ordered by path.
- `MergeSide::{Base, Ours, Theirs}` identifies unsafe inputs.
- `MergeError::{InvalidInput, InputCalculation, InvalidMergedDocument, MergedCalculation}` rejects unsafe inputs and combined candidates.

- [ ] **Step 1: Write failing conflict and validation tests**

Add tests with hand-derived assertions:

```rust
assert_eq!(conflicts[0].path, "entities.iron_sword.fields.damage");
assert_eq!(conflicts[0].base, Some(MergeValue::FieldValue(Value::Number(36.0))));
assert_eq!(conflicts[0].ours, Some(MergeValue::FieldValue(Value::Number(45.0))));
assert_eq!(conflicts[0].theirs, Some(MergeValue::FieldValue(Value::Number(50.0))));
```

Cover same-field divergence, delete-versus-modify, different concurrent
additions, two conflicts returned in lexical path order, an invalid input tagged
with its side, a combined broken reference rejected as `InvalidMergedDocument`,
and cross-branch formula/reference changes that produce division by zero only
after combination rejected as `MergedCalculation`.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test -p tachiko-merge-engine --test three_way_merge`

Expected: conflict payload, ordering, and invalid-candidate assertions fail.

- [ ] **Step 3: Implement typed conflict capture and final validation**

Every failed scalar or optional-entry choice pushes one typed conflict. Sort
conflicts by `path`; return `MergeOutcome::Conflicted` when non-empty. Otherwise:

```rust
validate_and_calculate_input(MergeSide::Base, base)?;
validate_and_calculate_input(MergeSide::Ours, ours)?;
validate_and_calculate_input(MergeSide::Theirs, theirs)?;
let diagnostics = validate_document(&candidate);
if !diagnostics.is_empty() {
    return Err(MergeError::InvalidMergedDocument { diagnostics });
}
calculate(&candidate).map_err(MergeError::MergedCalculation)?;
Ok(MergeOutcome::Merged(candidate))
```

- [ ] **Step 4: Run merge-engine tests and verify GREEN**

Run: `cargo test -p tachiko-merge-engine --all-targets`

Expected: all compatible, conflict, ordering, and safety cases pass.

- [ ] **Step 5: Commit conflict and safety behavior**

Run: `rtk git add crates/merge-engine`

Run: `rtk git commit -m "feat: report typed merge conflicts"`

### Task 3: Safe CLI merge workflow

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/tests/cli.rs`

**Interfaces:**
- Adds `tachiko merge <base> <ours> <theirs> --output <path>`.
- Successful output prints semantic impact from base plus the created path.
- Conflict output exits unsuccessfully, lists all stable paths and values, and writes nothing.

- [ ] **Step 1: Write failing CLI process tests**

Create real `.ro` fixtures with storage `save`, invoke the binary, and assert:

```rust
assert!(output.status.success());
assert_eq!(load(&merged_path).unwrap().entities["sword"].fields["damage"], Value::Number(120.0));
assert!(stdout.contains("wrote"));
assert!(stdout.contains("affected dps"));
```

For a conflicting damage edit assert unsuccessful status, stderr contains
`entities.sword.fields.damage`, and the output path does not exist. Add an
existing-output case and assert its bytes remain unchanged.

- [ ] **Step 2: Run CLI tests and verify RED**

Run: `cargo test -p tachiko-cli --test cli merge_`

Expected: clap rejects the absent `merge` subcommand.

- [ ] **Step 3: Implement the CLI adapter**

Load all inputs with `tachiko_storage::load`, call `tachiko_merge_engine::merge`,
render conflicts with a `CommandError::MergeConflicts` message, and use existing
`write_new` plus `to_canonical_string` only for `MergeOutcome::Merged`. Generate
the success summary with `tachiko_diff_engine::diff(base, merged)`.

- [ ] **Step 4: Run CLI and workspace tests and verify GREEN**

Run: `cargo test -p tachiko-cli --all-targets`

Run: `cargo test --workspace --all-targets`

Expected: merge process tests and all prior behavior pass.

- [ ] **Step 5: Commit CLI merge**

Run: `rtk git add crates/cli Cargo.lock`

Run: `rtk git commit -m "feat: expose safe semantic merge CLI"`

### Task 4: Collaboration onboarding and executable journey

**Files:**
- Modify: `README.md`
- Modify: `examples/game-balance/README.md`
- Create: `scripts/collaboration-smoke.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `docs/architecture/rust-crate-architecture.md`
- Modify: `.jspace/WORKSPACE.md`

**Interfaces:**
- The smoke script creates base, ours, and theirs from the canonical starter,
  merges independent edits, validates the result, and asserts the semantic diff.
- CI executes both first-user and collaboration smoke contracts.

- [ ] **Step 1: Create and run the collaboration smoke script against the live CLI**

Use `mktemp -d`, exclusive output names, `tachiko set` for each branch, `tachiko
merge`, `tachiko validate`, and `tachiko diff`. Assert merged damage `45`, merged
attack interval `0.8`, and calculated DPS `56.25`. Run:
`bash scripts/collaboration-smoke.sh`.

Expected before final wiring: failure at the first missing or mismatched product behavior; after convergence: exit 0 with a one-line success summary.

- [ ] **Step 2: Document the branch collaboration workflow**

Add the exact three-way command to the root and example READMEs. Update the live
crate dependency graph and j-space evidence. Add a CI step that executes the
script rather than grepping its source.

- [ ] **Step 3: Run both smoke contracts**

Run: `bash scripts/first-user-smoke.sh`

Run: `bash scripts/collaboration-smoke.sh`

Expected: both exit 0 and report their complete journeys.

- [ ] **Step 4: Commit onboarding and smoke coverage**

Run: `rtk git add README.md examples scripts .github docs .jspace`

Run: `rtk git commit -m "docs: add semantic collaboration workflow"`

### Task 5: Release verification and independent review

**Files:**
- Modify only as findings require.
- Modify: `.jspace/WORKSPACE.md`
- Modify: `docs/superpowers/plans/2026-08-20-semantic-merge.md`

**Interfaces:**
- Completion requires all quality gates, both real workflows, source install,
  and independent standards/spec review with no actionable findings.

- [ ] **Step 1: Run full fresh verification**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Run: `cargo test --workspace --all-targets`

Run: `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`

Run both smoke scripts and a locked source installation in a fresh temporary root.

- [ ] **Step 2: Review the complete phase diff against ADR-0011 and repository standards**

Review from checkpoint `393bc69`, including untracked files. Fix each actionable
finding with a failing regression test where behavior changes.

- [ ] **Step 3: Repeat all gates and update exact j-space evidence**

Record test count, suite count, both smoke outcomes, install result, and review
outcomes. Mark every plan checkbox complete only after evidence exists.

- [ ] **Step 4: Commit the verified semantic collaboration release**

Run: `rtk git add -A`

Run: `rtk git commit -m "feat: ship semantic collaboration release"`
