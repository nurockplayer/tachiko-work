# Canonical Reconciliation Register

Status: Accepted register when merged

Last reconciliation: 2026-08-23

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
| ADR-0010 first usable product workflow | Accepted | Current CLI-first usable workflow. Stale wording about ADR-0003 being unresolved must not be treated as authority. |
| ADR-0011 semantic three-way merge | Accepted for implemented merge contract | Defines the current model-level merge behavior. Broader protocol/versioned conflict semantics remain Open Questions in #45/#46. |
| ADR-0012 tag-gated release distribution | Accepted | Release/distribution contract; independent from semantic architecture. |
| ADR-0013 validated semantic entity lifecycle | Accepted for v0.1 lifecycle contract | Preview-first immutable mutation, typed relationship safety, and non-cascading removal remain authoritative. ADR-0015 supersedes only the parts that treat a human-facing entity identifier as durable identity. |
| ADR-0014 bounded computational formula authoring | Accepted | Current bounded deterministic authoring workflow; deeper binding/numeric semantics are defined by Accepted ADR-0018 and owned by downstream implementation work. |
| ADR-0015 stable semantic identity and mutable human keys | Accepted | Durable objects use stable opaque typed surrogate IDs independent of names, paths, presentation, and content. UUIDv7 is a preferred Provisional generator, not permanent semantic meaning. |
| ADR-0016 Milestone 02 Rust crate layering | Accepted | Accepts the current eight-crate Milestone 02 baseline, evolves workflow into the shared workspace engine, and fixes forbidden dependency directions. Narrower validation/API/runtime seams may be refined only through explicit Accepted decisions. |
| ADR-0017 versioned storage DTOs, explicit migration, and canonical representation | Accepted | Storage owns immutable version-specific DTOs and explicit migrations; version-gated readers fail closed on unsupported/newer semantics; canonical output is version-defined and must preserve ADR-0018's Accepted numeric meaning. |
| ADR-0018 bound formulas and deterministic finite binary64 semantics | Accepted | Stable-ID bound ASTs and partial authoring projection, atomic rename preservation of ADR-0014's byte limit, finite binary64 with one semantic zero, representation-admitted numeric conversion, deterministic arithmetic, static dependencies, recomputation equivalence, and persisted number spelling are current authority. |
| ADR-0019 staged semantic validation and diagnostics | Accepted | Separates hard admission, diagnosable semantic candidates, deterministic full validation, semantic-ID diagnostic meaning, operation gating, and deterministic extension validators without introducing a validation framework or external transport contract. |

## Architecture and specification map

