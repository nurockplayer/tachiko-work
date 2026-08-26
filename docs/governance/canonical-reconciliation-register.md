# Canonical Reconciliation Register

Status: Accepted register when merged

Last reconciliation: 2026-08-26

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
| ADR-0007 AI semantic interaction | Accepted | AI is a delegated semantic client with no intrinsic authority; validity/gating and authorization/approval are independent prerequisites; current MVP AI-originated canonical mutation remains explicitly approval-gated. |
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
| ADR-0020 first-class Headless Semantic API boundary | Accepted | Makes one transport-neutral Semantic API mandatory for first-party semantic clients; accepts query/command, Propose/Execute, gate/result, atomic publication/batch, capability-addressability, and compatibility laws while explicitly keeping current Rust/serde/transport/runtime shapes replaceable. |
| ADR-0021 progressive semantic strengthening | Accepted | Semantic-first does not imply schema-first; legitimate weaker semantic content may be explicitly strengthened without weakening the current typed Entity/Reference/Formula contracts or fabricating universal identity. |
| ADR-0022 resident semantic runtime and host boundary | Accepted | Interactive authoritative semantic state belongs to the shared Rust semantic/application runtime; resident topology is preferred; frontend projection state and host persistence/capabilities remain non-authoritative; native/WASM preserve equivalent Stable semantics while concrete session/revision/transport/persistence mechanics remain Deferred. |
| ADR-0023 `.roproj/v1` canonical tree and entity sharding | Accepted | The editable-source v1 namespace has one exact 18-file tree, manifest-first dispatch, complete version-owned DTOs, fixed SHA-256-based physical placement, canonical JSON/JSONL bytes, inline formulas, and no path/line semantic identity. Production codecs, resource/error profiles, host durability, and adjacent package/Git/delta/merge protocols remain Deferred. |
| ADR-0024 revision-pinned SemanticPatch proposal envelope | Accepted | One immutable opaque proposal occurrence binds the Semantic API compatibility contract, exact semantic base, body kind, complete typed Command semantics, and AtomicBatch order without defining another operation/version vocabulary. Proposal/revision encodings and lifecycle/runtime implementation remain Provisional/Deferred to #29/#93; ADR-0026 consumes the structural exact binding without selecting canonical bytes or a digest/token profile. |
| ADR-0026 scoped semantic authorization and approval | Accepted | Defines opaque domain-scoped Human/Delegated principals, independent Query/Propose/Execute/Approve and mutation-class dimensions, document-local stable-ID scope concepts, trusted AuthorizationFootprint derivation with associated class/scope coverage, non-reusable default-deny Grant occurrences, exact finite Human Approval for Delegated-origin or Delegated-authority publication, atomic consumption with at-most-one successful semantic publication, live authority rechecks, replay/revocation, minimum provenance, and semantic/external-effect separation. ADR-0007 MVP Query/Propose behavior is preserved through explicit host provisioning. Authorizing Approve Grant references must remain valid; fresh Execute authority is rechecked. Exact DTO/storage/clock/result/wire mechanisms remain Provisional; canonical bytes/digest/signature/MAC/portable tokens and broader IAM/policy scope remain Deferred. |

## Architecture and specification map

