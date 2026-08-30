# Canonical Reconciliation Register

Status: Accepted register when merged

Last reconciliation: 2026-08-29

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
| ADR-0003 `.roproj` source / `.ro` portable representation | Accepted | Long-term representation relationship. Direct JSON remains the ordinary writer, #123 implements the explicit canonical `.roproj/v1` codec/native host workflow, and #3 implements the derived packaged `.ro` path without superseding either role. The `.ro` name itself remains provisional until release identity is intentionally frozen. |
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
| ADR-0020 first-class Headless Semantic API boundary | Accepted | Makes one transport-neutral Semantic API mandatory for first-party semantic clients; accepts query/command, Propose/Execute, gate/result, atomic publication/batch, capability-addressability, and compatibility laws. M04 amendments accept logical formula reasoning, exact-snapshot read-only scenarios, formula update, and the bounded typed semantic Analysis Query with exact Count/Number Min/Max, structured lineage, and complete-result disclosure while keeping exact Rust/serde/transport/runtime/catalogue shapes replaceable. |
| ADR-0021 progressive semantic strengthening | Accepted | Semantic-first does not imply schema-first; legitimate weaker semantic content may be explicitly strengthened without weakening the current typed Entity/Reference/Formula contracts or fabricating universal identity. |
| ADR-0022 resident semantic runtime and host boundary | Accepted | Interactive authoritative semantic state belongs to the shared Rust semantic/application runtime; resident topology is preferred; frontend projection state and host persistence/capabilities remain non-authoritative; native/WASM preserve equivalent Stable semantics. #93 implements current internal session/revision mechanics while public transport/persistence remain Deferred. |
| ADR-0023 `.roproj/v1` canonical tree and entity sharding | Accepted | The editable-source v1 namespace has one exact 18-file tree, manifest-first dispatch, complete version-owned DTOs, fixed SHA-256-based physical placement, canonical JSON/JSONL bytes, inline formulas, and no path/line semantic identity. The production pure codec and native materialize/canonical-only-validate/explicit-canonicalize workflow are implemented by #123; #3 strengthens final publication with an atomic no-replace primitive; #44 adds optional Git/CI composition without changing the tree. Broader hostile source/path handling, durability, and delta/merge protocols remain Deferred; package authority is separately resolved by ADR-0025. |
| ADR-0024 revision-pinned SemanticPatch proposal envelope | Accepted | One immutable opaque proposal occurrence binds the Semantic API compatibility contract, exact semantic base, body kind, complete typed Command semantics, and AtomicBatch order without defining another operation/version vocabulary. Issue #29 implements the current provisional snapshot lifecycle for stable-ID typed Commands and ordered AtomicBatch; #93 supplies internal resident revision/state installation. Public proposal/revision encodings and the complete operation catalogue remain Provisional. ADR-0026 consumes the structural exact binding without selecting canonical bytes or a digest/token profile. |
| ADR-0025 portable package v1 and payload integrity root | Accepted | Portable package v1 is one deterministic 19-entry, store-only ZIP32 envelope over exact `.roproj/v1` bytes, with an unauthenticated path-separated SHA-256 payload root, byte-lossless pack/unpack laws, fail-closed content framing, and tracked-source conflict authority. #123 supplies the production payload codec; #3 implements the bounded package codec/native host/CLI workflow, native/WASM bytes, and atomic no-replace destination publication; #44 composes its read-only source comparison into optional provider-neutral CI. Broader hostile-container policy and signatures remain with #52/#53. |
| ADR-0026 scoped semantic authorization and approval | Accepted | Defines opaque domain-scoped Human/Delegated principals, independent Query/Propose/Execute/Approve, operation-family, and mutation-class dimensions, document-local stable-ID scope concepts, trusted AuthorizationFootprint derivation with associated operation-family/mutation-class/scope coverage combined with the requested action at each check, non-reusable default-deny Grant occurrences, exact finite Human Approval for Delegated-origin or Delegated-authority publication, effective-policy-version equality through publication, atomic consumption with at-most-one successful semantic publication, live authority rechecks, replay/revocation, minimum provenance, and semantic/external-effect separation. ADR-0007 MVP Query/Propose behavior is preserved through explicit host provisioning. Authorizing Approve Grant references must remain valid; fresh Execute authority is rechecked. Issue #29 implements the provisional trusted lifecycle/state/receipt seam, #30 its provider-facing hostile-client composition, and #93 guarded resident publication with host-supplied trusted time. Exact operation-family catalogues, concrete host identity, durable DTO/storage/clock/wire mechanisms, canonical bytes/digest/signature/MAC/portable tokens, and broader IAM/policy scope remain Provisional/Deferred. |
| ADR-0027 open format and interoperability policy | Accepted | Reuse mature standards before invention; external formats remain explicit adapter boundaries unless separately Accepted; Tachiko-native ownership paths remain open and independently implementable; and material fidelity loss or changed meaning must be explicit. The current `.roproj`/portable `.ro`/Git-CI path satisfies Game Dev Alpha policy, while concrete Office/ODF/CSV mappings and broad migration work remain with #18/#34 and later roadmap owners. |
| ADR-0028 game-engine host extension boundary | Accepted | Unity, Unreal Engine, and Godot integrations are host adapters over the existing Semantic API/runtime/authorization boundaries; engine effects remain separately authorized host effects, and M04 stabilizes no general plugin-platform contract. |