| Artifact | Decision state | Implementation state | Open decision owner |
| --- | --- | --- | --- |
| `docs/architecture/document-model.md` | Accepted direction; detailed graph shape constrained by ADR-0015, ADR-0018, and ADR-0019 | M02 stable identity/bound formula aggregate implemented; richer graph future | ADR-0015, ADR-0018, ADR-0019, #13 |
| `docs/architecture/unified-semantic-model.md` | Accepted direction | Partially implemented | ADR-0015, #13 |
| `docs/architecture/rust-crate-architecture.md` | Accepted ADR-0016 boundary; exact Rust API remains Provisional | Eight-crate workspace-engine target implemented by #72 | ADR-0016, #10 |
| `docs/architecture/ro-and-roproj-format.md` | Accepted direction constrained by ADR-0017 storage boundary | `.ro` direct JSON implemented; `.roproj` not implemented | ADR-0003, ADR-0017, #41, #43 |
| `docs/architecture/ai-native-architecture.md` | Accepted direction | Partially implemented | #10, #27, #28, #30 |
| `docs/architecture/frontend-backend-boundary.md` | Accepted direction; detailed runtime seam Provisional | Partially implemented | #26 |
| `docs/architecture/wasm-strategy.md` | Hypothesis / Open Question | Not implemented as product runtime | #26 |
| `docs/architecture/distributed-collaboration.md` | Hypothesis / Open Question | Not implemented | #12, #45, #46, #48-#50 |
| `docs/architecture/rendering-system.md` | Hypothesis | Not current milestone | Designer MVP future work |
| `docs/architecture/performance-model.md` | Provisional guidance | Mixed | Evidence-driven future work |
| `docs/specs/schema-system.md` | Mixed: current durable declaration boundary Accepted under ADR-0015/ADR-0019; richer constraint vocabulary future | Current M02 type/required/reference declarations implemented | ADR-0015, ADR-0019, #13 |
| `docs/specs/validation-engine.md` | Mixed: staged validation/candidate/full-oracle semantics Accepted under ADR-0019; exact APIs/incremental mechanisms Provisional | Partial: collected semantic diagnostics and strict finalization implemented; validation/finalization symmetry and complete formula failure oracle pending | ADR-0019, ADR-0018 |
| `docs/specs/diagnostics-contract.md` | Mixed: semantic diagnostic stability rules Accepted under ADR-0019; exact Rust/wire/code catalog Provisional or Deferred | Partial: current diagnostics expose codes/path/message but not the complete semantic envelope | ADR-0019, #10, #26 |
| `docs/specs/ro-format-and-roproj-spec.md` | Accepted direction with explicit current-state split | `.ro` direct JSON implemented; `.roproj` future | ADR-0003, ADR-0017 |
| `docs/specs/storage-versioning-and-migration.md` | Mixed: Accepted invariants under ADR-0017; M02 wire mechanics Provisional where marked | Strict v1, deterministic migration, and direct-ro/v2 implemented | ADR-0017, #40 |
| `docs/specs/canonical-json-profile.md` | Mixed: Accepted deterministic/semantic-preservation and admitted-token binary64 rules; exact M02 profile/resource limits version-specific | Implemented independent direct-ro/v2 writer | ADR-0017, ADR-0018, #40 |
| `docs/specs/ro-format-v1.md` | Normative legacy direct-`.ro` JSON compatibility/migration profile | Implemented immutable compatibility reader/writer and migration source | ADR-0017, #40 |
| `docs/specs/ro-format-v2.md` | Mixed: Accepted ADR-0015/ADR-0017/ADR-0018 invariants; M02 wire/resource mechanics Provisional | Implemented current semantic writer | ADR-0015, ADR-0017, ADR-0018, #40 |
| `docs/specs/roproj-format.md` | Accepted direction | Not implemented | ADR-0003, ADR-0017, #41 |
| `docs/specs/roproj-layout-v1.md` | Provisional | Not implemented | #41 |
| `docs/specs/formula-engine-spec.md` | Mixed: Accepted ADR-0014 authoring and ADR-0018 binding/projection, rename preflight, numeric, dependency, and recomputation rules; implementation mechanisms Provisional | Stable binding/projection, atomic rename preflight, normalized Number, static dependencies, and full calculation implemented; complete failure oracle/incremental evaluator pending | ADR-0018 and later formula-engine work |
| `docs/specs/ai-agent-api.md` | Provisional implemented contract under ADR-0007 | Implemented v0.1 read/explain/suggest surface | #10, #27, #28 |
| `docs/specs/collaboration-model.md` | Mixed: current merge Accepted, future collaboration Open Question | Merge implemented; broader collaboration future | ADR-0011, #12, #45, #46 |
| `docs/specs/conflict-resolution.md` | Provisional around current merge; future conflict model Open Question | Partial | #46 |
| `docs/specs/operation-log-model.md` | Open Question | No first-class persisted log in v0.1 | #12, #48 |
| `docs/specs/event-sourcing-model.md` | Hypothesis | Not implemented | #12, #49 |
| `docs/specs/plugin-system.md` | Accepted extensibility direction; concrete runtime Hypothesis/Open Question | No public plugin runtime | #17 |
| `docs/specs/migration-framework.md` | Accepted direction; concrete adapters/mappings Hypothesis | Not implemented as broad migration system | #14, #18, #34 |
| `docs/specs/runtime-export-v1.md` | Frozen historical contract | Superseded as current writer by v2 | compatibility evidence |
| `docs/specs/runtime-export-v2.md` | Provisional derived-output contract | Implemented current runtime writer | implementation evidence / future versioning work |