| Artifact | Decision state | Implementation state | Open decision owner |
| --- | --- | --- | --- |
| `docs/architecture/document-model.md` | Accepted direction constrained by ADR-0015/ADR-0018/ADR-0019/ADR-0021; exact future graph/content kinds remain Provisional | M02 stable identity/bound formula aggregate implemented; richer mixed-content graph future | ADR-0015, ADR-0018, ADR-0019, ADR-0021; future object-model work |
| `docs/architecture/unified-semantic-model.md` | Accepted direction; progressive strengthening constrained by ADR-0021 | Partially implemented | ADR-0015, ADR-0021; future object-model work |
| `docs/architecture/rust-crate-architecture.md` | Accepted ADR-0016 crate boundary + ADR-0020 Semantic API mapping + ADR-0024 proposal ownership + ADR-0026 authorization boundary + ADR-0022 runtime/host ownership; exact Rust/session/authorization/transport mechanisms remain Provisional/Deferred | Eight-crate workspace-engine target implemented by #72; validation composition by #89; current inert one-field suggestion is neither SemanticPatch nor Approval; runtime and authorization enforcement remain unimplemented | ADR-0016, ADR-0019, ADR-0020, ADR-0022, ADR-0024, ADR-0026; #29/#30/#93–#95 implementation |
| `docs/architecture/ro-and-roproj-format.md` | Accepted source/artifact direction plus exact `.roproj/v1` materialization under ADR-0023 | `.ro` direct JSON implemented; `.roproj` codec not implemented | ADR-0003, ADR-0017, ADR-0023, #43 |
| `docs/architecture/ai-native-architecture.md` | Accepted direction constrained by ADR-0007/ADR-0020/ADR-0021/ADR-0024/ADR-0026 | Partially implemented; no general SemanticPatch/Approval/Execute path | ADR-0007, ADR-0020, ADR-0021, ADR-0024, ADR-0026; #29/#30/#93 implementation |
| `docs/architecture/frontend-backend-boundary.md` | Accepted Semantic API client boundary under ADR-0020, revision-pinned proposal authority under ADR-0024, scoped authorization/Approval under ADR-0026, and resident runtime/host separation under ADR-0022; concrete mechanisms Deferred | Projection/UI boundary partially implemented; authorization, proposal lifecycle, and resident runtime/session implementation deferred to #29/#30/#93–#95 | ADR-0020, ADR-0022, ADR-0024, ADR-0026; #29/#30/#93–#95 implementation |
| `docs/architecture/wasm-strategy.md` | Accepted runtime direction under ADR-0022; Worker/session/ABI/persistence mechanics Deferred | Portable/native-WASM conformance and PR #91 topology evidence exist; production browser runtime not implemented | ADR-0022; #93–#95 and future transport/host implementation |
| `docs/architecture/distributed-collaboration.md` | Hypothesis / Open Question | Not implemented | #12, #45, #46, #48-#50 |
| `docs/architecture/rendering-system.md` | Hypothesis | Not current milestone | Designer MVP future work |
| `docs/architecture/performance-model.md` | Provisional guidance | Mixed | Evidence-driven future work; ADR-0022 benchmark is topology evidence, not SLA |
| `docs/specs/semantic-api.md` | Mixed: ADR-0020 first-class boundary and semantic laws Accepted; ADR-0024 immutable proposal/exact-base laws Accepted; ADR-0026 defines the authorization/Approval contract that consumes them; ADR-0021/ADR-0022 constrain strengthening/runtime hosting; exact Rust API, complete operation catalogue, encodings, result shapes, session, and wire mappings Provisional/Deferred | Partially implemented by workspace-engine; no general SemanticPatch, AtomicBatch, authorization, or Approval path | ADR-0020, ADR-0021, ADR-0022, ADR-0024, ADR-0026; #29/#30/#93–#95 implementation |
| `docs/specs/semantic-authorization.md` | Normative Accepted Principal/capability/scope/Grant/AuthorizationFootprint/exact-Approval/expiry-replay-revocation/provenance/effect-separation contract under ADR-0026; exact identifiers, DTOs, storage, clocks, codes, and wire formats Provisional; canonical bytes/digest/signature/MAC/token Deferred | Not implemented; current `Suggestion.requires_approval` is only inert safety evidence | ADR-0026; #29 lifecycle/state/receipts, #30 enforcement/denials, #93 revision/concurrency |
| `docs/specs/schema-system.md` | Mixed: current durable declaration boundary Accepted under ADR-0015/ADR-0019; progressive strengthening/mixed-strength rules Accepted under ADR-0021; richer schema vocabulary future | Current M02 type/required/reference declarations implemented; no general freeform/inference runtime | ADR-0015, ADR-0019, ADR-0021; future schema/promotion work |
| `docs/specs/validation-engine.md` | Mixed: staged validation/candidate/full-oracle semantics Accepted under ADR-0019; ADR-0020 maps report/gate meaning into the Semantic API; ADR-0021 makes applicability follow declared semantic facts; exact APIs/incremental mechanisms Provisional | M02 validation oracle implemented by #89 over #90's formula oracle | ADR-0019, ADR-0018, ADR-0020, ADR-0021; #95 incremental implementation |
| `docs/specs/diagnostics-contract.md` | Mixed: semantic diagnostic stability rules Accepted under ADR-0019; ADR-0020 adds unknown-code and authoritative-gate client compatibility laws; exact Rust/wire/catalog Provisional/Deferred | Internal semantic-first envelope and workspace report implemented by #89; concrete external transport mapping deferred | ADR-0019, ADR-0020, ADR-0022; future transport mapping |
| `docs/specs/ro-format-and-roproj-spec.md` | Accepted source/artifact direction and `.roproj/v1` representation boundary with explicit current-state split | `.ro` direct JSON implemented; `.roproj` codec future | ADR-0003, ADR-0017, ADR-0023 |
| `docs/specs/storage-versioning-and-migration.md` | Mixed: Accepted ADR-0017 invariants and `.roproj/v1` namespace/dispatch under ADR-0023; direct-JSON mechanics Provisional where marked | Strict direct-ro/v1, deterministic migration, and direct-ro/v2 implemented; `.roproj/v1` codec pending | ADR-0017, ADR-0023, #40 |
| `docs/specs/canonical-json-profile.md` | Mixed: Accepted deterministic/semantic-preservation and binary64 rules plus `.roproj/v1` JSON/JSONL/tree profile; direct-JSON resource limits remain version-specific | Implemented direct-ro/v2 writer; `.roproj/v1` writer pending | ADR-0017, ADR-0018, ADR-0023, #40 |
| `docs/specs/ro-format-v1.md` | Normative legacy direct-`.ro` JSON compatibility/migration profile | Implemented immutable compatibility reader/writer and migration source | ADR-0017, #40 |
| `docs/specs/ro-format-v2.md` | Mixed: Accepted ADR-0015/ADR-0017/ADR-0018 invariants; M02 wire/resource mechanics Provisional | Implemented current semantic writer | ADR-0015, ADR-0017, ADR-0018, #40 |
| `docs/specs/roproj-format.md` | Normative Accepted `.roproj/v1` version-owned DTO and wire contract | Not implemented | ADR-0017, ADR-0018, ADR-0023 |
| `docs/specs/roproj-layout-v1.md` | Normative Accepted `.roproj/v1` tree, sharding, path, and canonicalization contract | Not implemented | ADR-0023 |
| `docs/specs/formula-engine-spec.md` | Mixed: Accepted ADR-0014 authoring and ADR-0018 binding/projection, rename preflight, numeric, dependency, and recomputation rules; implementation mechanisms Provisional | Stable binding/projection, atomic rename preflight, normalized Number, static dependencies, and complete atomic full-recompute oracle implemented; incremental evaluator pending | ADR-0018 and later formula-engine work |
| `docs/specs/ai-agent-api.md` | Mixed: AI delegated-client direction Accepted under ADR-0007/ADR-0020; revision-pinned proposal rules under ADR-0024; scoped authorization/Approval under ADR-0026; strengthening constrained by ADR-0021; current adapter DTOs Provisional | Implemented v0.1 read/explain/suggest adapter; current `Suggestion` is neither SemanticPatch nor Approval; no general apply path | ADR-0007, ADR-0020, ADR-0021, ADR-0024, ADR-0026; #29/#30/#93 implementation |
| `docs/specs/security-model.md` | Mixed: ADR-0007/ADR-0026 semantic authorization laws Accepted; plugin isolation, migration sandboxing, exact enforcement, and external-effect mechanisms Provisional/Deferred or separately owned | Documentation only; authorization lifecycle/enforcement pending #29/#30/#93 | ADR-0007, ADR-0026; #17/#29/#30/#93 and domain decisions |
| `docs/security/threat-model.md` | Mixed: ADR-0007/ADR-0026 authorization threats/laws Accepted; supply-chain controls, trust labels, denial codes, bypass, and host-effect mechanisms Provisional/Deferred or separately owned | Documentation only; security enforcement and regression tests pending #30 | ADR-0007, ADR-0026; #30 and domain decisions |
| `docs/specs/collaboration-model.md` | Mixed: current merge Accepted, future collaboration Open Question | Merge implemented; broader collaboration future | ADR-0011, #12, #45, #46 |
| `docs/specs/conflict-resolution.md` | Provisional around current merge; future conflict model Open Question | Partial | #46 |
| `docs/specs/operation-log-model.md` | Open Question | No first-class persisted log in v0.1 | #12, #48 |
| `docs/specs/event-sourcing-model.md` | Hypothesis | Not implemented | #12, #49 |
| `docs/specs/plugin-system.md` | Accepted extensibility direction; concrete runtime Hypothesis/Open Question | No public plugin runtime | #17 |
| `docs/specs/migration-framework.md` | Accepted direction; concrete adapters/mappings Hypothesis | Not implemented as broad migration system | #14, #18, #34; ADR-0021 supplies strengthening/mapping review principles only |
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
| #9 AI authority / canonical source of truth | Resolved by amended ADR-0007 | AI has no intrinsic authority; validation/gating and authorization/approval are separate; current MVP delegated mutation remains approval-gated. ADR-0024 supplies the immutable proposal/base contract and ADR-0026 the scoped authorization/exact-Approval contract. ADR-0007's current Query/Propose defaults are preserved through explicit host provisioning; enforcement remains #29/#30/#93. |
| #10 Headless Semantic API | Resolved by ADR-0020 | First-class transport-neutral semantic boundary is Accepted; complete operation catalogue and concrete wire mapping remain separately owned, while ADR-0026 now resolves the narrow MVP authorization contract. |
| #11 permissions/provenance/transactions | Open Question | Broad team/collaboration decision; ADR-0026 resolves only the Game Dev Alpha minimum and leaves enterprise/team policy, reusable approvals, and recovery questions here. |
| #12 mutation history / event sourcing / CRDT / Git | Open Question | Event sourcing/CRDT docs remain non-authoritative hypotheses until promoted. |
| #13 progressive typing | Resolved by ADR-0021 | Progressive semantic strengthening is Accepted without weakening the current strongly typed core; concrete freeform kinds, identity thresholds, promotion commands, storage, and UI remain Deferred. |
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
- #23 schema declaration, validation pipeline, diagnostics — resolved by Accepted ADR-0019; first-party implementation/conformance is completed by #89.
- #24 formula AST, binding, dependency graph, numeric semantics — resolved by Accepted ADR-0018, the reconciled formula/canonical JSON specifications, research record, and executed native/WASM evidence; the accepted M02 scope is implemented through #70/#40 and the #90-owned full failure oracle.
- #10 Headless Semantic API — resolved by Accepted ADR-0020 and `semantic-api.md`; it stabilizes semantic laws and ownership, not current Rust/serde/transport shapes.
- #13 progressive typing — resolved by Accepted ADR-0021; it accepts explicit semantic strengthening and mixed-strength content while deferring concrete freeform runtime/object/storage mechanics.
- #26 native/WASM runtime boundary — resolved by Accepted ADR-0022 and PR #91 evidence; resident runtime ownership, host separation, explicit snapshot boundaries, and native/WASM semantic parity are Accepted while concrete session/revision/Worker/ABI/persistence mechanics remain Deferred.
- #72 workflow-to-workspace-engine migration — implementation of ADR-0016 completed by PR #85; it provides implementation evidence for ADR-0020/ADR-0022 without defining a public source/session/transport contract.

