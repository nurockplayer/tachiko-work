# Issue 29 Semantic Patch Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the trusted, provider-neutral lifecycle that turns one immutable revision-pinned semantic proposal into scoped review evidence, exact finite Human Approval, one atomic semantic publication, and a verified execution receipt.

**Architecture:** `tachiko-workspace-engine` will own a provisional in-process lifecycle/authorization module because it is the current trusted Semantic API application boundary. The module will reuse one typed stable-ID `SemanticCommand` vocabulary for both Propose and Execute, evaluate ordered batches on cloned snapshots through the existing validation/formula/diff path, and keep proposal/Approval state outside the canonical `Document`. A `SemanticPublicationAuthority` trait will let a caller provide an opaque current revision plus compare-and-publish mechanics; #29 will orchestrate reservation, live rechecks, approval consumption, and verification while leaving concrete revision tokens, resident sessions, revision advancement, and state installation to #93.

**Tech Stack:** Rust 2024 (MSRV 1.85.0), existing semantic/workspace/formula/diff engines, `thiserror`, Cargo, and the repository release gate. No new dependency, wire format, provider, filesystem effect, or public protocol stabilization.

**Scope:** GitHub Issue #29 only.

**Authority:** ADR-0007, ADR-0015, ADR-0019, ADR-0020, ADR-0022, ADR-0024, ADR-0026, [`semantic-api.md`](../../specs/semantic-api.md), and [`semantic-authorization.md`](../../specs/semantic-authorization.md). Issue wording such as “approval digest mismatch” is implemented as Accepted trusted structural `ExactChangeBinding` mismatch; no digest or canonical bytes are selected.

## Global constraints

- Reuse ADR-0020 `Command | AtomicBatch` meaning; do not add a JSON/storage/AI patch-operation vocabulary.
- Target stable `EntityId + FieldId` and bind complete typed `Value`/bound `Expression` meaning before proposal identity is issued.
- Keep immutable proposal contents, authorization state, approval state, lifecycle evidence, and receipts outside `semantic-core::Document` and storage representations.
- Derive relational operation-family/mutation-class/scope requirements in trusted workspace code. Never accept a client-declared footprint or flatten capability dimensions for authorization.
- Require explicit live Query coverage before returning preview/review evidence and live Propose, Approve, and Execute coverage for their independent actions.
- Require one exact finite Human Approval whenever origin or execution authority is Delegated; bind proposal occurrence, complete exact change/base, originator, executor, relational footprint, and effective policy selection.
- Preserve the common direct-Human publication law: require an active Human executor with live Execute authority, use the originator occurrence's retained immutable kind to select the path, and require originator activity only when execution is Approval-gated.
- Recheck base, principal occurrences, Grant state, policy continuity, exact binding, validation, and gate immediately before publication.
- Evaluate every batch on a clone and offer only the final validated candidate to the publication boundary. Never publish a successful prefix.
- Consume Approval only after the publication authority proves one successful compare-and-publish; failure before publication leaves it unconsumed, and replay fails distinctly.
- Verify the exact immutable installed occurrence/document/revision snapshot returned by the guarded publication result, without rereading a later mutable head, and return semantic diff, validation, revision, and minimum provenance evidence.
- Leave concrete revision encoding/generation, resident state/session topology, concurrency algorithm, and state installation to #93; leave disclosure redaction/side-channel hardening, raw bypass, and external effects to #30.
- Do not add persistence, `.roproj` writes, Git/network/process effects, event sourcing, undo, reusable approval policy, enterprise IAM, or public Rust/Serde/wire stability promises.

---

### Task 1: Lifecycle contract and failing acceptance tests

**Files:**
- Create: `crates/workspace-engine/tests/patch_lifecycle.rs`
- Create: `crates/workspace-engine/src/patch_lifecycle.rs`
- Modify: `crates/workspace-engine/src/lib.rs`

- [x] Add failing integration tests for a successful Delegated one-field proposal through preview, Human Approval, atomic publication, post-apply verification, resulting revision, semantic diff, and complete provenance.
- [x] Add failing tests for ordered multi-operation publication, final validation rejection, a middle-command failure with no prefix write, and formula/dependency impact evidence.
- [x] Add failing tests for stale base before and during publication, exact Approval/proposal binding mismatch, missing Propose/Approve/Execute capability, Value-versus-Formula separation, and relational scope coverage.
- [x] Add failing tests for finite expiry, revocation, live Grant loss, replay of consumed Approval, policy transition/rollback invalidation, wrong executor, and direct-Human execution without fabricated Approval provenance.
- [x] Run the focused test target and capture the initial compilation failure proving the lifecycle API does not yet exist.