## GitHub Issue classification

A GitHub Issue is never automatically an Accepted decision. The table below classifies the current backlog by what kind of authority it carries.

### Historical Developer MVP issues

| Issue | Classification | Reconciliation action |
| --- | --- | --- |
| #1 MVP Freeze | Superseded as active work / historical accepted scope | Current roadmap is governed by unified milestones and later ADRs. |
| #2 semantic document model MVP | Implemented historical task | Preserve as implementation history. |
| #3 thin `.ro` portable artifact packaging | Superseded as task shape | Future ADR-0003 `.roproj`-derived package work is tracked by #43. |

### Decision and strategy issues

| Issue | Classification | Notes |
| --- | --- | --- |
| #9 AI authority / canonical source of truth | Accepted direction already captured by ADR-0007; residual details moved | Approval/capability/security detail belongs to #28/#30. |
| #10 Headless Semantic API | Open Question | Current Core & Format Hardening decision. |
| #11 permissions/provenance/transactions | Open Question | Broad team/collaboration decision; Game Dev Alpha minimum is narrowed by #28. |
| #12 mutation history / event sourcing / CRDT / Git | Open Question | Event sourcing/CRDT docs remain non-authoritative hypotheses until promoted. |
| #13 progressive typing | Open Question | Product/architecture compatibility decision. |
| #14 open format/interoperability policy | Open Question constrained by Principles | Reuse-before-invention and user escape paths are Principles; concrete standard policy remains unresolved. |
| #15 licensing/commercial boundary | Open Question | Founder/governance decision after research/legal review. |
| #17 plugin ecosystem / Office migration | Mixed | Extensible-ecosystem direction is Accepted; runtime tiers, sandbox, compatibility, and migration mechanics remain Open Questions. |
| #18 Japan enterprise / gradual Excel migration | Accepted product direction with Hypotheses | Japan as a priority research environment and gradual migration are accepted; individual pain-point claims require user evidence. |
| #19 canonical docs / ADR reconciliation | Completed reconciliation task | Closed after establishing authority precedence, ADR numbering, and canonical reconciliation rules. |

### Core & Format Hardening decisions

- #20 Rust crate layering and dependency direction — resolved by ADR-0016.
- #21 semantic identity, document graph, typed references — durable identity/graph invariants resolved by ADR-0015 and implemented by #70.
- #25 storage DTO / migration boundary — durable architecture resolved by ADR-0017; the M02 implementation and conformance closure are complete through #40.
- #37 format/version envelope — durable fail-closed/version-gated behavior resolved by ADR-0017 and `storage-versioning-and-migration.md`; the M02 implementation is complete through #40.
- #38 canonical value encoding/order — structural/Unicode/order invariants are resolved by ADR-0017, exact numeric spelling by Accepted ADR-0018, and M02 implementation/conformance by #40.
- #23 schema declaration, validation pipeline, diagnostics — resolved by Accepted ADR-0019; implementation/conformance follow-up remains separate from the decision issue.
- #24 formula AST, binding, dependency graph, numeric semantics — resolved by Accepted ADR-0018, the reconciled formula/canonical JSON specifications, research record, and executed native/WASM evidence; the accepted M02 scope is implemented through #70/#40.
- #26 native/WASM runtime boundary — Open Question.
- #72 workflow-to-workspace-engine migration — implementation of ADR-0016 completed by PR #85; it does not settle #10/#26.

#40 is a completed implementation/evidence task that consumed ADR-0015,
ADR-0017, and Accepted ADR-0018 without inventing format semantics.

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

## Reconciliations made in this pass

