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

- Every new behavior follows a red/green test cycle; added coverage for already
  correct merge rules is recorded honestly as characterization evidence.
- Engine tests cover compatible, conflicting, invalid, and deterministic cases.
- CLI process tests cover success, conflict, and overwrite safety.
- A real collaboration smoke test creates canonical base/ours/theirs inputs,
  asserts merged damage `45`, attack interval `0.8`, and DPS `56.25`, then
  validates and reviews the semantic impact.
- Full formatting, clippy, tests, docs, and both smoke journeys must pass.

### Outcome

- Added the UI-independent `tachiko-merge-engine` with recursive three-way
  reconciliation for document metadata, schemas, entities, definitions,
  membership, stored values, references, and formulas.
- Independent changes merge deterministically; divergent intent returns typed,
  path-ordered base/ours/theirs conflicts without a partial document.
- Base, ours, theirs, and the conflict-free candidate each validate and
  calculate before success, with errors identifying the failing stage and side.
- Added safe `tachiko merge BASE OURS THEIRS --output MERGED.ro`; it exclusively
  creates successful output and prints base-to-merged semantic impact.
- Extended semantic diff with typed document and schema changes so title-only,
  schema-only, and entity changes all produce truthful deterministic summaries.
- Added real collaboration onboarding and CI smoke coverage for independent
  damage/attack-interval edits merging to 56.25 DPS.
- Final review found incomplete document/schema summaries and missing direct
  merge-rule coverage; the fix added typed diff variants plus title/schema CLI
  regressions and successful add/delete/membership tests across every merge
  surface. Scoped re-review found all findings addressed with no new breakage.
- Final gates after review fixes: formatting passed; warnings-as-errors Clippy
  passed; 98 tests passed across 17 suites; warning-free docs passed; first-user
  and collaboration smoke journeys passed; locked source install reported
  `tachiko 0.1.0`.

## Active phase: release distribution

### Goal

Turn the verified semantic product into a trustworthy external distribution:
packageable source, tested minimum Rust support, checksummed native binaries,
and an explicit release-authorization boundary.

### Reconciled authority

- Product checkpoint: `0c3d2dd` (`feat: ship semantic collaboration release`).
- Product decision: `docs/decisions/ADR-0012-release-distribution-contract.md`.
- Design: `docs/superpowers/specs/2026-08-20-release-distribution.md`.
- Plan: `docs/superpowers/plans/2026-08-20-release-distribution.md`.
- The declared Rust 1.85 toolchain successfully checked the entire locked
  workspace before this phase; Cargo source packaging failed because internal
  path dependencies did not declare registry versions.
- Current official upstream interfaces were reconciled on 2026-08-20: supported
  fixed native runner labels, checkout v7, upload-artifact v7,
  download-artifact v8, and `gh release create --verify-tag --draft`.

### Locked boundaries

1. Ordinary push and pull-request CI stays read-only.
2. An existing exact `v${workspace_version}` tag authorizes artifact building.
3. The workflow creates a draft release only; a human retains publish authority.
4. Crate archives are validated but are not published to crates.io.
5. Each native binary archive includes both Tachiko licenses, audited
   third-party notices, README, and changelog plus an adjacent SHA-256 checksum.
6. Code signing, notarization, attestations, package managers, and auto-update
   remain follow-up release capabilities.

Cargo package inspection established that a member archive can inherit the
workspace README, while Cargo's singular `license-file` replaces rather than
augments the dual-license SPDX expression. The release keeps
`Apache-2.0 OR MIT` as authoritative package metadata and requires both exact
texts in the repository and every binary archive. Per-member license copies and
the final crates.io archive policy remain coupled to the explicitly deferred
registry-publication decision.

### Verification gates

- Every crate packages from the workspace with versioned internal dependencies.
- Stable quality gates and exact Rust 1.85 all-target checking both pass.
- Every native artifact is extracted and executed on its build architecture.
- Release workflow validation proves tag/version equality, draft-only behavior,
  existing-tag verification, and least-privilege permissions.
- A single local release-check script reproduces all platform-independent gates.

### Task 1 evidence: package and legal contract

- Commit `93316aa` adds complete shared metadata, crate-specific descriptions,
  compatible versions for all 19 internal path dependencies, canonical dual
  license texts, and the factual `0.1.0` changelog.
- `cargo package --workspace --locked --no-verify` produced all eight source
  archives; each retained the inherited README, SPDX expression, repository,
  and normalized registry dependency versions.
- Locked workspace check and all 98 tests across 17 suites passed.
- Independent package/standards review reported zero Critical, Important, or
  Minor findings.

### Task 2 evidence: reproducible native artifacts

- Commits `aa8f564`, `cdc0c2b`, and `1ff6abf` add deterministic native archive
  creation, exact safe-payload verification, portable checksums, a shared
  release policy helper, and one complete local release gate.
- The release check passed formatting, warnings-as-errors Clippy, 98 tests
  across 17 suites, warning-free docs, exact Rust 1.85, all eight source
  packages, both product smokes, and native archive execution.
- Identical inputs produced byte-identical archives under umasks `022` and
  `077`; tampered checksums were rejected.
- Actual BSD tar on macOS and GNU tar 1.35 under Ubuntu 24.04 both produced the
  normalized payload. The remaining Windows behavior is assigned to its native
  matrix runner.
- Review found and fixes closed GNU-tar incompatibility, umask-dependent root
  mode, concurrent publication, both post-move and lock-acquisition signal
  windows, and release-policy duplication. Final scoped re-review approved the
  task with no remaining Critical or Important finding.

### Task 3 evidence: CI and tag-gated draft release

- Commits `86e603e` and `918a0c3` pin reconciled action commits, add read-only
  stable and exact-Rust-1.85 CI, and add the four-target tag workflow.
