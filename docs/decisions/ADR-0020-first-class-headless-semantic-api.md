# ADR-0020: First-class Headless Semantic API boundary

## Status

Accepted

Decision issue: [#10](https://github.com/nurockplayer/tachiko-work/issues/10)

Amendment decision issue: [#32](https://github.com/nurockplayer/tachiko-work/issues/32)

Research: [`2026-08-24-headless-semantic-api-boundary.md`](../research/2026-08-24-headless-semantic-api-boundary.md)

Specified by: [`semantic-api.md`](../specs/semantic-api.md)

## Context

Tachiko Work now has Accepted authority for stable semantic identity (ADR-0015), Milestone 02 crate ownership and a shared first-party application boundary (ADR-0016), versioned storage/representation ownership (ADR-0017), bound deterministic formula meaning (ADR-0018), and staged semantic validation/diagnostics with explicit operation gating (ADR-0019).

Implementation through the workspace-engine migration and validation/formula hardening provides evidence that CLI, AI, native, and WASM-facing behavior can share one semantic application authority. That evidence does not decide which current Rust functions, result structs, errors, serde shapes, or transport DTOs should become long-lived public contracts.

Issue #10 must therefore decide **what product-semantic laws clients share** before #26 chooses runtime/state/transport mechanics.

The Product Constitution requires meaning to outrank representation, versionability to be first-class, AI to operate on capabilities and meaning, and the stable core to remain small. The project should freeze semantic ownership and expensive-to-reverse behavioral laws while keeping source-level and transport mechanisms replaceable.

## Decision

### 1. The Headless Semantic API is a first-class product boundary

Tachiko Work defines a first-class, transport-neutral, presentation-neutral, and storage-neutral Semantic API contract.

All first-party clients that read product-semantic facts, request semantic explanation/validation, propose semantic mutation, or execute semantic mutation MUST obtain that behavior through the shared Semantic API contract.

This includes, where applicable:

- CLI and CI/automation;
- desktop/Tauri GUI;
- Web/WASM GUI;
- future mobile clients;
- AI/agent adapters;
- first-party integrations; and
- a future first-party plugin host when it performs semantic operations.

The rule is about shared semantic behavior, not one invocation mechanism. Native calls, WASM bridges, IPC, FFI, or future network transports may map the same contract differently.

### 2. workspace-engine implements the contract; its Rust surface is not the contract

`tachiko-workspace-engine` is the current Rust implementation/application authority for first-party Semantic API behavior under ADR-0016.

Its current Rust function names, signatures, re-exports, public fields, error enums, result structs, module layout, and serde derives are workspace implementation details unless an explicit public API specification separately stabilizes them.

Rust `pub` means visibility, not automatic Tachiko Semantic API membership.

No new public `semantic-api` Rust crate is created by this decision. An embedded Rust facade remains Deferred until real downstream pressure justifies a separate lifecycle/stability boundary.

### 3. Queries are deterministic semantic reads

A Query reads semantic facts from a semantic context/snapshot and MUST NOT publish a change to canonical semantic state.

Queries may return use-case semantic projections, calculated values, validation reports, formula analysis, explanations, comparisons, or other semantic facts.

Stable semantic IDs are the authoritative references. Human keys, formula source addresses, UI coordinates, source spans, and storage paths may be returned as projections but MUST NOT replace stable semantic targeting.

The complete Stable query catalogue is not frozen here. Operations are promoted to Stable only when their semantic meaning has sufficient Accepted authority and real client pressure.

### 4. Commands express typed semantic intent, not representation CRUD

A Command represents a semantic operation over stable identities and typed semantic values.

Clients MUST NOT depend on physical storage paths, JSON pointers, array indexes, Rust struct fields, or UI coordinates as the durable mutation contract.

The shared application authority executes relevant semantic preconditions, candidate construction, formula binding/projection, validation/calculation, operation-specific gates, and atomic publication rules. A first-party client MUST NOT create an alternate mutate-then-validate policy path.

### 5. Propose and Execute share one command meaning

The contract distinguishes:

```text
Query
  read semantic facts; no semantic publication

Propose(Command | AtomicBatch)
  evaluate the same semantic intent and authoritative rules
  without publishing the semantic transition

Execute(Command | AtomicBatch)
  request authoritative publication using the same semantic intent and rules
```

`Propose` MUST NOT use weaker semantic rules than `Execute`.

`Preview` is a projection/review experience over a proposal and is not a separate canonical semantic lifecycle state.

Finalization is an authoritative operation-gate concept over a candidate/purpose; it is not a requirement for a client-visible stateful two-phase commit protocol. Execute MUST evaluate authoritative preconditions/gates for the state it actually acts on and MUST NOT trust a stale earlier client-side allow/deny calculation as authority.

Proposal IDs, stale-proposal handling, resident state, revision tokens, approval tokens, and commit/session mechanics remain outside this ADR. Subsequent ADR-0024 accepts the representation-neutral proposal occurrence, exact-change, compatibility-binding, semantic-base, and stale laws without changing the Command/Propose/Execute meaning accepted here; concrete revision, approval, and transport mechanisms remain separately owned.

### 6. Semantic publication is atomic

A single semantic command either publishes its complete authoritative semantic transition or publishes none of it.

Milestone 02 also accepts an **Atomic Command Batch** concept:

- a batch is an ordered collection of semantic commands evaluated against one semantic base/context to produce one candidate transition;
- final semantic publication is all-or-nothing; and
- no prefix of a failed batch becomes authoritative semantic state.

The implementation is not required to apply a final operation gate after each internal batch step. Intermediate working candidates MAY temporarily contain higher-level diagnosable invalidity when the batch is intended to repair it, provided intrinsic representability/admission invariants remain satisfied and the final candidate passes the authoritative gate required by the operation.

This ADR does not define nested transactions, `begin`/`commit`/`rollback` sessions, database isolation, distributed transactions, filesystem durability/rollback, concurrency algorithms, event sourcing, undo/history, proposal tokens, or intra-batch temporary-object handle syntax.

### 7. Semantic results preserve meaning without freezing current Rust types

The Semantic API MUST preserve enough machine-readable result meaning for a conforming client to distinguish, where applicable:

- completed semantic operation results;
- failure before an admissible semantic candidate exists;
- semantic precondition/inapplicability failure;
- rejection by an authoritative operation gate, including the relevant validation/gate result; and
- operation-specific domain outcomes such as merge conflicts.

The exact public enum/type names, tagged-union encoding, field spelling, Rust hierarchy, and wire representation remain Provisional.

Storage/version/migration failures and host/transport failures remain separate representation/host failure families rather than being relabeled as universal semantic diagnostics.

### 8. ValidationReport meaning and operation gates are first-class result semantics

ADR-0019 diagnostic meaning is part of the Semantic API result contract where an operation performs semantic validation.

Stable semantic meaning includes, as applicable:

- published symbolic diagnostic code meaning;
- stable semantic subjects;
- semantically relevant related subjects/facts;
- validator/provider provenance;
- machine-readable classification concept; and
- ADR-0018 formula facts already accepted as semantic outcomes.

The exact Rust `ValidationReport` structure, diagnostic severity enum, facts container, source spans, human-key paths, message wording, sort implementation, and wire layout are not stabilized by this ADR.

Operation gates and diagnostic severity remain distinct. Clients MUST use the authoritative gate outcome for operation allow/deny behavior rather than derive policy from severity ordinal, localized prose, or merely whether the report is empty.

New formula authoring that fails parse/bind/type construction remains an admission/command failure before a new semantic candidate exists. Formula graph/evaluation failures over an existing structurally admissible candidate remain validation findings according to ADR-0018/ADR-0019.

### 9. Semantic operations are capability-addressable

Every semantic operation or operation family MUST be independently addressable for capability/authorization purposes.

Granting query/read/propose authority MUST NOT implicitly grant execute or arbitrary mutation authority.

This ADR accepts capability-addressability only. ADR-0024 defines the exact proposal/base binding consumed by later authorization. Capability identifiers, principals, grants, approval tokens, provenance records, digest/integrity, and security protocol remain #28.

### 10. Semantic API versioning is independent from representation and transport versioning

Semantic API compatibility/versioning is a separate axis from:

- `.ro` / `.roproj` representation versions;
- Rust crate/package versions;
- diagnostic provider implementation versions;
- native/WASM/IPC/FFI/network transport versions; and
- runtime/session revision identifiers.

A Semantic API change is breaking when a conforming client relying on a Stable semantic contract must change a semantic assumption, including incompatible changes to Stable operation meaning, state effects, stable-ID targeting, accepted atomicity, published diagnostic-code meaning, required inputs, or Accepted gate/formula/validation behavior.

A conformance fix that restores implementation to already Accepted authority does not create a permanent compatibility entitlement to undocumented buggy behavior.

Additive evolution may include new opt-in operations/capabilities, optional projections/facts, presentation fields, adapters/transports, and compatible new diagnostic codes.

Published diagnostic code meaning MUST NOT be silently reused. Unknown diagnostic codes MUST remain representable to older clients; clients MUST NOT require exhaustive code matching to derive gate policy because authoritative gate outcomes are explicit.

Adding a new blocking semantic rule is not automatically additive merely because a representation only gained a new diagnostic code. If it changes Accepted semantic behavior, it requires the corresponding decision/version process.

### 11. Bypass is role-based and explicitly bounded

Allowed internal implementation paths include:

- `workspace-engine` calling the lower semantic/formula/diff/merge owners fixed by ADR-0016;
- storage codecs/migration mapping representations at the ADR-0017 boundary;
- host composition such as `load -> semantic operation -> canonical save`;
- focused unit/conformance tests directly exercising the owner they test; and
- deterministic read-only domain/extension validators through ADR-0019's provider seam.

These are not alternate first-party semantic client authorities.

A first-party GUI, CLI, AI, runtime bridge, or integration MUST NOT bypass the Semantic API merely because it shares a process, language, or Rust crate graph with the implementation.

### 12. M04 promotes formula reasoning, scenarios, and formula update meaning

Game Studio Beta promotes three related logical operation families into the
Accepted Semantic API contract. Their semantic meaning is stable; exact
operation names, Rust types, result field names, wire encodings, transport
identifiers, and catalogue layout remain Provisional.

#### Formula reasoning is Query behavior

A bounded formula-reasoning Query MUST support, as requested and applicable:

- inspect one formula's complete typed bound expression meaning without making
  the current Rust AST layout public API;
- identify its stable-ID direct inputs and direct dependents;
- evaluate it through the authoritative ADR-0018 formula engine; and
- return deterministic dependency, impact, calculation, and applicable
  ADR-0019 validation facts needed to explain the result.

Stable semantic identities and structured expression meaning are
authoritative. Formula source text and mutable human addresses remain
ADR-0018 authoring/presentation projections. Natural-language explanation is
an optional adapter projection over the structured result and is never formula
or calculation authority. No client or AI adapter may create a separate
formula evaluator, dependency graph, or validation path.

#### A scenario is a pure read-only hypothetical evaluation

The minimum M04 scenario Query means:

```text
one exact source semantic revision
+ one bounded ordered collection of typed Number overrides
+ one bounded set of requested stable result/inspection targets
```

Each override targets by stable semantic identity one existing field whose
current semantic value is a Number, not a Formula. A target occurs at most once.
Normalization applies ADR-0018 Number normalization and preserves request order.
The application authority applies all overrides to one transient candidate
derived from the exact source snapshot, then runs the same authoritative
formula calculation and validation used outside the scenario. Override order
is reproducibility evidence, not a sequence of intermediate publications.
Requested targets form a stable-identity set: duplicate request occurrences
normalize to one member, and target request order is not semantic.

The result preserves enough structured meaning to identify:

- the exact source revision/context;
- the normalized ordered stable-target/typed-Number overrides;
- authoritative baseline and scenario formula outcomes for requested subjects;
- deterministic changed and affected subjects available from the existing
  semantic engines;
- applicable validation/diagnostic outcomes; and
- dependency facts sufficient to explain why a requested outcome changed.

The same exact source revision, normalized overrides, and requested targets
produce the same semantic result. Invalid or duplicate overrides, missing or
wrong-typed targets, formula failures, cycles, and validation failures return
structured failure evidence and publish nothing. A scenario is not canonical
state, a SemanticPatch, a saved branch, a mutation transaction, or a persisted
scenario object.

Formula, schema, or structural mutation inside a scenario; parameter sweeps;
optimization; randomness; statistics; and persisted scenarios remain Deferred.
Exact finite request limits, normalization encoding, DTOs, and revision-token
encoding remain Provisional and must not change the logical laws above.

#### Formula update is one normal typed Command

The formula-update Command meaning includes:

- the stable target identity;
- the complete accepted typed bound formula meaning;
- every stable reference in that bound formula; and
- command-owned semantic preconditions required by the Accepted formula
  contract.

Authoring source text is only bounded parse/bind/type-check input. The complete
typed bound meaning is fixed before proposal identity is issued. A reviewable
formula update therefore uses the existing path:

```text
formula-update Command
  -> Propose
  -> immutable revision-pinned SemanticPatch
  -> candidate / diff / dependency impact / validation
  -> authorization and exact Approval where ADR-0026 requires it
  -> Execute through the shared lifecycle
```

This amendment creates neither `FormulaPatch`, a formula-specific approval
token, nor an AI-only mutation vocabulary.

#### Existing authorization and approval laws apply unchanged

Formula reasoning and scenario operations require applicable Query authority
for their operation family and every disclosed semantic scope. They do not
require mutation Approval merely because an AI adapter consumes the result.

Formula-update proposal and execution require the existing Propose/Execute
actions, Formula mutation class, trusted disclosure/write-footprint
derivation, and exact Approval rules from ADR-0026. Preview evidence outside
live Query authority is denied or safely reduced. Delegated-origin or
Delegated-authority publication requires the existing exact finite Human
Approval. A successful calculation, validation, or scenario never grants
Propose or Execute authority.

Production implementation is not authorized by this amendment alone. It
requires a separate implementation Issue that consumes this Accepted contract.

## #26 dependency boundary

After this ADR, #26 owns **how** the Semantic API is hosted and transported, not **what its semantic behavior means**.

#26 may decide:

- resident state placement and runtime/session representation;
- native/WASM/IPC/FFI/network mapping;
- Web Worker placement;
- revision/concurrency mechanics;
- push/pull delivery and projection invalidation;
- host capabilities;
- filesystem/browser persistence composition; and
- concrete serialization/ABI.

#26 MUST preserve the following invariant:

> A runtime or transport may host, retain, cache, serialize, or deliver the Semantic API; it may not redefine its semantic behavior.

For the same relevant semantic base/context and capability, native and WASM implementations must preserve equivalent Stable semantic results, gate decisions, diagnostics/formula facts, and atomicity even when their concrete transport/runtime mechanisms differ.

## Project Memory / #104 boundary

Issue #104 remains Research/Hypothesis and may pressure-test this Semantic API as a reference application.

It MUST NOT use #10 to promote Project Memory-specific concepts such as `Decision`, `ADR`, `GitHubIssue`, `Commit`, `WhyQuery`, or provenance workflow into semantic core.

Project Memory should first test domain-specific typed entities/relations/metadata, shared query/diagnostic behavior, read-only adapters, capability-limited AI access, and the generic Atomic Batch contract before arguing for new generic primitives.

## Deliberately Provisional or Deferred

The following remain intentionally replaceable or unresolved:

- current workspace-engine function names/signatures;
- current `WorkspaceError` variants;
- current `EditPreview { Document, SemanticDiff }` shape;
- exact Rust `ValidationReport` type/methods;
- exact diagnostic severity vocabulary;
- exact diagnostic namespace/catalog spelling, while published code meanings remain stable;
- exact related/facts encoding;
- full externally Stable operation catalogue;
- exact formula-reasoning, scenario, and formula-update operation-family
  identifiers or catalogue entries;
- exact scenario request limits, normalization/ordering representation, and
  result DTO fields;
- exact result field/tagged-union representation;
- semantic effect/diff projection shape;
- concrete revision/precondition token representation (#93);
- proposal-ID encoding/generation and transport shape (Provisional under ADR-0024);
- canonical proposal bytes, digest, signature, MAC, and approval binding (#28);
- intra-batch created-object reference syntax;
- capability ID/grant/approval/provenance format (#28);
- embedded Rust SDK or new public API crate;
- native/WASM/IPC/FFI/network serialization (#26); and
- stable API deprecation-support window beyond the compatibility laws above.

## Explicit non-goals

This ADR does not:

- select JSON, Protobuf, IPC, FFI, WASM ABI, or a network protocol;
- decide #26 resident state, Worker placement, revision/concurrency, or persistence composition;
- design plugin ABI/runtime/sandbox/distribution (Deferred; ADR-0028 later
  resolves only the M04 game-engine host-adapter classification);
- redesign `.roproj` (#41);
- reopen ADR-0015 through ADR-0019;
- create a generic CRUD/JSON-Patch platform;
- create a generic transaction scripting language;
- require operation logs, event sourcing, undo, or history for batch atomicity;
- implement formula reasoning, scenario evaluation, or formula update;
- define persisted scenarios, scenario mutation, sweeps, optimization,
  randomness, statistics, or a generic data-analysis IR;
- promote Project Memory vocabulary or provenance workflow into semantic core; or
- introduce production code.

## Alternatives considered

### Keep workspace-engine internal-only and let each client define an external semantic API

Rejected. This would recreate semantic policy duplication and behavior drift at the client boundary.

### Make semantic-core the public API

Rejected. Intrinsic model/invariants are below application preconditions, validation orchestration, formula/diff/merge composition, and operation gating.

### Freeze the current workspace-engine Rust API

Rejected. It would accidentally stabilize replaceable functions, re-exports, structs, errors, result shapes, and source-level ownership details.

### Build generic CRUD / JSON Patch

Rejected. Representation layout would become the product contract and clients could bypass intent-level invariant/capability boundaries.

### Freeze a stateful prepare/commit transaction API now

Deferred. Semantic Propose/Execute and atomic publication are accepted; runtime/session/concurrency/durability mechanics lack sufficient evidence and belong to #26/#28.

### Add a new public semantic-api Rust crate now

Deferred. No current downstream lifecycle requires it, and ADR-0016 rejects speculative layering.

## Consequences

Positive:

- GUI, CLI, AI, automation, and future clients share one explicit semantic product boundary;
- #26 can design runtime/transport without inventing semantic behavior;
- representation, Rust source layout, and transport can evolve without silently redefining meaning;
- AI capability and approval work gains a stable semantic operation surface without #10 pre-designing security tokens;
- native/WASM conformance can test semantic equivalence rather than source-level type equality; and
- #104 gains a real domain pressure test without contaminating semantic core.

Costs:

- current internal workspace-engine APIs do not yet constitute a complete deliberate public operation catalogue;
- adapters that currently inspect re-exported/internal document fields may need future query projections;
- #29 implements a provisional field-value/ordered-batch lifecycle, while the
  complete public operation catalogue and runtime/transport mapping remain
  implementation gaps; and
- future public exposure requires an intentional versioned mapping rather than exporting existing Rust/serde types directly.

## Required follow-up

- `docs/specs/semantic-api.md` defines the normative contract and stability classification.
- Reconcile `diagnostics-contract.md`, `validation-engine.md`, `ai-agent-api.md`, `rust-crate-architecture.md`, `frontend-backend-boundary.md`, and the canonical reconciliation register.
- Close #10 with a Decision Capsule pointing to this ADR/spec.
- #26 may now proceed against the Accepted Semantic API boundary.
- ADR-0024 resolves #27's revision-pinned immutable proposal contract without adding another operation vocabulary.
- #28 continues to own capability/approval/provenance and digest/integrity protocol.
- #32 is resolved by the M04 amendment above and the normative Semantic API
  specification; a separate implementation Issue must own the first
  provider-neutral workspace/CLI slice.
- #104 remains a later read-only-first reference/dogfood pressure test.

## Related

- Product Constitution §§2.2, 2.5, 2.6, 2.7, 6, 7
- ADR-0007
- ADR-0015
- ADR-0016
- ADR-0017
- ADR-0018
- ADR-0019
- ADR-0024
- Issues #10, #17, #26, #27, #28, #32, #104
