# Issue #10 Research: Headless Semantic API as a first-class product boundary

Status: Research / decision evidence

Decision issue: [#10](https://github.com/nurockplayer/tachiko-work/issues/10)

Related: #26, #27, #28, #104

Authority: this record is evidence for #10. It does not outrank the Product Constitution, Accepted ADRs, or the resulting #10 decision.

## Question

Should Tachiko Work promote a Headless Semantic API into the first-class product boundary shared by GUI, CLI, AI, automation, plugins/integrations, and future clients? If so, what is the smallest durable contract worth freezing before #26 chooses runtime and transport mechanics?

## Recommendation

Yes, with a strict boundary on what becomes stable.

Tachiko Work should define a first-class, transport-neutral, presentation-neutral, and storage-neutral Semantic API contract as the mandatory product boundary for first-party semantic use cases.

`tachiko-workspace-engine` is the current Rust implementation/application authority for that contract, but its Rust `pub` surface is not itself the public contract. Rust visibility, re-exported structs, `serde` shapes, crate layout, WASM ABI, JSON DTOs, IPC, FFI, and network encodings remain implementation or transport concerns unless a later explicit specification stabilizes them.

This is a promotion of **semantic laws and ownership**, not a promotion of the current implementation shape.

## Repository authority baseline

The recommendation follows existing Accepted authority rather than inferring architecture from shipped code:

- The Product Constitution places meaning above representation, makes versionability first-class, requires AI to operate on capabilities and meaning, and says the stable core should stay small.
- ADR-0007 makes AI a semantic client and keeps direct mutation approval-gated.
- ADR-0015 makes opaque stable IDs semantic identity while human keys, UI coordinates, content, and storage paths remain mutable addresses or projections.
- ADR-0016 makes `workspace-engine` the shared first-party application boundary and forbids CLI/AI/GUI adapters from reimplementing semantic mutation, validation, formula, diff, or merge policy. It explicitly defers external API stability to #10.
- ADR-0017 separates storage DTOs, serialization, migration, and persisted representation from the semantic model.
- ADR-0018 fixes formula binding, deterministic finite-binary64 meaning, dependency/failure authority, and atomic candidate preflight without making current Rust errors the public API.
- ADR-0019 distinguishes admission, structurally admissible semantic candidates, authoritative validation, and operation gates; it also accepts semantic-first diagnostics while deferring exact Rust/wire shapes to #10/#26.

Implementation through #72, #89, and #90 is useful conformance evidence that these authorities can be composed behind one application boundary. It is not the source of the decision.

## First-class client rule

A first-party client that reads product-semantic facts, asks for semantic explanation/validation, proposes semantic mutation, or executes semantic mutation must use the shared Semantic API contract.

This includes, as applicable:

- desktop/Tauri GUI;
- Web/WASM GUI;
- future mobile clients;
- CLI and CI/automation;
- AI/agent adapters;
- first-party integrations; and
- a future first-party plugin host when it performs semantic operations.

The rule applies to **semantic behavior**, not to one transport. A native CLI may call Rust directly, Web may cross a WASM bridge, Tauri may use IPC, and a future service may use a network protocol. Those transports may differ while mapping to the same semantic operation laws.

## Allowed internal bypasses

The first-class boundary is a client boundary, not a requirement that lower-level implementation code recursively call a public facade.

Allowed internal roles include:

- `workspace-engine` calling semantic-core, formula, diff, and merge engines according to ADR-0016;
- storage codecs/migration mapping representation to/from semantic state under ADR-0017;
- host composition such as `load -> semantic operation -> canonical save`;
- focused unit/conformance tests invoking the layer they test directly; and
- deterministic domain/extension validators participating through ADR-0019's read-only validation provider seam.

Not supported as product bypasses:

- GUI/CLI/AI directly mutating `Document` fields and then asking the engine to validate the result;
- native/WASM/IPC adapters implementing independent validation/formula/mutation policy; or
- future plugins receiving unrestricted mutable internal semantic structures merely because they run in-process.

## Query law

A Query is a deterministic read over a semantic context/snapshot. It does not publish a change to canonical semantic state.

Queries may return semantic objects or use-case projections, calculated values, formula analysis, validation reports, explanations, comparisons, or other semantic facts. Query outputs should be use-case-oriented rather than an accidental dump of internal Rust aggregate layout.

Stable semantic IDs are authoritative references in query results. Human keys, source paths, formula source, and presentation coordinates may appear as derived projections but cannot replace stable targeting.

The complete externally Stable query catalogue should be promoted operation-by-operation as real pressure appears. #10 does not require a generic `get(path)` or JSON-pointer API.

## Command law

A Command expresses typed semantic intent rather than representation CRUD.

An operation equivalent in meaning to `SetFieldValue(EntityId, FieldId, typed Value)` can be a semantic command because the target is semantic identity and execution remains subject to schema, formula, validation, and gate authority.

A representation patch such as `Patch("/entities/3/fields/7/value", ...)` is not a suitable stable semantic contract because it makes physical layout an API and weakens capability and invariant boundaries.

Commands are executed by the shared application authority, which owns applicable preconditions, candidate construction, formula binding/projection, validation/calculation, operation-specific gating, and atomic publication behavior. Clients do not mutate first and validate afterward as an alternate policy path.

## Propose and execute

The durable execution distinction should be small:

```text
Query
  read semantic facts; no semantic publication

Propose(Command | AtomicBatch)
  evaluate the same intent and authoritative rules
  without publishing the semantic transition

Execute(Command | AtomicBatch)
  request authoritative publication using the same intent and rules
```

`Propose` and `Execute` must share command semantics and gates. A proposal is not a weaker alternate validation system.

`Preview` is a presentation/review projection of a proposal rather than a separate semantic lifecycle state.

Finalization is an operation-gate concept over a candidate/purpose, not a mandatory client-visible two-phase state machine. Execute must re-evaluate authoritative preconditions/gates for the semantic state it actually acts on; a client cannot present a stale earlier `allowed=true` result as authority.

Proposal IDs, approval tokens, stale-proposal handling, resident state, revision tokens, and commit/session mechanics remain #26/#28 concerns.

## Semantic atomicity

The minimum batch contract worth accepting is **atomic semantic publication**, not a long-lived transaction API.

An Atomic Command Batch is an ordered set of semantic commands evaluated against one semantic base/context to produce one candidate transition. Publication succeeds for the whole batch or no semantic transition is published.

A conforming implementation is not required to apply the final operation gate after each internal batch step. Intermediate working candidates may temporarily violate higher-level diagnosable constraints when the batch itself is intended to repair them, but intrinsic representability/admission invariants remain intact and the final candidate must pass the authoritative gate required by the operation.

This supports the ADR-0015 case where deletion can be paired atomically with reference removal/retargeting and the ADR-0019 distinction between structurally admissible candidates and finalized snapshots.

The following are deliberately not frozen by #10:

- `begin`/`commit`/`rollback` session handles;
- nested transactions;
- database isolation levels;
- distributed transactions;
- filesystem rollback or durability semantics;
- concurrency/revision algorithms;
- event sourcing or undo/history;
- proposal identity/token format; and
- intra-batch temporary-object handle syntax.

## Result and failure meaning

The external contract should preserve semantic meaning without freezing current `WorkspaceError`, `EditPreview`, or other Rust enums/structs.

At minimum, clients must be able to distinguish:

- a completed semantic operation and its operation-specific result;
- failure before an admissible semantic candidate exists;
- semantic precondition/inapplicability failure;
- rejection by an authoritative operation gate, including the relevant `ValidationReport`/gate result; and
- operation-specific domain results such as merge conflicts where non-success is itself the typed result rather than a generic diagnostic.

Exact enum names, tagged-union encoding, field spelling, and Rust error hierarchy remain Provisional.

Storage/version/migration failures and transport/host failures remain separate representation/host families. They may be wrapped by adapters but do not become universal semantic diagnostics.

## Validation, gates, and formula outcomes

The public Semantic API depends on ADR-0019's semantic meaning of `ValidationReport`, not on the current Rust report struct.

Stable result meaning includes, as applicable:

- symbolic diagnostic code meaning;
- stable semantic subject identity;
- semantically relevant related subjects/facts;
- provider provenance;
- machine-readable classification concept; and
- formula facts already fixed by ADR-0018.

Human message wording, human-key paths, source spans, chosen cycle witnesses, exact severity enum, exact facts container, and Rust layout remain presentation or Provisional details.

Operation gating remains distinct from severity. Clients must consume the authoritative gate outcome rather than infer allow/deny from severity ordinals or from whether a report is empty.

New formula authoring that fails parse/bind/type construction belongs to admission/command failure before a new candidate exists. Formula graph/evaluation failures over an existing admissible candidate remain validation findings under ADR-0018/ADR-0019.

## Capability-addressability

Every semantic operation or operation family must be independently addressable for capability/authorization purposes.

Granting query/read/propose authority must not implicitly grant execute or arbitrary mutation authority.

#10 accepts this capability-addressability principle only. Capability ID grammar, principals, grants, approval tokens, provenance records, and security protocol remain #27/#28.

## Versioning and compatibility

Semantic API compatibility/versioning is independent from:

- `.ro` / `.roproj` representation versions;
- Cargo/crate package versions;
- diagnostic provider implementation versions;
- native/WASM/IPC/network transport versions; and
- runtime/session revision identifiers.

A breaking Semantic API change is one that requires a conforming client relying on a Stable semantic contract to change its semantic assumptions. Examples include changing an existing command's meaning/side effects, making a query mutate semantic state, changing stable-ID targeting, weakening/changing accepted atomicity, reinterpreting a published stable diagnostic code, removing a Stable operation/capability, adding a new mandatory input, or changing an Accepted semantic gate/formula/validation rule without the corresponding authority/version transition.

A conformance fix that corrects implementation to an already Accepted semantic contract is not made permanently breaking merely because undocumented buggy behavior existed.

Additive evolution may include new opt-in operations/capabilities, optional projections/facts, presentation fields, transport adapters, and new diagnostic codes compatible with the published unknown-code rules.

Published diagnostic code meaning cannot be silently reused. Unknown diagnostic codes must remain representable to older clients; clients must not depend on exhaustive code switches to decide operation eligibility because the authoritative gate is explicit.

Adding a new blocking semantic rule is not automatically an additive change merely because the wire payload only gained a new code. If it changes Accepted semantic behavior, it requires the corresponding decision/version process.

## Why Rust `pub`, serde, and crate layout are not the contract

The live Rust workspace deliberately exposes implementation conveniences:

- re-exported semantic types;
- public structs with fields;
- internal result/error enums;
- `serde` derives; and
- crate/module boundaries optimized for the current workspace.

Stabilizing those accidentally would turn source-level refactors into ecosystem commitments and conflate embedded Rust SemVer with the transport-neutral product contract.

A Rust item is part of the public Semantic API only when an explicit API specification/version classifies that surface as such. A `serde` shape is a wire contract only when a specification explicitly says so.

## Precedent signals

The research compared mature systems for transferable design lessons rather than direct adoption:

- Microsoft tactical DDD: application services orchestrate use cases while domain meaning remains below them.
- CQRS: queries do not mutate state; commands model meaningful tasks/intents rather than arbitrary data patches.
- ProseMirror: immutable state/transactions and a command that can inspect applicability or dispatch effect support a shared propose/apply idea without borrowing editor-position semantics.
- LSP: many heterogeneous clients can share one intelligence contract while presentation coordinates remain adapters.
- MCP: protocol versioning and capability negotiation are explicit, and advertised capability is distinct from ambient authority.
- Kubernetes API lifecycle: stable/beta/alpha and explicit deprecation/version evolution illustrate the cost of long-lived compatibility promises.
- Cargo SemVer: accidental public Rust structs/enums/items rapidly create downstream compatibility debt.

References:

- https://learn.microsoft.com/en-us/azure/architecture/microservices/model/tactical-ddd
- https://learn.microsoft.com/en-us/azure/architecture/patterns/cqrs
- https://prosemirror.net/docs/ref/
- https://microsoft.github.io/language-server-protocol/
- https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
- https://kubernetes.io/docs/reference/using-api/deprecation-policy/
- https://doc.rust-lang.org/cargo/reference/semver.html

## Alternatives

### Keep workspace-engine internal and let each client expose its own semantic API

Rejected. It recreates semantic policy duplication at the client boundary and reintroduces the GUI/AI/CLI drift ADR-0016 was designed to remove.

### Make semantic-core itself the public API

Rejected. `semantic-core` owns intrinsic model/invariants; operation preconditions, validation orchestration, formula/diff/merge composition, and operation gates belong to the application boundary.

### Freeze the current workspace-engine Rust API

Rejected. It would accidentally stabilize replaceable function signatures, re-exported structs, error enums, result shapes, and source-level ownership details.

### Build a generic CRUD / JSON Patch inner platform

Rejected. It would leak representation into semantic intent, weaken capability granularity, and encourage mutate-then-validate client behavior.

### Freeze a stateful prepare/commit transaction protocol now

Deferred. Propose/Execute semantics and atomic semantic publication have enough evidence; resident sessions, concurrency, revision tokens, approval tokens, and durability do not.

### Add a dedicated public `semantic-api` Rust crate now

Deferred. An embedded Rust facade may become useful when real downstream pressure exists. ADR-0016 warns against speculative layers and no current use case requires a new crate merely to record the product contract.

## #26 boundary

After #10, #26 should not redefine:

- Query versus Command meaning;
- Propose versus Execute guarantees;
- operation-gate authority;
- stable diagnostic/formula result meaning;
- minimum semantic atomicity;
- capability-addressability; or
- semantic compatibility/breaking-change rules.

#26 owns where resident state lives, runtime/session representation, revision/concurrency mechanics, native/WASM/IPC/FFI/network mapping, worker placement, delivery/invalidation, host capabilities, persistence composition, and concrete serialization/ABI.

The invariant is:

> A runtime/transport may host, retain, cache, serialize, or deliver the Semantic API; it may not redefine its semantic behavior.

## #104 Project Memory pressure test

#104 remains Research/Hypothesis and must not cause Project Memory vocabulary to enter semantic core.

It can pressure-test generic properties:

- domain-specific `why`/`impact`/`history`/`context` queries reusable across CLI/AI/GUI;
- external GitHub/Markdown identifiers remaining evidence/addresses rather than semantic identity;
- domain validators using the shared diagnostic envelope;
- AI receiving query/propose capabilities without execute authority;
- read-only GitHub/Markdown import remaining a host/domain-adapter concern; and
- later evidence-driven write-back testing batch atomicity without pre-building a transaction language.

Project Memory should first test how far ordinary typed entities/relationships, domain metadata, shared query/diagnostic semantics, and the first-class Semantic API can go. Provenance/history is still an open #104 question, not a generic core primitive accepted by #10.

## Independent review adjustment

The decision promotion intentionally narrows two illustrative parts of the research recommendation:

1. The example `SemanticOutcome<T>` and failure-family names are not frozen as a public taxonomy. Only the required semantic distinctions are Accepted; exact type names and encodings remain Provisional.
2. Atomic batch publication is Accepted, but no stateful transaction/session or intra-batch reference mechanism is implied. The batch is one semantic candidate transition whose final publication is all-or-nothing.

These adjustments preserve the report's architectural conclusion while applying the repository's `freeze less, classify more` rule.