- The tag must exactly equal `v${workspace_version}` from the shared release
  helper. Every runner builds, packages, verifies, extracts, and executes its
  native target before upload.
- Asset aggregation requires exactly four archives and four checksums and
  revalidates every digest. Only the final job receives `contents: write`.
- An authenticated paginated preflight includes draft releases and rejects any
  existing exact tag record before `gh release create --verify-tag --draft`.
  No path invokes `cargo publish` or promotes the draft automatically.
- Review caught a malformed annotated rust-cache ref and GitHub's allowance for
  duplicate same-tag drafts; both were fixed. Actionlint, pinned-action
  resolution, static security checks, and final scoped re-review passed with no
  remaining Critical or Important finding.
- No tag, push, release, or other public state change was performed.

### Task 4 evidence: external-user and owner documentation

- The README now distinguishes the currently available source installation
  from the not-yet-tagged binary release, names all four future archives, and
  provides checksum, extraction, and exact-version checks for Unix and Windows.
- The executable owner runbook covers a clean reviewed commit, version and
  changelog alignment, the complete local release gate, an exact annotated tag,
  deliberate tag-push authorization, eight-asset draft review, four native
  clean-machine smokes, manual publication, external verification, and an
  immutable fail-forward policy.
- Contribution guidance records stable and exact-Rust-1.85 setup, focused and
  full quality gates, semantic/Git/no-overwrite/AI-approval boundaries, and
  review-ready test, ADR, changelog, and pull-request expectations.
- The security policy truthfully records that no private channel is currently
  advertised. It requests a detail-free ordinary coordination issue until
  GitHub private vulnerability reporting is enabled and promises no unsupported
  SLA.
- Documentation preserves crates.io publication, signing, and notarization as
  deferred release decisions. The owner runbook records current private-hosting
  verification without authorizing a visibility change; no tag or release state
  was created.

### Final release-review remediation

- A bounded consumer scan confirmed `tachiko_release_payloads` is the single
  member policy consumed by both native packaging and exact archive
  verification; adding `THIRD_PARTY_LICENSES.md` there updates both surfaces
  without parallel file lists. The four archives plus four checksums remain the
  same eight external assets.
- Dependency notices are generated from the locked all-target normal
  `tachiko-cli` closure and byte-exact vendored legal files. The inventory
  includes Windows-only and proc-macro packages, excludes Tachiko workspace
  crates, and deduplicates identical legal texts by SHA-256 while retaining
  every package/file attribution.
- The complete local gate now requires installed stable Rust, exports a
  process-local `RUSTUP_TOOLCHAIN=stable` before any main gate, reports the
  selected compiler, and retains an explicit `rustup run 1.85.0` MSRV check.
  This prevents a caller's older temporary override from weakening normal
  release verification without changing any persistent rustup setting.
- Deterministic regeneration under macOS system Bash 3.2 byte-matched the
  checked-in notice; a deliberately modified copy failed comparison. Its 30
  external package rows exactly matched Cargo's locked all-target normal tree,
  including `windows-link`, `windows-sys`, and the derive/proc-macro closure.
- Invoking the full clean-checkout release gate with an inherited
  `RUSTUP_TOOLCHAIN=1.85.0` reported and used stable Rust 1.97.1 for normal
  gates, then passed the explicit Rust 1.85.0 check, 98 tests across 17 suites,
  eight Cargo packages, both product smokes, and native archive verification.
  The exact seven-member archive contains one executable, README, changelog,
  both Tachiko licenses, and `THIRD_PARTY_LICENSES.md`; the verifier executed
  the extracted CLI successfully.

### Outcome

- Tachiko Work now has packageable source crates, canonical project and
  third-party license evidence, an enforced exact MSRV, deterministic
  checksummed native archives, and a least-privilege four-target draft-release
  workflow.
- The local release gate reproduces formatting, Clippy, 98 tests in 17 suites,
  warning-free docs, Rust 1.85 compatibility, eight source packages, both real
  product journeys, notice freshness, archive determinism, tamper rejection,
  interruption cleanup, concurrent no-clobber behavior, and extracted native
  CLI execution.
- Installation, contribution, vulnerability-reporting, and release-owner
  guidance match the current private hosting state and do not claim an
  unavailable binary, security channel, public audience, or signature.
- Final broad review found missing dependency notices and inherited default
  toolchain selection. Commit `9d62e9b` fixed both; scoped re-review approved
  the complete release phase with no remaining P0, P1, or P2 finding.
- The branch remains the delivery unit. No tag, push, GitHub draft, crates.io
  package, visibility change, or public release was created.

## Active phase: semantic entity lifecycle

### Goal

Let a game designer grow and reorganize the balance roster without hand-editing
canonical JSON, while preserving typed relationships, formulas, Git review, and
the product's no-overwrite safety boundary.

### Reconciled authority

- Product checkpoint: `37b76d3` (`build: ship release distribution contract`).
- Product decision: `docs/decisions/ADR-0013-semantic-entity-lifecycle.md`.
- Design: `docs/superpowers/specs/2026-08-20-entity-lifecycle.md`.
- Plan: `docs/superpowers/plans/2026-08-20-entity-lifecycle.md`.
- ADR-0010 intentionally deferred formula authoring, but the current semantic
  model already has typed entity references and formula field references that
  can support safe structural lifecycle changes.
- The canonical starter has only one weapon. Existing scalar editing can tune
  it but cannot create a second roster member without JSON knowledge.

### Locked boundaries

1. `duplicate` rebases formula self-references but preserves stored and formula
   relationships to other entities.
2. `rename` rewrites every typed entity and formula reference to the old ID.
3. `remove` is non-cascading and refuses sorted external dependents.
4. Every operation validates, calculates, and semantically compares its
   immutable candidate before returning.
