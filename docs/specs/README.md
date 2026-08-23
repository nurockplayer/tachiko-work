# Specifications

Specifications describe implementable contracts, provisional baselines, and future design targets for Tachiko Work.

A file living in `docs/specs/` is **not automatically Accepted**. Read its explicit decision state together with [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and the [`canonical reconciliation register`](../governance/canonical-reconciliation-register.md).

## Format and representation: read this first

The format documents have different roles. Do not treat similarly named files as parallel normative specifications.

| Document | Read it for | Current role |
| --- | --- | --- |
| [`ro-format-and-roproj-spec.md`](ro-format-and-roproj-spec.md) | Accepted `.roproj` source / `.ro` portable-artifact relationship and current implementation split | Accepted direction under ADR-0003 |
| [`storage-versioning-and-migration.md`](storage-versioning-and-migration.md) | Version namespaces, versioned DTO ownership, supported/unsupported behavior, migration architecture | Mixed: Accepted invariants under ADR-0017 plus Provisional M02 mechanics |
| [`canonical-json-profile.md`](canonical-json-profile.md) | Deterministic JSON/Unicode/order/whitespace contract and explicit numeric boundary | Mixed: Accepted invariants under ADR-0017/ADR-0018 plus version-specific M02 profile/resource-limit mechanisms |
| [`ro-format-v1.md`](ro-format-v1.md) | Exact deterministic direct `.ro` JSON behavior shipped by the v0.1 CLI | Immutable legacy compatibility / migration source |
| [`ro-format-v2.md`](ro-format-v2.md) | Current identity-aware direct `.ro` JSON DTO, canonical writer, Number/resource profile, and bound references | Mixed: Accepted ADR-0015/ADR-0017/ADR-0018 invariants plus Provisional M02 wire/resource mechanics |
| [`ro-format.md`](ro-format.md) | Compatibility/navigation entry point for older links | Non-normative navigation stub; follow the format documents above |
| [`roproj-format.md`](roproj-format.md) | Target `.roproj` representation | Accepted direction, not yet implemented |
| [`roproj-layout-v1.md`](roproj-layout-v1.md) | Candidate physical `.roproj` layout | Provisional, not yet implemented; #41 owns layout work |
| [`runtime-export-v1.md`](runtime-export-v1.md) | Historical evaluated runtime JSON export contract | Frozen historical contract |
| [`runtime-export-v2.md`](runtime-export-v2.md) | Current stable-identity/normalized-Number evaluated runtime JSON export | Provisional implemented contract |

The semantic model owns meaning. Physical formats are representations. ADR-0017 fixes the versioned storage boundary and canonical-representation invariants; ADR-0018 fixes the admitted-token binary64 conversion and spelling authority. Physical `.roproj` layout and future `.ro` packaging remain separately owned.

## Semantic core, schema, validation, formulas, and diff

| Document | Read it for |
| --- | --- |
| [`semantic-data-model.md`](semantic-data-model.md) | Semantic data-model contract and terminology |
| [`schema-system.md`](schema-system.md) | Durable schema declaration behavior and the boundary from runtime validation policy |
| [`validation-engine.md`](validation-engine.md) | ADR-0019 staged validation, candidate/finalization semantics, full-validation oracle, and operation gating |
| [`diagnostics-contract.md`](diagnostics-contract.md) | Semantic-ID-centered machine-readable diagnostic meaning and stability boundaries |
| [`formula-engine-spec.md`](formula-engine-spec.md) | Accepted bounded authoring and ADR-0018 stable-ID projection/rename, binary64, dependency, and recomputation contract |
| [`semantic-diff-spec.md`](semantic-diff-spec.md) | Semantic diff behavior |

ADR-0019 resolves the general validation/diagnostics architecture while keeping exact Rust APIs and external transports Provisional. Formula binding, failure precedence, SCC meaning, and numeric determinism remain governed by ADR-0018; stable identity by ADR-0015; storage representation failures by ADR-0017. #10 still owns public Semantic API/wire stability and #26 owns native/WASM/IPC runtime transport.

## AI, security, and extensibility

| Document | Read it for |
| --- | --- |
| [`ai-agent-api.md`](ai-agent-api.md) | Implemented read/explain/suggest AI surface under ADR-0007 |
| [`security-model.md`](security-model.md) | Security constraints and model |
| [`plugin-system.md`](plugin-system.md) | Accepted extensibility direction plus still-open runtime/sandbox design |
| [`migration-framework.md`](migration-framework.md) | Progressive migration direction and future adapter framework |

Concrete AI mutation, capability, provenance, plugin ABI/sandbox, and migration fidelity contracts remain narrower decision work even when the broader direction is Accepted. ADR-0019 allows deterministic read-only domain/extension validators to share diagnostic semantics without deciding plugin runtime mechanics.

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
