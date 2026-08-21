# Canonical Reconciliation Register

Status: Accepted register

Last reconciliation: 2026-08-21

## Purpose

This register applies the decision-state vocabulary in `knowledge-authority.md` to the current Tachiko Work knowledge base.

It is an authority map, not a replacement for the underlying ADRs, specifications, Issues, or history.

When this register marks a document as mixed-state, readers must respect the narrower state instead of treating the entire file as equally authoritative.

## Foundational documents

| Artifact | Decision state | Authority note |
| --- | --- | --- |
| `docs/vision/product-constitution.md` | Principle | Highest-level product constraints. |
| `docs/vision/mission.md` | Principle | Purpose, user ownership, anti-lock-in mission, progressive migration. |
| `docs/vision/design-principles.md` | Principle / Accepted | Durable guidance; implementation mechanisms may still evolve. |
| `docs/discussions/2026-08-20-origin-discussion.md` | Historical context | Preserves founding motivation and reasoning; not implementation authority. |
| `docs/discussions/2026-08-20-complete-session-history.md` | Historical context | Preserves evolution and superseded reasoning; current ADRs/specs win. |

## ADR authority map

| ADR | State | Reconciliation note |
| --- | --- | --- |
| ADR-0001 semantic platform, not Office clone | Accepted | Foundational architectural direction. |
| ADR-0002 game-development first wedge | Superseded | Replaced by ADR-0005, which retains the wedge and adds commercial/target-user specificity. Retain for history. |
| ADR-0003 `.roproj` source / `.ro` portable representation | Accepted | Long-term representation relationship. Current v0.1 still persists canonical `.ro` directly; implementation lag does not supersede the ADR. The `.ro` name itself remains provisional until release identity is intentionally frozen. |
| ADR-0004 MVP boundary | Accepted, historical milestone boundary | Defines Developer MVP scope. Its phrase `versioned .ro foundation` describes the implemented MVP persistence path and must not be read as overriding ADR-0003. |
| ADR-0005 game-development first commercial wedge | Accepted | Current first-wedge authority. |
| ADR-0006 CLI-first MVP interface | Accepted | Developer MVP interface decision; GUI remains a later projection. |
| ADR-0007 AI semantic interaction | Accepted | AI is a semantic client; direct mutation requires explicit approval at this stage. |
| ADR-0008 Developer MVP completion / next phase | Superseded | ADR-0009 is the surviving authority for the completion/next-phase boundary. Retain as decision history. |
| ADR-0009 Developer MVP validation / next phase | Accepted, historical milestone boundary | Confirms Developer MVP as a successful architectural validation point. The unified repository milestones now provide current roadmap ordering. |
| ADR-0010 first usable product workflow | Accepted | Current CLI-first usable workflow. |
| ADR-0011 semantic three-way merge | Accepted for implemented merge contract | Defines the current model-level merge behavior. Broader protocol/versioned conflict semantics remain Open Questions in #45/#46. |
| ADR-0012 tag-gated release distribution | Accepted | Release/distribution contract; independent from semantic architecture. |
| ADR-0013 stable semantic identity and typed references | Proposed | #21 research recommends UUIDv7-backed opaque surrogate identity, mutable human keys, typed stores/relationships, and runtime-only derived indexes. Becomes current authority when the ADR is accepted/merged. |

## Architecture and specification map