5. `tachiko entity` outputs are distinct, exclusive-create canonical files.
6. Schema, field, and formula authoring remain separately deferred contracts.

### Verification gates

- Focused test-first coverage for identifier, transformation, CLI, and output
  safety rules.
- A real duplicate, tune, rename, explain, blocked-remove, successful-remove
  game-balance journey in CI and the local release gate.
- Full release verification and independent product/safety review before the
  phase checkpoint.

### Task 1 evidence: semantic workflow

- Commit `7eb2cf6` exposes the stable semantic identifier predicate and adds
  immutable duplicate, rename, and remove workflow operations.
- Formula traversal covers every current expression shape. Duplicate rebases
  only formula self-references; rename rewrites stored and formula references;
  remove reports unique, sorted dependent field paths and ignores references
  owned by the removed entity.
- Semantic-core and workflow focused suites passed 29 tests. Formatting and
  warnings-as-errors Clippy passed, and the validation consumer scan found no
  divergent identifier grammar.

### Task 2 evidence: CLI lifecycle adapter

- Commit `6f79baf` adds the discoverable nested
  `tachiko entity duplicate|rename|remove` interface with canonical,
  exclusive-create outputs and semantic impact previews.
- The red process seam initially failed because `entity` was absent. The green
  CLI suite passed all 26 tests, including rewrite semantics, blocked removal,
  same-path refusal, and existing-output preservation; focused Clippy passed
  with warnings denied.

### Task 3 evidence: real roster journey

- The new smoke contract duplicates `iron_sword`, proves repeat output is
  byte-identical, names and tunes the copy, renames it to `moonblade`, explains
  the rebased 50-DPS formula, validates and exports it, and rejects removal of
  the still-referenced original with three dependent paths.
- Removing the unreferenced `moonblade` produces a canonical document that is
  byte-identical to the starter. All three product smokes passed twice against
  one built CLI; Bash syntax and ShellCheck passed.
- Ordinary CI and the local release gate now require the entity lifecycle
  smoke. The local `actionlint` executable is unavailable on this host; the CI
  change is a single existing-pattern shell step and remains covered by final
  workflow review.

### Outcome

- Commits `7eb2cf6`, `6f79baf`, and `670f9c8` ship the UI-independent semantic
  transformations, cohesive nested CLI, onboarding, Moonfall roster guide,
  CI contract, and local release-gate integration.
- The complete gate was invoked with inherited `RUSTUP_TOOLCHAIN=1.85.0` and
  selected stable Rust 1.97.1 for ordinary work. Formatting, warnings-as-errors
  Clippy, 113 tests across 19 suites, warning-free docs, exact Rust 1.85, all
  eight Cargo source packages, dependency-notice freshness, and all three real
  product journeys passed.
- The native macOS arm64 release archive was byte-identical across strict and
  ordinary umasks. Checksum and safe-payload validation, extracted
  `tachiko 0.1.0` execution, tamper rejection, both interruption windows, and
  concurrent no-clobber publication all passed.
- Independent phase review reproduced 55 focused semantic-core, workflow, and
  CLI tests plus the complete release check. It approved ADR/implementation,
  recursive reference semantics, deletion safety, canonical persistence,
  product guidance, CI, and the smoke contract with no actionable P0-P2 issue.
- Formula, schema, and field authoring; cascading reference migration; `.roproj`;
  graphical UI; signing; notarization; crates.io publication; and an actual
  tagged release remain explicit later decisions.
- No tag, push, draft release, registry publication, hosting visibility change,
  or other public state mutation was performed.

## Active phase: computational formula authoring

### Goal

Let a designer create and revise deterministic game-balance formulas without
editing the tagged canonical AST, while preserving typed references, semantic
review, bounded execution, Git-native outputs, and AI approval.

### Reconciled authority

- Product checkpoint: `e3af399` (`feat: ship semantic entity lifecycle`).
- Product decision: `docs/decisions/ADR-0014-computational-formula-authoring.md`.
- Design: `docs/superpowers/specs/2026-08-20-formula-authoring.md`.
- Plan: `docs/superpowers/plans/2026-08-20-formula-authoring.md`.
- ADR-0010 deliberately deferred formula authoring until the editing workflow
  stabilized. The product now validates scalar and entity lifecycle changes,
  but formula creation still requires hand-authored JSON AST nodes.
- The existing expression model already fixes the supported computational
  boundary, so this phase adds a projection onto it rather than a new runtime.

### Locked boundaries

1. Bracketed `[entity.field]` references are unambiguous and use the semantic
   identifier grammar.
2. The language maps only to finite numeric literals, current arithmetic,
   parentheses, unary signs, `min`, and `max`.
3. Parser and typed-AST limits are 4,096 source/canonical bytes, 256 nodes, and
   64 post-desugaring AST-depth levels; failures identify a stable byte position
   or typed complexity error before recursive processing.
4. Canonical explain/diff formatting is accepted parser input.
5. Formula edits validate, calculate, and compare an immutable candidate before
   exclusive output persistence.
6. Typed AI formula suggestions remain inert and approval-required; formulas
   cannot be silently replaced by scalar suggestions.
7. Formula clearing, new runtime operations, schema authoring, scripting, and
   spreadsheet compatibility remain deferred.

The pre-implementation review found that syntactic nesting alone did not bound
flat-chain AST depth or guarantee canonical reparse, and that typed AI formulas
could bypass text-parser limits. It also identified shell transport ambiguity
for leading hyphens. The contract now requires one shared iterative complexity
gate, canonical-byte admissibility with shortest finite-number rendering, and a
quoted named `--expression` option with explicit hyphen-value handling.

### Verification gates

- Test-first parser, workflow, AI, CLI, and output-safety seams.
- A real Moonfall formula-authoring journey with deterministic bytes and
  parse/reference/cycle rejection.