## Architecture and specification map

| Artifact | Decision state | Implementation state | Open decision owner |
| --- | --- | --- | --- |
| `docs/architecture/document-model.md` | Accepted direction constrained by ADR-0015/ADR-0018/ADR-0019/ADR-0021; exact future graph/content kinds remain Provisional | M02 stable identity/bound formula aggregate implemented; richer mixed-content graph future | ADR-0015, ADR-0018, ADR-0019, ADR-0021; future object-model work |
| `docs/architecture/unified-semantic-model.md` | Accepted direction; progressive strengthening constrained by ADR-0021 | Partially implemented | ADR-0015, ADR-0021; future object-model work |
| `docs/architecture/rust-crate-architecture.md` | Accepted ADR-0016 crate boundary + ADR-0020 Semantic API mapping + ADR-0024 proposal ownership + ADR-0026 authorization boundary + ADR-0022 runtime/host ownership; exact Rust/session/authorization/transport mechanisms remain Provisional/Deferred | Eight-crate workspace-engine target implemented by #72; validation by #89; #29 lifecycle/authorization; #30 provider-facing hostile boundary; #93 resident Document/revision/query/snapshot/guarded-installation mechanics; #94 selective projection/invalidation facts; #95 rebuildable retained runtime state with native/WASM evidence | ADR-0016, ADR-0019, ADR-0020, ADR-0022, ADR-0024, ADR-0026; #29/#30/#93/#94/#95 evidence; future transport completion |
| `docs/architecture/ro-and-roproj-format.md` | Accepted source/artifact direction plus exact `.roproj/v1` materialization under ADR-0023 and portable package v1 under ADR-0025 | Direct `.ro` JSON, production `.roproj/v1`, #3 packaged `.ro` codec/native host/CLI workflows, and #44 optional Git/CI composition implemented | ADR-0003, ADR-0017, ADR-0023, ADR-0025; #123/#3/#44 implementation evidence; #52/#53 broader hostile-container/trust work |
| `docs/architecture/ai-native-architecture.md` | Accepted direction constrained by ADR-0007/ADR-0020/ADR-0021/ADR-0024/ADR-0026 | Partially implemented; #29 workspace lifecycle, #30 provider-facing typed Propose/Execute hostile boundary, and #93 resident publication composition exist; concrete authentication, transport, and external capability remain | ADR-0007, ADR-0020, ADR-0021, ADR-0024, ADR-0026; #29/#30/#93 evidence; later host/transport completion |
| `docs/architecture/frontend-backend-boundary.md` | Accepted Semantic API client boundary under ADR-0020, revision-pinned proposal authority under ADR-0024, scoped authorization/Approval under ADR-0026, and resident runtime/host separation under ADR-0022; concrete mechanisms Deferred | Projection/UI boundary partially implemented; #29 lifecycle, #30 AI hostile-client composition, #93 internal resident session, #94 internal selective projections/invalidation, and #95 retained state exist, while public frontend transport/delivery remains later host work | ADR-0020, ADR-0022, ADR-0024, ADR-0026; #29/#30/#93/#94/#95 evidence; future host completion |
| `docs/architecture/wasm-strategy.md` | Accepted runtime direction under ADR-0022; public Worker/ABI/persistence mechanics Deferred | Portable/native-WASM conformance, PR #91 topology evidence, #93 resident-session parity, #94 selective projection/invalidation parity, #95 retained-state parity, #171's private Designer Worker/WASM projection, and #187's app-private canonical Open/IndexedDB Save As composition exist | ADR-0022; #171/#187 implementation evidence; future public transport/host work |
| `docs/architecture/distributed-collaboration.md` | Hypothesis / Open Question | Not implemented | #12, #45, #46, #48-#50 |
| `docs/architecture/rendering-system.md` | Hypothesis | Not current milestone | Designer MVP future work |
| `docs/architecture/performance-model.md` | Provisional guidance | Mixed | Evidence-driven future work; ADR-0022 benchmark is topology evidence, not SLA |
| `docs/specs/semantic-api.md` | Mixed: ADR-0020 first-class boundary and semantic laws Accepted, including #32's logical formula-reasoning Query, read-only scenario Query, formula-update Command, and #33's bounded typed semantic Analysis Query with exact Count/Number Min/Max, reproducibility lineage, and complete-result disclosure; ADR-0024 immutable proposal/exact-base laws and ADR-0026 authorization/Approval still apply; exact Rust API, complete catalogue, encodings, result shapes, predicate catalogue, request limits, session, and wire mappings Provisional/Deferred | Partially implemented by workspace-engine: #29 lifecycle, #30 typed AI adapter, #144 formula reasoning/scenario/formula update, #150 bounded Analysis Query, #93 internal resident revision/session, #94 internal selective projection/invalidation, and #95 full-oracle-equivalent retained state with native/WASM evidence | ADR-0020, ADR-0022, ADR-0024, ADR-0026; #32/#33 authority; #29/#30/#93/#94/#95/#144/#150 evidence; future transport completion |
| `docs/specs/semantic-authorization.md` | Normative Accepted Principal/capability/scope/Grant/AuthorizationFootprint/exact-Approval/expiry-replay-revocation/provenance/effect-separation contract under ADR-0026; exact identifiers, DTOs, storage, clocks, codes, and wire formats Provisional; canonical bytes/digest/signature/MAC/token Deferred | #29 trusted lifecycle/state/receipt, #30 provider-facing denial seam, and #93 guarded resident publication with host-supplied time exist; concrete identity, durable registry, and public DTO/wire remain | ADR-0026; #29/#30/#93 evidence; later identity/transport completion |
| `docs/specs/schema-system.md` | Mixed: current durable declaration boundary Accepted under ADR-0015/ADR-0019; progressive strengthening/mixed-strength rules Accepted under ADR-0021; richer schema vocabulary future | Current M02 type/required/reference declarations implemented; no general freeform/inference runtime | ADR-0015, ADR-0019, ADR-0021; future schema/promotion work |
| `docs/specs/validation-engine.md` | Mixed: staged validation/candidate/full-oracle semantics Accepted under ADR-0019; ADR-0020 maps report/gate meaning into the Semantic API; ADR-0021 makes applicability follow declared semantic facts; exact APIs/incremental mechanisms Provisional | M02 validation oracle implemented by #89 over #90's formula oracle; #95 retains revision-scoped reports and incremental formula outcomes with full-oracle checks | ADR-0019, ADR-0018, ADR-0020, ADR-0021; future mechanism changes |
| `docs/specs/diagnostics-contract.md` | Mixed: semantic diagnostic stability rules Accepted under ADR-0019; ADR-0020 adds unknown-code and authoritative-gate client compatibility laws; exact Rust/wire/catalog Provisional/Deferred | Internal semantic-first envelope and workspace report implemented by #89; concrete external transport mapping deferred | ADR-0019, ADR-0020, ADR-0022; future transport mapping |
| `docs/specs/ro-format-and-roproj-spec.md` | Accepted source/artifact direction, `.roproj/v1` representation boundary, and portable-package v1 relationship with explicit current-state split | Direct `.ro` JSON, production `.roproj/v1`, and packaged `.ro` codec/native host/CLI workflows implemented | ADR-0003, ADR-0017, ADR-0023, ADR-0025; #123 `.roproj` implementation; #3 package implementation |
| `docs/specs/storage-versioning-and-migration.md` | Mixed: Accepted ADR-0017 invariants, `.roproj/v1` namespace/dispatch under ADR-0023, and separate package namespace/dispatch boundary under ADR-0025; direct-JSON mechanics Provisional where marked | Strict direct-ro/v1, deterministic migration, direct-ro/v2, production `.roproj/v1`, and bounded packaged `.ro` workflows implemented | ADR-0017, ADR-0023, ADR-0025, #40; #123 `.roproj` implementation; #3 package implementation |
| `docs/specs/canonical-json-profile.md` | Mixed: Accepted deterministic/semantic-preservation and binary64 rules plus `.roproj/v1` JSON/JSONL/tree profile; direct-JSON resource limits remain version-specific; package manifest spelling is separately owned by ADR-0025 | Implemented direct-ro/v2, production `.roproj/v1`, and packaged `.ro` manifest writers with fixed native/WASM evidence | ADR-0017, ADR-0018, ADR-0023, ADR-0025, #40; #123 `.roproj` evidence; #3 package implementation |
| `docs/specs/ro-format-v1.md` | Normative legacy direct-`.ro` JSON compatibility/migration profile | Implemented immutable compatibility reader/writer and migration source | ADR-0017, #40 |
| `docs/specs/ro-format-v2.md` | Mixed: Accepted ADR-0015/ADR-0017/ADR-0018 invariants; M02 wire/resource mechanics Provisional | Implemented current semantic writer | ADR-0015, ADR-0017, ADR-0018, #40 |
| `docs/specs/roproj-format.md` | Normative Accepted `.roproj/v1` version-owned DTO and wire contract | Production storage-owned pure codec implemented by #123 with fixed native/WASM encode/decode/exact-re-encode evidence | ADR-0017, ADR-0018, ADR-0023; #123 implementation evidence |
| `docs/specs/roproj-layout-v1.md` | Normative Accepted `.roproj/v1` tree, sharding, path, and canonicalization contract | Production pure codec plus native materialize/canonical-only-validate/explicit-canonicalize and staged absent-destination publication implemented by #123; hostile races and broader durability Deferred | ADR-0023; #123 implementation evidence; ADR-0022 and future host work for Deferred durability mechanisms |
| `docs/specs/portable-package-v1.md` | Normative Accepted portable-package v1 container, integrity, round-trip, conflict, and conformance contract | #3 production codec/native host/CLI workflow implemented against the golden vector and production `.roproj/v1` seam with native/WASM exact-byte evidence; #44 composes exact source comparison into optional CI | ADR-0025; #3/#44 implementation; #45/#46/#52/#53 adjacent work |
| `docs/specs/formula-engine-spec.md` | Mixed: Accepted ADR-0014 authoring and ADR-0018 binding/projection, numeric, dependency, and recomputation rules; #32 scenario/query composition reuses that authority without changing pure engine meaning; implementation mechanisms Provisional | Stable formula oracle implemented; #144 implements the first provider-neutral Semantic API formula-reasoning/scenario workspace/CLI composition without changing pure formula-engine authority; #95 adds a rebuildable retained evaluator mechanically checked against the full oracle | ADR-0018; ADR-0020/#32 composition; #144/#95 implementation evidence |
| `docs/specs/ai-agent-api.md` | Mixed: AI delegated-client direction Accepted under ADR-0007/ADR-0020; #32 formula reasoning/scenario/formula update and #33 bounded analysis are shared provider-neutral Semantic API meaning; ADR-0024 proposal and ADR-0026 authorization/Approval apply; current adapter DTOs Provisional | Implemented read/explain/suggest plus #30 typed Propose/Execute hostile boundary over #29; #93 proves resident publication composition; #144 and #150 implement provider-neutral formula/scenario/formula-update and bounded Analysis Query workspace/CLI surfaces, while AI/public mapping remains deferred and current `Suggestion` remains inert | ADR-0007, ADR-0020/#32/#33, ADR-0024, ADR-0026; #29/#30/#93/#144/#150 evidence; later transport completion |
| `docs/specs/security-model.md` | Mixed: ADR-0007/ADR-0026 semantic authorization laws Accepted; plugin isolation, migration sandboxing, concrete authentication/audit, and external-effect mechanisms Provisional/Deferred or separately owned | #29 trusted lifecycle, #30 provider-facing denial boundary, and #93 guarded resident publication implemented; actual external capability and durable audit mechanisms remain Deferred | ADR-0007, ADR-0026, ADR-0028; #93 evidence, #134/#135 narrower policy, general plugin isolation/sandbox Deferred |
| `docs/security/threat-model.md` | Mixed: ADR-0007/ADR-0026 authorization threats/laws Accepted; current trust labels/codes Provisional; supply-chain, transport, durable audit, and actual host effects separately owned | #30 provider-facing enforcement and regression fixtures implemented over #29; #93 supplies guarded resident publication; broader transport/host security remains Deferred | ADR-0007, ADR-0026, ADR-0028; #93 evidence and later domain decisions |
| `docs/specs/collaboration-model.md` | Mixed: current merge Accepted, future collaboration Open Question | Merge implemented; broader collaboration future | ADR-0011, #12, #45, #46 |
| `docs/specs/conflict-resolution.md` | Provisional around current merge; future conflict model Open Question | Partial | #46 |
| `docs/specs/operation-log-model.md` | Open Question | No first-class persisted log in v0.1 | #12, #48 |
| `docs/specs/event-sourcing-model.md` | Hypothesis | Not implemented | #12, #49 |
| `docs/specs/plugin-system.md` | Mixed: ADR-0028 accepts the M04 game-engine host extension boundary; general plugin ABI/runtime/sandbox mechanics remain Deferred | No plugin or game-engine integration implemented by this decision | ADR-0028; #134/#135 own narrower private/public policy only; general mechanics unassigned |
| `docs/specs/migration-framework.md` | Accepted direction under ADR-0027; concrete adapters/mappings Hypothesis/Open Question | Not implemented as a broad legacy migration system; the current open `.roproj`/portable `.ro`/Git-CI ownership path satisfies Game Dev Alpha policy | ADR-0027 policy; #18/#34 concrete mapping and migration work; ADR-0021 strengthening review principles |
| `docs/specs/runtime-export-v1.md` | Frozen historical contract | Superseded as current writer by v2 | compatibility evidence |
| `docs/specs/runtime-export-v2.md` | Provisional derived-output contract | Implemented current runtime writer | implementation evidence / future versioning work |