| Artifact | Decision state | Implementation state | Open decision owner |
| --- | --- | --- | --- |
| `docs/architecture/document-model.md` | Accepted direction; detailed graph shape Provisional | Partially implemented | #21 / proposed ADR-0013 |
| `docs/architecture/unified-semantic-model.md` | Accepted direction | Partially implemented | #21, #13 |
| `docs/architecture/rust-crate-architecture.md` | Provisional implementation baseline | Implemented v0.1 | #20 |
| `docs/architecture/ro-and-roproj-format.md` | Accepted direction | `.ro` implemented, `.roproj` not implemented | #25, #37, #38, #41, #43 |
| `docs/architecture/ai-native-architecture.md` | Accepted direction | Partially implemented | #10, #27, #28, #30 |
| `docs/architecture/frontend-backend-boundary.md` | Accepted direction; detailed runtime seam Provisional | Partially implemented | #26 |
| `docs/architecture/wasm-strategy.md` | Hypothesis / Open Question | Not implemented as product runtime | #26 |
| `docs/architecture/distributed-collaboration.md` | Hypothesis / Open Question | Not implemented | #12, #45, #46, #48-#50 |
| `docs/architecture/rendering-system.md` | Hypothesis | Not current milestone | Designer MVP future work |
| `docs/architecture/performance-model.md` | Provisional guidance | Mixed | Evidence-driven future work |
| `docs/specs/ro-format-and-roproj-spec.md` | Accepted direction with explicit current-state split | `.ro` implemented; `.roproj` future | ADR-0003, #25 |
| `docs/specs/ro-format-v1.md` | Provisional public-format baseline for Core & Format Hardening | Implemented v0.1 | proposed ADR-0013, #25, #37, #38, #40 |
| `docs/specs/roproj-format.md` | Accepted direction | Not implemented | #25, #41 |
| `docs/specs/roproj-layout-v1.md` | Provisional | Not implemented | #41 |
| `docs/specs/formula-engine-spec.md` | Provisional implemented contract | Implemented v0.1 | #24 |
| `docs/specs/ai-agent-api.md` | Provisional implemented contract under ADR-0007 | Implemented v0.1 read/explain/suggest surface | #10, #27, #28 |
| `docs/specs/collaboration-model.md` | Mixed: current merge Accepted, future collaboration Open Question | Merge implemented; broader collaboration future | ADR-0011, #12, #45, #46 |
| `docs/specs/conflict-resolution.md` | Provisional around current merge; future conflict model Open Question | Partial | #46 |
| `docs/specs/operation-log-model.md` | Open Question | No first-class persisted log in v0.1 | #12, #48 |
| `docs/specs/event-sourcing-model.md` | Hypothesis | Not implemented | #12, #49 |
| `docs/specs/plugin-system.md` | Accepted extensibility direction; concrete runtime Hypothesis/Open Question | No public plugin runtime | #17 |
| `docs/specs/migration-framework.md` | Accepted direction; concrete adapters/mappings Hypothesis | Not implemented as broad migration system | #14, #18, #34 |
| `docs/specs/runtime-export-v1.md` | Provisional implemented contract | Implemented v0.1 | implementation evidence / future versioning work |

## GitHub Issue classification

A GitHub Issue is never automatically an Accepted decision.

### Reconciliation gate

| Issue | Classification | Status |
| --- | --- | --- |
| #19 canonical docs / ADR reconciliation | Completed | Closed after PR #58 merged at `c48c6ee8f4259f1f0507ee662f3a149f80e0d337`. `knowledge-authority.md` and this register now define the repository authority model. |

### Historical Developer MVP issues

| Issue | Classification | Reconciliation action |
| --- | --- | --- |
| #1 MVP Freeze | Superseded as active work / historical accepted scope | Close as completed after confirming historical record. |
| #2 semantic document model MVP | Implemented historical task | Close as completed after verifying no residual task is hidden in stale representation wording. |
| #3 thin `.ro` portable artifact packaging | Superseded as task shape | Current v0.1 `.ro` is direct persistence; future ADR-0003 `.roproj`-derived package work is tracked by #43. |

### Decision and strategy issues

| Issue | Classification | Notes |
| --- | --- | --- |
| #9 AI authority / canonical source of truth | Accepted direction already captured by ADR-0007; residual details moved | Approval/capability/security detail belongs to #28/#30. |
| #10 Headless Semantic API | Open Question | Core & Format Hardening decision. |
| #11 permissions/provenance/transactions | Open Question | Broad team/collaboration decision; Game Dev Alpha minimum is narrowed by #28. |
| #12 mutation history / event sourcing / CRDT / Git | Open Question | Explicitly prevents event sourcing/CRDT docs from becoming accidental authority. |
| #13 progressive typing | Open Question | Product/architecture compatibility decision. |
| #14 open format/interoperability policy | Open Question constrained by Principles | Reuse-before-invention and user escape paths are Principles; concrete standard policy remains unresolved. |
| #15 licensing/commercial boundary | Open Question | Founder/governance decision after research/legal review. |
| #17 plugin ecosystem / Office migration | Mixed | Extensible-ecosystem direction is Accepted; runtime tiers, sandbox, compatibility, and migration mechanics remain Open Questions. |
| #18 Japan enterprise / gradual Excel migration | Accepted product direction with Hypotheses | Japan as a priority research environment and gradual migration are accepted; individual pain-point claims require user evidence. |

