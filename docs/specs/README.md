# Specifications

Specifications describe implementable contracts, provisional baselines, and future design targets for Tachiko Work.

A file living in `docs/specs/` is **not automatically Accepted**. Read its explicit decision state together with [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and the [`canonical reconciliation register`](../governance/canonical-reconciliation-register.md).

## Format and representation: read this first

The format documents have different roles. Do not treat similarly named files as parallel normative specifications.

| Document | Read it for | Current role |
| --- | --- | --- |
| [`ro-format-and-roproj-spec.md`](ro-format-and-roproj-spec.md) | Accepted `.roproj` source / `.ro` portable-artifact relationship and current implementation split | Accepted direction under ADR-0003 |
| [`storage-versioning-and-migration.md`](storage-versioning-and-migration.md) | Version namespaces, versioned DTO ownership, supported/unsupported behavior, migration architecture | Mixed: Accepted ADR-0017 invariants and `.roproj/v1` rules under ADR-0023 plus Provisional direct-JSON M02 mechanics |
| [`canonical-json-profile.md`](canonical-json-profile.md) | Deterministic JSON/Unicode/order/whitespace contract and explicit numeric boundary | Mixed: Accepted ADR-0017/ADR-0018 invariants and `.roproj/v1` profile under ADR-0023 plus version-specific direct-JSON resource mechanics |
| [`ro-format-v1.md`](ro-format-v1.md) | Exact deterministic direct `.ro` JSON behavior shipped by the v0.1 CLI | Immutable legacy compatibility / migration source |
| [`ro-format-v2.md`](ro-format-v2.md) | Current identity-aware direct `.ro` JSON DTO, canonical writer, Number/resource profile, and bound references | Mixed: Accepted ADR-0015/ADR-0017/ADR-0018 invariants plus Provisional M02 wire/resource mechanics |
| [`ro-format.md`](ro-format.md) | Compatibility/navigation entry point for older links | Non-normative navigation stub; follow the format documents above |
| [`roproj-format.md`](roproj-format.md) | Complete version-owned `.roproj/v1` DTO and wire contract | Accepted under ADR-0023; production pure codec implemented by #123 |
| [`roproj-layout-v1.md`](roproj-layout-v1.md) | Exact `.roproj/v1` canonical tree, sharding, path, and canonicalization contract | Accepted under ADR-0023; native materialize/validate/explicit canonicalize workflow implemented by #123 |
| [`portable-package-v1.md`](portable-package-v1.md) | Exact portable-package v1 ZIP32 bytes, payload integrity root, pack/unpack laws, conflict behavior, and conformance outcomes | Accepted under ADR-0025; production codec/native host/CLI workflow implemented by #3 |
| [`runtime-export-v1.md`](runtime-export-v1.md) | Historical evaluated runtime JSON export contract | Frozen historical contract |
| [`runtime-export-v2.md`](runtime-export-v2.md) | Current stable-identity/normalized-Number evaluated runtime JSON export | Provisional implemented contract |

The semantic model owns meaning. Physical formats are representations. ADR-0017 fixes the versioned storage boundary and canonical-representation invariants; ADR-0018 fixes the admitted-token binary64 conversion and spelling authority; ADR-0023 fixes the `.roproj/v1` physical and wire contract without making layout semantic identity. Issue #123 implements that production pure codec and the native explicit host workflow. ADR-0025 separately fixes the portable-package v1 envelope and integrity contract; #3 implements its production codec, bounded host workflow, and CLI pack/unpack/compare operations.

## Semantic API, core, schema, validation, formulas, and diff

| Document | Read it for |
| --- | --- |
| [`semantic-api.md`](semantic-api.md) | ADR-0020 first-class transport-neutral Semantic API, ADR-0024 SemanticPatch, ADR-0026 authorization/approval, and ADR-0032 Execute-attempt/`NoChange`/revision-occurrence/optional-transition taxonomy, including #32/#33's Accepted query/command additions; ADR-0022 runtime-host constraints still apply and exact DTO/session/transport/history mechanics remain Deferred |
| [`semantic-authorization.md`](semantic-authorization.md) | ADR-0026 principal, capability, stable-ID scope, trusted footprint, Grant, exact Human Approval, expiry/replay/revocation, provenance, and effect-domain contract |
| [`semantic-data-model.md`](semantic-data-model.md) | Semantic data-model contract and terminology |
| [`schema-system.md`](schema-system.md) | Durable schema declaration behavior and the boundary from runtime validation policy |
| [`validation-engine.md`](validation-engine.md) | ADR-0019 staged validation, candidate/finalization semantics, full-validation oracle, and operation gating |
| [`diagnostics-contract.md`](diagnostics-contract.md) | Semantic-ID-centered machine-readable diagnostic meaning and stability boundaries |
| [`formula-engine-spec.md`](formula-engine-spec.md) | Accepted bounded authoring and ADR-0018 stable-ID projection/rename, binary64, dependency, and recomputation contract; #32's Semantic API scenarios compose this same oracle without creating a second evaluator |
| [`semantic-diff-spec.md`](semantic-diff-spec.md) | ADR-0030 canonical Semantic Delta v1 logical contract, closed direct-change vocabulary, stable targets, deterministic ordering, and separation from derived impact; current Rust/wire shapes remain unstandardized |

ADR-0020 makes the Headless Semantic API the mandatory first-party semantic
product boundary while keeping current Rust APIs, serde shapes, complete
operation catalogue, and transport mechanics replaceable. ADR-0024 defines one
immutable proposal occurrence around `Propose(Command | AtomicBatch)`, exact
Semantic API/base/change binding, and fail-closed stale meaning. ADR-0026 adds
the closed MVP authorization contract: domain-scoped Human/Delegated
principals, independent Query/Propose/Approve/Execute actions and operation
families, stable-ID document-local scopes, trusted relational authorization-
footprint derivation, Value/Formula/Structure/Schema/Destructive classes, exact
finite Human Approval for Delegated-origin or Delegated-authority publication,
replay/revocation rules, minimum provenance, and separation from host/external
effects. Issue #29 implements the current provisional workspace-engine
lifecycle/state/publication seam, and #30 adds the provisional provider-facing
instruction/data, trusted-context, bypass-denial, and host-effect-denial seam.
Issue #93 implements the current internal resident revision/session mechanics.
ADR-0022 accepts the resident shared Rust runtime,
no-second-canonical-client-model rule, host separation, explicit snapshot
boundaries, and native/WASM semantic parity while leaving public session,
cross-host concurrency, Worker/ABI, and persistence mechanics Deferred to
future host/transport implementation. Issue #94 implements the current internal
selective-query/projection-invalidation slice without stabilizing its public
shape; Issue #95 retains rebuildable full-oracle-equivalent runtime state.
ADR-0019 resolves the validation/diagnostics
architecture; formula binding/failure/numeric semantics remain governed by
ADR-0018; stable identity by ADR-0015; storage representation by ADR-0017.
ADR-0020's #32 amendment additionally accepts logical structured formula
reasoning, exact-snapshot read-only scenarios, and the typed formula-update
Command. Issue #144 implements their first Provisional provider-neutral
workspace/CLI slice; public wire/SDK and AI-facing mappings remain deferred.
ADR-0020's #33 amendment accepts the bounded typed semantic Analysis Query,
exact selected membership/Count/Number Min/Max and per-member Number
observations, same-definition two-context evaluation, reproducibility lineage,
and complete-or-denied aggregate disclosure. Sum/Mean, ranking/statistics,
general predicate ASTs, joins/UDFs, persisted analysis, analytics storage,
and public DTO/wire shapes remain Deferred or Provisional as marked in
`semantic-api.md`. Issue #150 implements the first Provisional provider-neutral
workspace/CLI slice without stabilizing those replaceable shapes.

## AI, security, and extensibility

| Document | Read it for |
| --- | --- |
| [`ai-agent-api.md`](ai-agent-api.md) | Implemented AI-facing read/explain/suggest adapter plus provisional typed Propose/Execute hostile boundary; #144 and #150 implement provider-neutral formula/scenario/formula-update and bounded Analysis Query workspace/CLI slices, while AI/public mappings remain deferred and current `Suggestion` remains neither SemanticPatch nor Approval |
| [`security-model.md`](security-model.md) | Security constraints and model |
| [`plugin-system.md`](plugin-system.md) | ADR-0028 game-engine host extension boundary; general plugin ABI/runtime/sandbox mechanics remain Deferred, while #134/#135 own narrower private/public policy |
| [`migration-framework.md`](migration-framework.md) | Progressive migration direction and future adapter framework |

Concrete AI mutation lifecycle and enforcement remain narrower implementation
work. ADR-0026 resolves #28's principal/capability/scope/Grant/Approval,
footprint, provenance, and structural exact-binding contract without selecting
canonical bytes, a digest profile, public DTOs, or enterprise IAM. #29 now
implements the provisional preview/apply lifecycle and Approval-state/atomic-
consumption seam; #30 implements the current provider-facing trusted-context,
prompt/data separation, bypass prevention, safe-code projection, and external-
effect denial seam; #93 supplies the internal semantic revision/session
mechanics. Public authentication and wire mapping remain deferred.
ADR-0019 allows deterministic read-only domain/extension validators to share
diagnostic semantics without deciding plugin runtime mechanics.

## Collaboration and history models

| Document | Read it for | Authority caution |
| --- | --- | --- |
| [`collaboration-model.md`](collaboration-model.md) | Current merge behavior, Accepted Semantic Conflict v1 evidence, ADR-0030 canonical delta evidence, ADR-0032 execution/transition taxonomy, ADR-0033 snapshot-first history profiles, and broader collaboration direction | ADR-0011 merge behavior, ADR-0031 conflict evidence, ADR-0029 snapshot/history boundary, ADR-0030 direct-state evidence, ADR-0032 taxonomy, and ADR-0033 bounded history/checkpoint guarantees are Accepted; broader collaboration remains Open Question |
| [`conflict-resolution.md`](conflict-resolution.md) | Normative `tachiko.semantic-conflict/v1` logical contract: typed targets/facets, three structural conflict kinds, logical identity, canonical ordering, and separation from semantic finalization failure | Accepted under ADR-0031 and realized by #223 in the production merge/workspace boundary; concrete Rust/CLI shapes remain replaceable and no codec, wire, or SDK contract is stabilized |
| [`operation-log-model.md`](operation-log-model.md) | Reconciled Command/attempt/revision/delta/receipt/optional-transition vocabulary plus ADR-0033 history profiles, checkpoints, replay verification, compaction, failure recovery, and Git boundary | ADR-0029, ADR-0032, and ADR-0033 logical guarantees are Accepted; concrete DTO/wire/storage/engine/adapter mechanics remain Deferred to separately Ready work |
| [`event-sourcing-model.md`](event-sourcing-model.md) | Reconciled semantic-event meaning and bounded snapshot-first use of event-sourcing techniques | ADR-0029 rejects core event sourcing, ADR-0032 defines semantic event as optional retained publication evidence, and ADR-0033 accepts only explicit retained-evidence or verified-tail techniques; concrete implementation remains Deferred |

Do not infer that event sourcing, universal CRDT/OT, or a persisted operation
log are selected merely because design documents exist for them. ADR-0029
accepts complete current-state snapshots and optional non-authoritative history
as the governing boundary. ADR-0032 fixes the attempt/revision/optional-event
taxonomy and receipt separation. ADR-0033 fixes the bounded snapshot-only,
retained-evidence, and verified-tail profiles plus checkpoint, replay,
compaction, failure-recovery, commitment, and Git-association boundaries without
selecting production DTOs or mechanisms. ADR-0031 separately fixes the
structural conflict evidence contract without selecting realtime transport or
conflict-resolution UI.

## Choosing the right source

When you need an answer:

1. Check the relevant Accepted ADR first.
2. Use this index to find the specification with the right role.
3. Read the specification's decision and implementation state.
4. Check the reconciliation register for mixed-state or supersession notes.
5. Use implementation/tests as evidence of current behavior, not automatic permanent authority.
6. If the required contract is still an Open Question, follow its Decision Issue instead of inventing a durable answer in implementation code.

When adding a new specification, prefer extending the most narrowly relevant existing contract over creating another similarly named parallel file.
