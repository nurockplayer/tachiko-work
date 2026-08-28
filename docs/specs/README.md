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
| [`semantic-api.md`](semantic-api.md) | ADR-0020 first-class transport-neutral Semantic API, ADR-0024 immutable revision-pinned SemanticPatch, ADR-0026 authorization/approval integration, and ADR-0022 runtime-host ownership constraints; exact DTO/session/transport mechanics remain Deferred |
| [`semantic-authorization.md`](semantic-authorization.md) | ADR-0026 principal, capability, stable-ID scope, trusted footprint, Grant, exact Human Approval, expiry/replay/revocation, provenance, and effect-domain contract |
| [`semantic-data-model.md`](semantic-data-model.md) | Semantic data-model contract and terminology |
| [`schema-system.md`](schema-system.md) | Durable schema declaration behavior and the boundary from runtime validation policy |
| [`validation-engine.md`](validation-engine.md) | ADR-0019 staged validation, candidate/finalization semantics, full-validation oracle, and operation gating |
| [`diagnostics-contract.md`](diagnostics-contract.md) | Semantic-ID-centered machine-readable diagnostic meaning and stability boundaries |
| [`formula-engine-spec.md`](formula-engine-spec.md) | Accepted bounded authoring and ADR-0018 stable-ID projection/rename, binary64, dependency, and recomputation contract |
| [`semantic-diff-spec.md`](semantic-diff-spec.md) | Semantic diff behavior |

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
lifecycle/state/publication seam; hostile-boundary enforcement and concrete
resident revision/session mechanics remain #30/#93. ADR-0022 accepts the resident shared Rust runtime,
no-second-canonical-client-model rule, host separation, explicit snapshot
boundaries, and native/WASM semantic parity while leaving exact
session/revision/Worker/ABI/persistence mechanics Deferred to #93–#95 and future
host/transport implementation. ADR-0019 resolves the validation/diagnostics
architecture; formula binding/failure/numeric semantics remain governed by
ADR-0018; stable identity by ADR-0015; storage representation by ADR-0017.

## AI, security, and extensibility

| Document | Read it for |
| --- | --- |
| [`ai-agent-api.md`](ai-agent-api.md) | Implemented AI-facing read/explain/suggest adapter over ADR-0007 delegated authority, ADR-0020 Semantic API behavior, ADR-0024 proposal rules, and ADR-0026 authorization/approval; current `Suggestion` is not SemanticPatch or Approval |
| [`security-model.md`](security-model.md) | Security constraints and model |
| [`plugin-system.md`](plugin-system.md) | ADR-0028 game-engine host extension boundary; general plugin ABI/runtime/sandbox mechanics remain Deferred, while #134/#135 own narrower private/public policy |
| [`migration-framework.md`](migration-framework.md) | Progressive migration direction and future adapter framework |

Concrete AI mutation lifecycle and enforcement remain narrower implementation
work. ADR-0026 resolves #28's principal/capability/scope/Grant/Approval,
footprint, provenance, and structural exact-binding contract without selecting
canonical bytes, a digest profile, public DTOs, or enterprise IAM. #29 now
implements the provisional preview/apply lifecycle and Approval-state/atomic-consumption seam; #30
owns trusted enforcement, prompt/data separation, bypass prevention, and
external-effect denial; #93 owns concrete semantic revision/session mechanics.
ADR-0019 allows deterministic read-only domain/extension validators to share
diagnostic semantics without deciding plugin runtime mechanics.

## Collaboration and history models

| Document | Read it for | Authority caution |
| --- | --- | --- |
| [`collaboration-model.md`](collaboration-model.md) | Current merge behavior and broader collaboration direction | Current semantic merge is Accepted; broader collaboration remains Open Question |
| [`conflict-resolution.md`](conflict-resolution.md) | Current conflict behavior and future conflict model | Provisional around current merge; future model remains Open Question |
| [`operation-log-model.md`](operation-log-model.md) | Persisted operation-log proposal | Open Question; no first-class persisted log in v0.1 |
| [`event-sourcing-model.md`](event-sourcing-model.md) | Event-sourcing hypothesis | Hypothesis, not accepted architecture |

Do not infer that event sourcing, CRDTs, or a persisted operation log are selected merely because design documents exist for them.

## Choosing the right source

When you need an answer:

1. Check the relevant Accepted ADR first.
2. Use this index to find the specification with the right role.
3. Read the specification's decision and implementation state.
4. Check the reconciliation register for mixed-state or supersession notes.
5. Use implementation/tests as evidence of current behavior, not automatic permanent authority.
6. If the required contract is still an Open Question, follow its Decision Issue instead of inventing a durable answer in implementation code.

When adding a new specification, prefer extending the most narrowly relevant existing contract over creating another similarly named parallel file.