## GitHub Issue classification

A GitHub Issue is never automatically an Accepted decision. The table below classifies the current backlog by what kind of authority it carries.

### Historical Developer MVP issues

| Issue | Classification | Reconciliation action |
| --- | --- | --- |
| #1 MVP Freeze | Superseded as active work / historical accepted scope | Current roadmap is governed by unified milestones and later ADRs. |
| #2 semantic document model MVP | Implemented historical task | Preserve as implementation history. |
| #3 thin `.ro` portable artifact packaging | Implemented task with reconciled scope | Implements ADR-0025 and the portable-package v1 specification without redesigning the Accepted contract. |

### Decision and strategy issues

| Issue | Classification | Notes |
| --- | --- | --- |
| #9 AI authority / canonical source of truth | Resolved by amended ADR-0007 | AI has no intrinsic authority; validation/gating and authorization/approval are separate; current MVP delegated mutation remains approval-gated. ADR-0024 supplies the immutable proposal/base contract and ADR-0026 the scoped authorization/exact-Approval contract. #29 implements the provider-neutral lifecycle, #30 its hostile-client AI composition, and #93 resident publication integration; concrete authentication and public transport remain later host work. |
| #10 Headless Semantic API | Resolved by ADR-0020 | First-class transport-neutral semantic boundary is Accepted; complete operation catalogue and concrete wire mapping remain separately owned, while ADR-0026 now resolves the narrow MVP authorization contract. |
| #11 permissions/provenance/transactions | Open Question | Broad team/collaboration decision; ADR-0026 resolves only the Game Dev Alpha minimum and leaves enterprise/team policy, reusable approvals, and recovery questions here. |
| #12 mutation history / event sourcing / CRDT / Git | Open Question | Event sourcing/CRDT docs remain non-authoritative hypotheses until promoted. |
| #13 progressive typing | Resolved by ADR-0021 | Progressive semantic strengthening is Accepted without weakening the current strongly typed core; concrete freeform kinds, identity thresholds, promotion commands, storage, and UI remain Deferred. |
| #14 open format/interoperability policy | Resolved by ADR-0027 | Reuse-before-invention, explicit external-format boundaries, an open independently implementable Tachiko ownership path, explicit fidelity claims, and the current milestone boundary are Accepted; concrete format mappings/adapters remain separately owned by #18/#34 and later roadmap work. |
| #15 licensing/commercial boundary | Open Question | Founder/governance decision after research/legal review. |
| #17 game-engine host extension boundary | Resolved by ADR-0028 | M04 classifies Unity, Unreal Engine, and Godot integrations as host adapters over existing semantic/runtime/authorization authority and stabilizes no general plugin-platform contract. |
| #18 Japan enterprise / gradual Excel migration | Accepted product direction with Hypotheses | Japan as a priority research environment and gradual migration are accepted; individual pain-point claims require user evidence. |
| #19 canonical docs / ADR reconciliation | Completed reconciliation task | Closed after establishing authority precedence, ADR numbering, and canonical reconciliation rules. |
| #32 formula reasoning, scenario query, and formula proposal | Resolved by amended ADR-0020 and `semantic-api.md` | Accepts logical structured formula reasoning, exact-snapshot read-only scenarios, and a typed formula-update Command reusing SemanticPatch and ADR-0026; #144 implements the first provider-neutral workspace/CLI slice while exact Rust/wire/catalogue shapes remain Provisional. |
| #33 semantic data-analysis query and lineage | Resolved by amended ADR-0020 and `semantic-api.md` | Accepts only the bounded typed Analysis Query family, exact membership/Count/Number Min/Max and bounded per-member Number observations, same-definition two-context evaluation, reproducibility lineage, and complete-or-denied disclosure. #150 supplies the first Provisional workspace/CLI slice; Sum/Mean, ranking/statistics, general query language features, persistence, broader production surfaces, and public stabilization remain separate. |
| #134 legacy Office/VBA migration and private enterprise extensions | Open Question, M07 | Owns migration/equivalence and organization-private extension policy without reopening the M04 engine-host boundary. |
| #135 public plugin distribution and support policy | Open Question, M08 | Owns marketplace/distribution, signing, compatibility, lifecycle, and support promises without implying a currently stable plugin ABI/runtime. |

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
- #26 native/WASM runtime boundary — resolved by Accepted ADR-0022 and PR #91 evidence; resident runtime ownership, host separation, explicit snapshot boundaries, and native/WASM semantic parity are Accepted. #93 supplies current internal session/revision mechanics while Worker/ABI/persistence remain Deferred.
- #72 workflow-to-workspace-engine migration — implementation of ADR-0016 completed by PR #85; it provides implementation evidence for ADR-0020/ADR-0022 without defining a public source/session/transport contract.

