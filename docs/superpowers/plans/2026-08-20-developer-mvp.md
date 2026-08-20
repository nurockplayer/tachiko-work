# Tachiko Work Developer MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a runnable Rust developer MVP for semantic game-balance documents, deterministic formulas, meaningful diff, CLI workflows, and a read-only AI-facing projection.

**Architecture:** A small Cargo workspace separates the stable semantic model from format, computation, diff, AI, and CLI consumers. The MVP `.ro` representation is canonical UTF-8 JSON with an explicit version gate; it is a serialization of the semantic model, not a spreadsheet or UI model.

**Tech Stack:** Stable Rust 2024 edition, `serde`, `serde_json`, `thiserror`, `clap`, and Rust's built-in test framework.

**Spec:** `docs/product/game-dev-mvp-spec.md`, Issues #1-#8, `docs/decisions/ADR-0001-semantic-platform-not-office-clone.md`, `docs/decisions/ADR-0004-mvp-boundary.md`, and the matching specifications under `docs/specs/`.

## Global Constraints

- Preserve accepted ADRs; treat ADR-0003's dual representation as proposed direction rather than an accepted MVP requirement.
- Do not add spreadsheet UI, Office compatibility, realtime collaboration, SaaS infrastructure, or engine plugins.
- Equivalent semantic documents must serialize identically and all user-visible collections must have stable ordering.
- Direct AI mutation is absent; suggestions are inert data requiring caller approval.
- Keep Git changes uncommitted until the user explicitly authorizes a commit.

---

### Task 1: Workspace and semantic core

**Files:**
- Create: `Cargo.toml`, `crates/semantic-core/Cargo.toml`
- Create: `crates/semantic-core/src/lib.rs`, `crates/semantic-core/src/model.rs`, `crates/semantic-core/src/validation.rs`
- Test: `crates/semantic-core/tests/model_validation.rs`

**Interfaces:**
- Produces: `Document`, `Schema`, `FieldDefinition`, `FieldType`, `Entity`, `Value`, `Expression`, `FieldRef`, `Diagnostic`, and `validate_document(&Document) -> Vec<Diagnostic>`.
- Ordering: identifiers and field/entity/schema maps use `BTreeMap`; diagnostics derive `Ord` and are returned sorted.

- [x] **Step 1: Add workspace manifests and an integration test that constructs a valid typed weapon document, then asserts missing required fields, wrong value kinds, broken typed references, key/id mismatches, and non-finite numbers produce stable diagnostics.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-semantic-core --test model_validation`; verify compilation fails because the public model API is absent.**
- [x] **Step 3: Implement the exact public types and structural/reference validation. Numeric schema fields accept literal numbers or formula expressions; reference fields carry a target schema and `Value::Reference` carries an entity identifier.**
- [x] **Step 4: Re-run the focused test and `rtk cargo test -p tachiko-semantic-core`; verify both pass.**

### Task 2: Canonical `.ro` storage

**Files:**
- Create: `crates/storage/Cargo.toml`
- Create: `crates/storage/src/lib.rs`
- Test: `crates/storage/tests/ro_format.rs`
- Modify: `docs/specs/ro-format-v1.md`

**Interfaces:**
- Consumes: `Document` and `validate_document` from `tachiko-semantic-core`.
- Produces: `FORMAT_VERSION: u32 = 1`, `to_canonical_string(&Document)`, `from_str(&str)`, `load(path)`, and `save(path, document)`.
- Errors: syntax, unsupported version, invalid document diagnostics, UTF-8/path I/O, and overwrite policy remain distinguishable.

- [x] **Step 1: Write tests proving a valid document round-trips, different insertion orders yield identical bytes ending in one newline, unknown fields fail, unsupported versions fail explicitly, and invalid semantic content cannot be serialized or loaded.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-storage --test ro_format`; verify failure because storage APIs do not exist.**
- [x] **Step 3: Implement strict serde decoding, version probing, validation gates, pretty canonical JSON, and path helpers. Document the v1 JSON envelope and compatibility rule in `ro-format-v1.md`.**
- [x] **Step 4: Re-run focused and crate tests; verify pass.**

### Task 3: Deterministic formula engine

**Files:**
- Create: `crates/formula-engine/Cargo.toml`
- Create: `crates/formula-engine/src/lib.rs`
- Test: `crates/formula-engine/tests/calculation.rs`

**Interfaces:**
- Consumes: `Document`, `Expression`, `FieldRef`, and numeric/formula `Value` variants.
- Produces: `calculate(&Document) -> Result<Calculation, CalculationError>`; `Calculation::value(&FieldRef)` and `Calculation::affected_by(&FieldRef)`.
- Expression operations: number literal, field reference, add, subtract, multiply, divide, minimum, and maximum.