#40 is a completed implementation/evidence task that consumed ADR-0015, ADR-0017, and Accepted ADR-0018 without inventing format semantics.

### Game Dev Alpha / AI-safe mutation work

- #27 SemanticPatch: resolved by ADR-0024 as an immutable revision-pinned proposal envelope around ADR-0020 `Propose(Command | AtomicBatch)`; ADR-0026 consumes its structural binding without selecting digest/token bytes; exact wire/ID/revision and lifecycle implementation remain Provisional/#29/#93.
- #28 capability, approval, provenance: resolved by ADR-0026 and `semantic-authorization.md` as the narrow MVP safety contract; broader enterprise/team policy remains #11.
- #29 patch lifecycle: implementation task consuming ADR-0024/ADR-0026; it must not invent another mutation vocabulary or authorization contract.
- #30 AI security boundary: implementation/security task consuming ADR-0026; owns instruction/data separation, raw-mutation bypass prevention, disclosure-safe denials, external-effect enforcement, and security regression tests.
- #31 Semantic Analyst: Implementation/evidence task; deterministic semantic facts remain core authority and queries use ADR-0020's boundary.
- #41 `.roproj` layout: durable decision resolved by ADR-0023 and the normative `.roproj/v1` format/layout specifications; production codec implementation remains separate.
- #43 `.ro` package profile: Open Question / protocol design.
- #44 Git/CI integration: Implementation task after representation contracts stabilize.