- Full release verification and independent P0-P2 review before checkpointing.

### Task 1 evidence: bounded formula language

- Commits `35c16c4` and `bfcddeb` add bracketed semantic references,
  precedence-aware arithmetic, unary signs, parentheses, `min`/`max`, stable
  byte-position errors, and one canonical formatter.
- Review exposed flat-chain depth/canonical-roundtrip and typed-AI bypass gaps.
  The fix added one public iterative gate for 256 nodes, 64 post-desugaring AST
  depth, and 4,096 canonical bytes, plus bit-exact shortest Display/scientific
  finite-number selection.
- All 24 formula-engine tests passed, including exact and excess boundaries,
  balanced node trees, flat and unary depth, canonical expansion, extreme
  numbers, source bytes, deterministic round trips, and all expression shapes.
  Warnings-as-errors Clippy and exact Rust 1.85 all-target checking passed.

### Task 2 evidence: validated computational edits

- Commit `c7b523a` adds immutable `set_formula` for schema-numeric fields through
  the shared validate → calculate → semantic-diff finalizer.
- Workflow and semantic-diff rendering now consume the formula engine's
  canonical bracketed formatter instead of maintaining divergent renderers.
- All 20 workflow and eight diff-engine tests passed. Coverage includes
  numeric-to-formula, formula-to-formula, no-op, wrong type, invalid syntax,
  missing reference, cycle, division by zero, derived impact, source
  immutability, and copy/paste diff/explain syntax.

### Task 3 evidence: AI approval and CLI transport

- Commit `d56e2b6` allows bounded typed formula suggestions for numeric fields
  while preserving inert approval, validation, calculation, source
  immutability, no-op refusal, and formula-to-scalar protection.
- `tachiko formula set` uses a quoted named `--expression` option with explicit
  hyphen-value handling, canonical exclusive output, semantic impact, and an
  explain next step.
- All eight AI and 31 CLI tests passed. Typed ASTs at the node/depth boundaries
  succeed; excess depth, nodes, or canonical bytes return typed errors before
  recursion. Process tests prove canonical explain output, `-1`, unary reference
  negation, multiplication, spaces, and `--output` reach the correct parser and
  preserve every file on failure.

### Task 4 evidence: computational-authoring journey

- The new real journey revises Moonfall DPS to
  `min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)`, verifies
  the 45-DPS result, canonical explanation and diff, deterministic repeated
  bytes, validation, calculation, and runtime export.
- Invalid syntax, a missing reference, and a dependency cycle each return an
  actionable error and create no output. Bash syntax and ShellCheck passed.
- README and the Moonfall guide document quoted authoring syntax, operations,
  limits, error behavior, AI approval, and the copy/paste explain contract.
  All four product journeys passed twice against the same built CLI; ordinary
  CI and the local release gate now require all four.

- No durable independent review report artifact was recovered from the interrupted
  session; `.jspace` contains only the single ledger and the checkpoint path was
  continued in place.

- Fresh read-only closeout review rechecked the full post-checkpoint diff
  `e3af399..HEAD` and accepted it without additional blockers:
  `c53a45f` decision commit, `35c16c4` bounded parser, `bfcddeb` complexity
  enforcement, `c7b523a` workflow validation/calculation/finalization, `d56e2b6`
  AI approval transport, and `a47d67f` formula-authoring journey/docs.

- Closure validation (`bash scripts/release-check.sh`) on this resumed state was
  fully green: formatting, warnings-as-errors Clippy, `cargo test --workspace
  --all-targets` (140 tests, 20 suites), warning-free docs, Rust 1.85 MSRV check,
  cargo source package all eight crates, dependency-notice freshness, artifact
  determinism/tamper/cleanup/concurrency checks, all four product journeys, and
  extracted native execution.

- Final review outcome:
  - No P0/P1/P2 findings.
  - No code defects requiring fixes remained after this independent review.
  - No speculative functionality was added; no feature phase was started.

- Documentation reconciliation outcome (post-checkpoint):
  - Current `v0.1` CLI-first implementation is now reflected across
    `README.md`, `docs/decisions`, `docs/architecture`, `docs/specs`, and
    `docs/product`.
  - Explicitly marked as deferred/future: GUI/UI, `.roproj` production workflow,
    engine plugins, cloud/realtime collaboration, CRDT/event-sourcing, and
    realtime AI editing workflows.
  - Formula docs now distinguish implemented syntax (`+ - * /`, parentheses,
    unary operators, `min`/`max`, bounded size/depth limits) from future
    capabilities.
  - Semantic merge docs now match implemented typed three-way `.ro` merge with
    typed conflict output, deterministic path ordering, and no partial writes.
- AI docs now match the implemented read/explain/suggest model and explicit
  approval gate.

## Active phase: ADR-0017 storage hardening

### Goal

Freeze the shipped `legacy-direct-ro/v1` wire contract behind storage-owned
DTOs, strict version-first decoding, and canonical emission without inventing
the ADR-0015 persisted identity model that remains owned by #70.

### Reconciled authority and baseline

- Worktree: `/Users/tachikoma/Developer/tachiko-work-adr0017` on
  `codex/adr0017-storage-hardening`, created from `origin/main` `c852840`.
- Merged PR #75 is present in the base history at `1e422f1`.
- Authority: ADR-0015, ADR-0016, ADR-0017,
  `docs/specs/ro-format-v1.md`,
  `docs/specs/storage-versioning-and-migration.md`,
  `docs/specs/canonical-json-profile.md`, and the latest #74
  `agent-handoff:v1`; #40 owns conformance evidence and #70 owns the later
  stable-identity migration.
- Clean baseline: workspace build passed; 140 tests passed across 20 suites.

### Locked implementation boundaries

1. Decode bytes through UTF-8, JSON/duplicate validation, lexical version
   probing, exact v1 dispatch, storage DTO validation/conversion, then semantic
   validation.