- [x] **Step 1: Write tests for cross-entity numeric references, deterministic results, direct/transitive dependency impact, changed-input recalculation, division by zero, non-numeric references, missing references, and cycles with an explicit dependency path.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-formula-engine --test calculation`; verify failure because calculation APIs are absent.**
- [x] **Step 3: Implement sorted depth-first evaluation with visiting/complete states, finite-result checks, direct dependency capture, and deterministic transitive impact traversal.**
- [x] **Step 4: Re-run focused and crate tests; verify pass.**

### Task 4: Semantic diff and formula impact

**Files:**
- Create: `crates/diff-engine/Cargo.toml`
- Create: `crates/diff-engine/src/lib.rs`
- Test: `crates/diff-engine/tests/semantic_diff.rs`

**Interfaces:**
- Consumes: semantic documents plus `calculate` results.
- Produces: `diff(&Document, &Document) -> Result<SemanticDiff, DiffError>` and `SemanticDiff::render_text()`.
- Changes: entity added/removed, field added/removed/changed, and affected formula result before/after.

- [x] **Step 1: Write tests whose sword damage change renders the entity/field transition and affected DPS result, plus deterministic added/removed entity and field cases.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-diff-engine --test semantic_diff`; verify failure because diff APIs are absent.**
- [x] **Step 3: Implement ordered entity/field union comparison, literal semantic value formatting, old/new calculations, and derived impact emission only when formula results differ.**
- [x] **Step 4: Re-run focused and crate tests; verify pass.**

### Task 5: CLI workflow and deterministic export

**Files:**
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`, `crates/cli/src/commands.rs`
- Test: `crates/cli/tests/cli.rs`

**Interfaces:**
- Produces binary `tachiko` with `init`, `validate`, `calculate`, `diff`, and `export` subcommands.
- `calculate` emits sorted JSON keyed by `entity.field`; `export` emits sorted evaluated entity objects suitable for downstream tooling without adding an engine-specific plugin.

- [x] **Step 1: Write process-level tests for all five commands, including nonzero invalid-input behavior and overwrite refusal.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-cli --test cli`; verify failure because the binary/commands are absent.**
- [x] **Step 3: Implement clap parsing and thin command handlers over storage, validation, formula, and diff APIs; keep all output deterministic and CI-safe.**
- [x] **Step 4: Re-run focused and crate tests; verify pass.**

### Task 6: AI semantic facade and game-balance proof

**Files:**
- Create: `crates/ai-api/Cargo.toml`, `crates/ai-api/src/lib.rs`
- Test: `crates/ai-api/tests/semantic_queries.rs`
- Create: `examples/game-balance/game-balance.ro`, `examples/game-balance/buffed-sword.ro`, `examples/game-balance/README.md`

**Interfaces:**
- Produces: `describe_document`, `explain_formula`, `explain_impact`, and `suggest_field_change`; the last returns `Suggestion { requires_approval: true, ... }` and cannot modify a document.
- Example schemas/entities: characters, weapons, items, and economy with derived DPS and price calculations.

- [x] **Step 1: Write AI facade tests for sorted structure, formula/dependency explanation, impact explanation, and immutable approval-required suggestions.**
- [x] **Step 2: Run `rtk cargo test -p tachiko-ai-api --test semantic_queries`; verify failure because the facade is absent.**
- [x] **Step 3: Implement read-only projections over semantic-core/formula/diff. In parallel, author the two canonical example documents and concise CLI workflow guide against the stabilized v1 JSON contract.**
- [x] **Step 4: Validate, calculate, diff, and export the example with the built CLI; verify direct and affected values are visible.**

### Task 7: Repository verification and review

**Files:**
- Modify: `.jspace/WORKSPACE.md`
- Modify only as findings require: files created in Tasks 1-6.

**Interfaces:**
- Verification contract: formatting, clippy with warnings denied, workspace tests, docs tests, example smoke workflow, deterministic repeat output, and two-axis standards/spec review.

- [x] **Step 1: Run `rtk cargo fmt --all -- --check`, `rtk cargo clippy --workspace --all-targets -- -D warnings`, and `rtk cargo test --workspace --all-targets`; fix any failures.**
- [x] **Step 2: Run the five-command example workflow twice and byte-compare calculate/export output.**
- [x] **Step 3: Review the complete diff against repo standards and Issues #1-#8, repair actionable findings, and repeat all relevant verification.**
- [x] **Step 4: Update the j-space outcome with exact test/smoke evidence and leave the branch ready for the user's commit decision.**
