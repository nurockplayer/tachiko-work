# J-Space Workspace Ledger

## Goal

Turn Tachiko Work's validated semantic developer MVP into the first usable
game-balance product: immediately explorable, safely editable, reviewable, and
exportable without requiring a user to understand `.ro` internals.

## Reconciled authority

- Base: `origin/main` `2883fdd` after fast-forwarding the stale local checkout.
- Branch: `codex/developer-mvp`.
- Product authority: Issues #1-#8, accepted ADR-0001/2/4/5/6/7, and the
  architecture/specification documents under `docs/`.
- ADR-0003 is proposed rather than accepted. The MVP will establish canonical,
  versioned `.ro` JSON now and preserve a format-version migration boundary;
  `.roproj` packaging remains a later representation over the same model.
- Baseline: no Rust workspace or implementation existed at the base revision.

## Locked implementation boundaries

- `semantic-core`: document, schema, typed values/references, diagnostics.
- `storage`: canonical `.ro` parsing/serialization and version compatibility gate.
- `formula-engine`: typed numeric expression evaluation and dependency impact.
- `diff-engine`: entity/field changes plus recalculated formula impact.
- `ai-api`: read/explain/suggest-only semantic projection; no direct mutation API.
- `merge-engine`: typed three-way model reconciliation, typed conflict reporting,
  and validation/calculation of conflict-free candidates before persistence.
- `tachiko-cli`: `init`, `validate`, `calculate`, `diff`, `merge`, and `export`
  workflows with exclusive output creation.

## Dependency graph

1. Semantic core contracts and validation.
2. Storage and formula engine against the stable core.
3. Diff and merge engines against formula results.
4. CLI integration across all core crates; merge writes only conflict-free,
   valid semantic candidates and never configures a Git driver.
5. In parallel after contracts stabilize:
   - AI read/explain/suggestion facade.
   - Game-balance example and workflow documentation.
6. Full formatting, linting, tests, CLI smoke test, and independent diff review.

## Verification gates

- Each behavioral seam follows a red/green test cycle.
- Canonical `.ro` output must be byte-stable across equivalent insertion orders.
- Formula dependency and cycle behavior must be deterministic and explainable.
- Semantic diff must report both direct value changes and derived impact.
- The committed example must validate, calculate, diff, and export through the CLI.

## Outcome

- Implemented six focused Rust crates: semantic core, storage, formula engine,
  semantic diff, AI semantic facade, and CLI.
- Added canonical v1 `.ro` documents and workflow documentation for the
  Moonfall game-balance example.
- Reconciled stale MVP roadmaps with accepted ADR-0004 and ADR-0006.
- Corrected review findings by moving `format_version` entirely into the
  storage wire envelope, making CLI validation evaluate formulas, and refusing
  destructive storage/export overwrites.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace --all-targets`: 42 tests passed across 12 suites.
- `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`: passed.
- Real CLI smoke: all five commands passed; repeated calculation/export output
  was byte-identical (`92291e47...` calculation, `98a54a50...` export).
- Final independent standards review: no P0-P3 findings.
- Final independent spec/ADR review: no P0-P3 findings.
- Branch `codex/developer-mvp` is intentionally uncommitted pending explicit
  user authorization.

## Active phase: first usable product

### Goal

Turn the validated developer MVP into the version a first real game developer
can use without understanding `.ro` internals.

### Reconciled authority

- Live base: `origin/main` `f8f8171`, with its duplicate accepted ADR number
  corrected locally: the later validation decision is ADR-0009.
- Product decision: `docs/decisions/ADR-0010-first-usable-product-workflow.md`.
- Design: `docs/superpowers/specs/2026-08-20-first-usable-product.md`.
- Plan: `docs/superpowers/plans/2026-08-20-first-usable-product.md`.

### Dependency graph

1. UI-independent workflow crate: starter, overview, explanation, typed edit.
2. CLI product adapter: discoverable help and safe creation-to-review journey.
3. Canonical starter/example contract and onboarding.
4. CI plus real first-user smoke workflow.
5. Independent standards and product-spec review.

### Locked safety decisions

- Game-balance is the useful default starter; `--template empty` is explicit.
- Scalar edits always create a new output and never overwrite.
- Computed fields cannot be erased through scalar editing.
- `.roproj`, in-place editing, formula authoring, and graphical UI remain
  intentionally deferred.

### Outcome

- Added the UI-independent `tachiko-workflow` crate with a byte-stable
  game-balance starter, deterministic document overview, field/dependency
  explanations, schema-typed scalar editing, complete validation, calculation,
  and semantic impact preview.
- Extended the CLI with discoverable help plus `show`, `explain`, and safe
  `set --output`; `init` now creates useful content by default and retains an
  explicit `--template empty` escape hatch.
- Added repeat-safe onboarding, an executable end-to-end smoke contract, and CI
  gates for formatting, warnings-as-errors clippy, tests, docs, and the actual
  first-user journey.
- Made invalid edit errors actionable, rejected identical input/output paths,
  and preserved exclusive-create behavior for every output.
- Corrected the upstream duplicate accepted ADR number without changing its
  decision, then recorded the product workflow as ADR-0010.
- Release review fixes now enforce addressable identifier grammar, expose typed
  reference targets, retain typed semantic diff values, independently version
  runtime export, validate AI suggestions, and document source installation.
- Final gates: formatting passed; clippy passed with warnings denied; 60 tests
  passed across 15 suites; warning-free docs passed; the first-user smoke and a
  locked source installation both passed.
- Final independent standards review: no remaining actionable findings.
- Final independent product/spec review: no remaining actionable findings.
- The user's release-ownership instruction authorizes committing this verified
  checkpoint before beginning the next product phase.

## Active phase: semantic collaboration

### Goal

Let a game team safely combine concurrent balance branches through typed,
validated three-way merge instead of resolving raw JSON conflicts.

### Reconciled authority

- Product checkpoint: `393bc69` (`feat: ship first usable Tachiko Work product`).
- Product decision: `docs/decisions/ADR-0011-semantic-three-way-merge.md`.
- Design: `docs/superpowers/specs/2026-08-20-semantic-merge.md`.
- ADR-0002 requires semantic merge for the game-development wedge; ADR-0008
  deferred it until the now-verified usability milestone.
- ADR-0003 remains proposed, so this phase operates on the semantic model and
  canonical `.ro` inputs without inventing `.roproj` behavior.

### Locked boundaries

1. A UI-independent merge engine owns three-way model reconciliation.
2. Independent fields inside existing schemas/entities merge recursively.
3. Conflicts preserve typed base/ours/theirs meaning in stable path order.
4. Conflict-free candidates must validate and calculate before persistence.
5. CLI output is exclusive-create; conflict and invalid candidates write nothing.
6. Git-driver configuration and interactive resolution remain follow-up adapters.

### Verification gates

- Every merge rule follows a red/green test cycle.
- Engine tests cover compatible, conflicting, invalid, and deterministic cases.
- CLI process tests cover success, conflict, and overwrite safety.
- A real collaboration smoke test creates canonical base/ours/theirs inputs,
  asserts merged damage `45`, attack interval `0.8`, and DPS `56.25`, then
  validates and reviews the semantic impact.
- Full formatting, clippy, tests, docs, and both smoke journeys must pass.