Issue `#40` is a completed implementation/evidence task that consumed ADR-0015, ADR-0017, and Accepted ADR-0018 without inventing format semantics.

### Game Dev Alpha / AI-safe mutation work

- #27 SemanticPatch: resolved by ADR-0024 as an immutable revision-pinned proposal envelope around ADR-0020 `Propose(Command | AtomicBatch)`; ADR-0026 consumes its structural binding without selecting digest/token bytes; #29 implements the lifecycle and #93 internal resident revision mechanics while exact wire/ID encodings remain Provisional.
- #28 capability, approval, provenance: resolved by ADR-0026 and `semantic-authorization.md` as the narrow MVP safety contract; broader enterprise/team policy remains #11.
- #29 patch lifecycle: current provisional workspace-engine implementation consumes ADR-0024/ADR-0026 without inventing another mutation vocabulary or authorization contract.
- #30 AI security boundary: implements provider-facing instruction/data separation, raw-mutation bypass prevention, disclosure-safe stable codes, external-effect denial, and security regressions over #29 without implementing external capabilities.
- #31 Semantic Analyst: Implementation/evidence task; deterministic semantic facts remain core authority and queries use ADR-0020's boundary.
- #41 `.roproj` layout: durable decision resolved by ADR-0023 and the normative `.roproj/v1` format/layout specifications; production codec/native host implementation was separately completed by #123.
- #123 `.roproj/v1` production codec and materialization: implemented the
  storage-owned pure codec plus native materialize, canonical-only validate,
  explicit bounded canonicalize, staged absent-destination publication, and
  fixed native/WASM exact-tree evidence without implementing package or Git/CI
  behavior.