2. Own the complete historical document/schema/field/entity/value/expression
   DTO graph in storage; semantic-core Serde layouts are not wire authority.
3. Reject recursive unknown members, mismatched map/nested IDs, and
   unresolvable v1 schema/field relationships instead of guessing.
4. Emit v1 with its fixed record-member order, sorted legacy-ID maps,
   historical number spelling, deterministic escaping, preserved Unicode,
   two-space indentation, LF, and one final LF.
5. Preserve existing valid v1 bytes, storage APIs, checked-in examples, CLI
   behavior, and no-overwrite behavior where practical.
6. Leave `direct-ro/v2`, surrogate-ID encoding, UUID migration namespace/input
   vectors, and the two-phase ID rewrite to #70 after semantic-core has the
   required stable-ID/key model.
7. Do not decide #24, #41, #43, #23, or #26 and do not add a crate or new
   dependency.

### Verification plan

- Test-first storage conformance for strict errors, every v1 discriminator and
  operator, recursive closed-world decoding, coherence, Unicode, canonical
  rewriting, insertion-order independence, byte-stable round trips, and both
  checked-in `.ro` examples.
- Focused storage tests during implementation, then formatting, docs
  consistency, warnings-as-errors Clippy, workspace tests/docs/packages, all
  four product smokes, Rust 1.85 checking, release-equivalent native checks,
  and independent standards/spec review.

### Outcome

- Replaced semantic-core-derived persistence with a complete storage-owned
  legacy-direct-ro/v1 DTO graph and explicit semantic conversions. A
  crate-private DTO seam exposes every one of the 12 frozen typed-ID
  occurrences for #70 before semantic conversion.
- Added the ordered byte-reader pipeline for invalid UTF-8, invalid JSON,
  decoded-name duplicate detection at every depth, missing/malformed/future
  version handling, exact v1 dispatch, closed-world DTO validation, and
  semantic validation.
- Added explicit representation validation for legacy identifier grammar,
  schema/entity map-key coherence, schema and field relationships, recursive
  references, and historical finite-f64 values.
- Canonical v1 writing now follows explicit record serializers and sorted
  legacy-ID DTO maps. The two existing .ro examples remain byte-identical,
  while a checked-in all-shapes golden freezes all four field types, five value
  kinds, eight expression operators, member order, numeric spelling, escaping,
  and Unicode preservation.
- Conformance grew from 140 tests in 20 suites to 171 tests in 22 suites; the
  storage crate now has 38 passing tests across its unit and integration
  suites.
- Formatting, documentation consistency, warning-denied workspace Clippy,
  all-target workspace tests, warning-denied Rustdoc, exact Rust 1.85 all-target
  checking, the audited portable-crate WASM check, and all four product smoke
  journeys passed.
- Two independent final reviews found no remaining code/API or ADR/spec
  findings. They specifically rechecked error-source preservation, explicit
  null handling, the complete typed-ID migration seam, exact canonical bytes,
  and the absence of decisions for #24, #41, #43, #23, or #26.
- The clean implementation commit is 21a5a52. The full release check passed all
  repeated quality gates, dependency-notice drift validation, all eight source
  packages, native archive determinism and execution, tamper rejection,
  interruption cleanup, and concurrent no-clobber publication.
- Remaining #70 work is deliberately not claimed: semantic-core still needs
  the accepted stable identity/key model before deterministic legacy-to-stable
  mapping, namespace/input vectors, two-phase rewrite of all 12 occurrences,
  and any future representation version can be implemented.

### PR #80 P1 remediation

- Inline review identified that the crate-visible migration DTO seam still
  called Serde directly, allowing duplicate schema, entity, and field map keys
  to collapse before DTO validation.
- The regression was proven red first: identical schema keys, entity keys,
  schema-field keys, entity-field keys, and escaped-equivalent schema keys were
  all accepted by the old seam.
- Commit d202be7 makes the byte-oriented strict reader the sole
  migration-facing DTO decoder. Public semantic decoding consumes the same
  function, and no crate-visible raw string-to-DTO helper remains.
- The new migration-seam regression rejects every duplicate case specifically
  as DuplicateMember before map collapse. Existing error precedence, canonical
  bytes, public behavior, and ADR scope are unchanged.
- Fresh remediation validation passed 38 storage tests and 171 workspace tests,
  formatting, docs consistency, warning-denied Clippy and Rustdoc, exact Rust
  1.85, all eight source packages, all four smokes, and native release/archive
  safety checks.
- Independent standards and spec reviews of 0ab898d...d202be7 both reported no
  findings.

## Active phase: ADR-0015 stable-identity transition (#70)

### Goal and authority

- Execute the latest #70 `agent-handoff:v1` as one atomic, reviewable semantic
  transition on `codex/stable-identity-transition` from `origin/main` `a27fd11`.
- Authority is Accepted ADR-0015 through ADR-0018 plus merged PR #80's frozen
  legacy-v1 DTO/strict-reader seam.
- Do not redo #74, start #40's broad corpus closure, begin #72's
  workflow→workspace-engine migration, design `.roproj`/packaging, or self-merge.

### Implemented transition

1. Semantic-core now separates opaque typed `DocumentId`/`SchemaId`/`FieldId`/
   `EntityId` from mutable schema/entity/field keys and builds deterministic
   runtime-only address indexes with typed ambiguity/stale-target failures.
2. Workflow owns a replaceable ID-generation seam. Normal CLI creation supplies
   UUIDv7; pure semantic/formula/diff/merge code has no clock or randomness.
3. Formula source parses to a bounded unbound human-address AST, binds and
   type-checks once to stable `EntityId + FieldId`, extracts static dependencies,
   and projects only through round-trip-proven current keys. Rename preserves
   stable IDs/bound ASTs and atomically enforces the 4,096/4,097-byte boundary.