### Later runtime, reasoning, migration, collaboration, and standardization

- #93: later Designer-MVP resident workspace session + revision-safe command implementation under ADR-0022.
- #94: later selective semantic query/projection invalidation implementation under ADR-0022.
- #95: later retained incremental engine-state implementation constrained by full-oracle equivalence.
- #32, #33: Open Questions for later reasoning/query APIs; any resulting first-party semantic operations must fit ADR-0020.
- #34: Hypothesis/Open Question for post-MVP migration assistant; ADR-0021 constrains any future promotion/mapping semantics.
- #35: Epic/index only; not decision authority.
- #36: Hypothesis/Open Question for collaboration assistant.
- #39: Hypothesis; explicitly future/post-1.0 unless evidence changes priority.
- #42: Epic/index only; not decision authority.
- #45-#55: Open Questions for later protocol, collaboration/history, conformance, security, integrity, extension, and interoperability contracts. Their existence is not authorization to implement them now.
- #56: Accepted roadmap/administrative direction; close when milestone creation/backfill is verified complete.
- #104 Project Memory / semantic decision provenance: Research/Hypothesis; may pressure-test ADR-0020 as a domain/reference application but may not promote Project Memory vocabulary into semantic core without separate evidence/decision work.

## Reconciliations made in this pass