- #43 `.ro` package profile: durable decision resolved by ADR-0025, the
  normative portable-package v1 specification, evidence probe, and golden
  vector; #3 implements the packaged `.ro` codec/native host/CLI workflow.
- #44 Git/CI integration: optional LF text attributes, exact-tree read-only semantic commands, provider-neutral CI validation/review, and generated-package consistency composition implemented without Git-shaped semantics.

### Later runtime, reasoning, migration, collaboration, and standardization

- #93: implements the first production Designer-MVP resident workspace session + revision-safe command publication under ADR-0022, without public transport stabilization.
- #94: implements the current internal selective semantic query/projection
  invalidation slice under ADR-0022 without public transport stabilization.
- #95: implements retained incremental engine state constrained by full-oracle equivalence, without durable cache meaning or a second invalidation authority.
- #171: implements the bounded first-party Designer table/edit/selective-refresh
  projection over the Rust-authoritative Worker/WASM occurrence without a
  public client or wire contract.
- #187: composes canonical `.roproj/v1` Open and exact-revision, create-only
  IndexedDB Save As around that occurrence, with private bounded project
  transfer and no overwrite/recovery/public storage contract.
- #32: resolved by ADR-0020's M04 amendment and `semantic-api.md`; #144 provides
  the first provider-neutral formula reasoning/scenario/formula-update
  workspace/CLI implementation evidence.