1. ADR-0002 and ADR-0005 are no longer treated as parallel Accepted authorities; ADR-0002 is historical/superseded.
2. ADR-0008 and ADR-0009 are no longer treated as parallel next-phase authorities; ADR-0008 is historical/superseded.
3. Current direct `.ro` persistence is explicitly separated from the accepted longer-term `.roproj` source / `.ro` portable-artifact architecture.
4. ADR-0015 separates durable semantic identity from mutable human keys and partially supersedes ADR-0013's rename-as-ID-replacement semantics while preserving ADR-0013 as v0.1 implementation history.
5. UUIDv7 is the preferred Provisional normal creation generator, not permanent semantic meaning.
6. ADR-0016 accepts the current Milestone 02 crate layering baseline and forbidden dependency directions while leaving narrower validation/API/runtime seams to later explicit Accepted decisions.
7. ADR-0017 separates semantic types from storage-owned version DTOs, requires explicit version-gated migration, rejects silent unknown/newer interpretation, and makes canonical bytes version-defined rather than serializer-defined.
8. Direct `.ro` JSON v1 is an immutable legacy compatibility/migration
   profile. Identity-aware direct `.ro` v2 is implemented in that namespace;
   future `.roproj` v1 remains a separate representation namespace.
9. Full RFC 8785 JCS is not the editable-source canonical profile; Tachiko reuses appropriate primitives while retaining Git-friendly whitespace/order and #24 numeric authority.
10. ADR-0018 accepts formula binding, deterministic finite-binary64 meaning, and exact numeric canonical spelling as current authority.
11. Full RFC 8785/JCS remains rejected for editable-source canonicalization; only its ECMAScript-compatible number primitive is Accepted for a representation that adopts ADR-0018.
12. ADR-0018's promotion corrections make canonical authoring projection partial, preserve ADR-0014's 4,096-byte limit atomically across rename, and place numeric-token/input resource admission in the representation/profile before semantic conversion without freezing a limit into Number meaning.
13. Event sourcing, public plugin runtime details, collaboration algorithms, `.roproj` sharding, `.ro` package mechanics, and host durability implementation remain outside ADR-0017 and ADR-0018.
14. #70 implements ADR-0015 as one atomic transition: opaque IDs and mutable
   keys, UUIDv7 creation seam, stable formula binding/projection, stable-ID
   diff/merge continuity, deterministic legacy UUIDv5 migration, and
   direct-ro/v2 preservation of ADR-0018 semantic meaning.
15. #40 completes the storage/canonicalization and native/WASM numeric
   conformance closure without reopening Accepted identity or numeric meaning.
16. #72 evolves workflow in place into the single workspace-engine application
   boundary, reduces AI to `ai-api → workspace-engine`, reduces CLI to
   `cli → workspace-engine, storage`, and preserves storage as a sibling.
17. ADR-0019 resolves #23 by separating hard admission from diagnosable semantic
   candidates, accepting one staged full-validation oracle and semantic-ID
   diagnostics, keeping severity distinct from operation gates, preserving
   storage-local failure ownership, and finding no evidence for a new
   validation/diagnostics crate.

## Current research queue

The ordered #70 → #40 → #72 Core & Format Hardening implementation sequence is
complete, and #23's validation/diagnostics architecture is resolved by
ADR-0019. Implementation should now close ADR-0019 conformance gaps without
stabilizing external transport.

#10 remains the next external-interface decision. #26 builds on #10 plus the
Accepted crate/validation boundaries and continues to own resident runtime,
Web Worker, IPC/FFI, and host capability mechanics. #13 remains separately open
for progressive/freeform typing.

If implementation discovers pressure that contradicts an Accepted ADR, return
to an explicit amendment/reconciliation rather than hiding the change in code.

## Founder escalation boundary

Do not ask the founder to choose UUIDv7 versus ULID, JSON map representations, parser libraries, Rust module boundaries, cache indexes, or similar technical mechanisms by preference.

Research those choices against the Constitution, accepted ADRs, migration cost, ecosystem constraints, implementation evidence, and relevant standards.

Return to founder judgment only if the result changes product identity, foundational openness/user-ownership commitments, a difficult-to-reverse public ecosystem promise, or material business/governance posture.