4. Validation, semantic diff, merge, entity lifecycle, CLI, and AI adapter paths
   preserve stable continuity while presenting human keys at authoring seams.
5. Storage performs a two-phase rewrite of all 12 frozen v1 typed-ID locations
   through deterministic UUIDv5 maps, preserving applicable legacy addresses as
   keys and never rewriting durable v1 merely by read/open.
6. `direct-ro/v2` owns complete storage DTOs and canonical identity-aware bytes:
   stable-ID ordering, no Unicode normalization, ECMAScript shortest-roundtrip
   Number tokens, 8 MiB complete-input limit, and 256-byte number-token limit.

### Frozen migration mechanism

- Namespace: `7a199010-e2db-5f4f-a216-07ddb708f5ef`, derived as UUIDv5(URL,
  `https://tachiko.work/migrations/legacy-direct-ro/v1`).
- Exact UTF-8 UUID names use NUL separators and typed prefixes:
  `document\0doc`, `schema\0doc\0schema`,
  `field\0doc\0schema\0field`, and `entity\0doc\0entity`.
- Schema-scoped field mapping and document/schema/field/entity golden UUIDs are
  frozen in code tests and `storage-versioning-and-migration.md`.

### Focused evidence before final gates

- CLI: 31 tests green with generated opaque IDs and human-key addressing.
- Semantic/formula/diff/merge/workflow/storage focused suites are green,
  including deterministic duplicate-key errors, stable rename/diff/merge,
  reused-address projection failure, exact 4,096/4,097 rename behavior,
  replaceable creation, all 12 migration locations, negative mapping classes,
  byte-stable v2 round trips, ADR-0018 numeric vectors, exact resource
  boundaries, Unicode preservation, and legacy read-without-rewrite.
- Final workspace/release-equivalent gates, staged commits, independent code and
  standards reviews, push, and PR creation remain before handoff.

### Final review and verification

- The transition is staged as four reviewable commits: `94b603b` semantic
  contracts, `42177ab` conformance coverage, `6fb4018` representation/specification,
  and `169db41` focused review remediation.
- The first independent review found four code/API gaps: combined merge renames
  could exceed canonical formula projection bounds, storage could recursively
  convert oversized bound ASTs, incoherent map/nested IDs could panic binding,
  and CLI help mislabeled human keys as stable identifiers. Regression-first
  fixes now enforce typed projection failures, iterative shared 256-node/64-depth
  bounds before recursive conversion, checked address/index lookup, and accurate
  human-address terminology.
- The first standards review also required executing—not merely compiling—the
  production semantic corpus under WASM and making the migration visible in the
  changelog. CI and the release gate now run one shared production-API corpus on
  native and `wasm32-unknown-unknown` and compare exact normalized values,
  failures, dependency/cycle evidence, operation order, rename projection, and
  no-silent-retarget results.
- Focused re-review confirmed every original finding is closed, found no new
  P0/P1/P2 code/API or standards/spec findings, and found no Accepted ADR
  amendment pressure. The work does not redo #74 or begin #40 or #72.
- Final `scripts/release-check.sh` passed on clean commit `169db41`: formatting,
  warning-denied Clippy, 200 tests across 27 suites, executed native/WASM parity,
  warning-denied Rustdoc, exact Rust 1.85 checking, audited notices, all eight
  Cargo packages, all four product journeys, native release/archive execution,
  tamper rejection, interrupted cleanup, and concurrent no-clobber publication.

### PR #81 writer/reader closure remediation

- The latest independent ChatGPT review accepted the overall #70 architecture
  and blocked only on one Codex P2: `to_canonical_string()` and `save()` could
  emit direct-ro/v2 bytes larger than the reader's unchanged 8 MiB complete-input
  profile.
- The regression was proven red on the reviewed head: canonical serialization
  admitted exactly 8,388,609 bytes, and `save()` created that output. The exact
  8,388,608-byte writer boundary already round-tripped successfully.
- Commit `f8c13b5` applies the existing v2 resource-profile validator to the
  final canonical string before it can return or reach exclusive file creation.
  Oversized otherwise-valid semantic documents now return the typed
  `FormatError::ResourceLimit`; no representation limit or ADR behavior changed.
- Exact new storage tests are
  `v2_writer_admits_the_exact_input_boundary_and_round_trips`,
  `v2_writer_rejects_canonical_output_one_byte_over_the_input_limit`, and
  `save_rejects_oversized_v2_before_creating_the_destination`. Existing exact
  reader and number-token boundary coverage is unchanged.
- Verification on the clean code commit passed 50 storage tests, 203 workspace
  tests across 27 suites, warning-denied Clippy and Rustdoc, Rust 1.85 all-target
  checking, all eight Cargo packages, all four product smokes, documentation
  consistency, exact native/WASM production parity, and the complete release
  archive/tamper/interruption/concurrency gate.
- PR #81 remains Draft. The existing Codex thread may be resolved only after the
  regression is present and green; final readiness still requires an independent
  review of the exact pushed head and green CI.

### PR #81 blocking-review stability remediation

- Exact reviewed head `7f1550b` reproduced both hard blockers before production
  changes: a cross-entity v2 formula target whose later-sorted entity names a
  missing schema panicked in `FieldRefV2::validate`, and a forward acyclic chain
  of 20,000 bound formulas aborted the calculation test process with `SIGABRT`.
- Commit `bd4172e` replaces the schema-order assumption with a typed
  representation failure and evaluates fields/expressions through explicit
  frames. It adds no durable dependency-chain limit and preserves existing
  left-to-right arithmetic, cycle paths, typed failures, and stable result
  ordering.
