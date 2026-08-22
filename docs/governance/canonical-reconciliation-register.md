# Canonical Reconciliation Register

Status: Accepted register when merged

Last reconciliation: 2026-08-22

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
| ADR-0014 bounded computational formula authoring | Accepted | Current bounded deterministic authoring workflow; deeper binding/numeric semantics remain owned by #24. |
| ADR-0015 stable semantic identity and mutable human keys | Accepted | Durable objects use stable opaque typed surrogate IDs independent of names, paths, presentation, and content. UUIDv7 is a preferred Provisional generator, not permanent semantic meaning. |
| ADR-0016 Milestone 02 Rust crate layering | Accepted | Accepts the current eight-crate Milestone 02 baseline, evolves workflow into the shared workspace engine, and fixes forbidden dependency directions. #23–#26 may explicitly amend narrower crate/runtime seams when evidence requires it. |
| ADR-0017 versioned storage DTOs, explicit migration, and canonical representation | Accepted | Storage owns immutable version-specific DTOs and explicit migrations; version-gated readers fail closed on unsupported/newer semantics; canonical output is version-defined and cannot invent unresolved #24 numeric meaning. |

## Architecture and specification map

| Artifact | Decision state | Implementation state | Open decision owner |
| --- | --- | --- | --- |
| `docs/architecture/document-model.md` | Accepted direction; detailed graph shape constrained by ADR-0015 | Partially implemented / identity migration pending | ADR-0015, #23, #24 |
| `docs/architecture/unified-semantic-model.md` | Accepted direction | Partially implemented | ADR-0015, #13 |
| `docs/architecture/rust-crate-architecture.md` | Provisional v0.1 baseline plus Accepted ADR-0016 target | Implemented v0.1; target migration pending | ADR-0016 |
| `docs/architecture/ro-and-roproj-format.md` | Accepted direction constrained by ADR-0017 storage boundary | `.ro` direct JSON implemented; `.roproj` not implemented | ADR-0003, ADR-0017, #41, #43 |
| `docs/architecture/ai-native-architecture.md` | Accepted direction | Partially implemented | #10, #27, #28, #30 |
| `docs/architecture/frontend-backend-boundary.md` | Accepted direction; detailed runtime seam Provisional | Partially implemented | #26 |
| `docs/architecture/wasm-strategy.md` | Hypothesis / Open Question | Not implemented as product runtime | #26 |
| `docs/architecture/distributed-collaboration.md` | Hypothesis / Open Question | Not implemented | #12, #45, #46, #48-#50 |
| `docs/architecture/rendering-system.md` | Hypothesis | Not current milestone | Designer MVP future work |
| `docs/architecture/performance-model.md` | Provisional guidance | Mixed | Evidence-driven future work |
| `docs/specs/ro-format-and-roproj-spec.md` | Accepted direction with explicit current-state split | `.ro` direct JSON implemented; `.roproj` future | ADR-0003, ADR-0017 |
| `docs/specs/storage-versioning-and-migration.md` | Mixed: Accepted invariants under ADR-0017; M02 wire mechanics Provisional where marked | Not yet implemented | ADR-0017, #25, #37 |
| `docs/specs/canonical-json-profile.md` | Mixed: Accepted deterministic/semantic-preservation rules; exact M02 profile mechanics version-specific | Not yet implemented as independent writer | ADR-0017, #38, #24 for numeric edge semantics |
| `docs/specs/ro-format-v1.md` | Normative legacy direct-`.ro` JSON compatibility/migration profile | Implemented v0.1 and still current writer until migration | ADR-0017, #25, #40, #70 |
| `docs/specs/roproj-format.md` | Accepted direction | Not implemented | ADR-0003, ADR-0017, #41 |
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
- #21 semantic identity, document graph, typed references — durable identity/graph invariants resolved by ADR-0015; implementation migration remains #70.
- #25 storage DTO / migration boundary — durable architecture resolved by ADR-0017; implementation work remains.
- #37 format/version envelope — durable fail-closed/version-gated behavior resolved by ADR-0017 and `storage-versioning-and-migration.md`; M02 implementation remains.
- #38 canonical value encoding/order — structural/Unicode/order invariants resolved by ADR-0017 and `canonical-json-profile.md`; exact numeric spelling remains Deferred to #24.
- #23 schema declaration, validation pipeline, diagnostics — Open Question.
- #24 formula AST, binding, dependency graph, numeric semantics — Open Question and current owner of numeric semantic edge cases.
- #26 native/WASM runtime boundary — Open Question.

#40 is an implementation/evidence task that consumes ADR-0015 and ADR-0017 rather than inventing format semantics. Numeric edge fixtures must wait for #24.

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
6. ADR-0016 accepts the current Milestone 02 crate layering baseline and forbidden dependency directions while leaving #23–#26 narrower seams amendable through later Accepted decisions.
7. ADR-0017 separates semantic types from storage-owned version DTOs, requires explicit version-gated migration, rejects silent unknown/newer interpretation, and makes canonical bytes version-defined rather than serializer-defined.
8. Direct `.ro` JSON v1 is now classified as an immutable legacy compatibility/migration profile. A future direct `.ro` v2 and future `.roproj` v1 occupy distinct representation namespaces.
9. Full RFC 8785 JCS is not the editable-source canonical profile; Tachiko reuses appropriate primitives while retaining Git-friendly whitespace/order and #24 numeric authority.
10. Exact numeric canonical spelling remains Deferred to #24 even though other #38 canonicalization invariants are Accepted.
11. Event sourcing, public plugin runtime details, collaboration algorithms, `.roproj` sharding, `.ro` package mechanics, and host durability implementation remain outside ADR-0017.

## Current research queue

The next highest-value Core & Format Hardening work is now:

1. #24 formula binding/numeric determinism and #23 schema/diagnostics, using ADR-0015/0016/0017 as inputs.
2. Implement #25/#37/#38 under ADR-0017, then execute #40 golden/negative evidence and integrate #70 identity migration.
3. #26 native/WASM host/runtime boundary after the semantic/storage/application seams are implemented enough to pressure-test.

Parallel implementation is allowed where it does not freeze unresolved #23/#24/#26 semantics. If implementation discovers pressure that contradicts an Accepted ADR, return to an explicit amendment/reconciliation rather than hiding the change in code.

## Founder escalation boundary

Do not ask the founder to choose UUIDv7 versus ULID, JSON map representations, parser libraries, Rust module boundaries, cache indexes, or similar technical mechanisms by preference.

Research those choices against the Constitution, accepted ADRs, migration cost, ecosystem constraints, implementation evidence, and relevant standards.

Return to founder judgment only if the result changes product identity, foundational openness/user-ownership commitments, a difficult-to-reverse public ecosystem promise, or material business/governance posture.