### Task 2: Immutable proposal, typed command batch, and review evidence

**Files:**
- Modify: `crates/workspace-engine/src/patch_lifecycle.rs`
- Modify: `crates/workspace-engine/src/lib.rs`

- [x] Add opaque provisional identifiers, `SemanticCommand::SetFieldValue`, non-empty ordered `AtomicBatch`, immutable `SemanticPatch`, and representation-neutral structural `ExactChangeBinding` types.
- [x] Apply the same command body for Propose and Execute by reusing existing stable-ID field candidate construction, full validation/calculation, formula projection, and semantic diff functions.
- [x] Derive trusted associated write requirements from each ordered command and derive disclosure requirements from every exact command target/value, changed values, bound references/formulas, formula impact subjects, and causes.
- [x] Return a reduced machine-readable semantic-change projection, formula dependency/impact evidence, authoritative `ValidationReport`, relational `AuthorizationFootprint`, and mutation-class risk summary without exposing the diff engine's private full snapshots.
- [x] Record external lifecycle stages for Draft, Planned, Previewed, Validated, AwaitingApproval, Approved, Applied, Verified, and the required failure states without mutating proposal contents.

### Task 3: Scoped authorization and exact finite Human Approval

**Files:**
- Modify: `crates/workspace-engine/src/patch_lifecycle.rs`

- [x] Add trusted host-provisioned, domain-scoped immutable Human/Delegated principal occurrences and non-reusable Grant occurrences with terminal revocation.
- [x] Implement the closed document-local scope atoms and containment checks, qualified by one non-reusable document-scope occurrence, while preserving complete action/family/class/scope associations.
- [x] Enforce independent Query, Propose, Approve, and Execute checks with deterministic live Grant selection and default denial.
- [x] Record proposal provenance with originator, Propose Grants, footprint, exact binding, and trusted effective policy version.
- [x] Require an approver-specific authorized preview before issuing one immutable finite Human Approval bound to the exact proposal, executor, footprint, policy version, policy-selection occurrence, and authorizing Approve Grants.
- [x] Implement Active to Consumed/Revoked/Expired state, non-reuse, live authorizing-Grant rechecks, and transition-aware policy invalidation that cannot be revived by rollback.

### Task 4: Atomic execution, verification, and receipt

**Files:**
- Modify: `crates/workspace-engine/src/patch_lifecycle.rs`
- Modify: `crates/workspace-engine/tests/patch_lifecycle.rs`

- [x] Define the abstract `SemanticPublicationAuthority` seam over caller-supplied opaque revisions, with errors contractually proving no publication.
- [x] Implement approval-gated and direct-Human Execute through one common path with exact binding, stale, live principal/Grant/policy, footprint, validation, and gate rechecks.
- [x] Hold exclusive lifecycle state while the host invokes a fresh trusted-time live-authorization callback and compares/publishes the complete final candidate, then mark Approval Consumed immediately after the infallible successful publication result; never consume on a proved no-publication failure.
- [x] Capture and return the exact installed occurrence/document/revision snapshot before releasing the publication guard, validate that immutable result even if a later writer advances the live head, and return a machine-readable execution receipt with diff, validation, base/result revisions, Grant references, principals, policy, and truthful Approval evidence.
- [x] Preserve machine-distinguishable stale, validation, capability, approval mismatch/expiry/revocation/replay, retryable proved-no-publication conflict, terminal integrity/verification conflict, and verification outcomes without exposing host-effect authority.

### Task 5: Documentation, validation, review, and delivery

**Files:**
- Modify: `docs/specs/semantic-api.md`
- Modify: `docs/specs/semantic-authorization.md`
- Modify: `docs/architecture/rust-crate-architecture.md`
- Modify: `docs/governance/canonical-reconciliation-register.md`
- Modify: `CHANGELOG.md`

- [x] Update implementation-status prose only: identify the provisional snapshot/publication seam now implemented by #29 and retain #30/#93 ownership boundaries unchanged.
- [x] Run focused workspace-engine tests, formatting, warnings-as-errors Clippy, workspace/all-target tests, exact Rust 1.85 compatibility, native/WASM conformance, and the clean-commit `scripts/release-check.sh` gate.
- [ ] Review the complete diff against Issue #29 and Accepted authority, request independent review, and address every actionable finding with focused regression coverage.
- [ ] Open one Issue #29 PR, monitor all required checks/review threads, merge with head-match protection, and verify live `main`, Issue closure, and Project status.
- [ ] Recalibrate live `main` and the Product Roadmap before selecting the next genuinely Ready critical-path Issue.