- The remaining scoped review actions preserve trimmed human file stems as
  default titles with a blank-stem fallback, report the current build ceiling
  from the legacy-v1 canonicalization helper, cover the exact nonnumeric binding
  payload, document Node.js for the release gate, and strengthen the formula
  smoke's missing-address diagnostic.
- Documentation now separates Accepted ADR-0015/ADR-0017/ADR-0018 invariants
  from Provisional direct-ro/v2 wire/resource mechanisms, describes numeric
  persistence as ADR-0018 semantic preservation, adds explicit authority state
  to the schema/diff/validation specifications, and distinguishes entity from
  numeric formula references.
- Focused storage, formula-engine, and CLI suites; warning-denied focused
  Clippy; formatting; documentation consistency; and the formula-authoring
  product smoke are green. Complete release-equivalent gates and fresh
  exact-head reviews remain required before the Draft PR returns to ChatGPT.

### PR #81 final overview-address closure

- The final ChatGPT adjudication accepted the remaining Codex P2 and narrowed
  closure to validating directly constructed documents before `overview()`
  creates a human-facing projection; no address or diagnostic semantics change.
- A workflow regression was proven red on exact head `dae70fa`: `overview()`
  admitted a directly constructed document with a duplicated schema key.
- `overview()` now reuses `validate_candidate(document)?` before calculation.
  The table-driven regression covers duplicate schema keys, duplicate entity
  keys, and duplicate field keys within one schema, asserting the typed
  `WorkflowError::InvalidDocument` result and `DiagnosticCode::DuplicateKey`.
- The exact regression and all 24 workflow integration tests are green,
  including the existing valid overview and deterministic ordering evidence.
  Complete clean-head release verification and a fresh exact-head Codex review
  remain required before final ChatGPT review.

## Active phase: ADR-0016 workspace-engine boundary (#72)

### Goal and authority

- Evolve `tachiko-workflow` in place into the single shared,
  capability-free `tachiko-workspace-engine` application boundary.
- Worktree branch: `codex/issue-72-workspace-engine`, created from current
  `origin/main` `e953877f2dfd05ae5cebc5262656c2d877c2ed9c`.
- Authority: Issue #72's latest `agent-handoff:v1`, Accepted ADR-0015 through
  ADR-0018, the Product Constitution and Design Principles, and the knowledge
  authority/reconciliation policies.
- Completed prerequisites #70 and #40 are treated as hardened identity,
  formula, storage, canonicalization, numeric, and conformance contracts.

### Audited ownership before migration

- `workflow -> diff-engine, formula-engine, semantic-core` owns starters,
  overview/explanation queries, lifecycle and field mutations, and the shared
  validate/calculate/diff candidate finalizer.
- `ai-api -> diff-engine, formula-engine, semantic-core` repeats formula
  analysis, impact, typed candidate cloning, type checks, formula projection,
  validation, and calculation.
- `cli -> storage, workflow, diff-engine, merge-engine, formula-engine,
  semantic-core` directly coordinates validation, calculation materialization,
  diff, merge-plus-impact, and runtime export in addition to its legitimate
  host parsing, filesystem, persistence, safe-write, and rendering work.
- The clean base builds and passes 215 workspace tests across 28 suites.

### Locked incremental migration plan

1. Add an executable Cargo-metadata assertion for the exact ADR-0016 graph and
   prove it fails against the pre-migration workspace.
2. Rename/evolve the workflow directory and package in place; preserve its
   document-local snapshot operations and host-supplied ID-generation seam.
3. Add workspace-owned validation, calculated-value, formula-analysis,
   semantic-impact, typed-proposal, merge, and runtime-export orchestration.
4. Rebase provider-free AI behavior onto those operations while keeping
   approval DTOs in the AI adapter.
5. Reduce CLI local dependencies to workspace-engine plus storage and retain
   only arguments, OS paths, UUIDv7 host composition, persistence, exclusive
   writes, and rendering.
6. Extend native/WASM production conformance through workspace-engine and AI,
   reconcile current architecture/docs, run release-equivalent verification,
   and independently review the final diff.

### Explicit deferrals

- #10 external Semantic API stability/versioning.
- #23 general validation/diagnostic envelope and temporary-invalid policy.
- #26 resident runtime, Web Worker, IPC/FFI, projection patches, browser/native
  persistence composition, and host capabilities.
- #27/#28 AI capability/approval protocol, #41 `.roproj`, and any new crate or
  semantic/storage/formula contract.

The detailed executable plan is
`docs/superpowers/plans/2026-08-23-workspace-engine-boundary.md`.

### Implemented boundary and pre-review evidence

- Renamed the existing crate/package in place to
  `tachiko-workspace-engine`; there is no second workflow or runtime aggregate.
- Workspace-engine now owns shared validation, complete calculation and
  human-address projection, semantic comparison, merge-plus-impact, formula
  analysis, inert typed proposal validation, mutations, and runtime-export
  projection.
- Provider-free AI delegates semantic analysis/comparison/proposal policy to
  workspace-engine while retaining the existing approval-required adapter DTO
  and error behavior.
- CLI local dependencies are reduced to workspace-engine plus storage. CLI
  retains arguments, host paths, UUIDv7 generation, storage composition,
  exclusive-create writes, and rendering.
- `scripts/workspace-dependency-check.mjs` enforces the exact ADR-0016 graph in
  CI and `scripts/release-check.sh`, including development dependency kinds.
- The portable corpus now executes workspace calculation plus provider-free AI
  formula/proposal behavior on native and `wasm32-unknown-unknown`; exact fixed
  oracles and native/WASM records pass.
- Pre-review verification passes: formatting, docs consistency, dependency
  graph, warning-denied workspace Clippy, 219 tests across 29 suites,
  warning-free Rustdoc, exact Rust 1.85, and all four product smoke journeys.