- #33: resolved by ADR-0020's bounded M04 semantic analysis amendment and
  `semantic-api.md`; #150 provides the first bounded provider-neutral
  workspace/CLI implementation evidence, while broader production-analysis
  surfaces beyond #150 remain separate.
- #34: Hypothesis/Open Question for post-MVP migration assistant; ADR-0021 constrains any future promotion/mapping semantics.
- #35: Epic/index only; not decision authority.
- #36: Hypothesis/Open Question for collaboration assistant.
- #39: Hypothesis; explicitly future/post-1.0 unless evidence changes priority.
- #42: Epic/index only; not decision authority.
- #45-#55: Open Questions for later protocol, collaboration/history, conformance, security, integrity, extension, and interoperability contracts. Their existence is not authorization to implement them now.
- #56: Accepted roadmap/administrative direction; close when milestone creation/backfill is verified complete.
- #104 Project Memory / semantic decision provenance: Research/Hypothesis; may pressure-test ADR-0020 as a domain/reference application but may not promote Project Memory vocabulary into semantic core without separate evidence/decision work.
- #134 and #135: later M07/M08 decision work for private enterprise migration/extensions and the public plugin ecosystem respectively; neither authorizes current implementation.

## Reconciliations made in this pass

1. ADR-0002 and ADR-0005 are no longer treated as parallel Accepted authorities; ADR-0002 is historical/superseded.
2. ADR-0008 and ADR-0009 are no longer treated as parallel next-phase authorities; ADR-0008 is historical/superseded.
3. Current direct `.ro` ordinary persistence is explicitly separated from the implemented explicit `.roproj/v1` canonical materialization path and the implemented derived packaged `.ro` portable-artifact path.
4. ADR-0015 separates durable semantic identity from mutable human keys and partially supersedes ADR-0013's rename-as-ID-replacement semantics while preserving ADR-0013 as v0.1 implementation history.
5. UUIDv7 is the preferred Provisional normal creation generator, not permanent semantic meaning.
6. ADR-0016 accepts the current Milestone 02 crate layering baseline and forbidden dependency directions while leaving narrower validation/API/runtime seams to later explicit Accepted decisions.
7. ADR-0017 separates semantic types from storage-owned version DTOs, requires explicit version-gated migration, rejects silent unknown/newer interpretation, and makes canonical bytes version-defined rather than serializer-defined.
8. Direct `.ro` JSON v1 is an immutable legacy compatibility/migration profile. Identity-aware direct `.ro` v2 is implemented in that namespace; the separately Accepted `.roproj/v1` contract remains an independent representation namespace.
9. Full RFC 8785 JCS is not the editable-source canonical profile; Tachiko reuses appropriate primitives while retaining Git-friendly whitespace/order and #24 numeric authority.
10. ADR-0018 accepts formula binding, deterministic finite-binary64 meaning, and exact numeric canonical spelling as current authority.
11. Full RFC 8785/JCS remains rejected for editable-source canonicalization; only its ECMAScript-compatible number primitive is Accepted for a representation that adopts ADR-0018.
12. ADR-0018's promotion corrections make canonical authoring projection partial, preserve ADR-0014's 4,096-byte limit atomically across rename, and place numeric-token/input resource admission in the representation/profile before semantic conversion without freezing a limit into Number meaning.
13. Event sourcing, public plugin runtime details, collaboration algorithms, and broader hostile-race/durability policy remain outside ADR-0017 and ADR-0018. `.roproj/v1` sharding is separately Accepted under ADR-0023 and implemented by #123; package mechanics remain separately Accepted under ADR-0025 and implemented by #3 without amending those ADRs.
14. #70 implements ADR-0015 as one atomic transition: opaque IDs and mutable keys, UUIDv7 creation seam, stable formula binding/projection, stable-ID diff/merge continuity, deterministic legacy UUIDv5 migration, and direct-ro/v2 preservation of ADR-0018 semantic meaning.
15. #40 completes the storage/canonicalization and native/WASM numeric conformance closure without reopening Accepted identity or numeric meaning.
16. #72 evolves workflow in place into the single workspace-engine application boundary, reduces AI to `ai-api -> workspace-engine`, reduces CLI to `cli -> workspace-engine, storage`, and preserves storage as a sibling.
17. ADR-0019 resolves #23 by separating hard admission from diagnosable semantic candidates, accepting one staged full-validation oracle and semantic-ID diagnostics, keeping severity distinct from operation gates, preserving storage-local failure ownership, and finding no evidence for a new validation/diagnostics crate.
18. The #90-owned formula-engine prerequisite implements ADR-0018's complete node-keyed/SCC failure oracle. #89 consumes that authority with generic semantic-core diagnostic primitives, one workspace `ValidationReport`, shared semantic finalization, explicit projection/output gates, and exact native/WASM stable-observation evidence under ADR-0019.
19. ADR-0020 resolves #10 by promoting a transport-neutral Semantic API product boundary, not the current workspace-engine Rust surface: first-party semantic clients share Query/Command, Propose/Execute, authoritative gates/results, single/batch atomic publication, capability-addressability, and semantic compatibility laws; runtime/transport mechanisms remain separately owned.
20. The ADR-0007 amendment resolves #9 by defining AI as a delegated principal with no intrinsic authority, separating semantic validity/gating from authorization/approval, preserving MVP approval, and separating semantic/durable/external effects. ADR-0024 resolves #27's exact proposal/base portion without granting authorization; ADR-0026 resolves #28's scoped authorization and exact-Approval meaning. Issue #29 supplies the trusted lifecycle implementation, #30 its hostile-client composition, and #93 the resident runtime/state-installation composition.
21. ADR-0021 resolves #13 by accepting progressive semantic strengthening and mixed-strength content while preserving the current strongly typed core, ADR-0015 identity threshold, ADR-0018 formula endpoint rules, ADR-0019 validation stages, and ADR-0020 Propose/Execute semantics; concrete freeform/runtime/promotion mechanisms remain Deferred.
22. ADR-0022 resolves #26 by accepting shared Rust ownership of authoritative interactive semantic state, resident runtime as the preferred interactive topology, frontend projection/authoring state as non-authoritative, host capability/persistence separation, explicit full-snapshot boundaries, and native/WASM semantic parity. PR #91 remains topology evidence; #93 implements current internal session/revision/guarded-installation mechanics with native/WASM evidence. Public session/transport, cross-host concurrency, Worker/ABI, persistence, and performance promises remain Deferred.
23. ADR-0023 resolves #41's durable representation decision by accepting the exact `.roproj/v1` tree, complete version-owned DTOs, fixed entity placement/order, canonical JSON/JSONL bytes, path nonidentity, inline formulas, and closed category boundary. PR #101 remains historical research/probe evidence; #123 implements the production pure codec, explicit native host workflow, staged absent-destination publication, and fixed native/WASM exact-tree evidence. #3 strengthens final directory publication with the required atomic no-replace primitive; #44 adds optional provider-neutral Git/CI composition without changing the tree. Broader source/path hardening, durability, #45, #46, and future fanout/category semantics remain Deferred. Package authority originally deferred to #43 is resolved separately by ADR-0025.
24. ADR-0024 resolves #27 by accepting SemanticPatch as an immutable opaque proposal occurrence around ADR-0020 `Propose(Command | AtomicBatch)`, binding the Semantic API compatibility contract, exact semantic base, body kind, complete typed command semantics, generated IDs/bound formulas, and AtomicBatch order. It rejects a second patch-operation/version vocabulary, generic precondition DSL, representation/storage/Git addressing, mutable same-ID proposals, and implicit stale rebase. Issue #29 implements the current provisional lifecycle and #93 its internal resident revision precondition/installation mechanics; public proposal/revision encodings and the complete catalogue remain Provisional.
25. ADR-0025 resolves #43 by accepting one exact store-only ZIP32 package over the 18 `.roproj/v1` files, a closed package manifest, domain- and path-separated SHA-256 payload root, strict lossless laws, fail-closed direct-JSON coexistence, and tracked-source conflict authority. The Issue #43 probe and golden vector remain independent evidence. Issue #123 supplies the production `.roproj/v1` payload seam; #3 implements the exact codec, bounded decoder, content framing, pack/unpack/compare host and CLI workflows, no-replace destination publication, and native/WASM evidence. #44 implements optional Git/CI validation/review/consistency composition; #45/#46/#52/#53 retain adjacent delta/merge, broader hostile-input, and trust work.
26. ADR-0026 resolves #28 by accepting domain-scoped Human/Delegated principals, independent action/operation-family/mutation dimensions, document-local stable-ID scope concepts, trusted AuthorizationFootprint derivation with associated operation-family/mutation-class/scope coverage combined with the requested action at each check, non-reusable default-deny Grant occurrences, exact finite Human Approval for Delegated-origin or Delegated-authority publication, effective-policy-version equality through publication, and atomic consumption with at-most-one successful semantic publication. ADR-0007's current Query/Propose defaults are preserved through explicit trusted-host provisioning. Approval records its authorizing Approve Grant references, which must remain valid; fresh Execute authority is rechecked. It also fixes replay/revocation, minimum provenance, and semantic/external-effect separation without selecting exact operation-family identifiers/catalogue, policy-version representation/selection mechanisms, a concrete TTL, canonical bytes, digest/hash/signature/MAC, portable token, public DTO, crate placement, or enterprise policy system. Issue #29 supplies the provisional lifecycle/state/receipt implementation, #30 its provider-facing denial boundary, and #93 guarded resident publication with host-supplied trusted time.
27. ADR-0027 resolves #14 by accepting reuse-before-invention, explicit external-format adapter boundaries, an open independently implementable Tachiko-native ownership path, explicit fidelity reporting, representation-local version/migration law, and the current milestone boundary. Concrete Office/ODF/CSV mappings, fidelity-ledger mechanisms, and broad migration implementation remain separately owned by #18/#34 and later roadmap work.
28. ADR-0028 resolves the narrowed #17 by classifying Unity, Unreal Engine, and Godot integrations as host adapters over ADR-0020/ADR-0022/ADR-0026 authority, preserving separate host-effect authorization, and declining to stabilize a general plugin ABI, runtime, sandbox, distribution, signing, or compatibility contract in M04. #134 and #135 own the deferred M07 private-enterprise and M08 public-ecosystem decisions.
29. Issue #30 implements the current provider-facing hostile-client seam without changing ADR-0007/ADR-0020/ADR-0024/ADR-0026/ADR-0028: host-proven context treatment remains distinct from semantic authorization; untrusted requests cannot supply effective identity/time or validation/Approval truth; typed proposal/execution delegates to #29; raw semantic/storage mutation and external effects are denied with stable in-process codes. Public wire/authentication/session mechanisms and actual external capabilities remain separately owned.
30. ADR-0020's Issue #32 amendment promotes the logical structured formula-reasoning Query, exact-source read-only Number-override scenario Query, and typed formula-update Command. It reuses ADR-0018/ADR-0019 calculation and validation, ADR-0024 SemanticPatch exact binding, and ADR-0026 authorization/Approval without freezing API names, DTOs, transports, resident revisions, or the #33 analytics IR; #144 supplies the first provider-neutral workspace/CLI implementation evidence.
31. ADR-0020's Issue #33 amendment promotes only a bounded typed semantic Analysis Query: one exact context/domain, bounded AND predicates, at most one grouping key, optional exact membership, exact Count, Number Min/Max, bounded per-member Number observations, same-definition two-context evaluation, reproducibility lineage, and ADR-0026 complete-or-denied aggregate disclosure. #150 supplies the first Provisional provider-neutral workspace/CLI implementation evidence; its exact CLI syntax and JSON shape are not a stabilized public wire or SDK contract. Sum/Mean, ranking/statistics, general query-language features, partial aggregates, persistence, analytics storage, exact Rust/CLI/wire shapes, broader production-analysis surfaces beyond #150, and public stabilization remain Deferred or Provisional.