### Core & Format Hardening decisions

- #21 semantic identity, document graph, typed references: **Provisional recommendation recorded; proposed ADR-0013 under review.**
- #25 storage DTOs, canonical serialization, migration contract: Open Question and next research gate after ADR-0013.
- #37 format/version envelope: Open Question, research together with #25.
- #38 canonical value encoding and deterministic ordering: Open Question, research together with #25.
- #24 formula AST, binding, dependency graph, numeric semantics: Open Question; consume stable identity decision.
- #23 schema declaration, validation pipeline, diagnostics: Open Question; consume stable identity decision.
- #20 Rust crate layering and dependency direction: Open Question; finalize around accepted semantic/storage/formula responsibilities rather than speculative subsystems.
- #26 native/WASM runtime boundary: Open Question; resolve after core ownership seams are clearer.

#40 is an implementation/evidence task that should execute accepted results rather than invent format semantics.

### Game Dev Alpha / AI-safe mutation work

- #27 SemanticPatch: Open Question / protocol design.
- #28 capability, approval, provenance: Open Question / narrow safety contract.
- #29 patch lifecycle: Implementation task after #27/#28.
- #30 AI security boundary: Implementation/security contract constrained by accepted security principles.
- #31 Semantic Analyst: Implementation/evidence task; deterministic semantic facts remain core authority.
- #41 `.roproj` layout: Open Question / Provisional design target.
- #43 `.ro` package profile: Open Question / protocol design.
- #44 Git/CI integration: Implementation task after representation contracts stabilize.

### Later reasoning, migration, collaboration, and standardization

- #32, #33: Open Questions for later reasoning/query APIs.
- #34: Hypothesis/Open Question for post-MVP migration assistant.
- #35: Epic/index only; not decision authority.
- #36: Hypothesis/Open Question for collaboration assistant.
- #39: Hypothesis; explicitly future/post-1.0 unless evidence changes priority.
- #42: Epic/index only; not decision authority.
- #45-#55: Open Questions for later protocol, collaboration/history, conformance, security, integrity, extension, and interoperability contracts. Their existence is not authorization to implement them now.
- #56: Accepted roadmap/administrative direction; close when milestone creation/backfill is verified complete.

## Reconciliations made

1. ADR-0002 and ADR-0005 are no longer parallel Accepted authorities; ADR-0002 is historical/superseded.
2. ADR-0008 and ADR-0009 are no longer parallel next-phase authorities; ADR-0008 is historical/superseded.
3. ADR-0003 is consistently Accepted rather than `proposed`.
4. Current direct `.ro` persistence is separated from the accepted longer-term `.roproj` source / `.ro` portable-artifact architecture.
5. `.ro` v1 implementation details are not automatically permanent identity/serialization invariants.
6. Event sourcing and a first-class persisted operation log remain Hypothesis/Open Question.
7. Concrete plugin runtime tiers and collaboration mechanisms remain Open Questions even though extensibility and semantic-first integration are accepted directions.
8. Implementation evidence remains evidence. It does not silently supersede Accepted ADRs.
9. #21 now has an explicit standards-backed surrogate-identity recommendation instead of asking the founder to choose an ID format by preference.

## Current research queue

Recommended order:

1. Review/promote ADR-0013 for #21 semantic identity and typed references.
2. #25 + #37 + #38 storage DTO, version envelope, and canonical encoding constraints.
3. #24 formula binding/numeric determinism and #23 schema/diagnostics, using ADR-0013 identity as input.
4. #20 crate layering, finalized around the responsibilities established above.
5. #40 executable golden/negative evidence.
6. #26 native/WASM host/runtime boundary after core ownership and dependency seams are clear.

Parallel research is allowed where it does not freeze contradictory contracts.

## Founder escalation boundary

Do not ask the founder to choose UUIDv7 versus ULID, JSON map representations, parser libraries, Rust module boundaries, cache indexes, or similar technical mechanisms by preference.

Research those choices against the Constitution, accepted ADRs, migration cost, ecosystem constraints, implementation evidence, and relevant standards.

Return to founder judgment only if the result changes product identity, foundational openness/user-ownership commitments, a difficult-to-reverse public ecosystem promise, or material business/governance posture.