- #10, #23, #26, #27/#28, and #41 remain explicitly deferred. No new crate,
  semantic aggregate, storage/formula contract, host capability, or
  target-selected semantic behavior was introduced.

The clean committed tree passes `scripts/release-check.sh`, including source
packages, notices, and native archive safety/concurrency checks. Independent
exact-head reviews remain before PR handoff.

## Active phase: ADR-0019 validation report and semantic diagnostics (#89)

### Goal and authority

- Implement the first authoritative first-party semantic ValidationReport and
  the complete ADR-0018 formula failure oracle as conformance work, not an
  architecture redesign.
- Worktree branch: `codex/issue-89-validation-report`, created from
  `origin/main` `342f69f2fc252554c240650d1438cc0d6cd82e2f` and rebased onto current
  `origin/main` `16289f8a5acd48ca7fa36b265b7fdfe7df0e4d12` after #92 and the isolated
  #26 spike landed. The #92 schema-authority and adversarial stack-safety
  coverage remains intact.
- Authority: Issue #89; Accepted ADR-0015 through ADR-0019; the validation,
  diagnostics, formula, and schema specifications; the Product Constitution
  and Design Principles; and the knowledge authority/reconciliation policies.
- The clean base passes 219 workspace tests across 29 suites.

### Audited ownership before migration

- semantic-core's accumulating validator owns current document rules, but its
  diagnostic identity is path-first and has no stable semantic subjects,
  related machine facts, severity, or provider provenance.
- formula-engine owns structural analysis, binding, dependencies, cycle
  detection, and evaluation, but exposes only fail-first `calculate()` errors
  and DFS cycle witnesses rather than the Accepted ADR-0018 full oracle.
- workspace-engine repeats validate/calculate sequencing across first-party
  operations and separately performs formula projection preflight in
  authoring/finalization paths.
- merge-engine, AI, CLI, storage, and the portable harness consume legacy
  surfaces. Storage representation validation remains a sibling responsibility
  and is not part of this migration.

### Locked migration plan

1. Add only generic semantic-core diagnostic/location/fact primitives,
   including opaque provider identity, with no formula or higher-layer
   taxonomy.
2. Implement an authoritative full formula outcome keyed by stable field
   subjects: structural, then binding/type/stale target, complete SCC
   membership, direct failed dependencies, and local evaluation.
3. Derive legacy fail-first `calculate()` behavior from that outcome and
   publish Calculation only on total success.
4. Compose core and formula observations once in workspace-engine as the
   authoritative semantic ValidationReport with deterministic ordering and
   prerequisite/cascade suppression.
5. Reuse the shared semantic outcome across first-party operations while
   retaining projection, authoring, export, and output preflights as explicit
   operation-specific gates rather than universal semantic validity.
6. Reconcile adapters and only clearly owned duplicate orchestration, extend
   native/WASM stable-observation conformance, update implementation-state
   documentation, run the release-equivalent gate, and perform two independent
   exact-head reviews.

### Explicit deferrals

- #10 public Semantic API and wire/version/transaction commitments.
- #13 progressive typing and invalid-draft lifecycle.
- #17 plugin runtime or ABI.
- #26 IPC, WASM/Web Worker/resident-runtime transport.
- #41 `.roproj`.
- New common diagnostic/validation crates, storage/numeric changes, generic
  constraint DSLs, and presentation paths/messages/spans/witnesses as semantic
  authority.

The detailed executable plan is
`docs/superpowers/plans/2026-08-23-validation-report-diagnostics.md`.

### Implemented ownership and pre-review evidence

- semantic-core now owns only generic symbolic diagnostic codes, provisional
  severity, stable semantic subject/location/fact primitives, and opaque
  provider identity plus its own core rules. It contains no formula/workspace
  provider taxonomy or reverse dependency.
- formula-engine's `calculate_full()` is the authoritative ADR-0018 outcome:
  structural, binding/type/stale target, complete cyclic SCC membership, direct
  failed dependencies, then left-to-right local evaluation. Failures are keyed
  by stable value nodes, all static edges are retained, and failed outcomes
  expose no partial `Calculation`. Compatibility `calculate()` is derived from
  that outcome; the DFS evaluator and cycle-witness authority were removed.
- workspace-engine composes core rules and formula outcomes into one
  deterministic `ValidationReport`. Stable observations include symbolic
  meaning, severity, stable subjects/related facts, and opaque provenance;
  human paths/messages and selected cycle presentation are excluded.
- Shared semantic validation is reused by validation, calculation, queries,
  proposals, mutations, comparison, and merge finalization. Canonical formula
  projection remains an explicit authoring/output gate after semantic
  validation, including merged-candidate preflight.
- merge-engine now owns model-level three-way reconciliation only. Workspace
  owns input/candidate semantic validation, projection gates, and impact.
  Storage representation validation remains unchanged and sibling-owned.
- AI propagates the workspace report rather than reconstructing semantic
  diagnostics; CLI continues to render workspace errors at the adapter.
- TDD evidence covers independent accumulation, cascade suppression,
  rename-stable observations, multi-subject duplicates/cycles/dependencies,
  full formula precedence, all-or-nothing calculation, and
  validation/finalization agreement.
- The post-rebase workspace passes 253 tests across 34 suites with
  warning-denied Clippy. The portable corpus executes 34 fixed production
  records and matches exact stable observations natively and on
  `wasm32-unknown-unknown`.
- Independent ADR-0018 and ADR-0019/#89 review cycles found no P0. Their P1
  and P2 findings are addressed by shared SCC membership storage,
  compatibility selection that follows stable left-to-right legacy behavior,
  phase- and fact-specific prerequisite filtering, explicit validation operand
  roles, complete portable dependency fingerprints, and generated/disjoint
  SCC determinism evidence. A clean release-equivalent gate and independent
  exact-head re-reviews of the last fixes remain before PR handoff.