## Current research queue

The ordered #70 -> #40 -> #72 Core & Format Hardening sequence is complete. The #90-owned formula oracle prerequisite and #89 workspace validation composition close the ADR-0018/ADR-0019 implementation gaps.

#9, #10, #13, #26, #41, #27, #43, and #28 are resolved by amended ADR-0007,
ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, ADR-0025, and ADR-0026
respectively.
#14 is separately resolved by ADR-0027 without promoting concrete external-format
adapters into the current milestone.
`#17` is separately resolved by ADR-0028 without stabilizing a general plugin
platform; #134 and #135 remain open at the M07 and M08 horizons.
`#32` is resolved by the ADR-0020 amendment and normative Semantic API
specification, with the first provider-neutral workspace/CLI slice implemented
by #144. `#33` is resolved by the bounded Analysis Query amendment and normative
Semantic API specification, with the first provider-neutral workspace/CLI slice
implemented by #150.
Production `.roproj/v1` pure codec/native host work is implemented by #123.
Portable-package v1 packaged `.ro` codec/native host/CLI work is implemented by
Issue `#3` and consumes, without redesigning, the Accepted package contract and
the production `.roproj/v1` seam. Issue `#44` implements the optional
provider-neutral Git/CI adapter over those standalone boundaries. Issue #29
implements the provider-neutral SemanticPatch/authorization lifecycle seam
without redesigning ADR-0024/ADR-0026, and #30 implements the current hostile-
client AI boundary over it. Issue #93 supplies the internal resident revision/
session implementation, Issue #94 adds internal selective projections plus
occurrence/revision-paired exact-publication invalidation facts, and Issue #95
retains rebuildable full-oracle-equivalent state across resident revisions.
Public authentication/wire remain host/runtime work and must not retroactively
block Milestone 02. #104 remains a later read-only-first Project Memory
reference/dogfood research track rather than current core scope.

If implementation discovers pressure that contradicts an Accepted ADR, return to an explicit amendment/reconciliation rather than hiding the change in code.

## Founder escalation boundary

Do not ask the founder to choose UUIDv7 versus ULID, JSON map representations, parser libraries, Rust module boundaries, cache indexes, or similar technical mechanisms by preference.

Research those choices against the Constitution, accepted ADRs, migration cost, ecosystem constraints, implementation evidence, and relevant standards.

Return to founder judgment only if the result changes product identity, foundational openness/user-ownership commitments, a difficult-to-reverse public ecosystem promise, or material business/governance posture.