1. ADR-0002 and ADR-0005 are no longer treated as parallel Accepted authorities; ADR-0002 is historical/superseded.
2. ADR-0008 and ADR-0009 are no longer treated as parallel next-phase authorities; ADR-0008 is historical/superseded.
3. Current direct `.ro` persistence is explicitly separated from the accepted longer-term `.roproj` source / `.ro` portable-artifact architecture.
4. ADR-0015 separates durable semantic identity from mutable human keys and partially supersedes ADR-0013's rename-as-ID-replacement semantics while preserving ADR-0013 as v0.1 implementation history.
5. UUIDv7 is the preferred Provisional normal creation generator, not permanent semantic meaning.
6. ADR-0016 accepts the current Milestone 02 crate layering baseline and forbidden dependency directions while leaving narrower validation/API/runtime seams to later explicit Accepted decisions.
7. ADR-0017 separates semantic types from storage-owned version DTOs, requires explicit version-gated migration, rejects silent unknown/newer interpretation, and makes canonical bytes version-defined rather than serializer-defined.
8. Direct `.ro` JSON v1 is an immutable legacy compatibility/migration profile. Identity-aware direct `.ro` v2 is implemented in that namespace; the separately Accepted `.roproj/v1` contract remains an independent representation namespace.
9. Full RFC 8785 JCS is not the editable-source canonical profile; Tachiko reuses appropriate primitives while retaining Git-friendly whitespace/order and #24 numeric authority.
10. ADR-0018 accepts formula binding, deterministic finite-binary64 meaning, and exact numeric canonical spelling as current authority.
11. Full RFC 8785/JCS remains rejected for editable-source canonicalization; only its ECMAScript-compatible number primitive is Accepted for a representation that adopts ADR-0018.
12. ADR-0018's promotion corrections make canonical authoring projection partial, preserve ADR-0014's 4,096-byte limit atomically across rename, and place numeric-token/input resource admission in the representation/profile before semantic conversion without freezing a limit into Number meaning.
13. Event sourcing, public plugin runtime details, collaboration algorithms, `.ro` package mechanics, and host durability implementation remain outside ADR-0017 and ADR-0018. `.roproj/v1` sharding is now separately Accepted under ADR-0023 without amending those ADRs.
14. #70 implements ADR-0015 as one atomic transition: opaque IDs and mutable keys, UUIDv7 creation seam, stable formula binding/projection, stable-ID diff/merge continuity, deterministic legacy UUIDv5 migration, and direct-ro/v2 preservation of ADR-0018 semantic meaning.
15. #40 completes the storage/canonicalization and native/WASM numeric conformance closure without reopening Accepted identity or numeric meaning.
16. #72 evolves workflow in place into the single workspace-engine application boundary, reduces AI to `ai-api -> workspace-engine`, reduces CLI to `cli -> workspace-engine, storage`, and preserves storage as a sibling.
17. ADR-0019 resolves #23 by separating hard admission from diagnosable semantic candidates, accepting one staged full-validation oracle and semantic-ID diagnostics, keeping severity distinct from operation gates, preserving storage-local failure ownership, and finding no evidence for a new validation/diagnostics crate.
18. The #90-owned formula-engine prerequisite implements ADR-0018's complete node-keyed/SCC failure oracle. #89 consumes that authority with generic semantic-core diagnostic primitives, one workspace `ValidationReport`, shared semantic finalization, explicit projection/output gates, and exact native/WASM stable-observation evidence under ADR-0019.
19. ADR-0020 resolves #10 by promoting a transport-neutral Semantic API product boundary, not the current workspace-engine Rust surface: first-party semantic clients share Query/Command, Propose/Execute, authoritative gates/results, single/batch atomic publication, capability-addressability, and semantic compatibility laws; runtime/transport mechanisms remain separately owned.
20. The ADR-0007 amendment resolves #9 by defining AI as a delegated principal with no intrinsic authority, separating semantic validity/gating from authorization/approval, preserving MVP approval, and separating semantic/durable/external effects. ADR-0024 resolves #27's exact proposal/base portion without granting authorization; ADR-0026 now resolves #28's scoped authorization and exact-Approval meaning while leaving enforcement to #29/#30/#93.
21. ADR-0021 resolves #13 by accepting progressive semantic strengthening and mixed-strength content while preserving the current strongly typed core, ADR-0015 identity threshold, ADR-0018 formula endpoint rules, ADR-0019 validation stages, and ADR-0020 Propose/Execute semantics; concrete freeform/runtime/promotion mechanisms remain Deferred.
22. ADR-0022 resolves #26 by accepting shared Rust ownership of authoritative interactive semantic state, resident runtime as the preferred interactive topology, frontend projection/authoring state as non-authoritative, host capability/persistence separation, explicit full-snapshot boundaries, and native/WASM semantic parity. PR #91 remains executable evidence; exact session/revision/commit/Worker/ABI/persistence/performance mechanics remain Deferred to later implementation work.
23. ADR-0023 resolves #41's durable representation decision by accepting the exact `.roproj/v1` tree, complete version-owned DTOs, fixed entity placement/order, canonical JSON/JSONL bytes, path nonidentity, inline formulas, and closed category boundary. PR #101 remains research/probe evidence; production codecs, resource/error profiles, host durability, #43, #44, #45, #46, and future fanout/category semantics remain Deferred.
24. ADR-0024 resolves #27 by accepting SemanticPatch as an immutable opaque proposal occurrence around ADR-0020 `Propose(Command | AtomicBatch)`, binding the Semantic API compatibility contract, exact semantic base, body kind, complete typed command semantics, generated IDs/bound formulas, and AtomicBatch order. It rejects a second patch-operation/version vocabulary, generic precondition DSL, representation/storage/Git addressing, mutable same-ID proposals, and implicit stale rebase. Proposal/revision encodings, lifecycle execution, and resident revision implementation remain Provisional/Deferred to #29/#93.
25. ADR-0026 resolves #28 by accepting domain-scoped Human/Delegated principals, independent capability/mutation dimensions, document-local stable-ID scope concepts, trusted AuthorizationFootprint derivation with associated action/class/scope coverage, non-reusable default-deny Grant occurrences, exact finite Human Approval for Delegated-origin or Delegated-authority publication, and atomic consumption with at-most-one successful semantic publication. ADR-0007's current Query/Propose defaults are preserved through explicit trusted-host provisioning. Approval records its authorizing Approve Grant references, which must remain valid; fresh Execute authority is rechecked. It also fixes replay/revocation, minimum provenance, and semantic/external-effect separation without selecting a concrete TTL, canonical bytes, digest/hash/signature/MAC, portable token, public DTO, crate placement, or enterprise policy system; implementation remains #29/#30/#93.

## Current research queue

The ordered #70 -> #40 -> #72 Core & Format Hardening sequence is complete. The #90-owned formula oracle prerequisite and #89 workspace validation composition close the ADR-0018/ADR-0019 implementation gaps.

#9, #10, #13, #26, #41, #27, and #28 are resolved by amended ADR-0007, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, and ADR-0026 respectively. Production `.roproj/v1` codec work remains unimplemented and must consume, not redesign, ADR-0023. SemanticPatch and authorization production/wire/lifecycle work remains unimplemented and must consume, not redesign, ADR-0024/ADR-0026 through #29/#30/#93. #93–#95 remain later Designer-MVP runtime/performance implementation and must not retroactively block Milestone 02. #104 remains a later read-only-first Project Memory reference/dogfood research track rather than current core scope.

If implementation discovers pressure that contradicts an Accepted ADR, return to an explicit amendment/reconciliation rather than hiding the change in code.

## Founder escalation boundary

Do not ask the founder to choose UUIDv7 versus ULID, JSON map representations, parser libraries, Rust module boundaries, cache indexes, or similar technical mechanisms by preference.

Research those choices against the Constitution, accepted ADRs, migration cost, ecosystem constraints, implementation evidence, and relevant standards.

Return to founder judgment only if the result changes product identity, foundational openness/user-ownership commitments, a difficult-to-reverse public ecosystem promise, or material business/governance posture.
