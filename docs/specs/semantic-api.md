# Semantic API Specification

Decision state: Mixed. The first-class boundary and semantic laws are Accepted
under [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md).
Runtime ownership, resident interactive topology, host separation, explicit
snapshot boundaries, and native/WASM semantic parity are Accepted under
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).
Exact Rust APIs, complete operation catalogue, wire schemas, transports,
session/revision mechanics, and several result/projection shapes remain
Provisional or Deferred as marked below.

Implementation state: partially implemented through `workspace-engine` as the
shared first-party application authority. Current Rust functions and result
structures are implementation evidence, not the versioned public product
contract. Current operations remain substantially snapshot-style; the resident
runtime/session implementation is later work under #93–#95.

Decision issue: [#10](https://github.com/nurockplayer/tachiko-work/issues/10)

## Purpose

Define the smallest transport-neutral semantic command/query/result contract
that GUI, CLI, AI, automation, and future first-party clients must share.

This specification answers **what semantic operations mean**. ADR-0022 answers
the durable runtime/host ownership rules. Neither specification freezes how
requests/results are serialized over native, WASM, IPC, FFI, or network
transports or the concrete session/revision protocol.

## Contract boundary

Conceptually:

```text
GUI / Web / Tauri / Mobile
CLI / CI / Automation
AI / Agents
future first-party integrations
future first-party plugin host
        |
        v
+-----------------------------------+
| First-class Semantic API          |
|                                   |
| Queries                           |
| Semantic Commands                 |
| Propose / Execute                 |
| Atomic Command Batch              |
| ValidationReport + Gate Outcome   |
| Stable machine outcomes           |
| Capability-addressable operations |
+-----------------------------------+
        |
        v
workspace-engine
   /       |        \
semantic  formula   diff/merge
 core      engine    engines

storage / filesystem / IndexedDB / Git / IPC / WASM bridge
= representation/host/transport concerns,
  not alternate semantic authority
```

`tachiko-workspace-engine` currently implements the application boundary under
ADR-0016. The diagram does not make its source-level Rust API the external
contract.

For interactive clients, ADR-0022 places authoritative in-memory semantic state
in the shared Rust semantic/application runtime and forbids a second
independently authoritative frontend document model. Host/storage capabilities
remain beside that runtime rather than becoming semantic authority.

## Mandatory first-party client rule

A first-party client MUST use the Semantic API contract when it:

- reads product-semantic facts;
- requests semantic validation, explanation, calculation, comparison, or impact;
- proposes a semantic change; or
- requests publication of a semantic change.

A first-party adapter MUST NOT reproduce alternate semantic mutation,
validation, formula, diff, merge, or gating policy.

Different invocation/transport mechanisms are allowed when they preserve the
same semantic contract.

## Semantic context

Queries and commands operate against a semantic context/snapshot whose semantic
identity and content are governed by the existing semantic model and Accepted
ADRs.

This specification does not freeze a new public `Workspace`, `Project`, session,
or revision type. Milestone 02 semantic references remain document-local where
ADR-0015 says they are document-local. ADR-0022 accepts a resident shared Rust
runtime as the preferred interactive topology while the exact session handle,
revision/precondition representation, concurrency/conflict policy, cancellation,
and runtime state-installation mechanics remain Deferred to #93 and related
runtime work.

## Stable targeting

Durable API targeting uses stable semantic identity as defined by ADR-0015.

Human-facing keys, labels, authoring paths, formula-source addresses, source
spans, UI coordinates, storage paths, and collection indexes MAY be accepted or
returned as authoring/presentation projections when an operation explicitly
supports them. They MUST NOT silently become durable target identity.

Representation addresses such as JSON Pointer or `.roproj` paths are not the
generic semantic mutation API.

## Query contract

A Query:

1. reads semantic facts from a semantic context/snapshot;
2. MUST NOT publish a change to canonical semantic state;
3. is deterministic when its authoritative inputs/configuration are
   deterministic; and
4. returns use-case semantic results/projections rather than requiring clients
   to depend on internal Rust aggregate layout.

Queries MAY expose concepts such as:

- semantic object inspection/description;
- validation reports and gate inspection;
- calculated values;
- formula analysis/explanation;
- semantic diff/comparison/impact;
- merge/reconciliation inspection where the operation is read-only; and
- domain-specific queries implemented above generic semantic foundations.

This list is illustrative. The complete externally Stable operation catalogue
is Provisional and is promoted operation-by-operation.

A generic `get(path)` / JSON-pointer query surface is not part of the Accepted
contract.

## Command contract

A Command expresses a typed semantic intent.

A conforming semantic command:

- targets semantic identity rather than storage/layout coordinates;
- supplies typed semantic input rather than arbitrary representation patches;
- is evaluated by the shared application authority;
- applies relevant semantic preconditions;
- forms a candidate transition according to Accepted semantic/formula rules;
- participates in authoritative validation/calculation/gating where required;
- and follows the atomic publication rules below.

An intent equivalent to changing a typed field value by `EntityId + FieldId` can
be a semantic command. An arbitrary mutation of an internal Rust field or JSON
path is not the stable product contract merely because an adapter can express
one mechanically.

## Query, Propose, and Execute

The Accepted semantic execution intents are:

```text
Query
  -> read semantic facts only

Propose(Command | AtomicBatch)
  -> evaluate the semantic intent and authoritative rules
  -> do not publish the semantic transition

Execute(Command | AtomicBatch)
  -> evaluate the same semantic intent and authoritative rules
  -> request authoritative semantic publication
```

### Shared semantics

`Propose` and `Execute` MUST share the same command meaning, validation authority,
and gate semantics. Propose is not a weaker alternate validation path.

### Preview

A Preview is a client/product projection of proposal facts for review. It is not
an independent canonical semantic state or mandatory protocol phase.

### Finalization and gates

Finalization means applying an operation-specific authoritative gate to the
candidate/purpose before publication. It does not require a long-lived public
prepare/finalize/commit state machine.

Execute MUST evaluate the authoritative preconditions/gates for the semantic
state it actually acts on. A client MUST NOT convert an earlier gate decision
into ambient authority for a later changed state.

Proposal IDs, stale proposal handling, revision tokens, approval tokens, and
concrete session/commit mechanics are outside the Stable contract. ADR-0022
accepts the resident runtime/state ownership law without freezing those
mechanisms.

## Semantic atomicity

### Single command

A semantic command either publishes the complete authoritative semantic
transition or publishes no semantic transition.

### Atomic command batch

An Atomic Command Batch:

- is an ordered collection of semantic commands evaluated against one semantic
  base/context;
- forms one candidate semantic transition; and
- publishes all of that final transition or none of it.

No failed batch prefix becomes authoritative semantic state.

An implementation is not required to apply the final operation gate after each
internal command in the batch. Intermediate working candidates MAY contain
higher-level diagnosable invalidity when a later command in the same batch is
intended to repair it, provided:

- intrinsic admission/representability invariants remain satisfied; and
- the final candidate passes the authoritative operation gate required for
  publication.

This enables one explicit atomic semantic operation to remove/retarget inbound
references together with deletion as allowed by ADR-0015 without weakening the
final validation contract.

### Not implied by atomic batch

Atomic batch does not by itself define:

- nested transactions;
- `begin` / `commit` / `rollback` handles;
- database isolation levels;
- distributed transactions;
- filesystem durability/rollback;
- runtime concurrency or revision-conflict algorithms;
- event sourcing or operation logs;
- undo/redo history;
- proposal tokens; or
- intra-batch temporary-object handle syntax.

ADR-0022 likewise does not turn semantic atomicity into a specific runtime
commit/swap/locking/cloning algorithm.

## Result contract

The Semantic API result must preserve operation-specific semantic meaning.
There is no requirement that all operations return one universal response bag.

A conforming client must be able to distinguish, where applicable:

1. completed semantic operation results;
2. failure before a new admissible semantic candidate exists;
3. semantic precondition/inapplicability failure;
4. rejection by the authoritative operation gate, including relevant validation
   and gate facts; and
5. typed operation-specific outcomes such as merge conflict/reconciliation
   results.

Exact public enum names, generic type constructors, tagged-union representation,
field spelling, and Rust error hierarchy are Provisional.

`WorkspaceError`, `CalculationError`, `EditPreview`, or another current internal
Rust type is not automatically a public Semantic API type.

## Failure family boundaries

### Admission / construction failure

A request can fail before a new structurally admissible semantic candidate
exists. Examples include newly authored formula source that fails Accepted
parse/bind/type construction or other Accepted intrinsic representability
barriers.

Exact API error encoding remains Provisional.

### Semantic precondition failure

An otherwise well-formed semantic command can be inapplicable to the current
semantic state, for example because a stable target does not exist or the
operation's semantic precondition does not hold.

Exact taxonomy remains Provisional.

### Gate rejection

A structurally admissible candidate can be rejected for publication by an
authoritative operation gate. The result must preserve the relevant
`ValidationReport` and gate outcome meaning.

### Operation-specific domain outcome

Some operations have typed outcomes that should not be flattened into generic
diagnostics. A semantic merge conflict is an example.

### Representation and host failure

Storage/version/migration errors remain representation-local under ADR-0017.
Filesystem, browser-host, transport disconnect, IPC, authentication, and similar
host/transport failures remain outside semantic diagnostics unless a separate
Accepted contract says otherwise.

Adapters MAY combine these result families for a client transport, but they
must preserve which authority produced the failure.

## ValidationReport contract

When an operation performs authoritative semantic validation, the result uses
the diagnostic meaning Accepted by ADR-0019 and `diagnostics-contract.md`.

Stable semantic observations include, where applicable:

- published symbolic diagnostic code meaning;
- stable semantic subject identity;
- semantically relevant related subjects/facts;
- validator/provider provenance;
- a machine-readable classification concept; and
- formula facts already Accepted by ADR-0018.

The following are not stabilized by this specification:

- exact Rust `ValidationReport` layout/methods;
- exact severity enum;
- exact diagnostic code namespace/catalog spelling, except that published code
  meanings cannot be silently reused;
- exact primary/related/facts container;
- localized message/help text;
- human-key paths;
- source spans;
- selected cycle witnesses;
- exact ordering implementation; and
- external wire schema.

## Gate outcome contract

Diagnostic classification/severity and operation gating are separate concepts.

A client MUST use the authoritative operation gate outcome to decide whether the
requested semantic operation may publish. It MUST NOT derive semantic
allow/deny from:

- severity ordinal alone;
- localized message wording;
- presence/absence of any diagnostic whatsoever; or
- a client-maintained copy of validation rules.

Interactive editing, strict mutation, export, and CI/workflow policy may apply
different gates to the same underlying diagnostic meaning without changing the
diagnostic code identity.

## Formula outcome relationship

ADR-0018 remains formula authority.

New authoring input that fails parse/bind/type construction does not create a
new semantic candidate and is reported through the admission/command-failure
side of the Semantic API.

For an existing structurally admissible semantic candidate, formula static/
graph/evaluation failures participate in ADR-0019 Stage 4/5 validation using the
ADR-0018 stable semantic facts such as stable field subjects, SCC membership,
direct failed-dependency sets, and evaluation-failure meaning.

Successful calculated values are operation/query facts, not diagnostics.

## Capability-addressability

Every semantic operation or operation family MUST be independently addressable
for authorization/capability purposes.

The following authorities are distinct:

- query/read;
- propose; and
- execute/mutation.

Granting one MUST NOT imply the others unless an explicit future authorization
policy says so.

This specification does not define capability identifier syntax, principal
identity, grant format, approval token, provenance record, or security protocol.
Those remain #27/#28.

## Compatibility and versioning

Semantic API versioning is independent from:

- `.ro` / `.roproj` representation versions;
- Rust crate/package SemVer;
- diagnostic provider implementation versions;
- transport protocol versions; and
- runtime/session revisions.

### Stable semantic contract

Only an explicitly specified and stability-classified semantic law, operation,
capability, result fact, or code meaning is a public compatibility promise.

Rust source visibility and serde derivation do not confer this status.

### Breaking semantic change

A change is breaking when a conforming client relying on Stable semantic meaning
must change its semantic assumptions. Examples include:

- changing an existing Stable command's intent or semantic side effects;
- making a Stable Query publish semantic mutation;
- changing stable-ID targeting semantics;
- changing Accepted single/batch atomicity;
- silently reinterpreting a published stable diagnostic code;
- removing or incompatibly changing a Stable operation/capability;
- adding a mandatory input to a Stable operation without a compatible version
  path; or
- changing Accepted gate/formula/validation semantics without the corresponding
  authority/version transition.

Correcting implementation that violated an already Accepted contract is a
conformance fix rather than automatic stabilization of undocumented buggy
behavior.

### Additive evolution

Potentially additive changes include:

- a new opt-in query/command/capability;
- new optional semantic projections/facts that older clients may ignore;
- new presentation-only fields;
- new transport adapters; and
- new diagnostic codes following the published unknown-code rules.

Adding a new blocking semantic rule is not necessarily additive merely because
an encoded report only gained a new code. If the semantic gate contract changes,
that change follows the semantic decision/version process.

### Diagnostic unknown-code rule

A published diagnostic code meaning MUST NOT be silently reused for an unrelated
rule.

A conforming client MUST be able to preserve/represent an unknown diagnostic
code as an opaque machine finding according to the relevant transport mapping.
It MUST NOT require an exhaustive known-code switch to derive operation gate
policy.

## Stability classification

| Concept | State |
| --- | --- |
| Headless Semantic API as mandatory first-party semantic boundary | Accepted |
| `workspace-engine` as current Rust implementation/application authority | Accepted under ADR-0016/ADR-0020 |
| Resident shared Rust runtime as preferred interactive topology | Accepted under ADR-0022 |
| Frontend projection/cache/authoring state is non-authoritative | Accepted under ADR-0022 |
| Host persistence/capabilities remain outside workspace-engine | Accepted under ADR-0016/ADR-0022 |
| Native/WASM equivalent Stable semantic observations where capabilities overlap | Accepted under ADR-0022 |
| Current workspace-engine Rust surface as external API | Internal / Provisional |
| Query does not publish semantic state | Accepted |
| Command is typed semantic intent rather than representation CRUD | Accepted |
| Stable semantic-ID targeting authority | Accepted under ADR-0015 |
| Propose is non-publishing and shares command semantics/gates with Execute | Accepted |
| Preview is proposal projection, not independent canonical state | Accepted |
| Finalization is operation-gate meaning, not mandatory stateful two-phase protocol | Accepted |
| Single-command semantic atomicity | Accepted |
| Ordered Atomic Command Batch all-or-nothing semantic publication | Accepted |
| Intrinsically admissible but higher-level-invalid intermediate batch working candidate | Accepted within ADR-0019 constraints |
| `ValidationReport` semantic observations as result meaning | Accepted |
| Exact Rust/wire `ValidationReport` shape | Provisional |
| Gate outcome distinct from diagnostic severity | Accepted |
| Formula Stage 4/5 facts remain ADR-0018/ADR-0019 authority | Accepted |
| Capability-addressability of operation/family | Accepted principle |
| Capability IDs/grants/approval/provenance | #27/#28 / Deferred |
| Semantic API version independent from storage/crate/transport versions | Accepted |
| Published diagnostic code meaning not silently reusable | Accepted |
| Complete externally Stable operation catalogue | Provisional, promote operation-by-operation |
| Exact semantic result tagged union / field spelling | Provisional |
| Exact effect/diff projection shape | Provisional |
| Revision/concurrency/precondition token | #93 / Provisional |
| Proposal identity/token | #28 and future patch/runtime protocol / Provisional |
| Intra-batch temporary-object handle syntax | Provisional |
| Public embedded Rust SDK / dedicated API crate | Deferred |
| Native/WASM/IPC/FFI/network serialization | ADR-0022-constrained future transport work / Deferred |

## Internal bypass policy

The following are implementation roles below or beside the Semantic API, not
alternate first-party client policy paths:

- `workspace-engine -> semantic-core/formula/diff/merge` under ADR-0016;
- storage codec/migration -> semantic model at the ADR-0017 representation
  boundary;
- host composition `load -> semantic operation -> save`;
- focused tests directly invoking an owner contract; and
- deterministic domain/extension validators through ADR-0019.

A first-party semantic client may not bypass the contract merely because it is
in the same process, language, or repository.

## ADR-0022 runtime/transport mapping rule

ADR-0022 fixes runtime ownership and host-separation laws: interactive
authoritative semantic state belongs to the shared Rust semantic/application
runtime; resident topology is preferred; frontends do not own a second semantic
authority; full snapshots are explicit boundaries; and native/WASM preserve
equivalent Stable semantic meaning where capabilities overlap.

Concrete resident session handles, revision/concurrency mechanics, Worker
lifecycle, projection delivery, IPC/FFI/network serialization/ABI, and
persistence/recovery remain Deferred to #93–#95 and future host/transport
implementation as applicable.

Every mapping MUST preserve the Semantic API Stable laws and outcomes. Runtime
or transport topology is not independent semantic authority.

## #104 reference pressure

Project Memory may use this contract as a later reference/dogfood application.
It remains a domain model/research hypothesis and does not add `Decision`, `ADR`,
`GitHubIssue`, `Commit`, provenance workflow, or Project Memory-specific queries
to semantic core by virtue of using the API.

## Explicitly not defined here

- JSON/Protobuf/MessagePack or any other wire encoding;
- JSON-RPC, HTTP, IPC, FFI, or WASM ABI;
- exact resident session/handle representation;
- revision/concurrency/conflict protocol;
- exact runtime commit/swap/locking/cloning mechanism;
- Worker lifecycle/loading/startup/memory behavior;
- proposal/approval token format;
- projection patch/delivery/invalidation protocol;
- native/browser persistence/recovery implementation;
- plugin ABI/runtime/sandbox;
- `.roproj` physical layout;
- generic CRUD/JSON Patch;
- generic transaction scripting language;
- event sourcing / operation log / undo history;
- complete Stable operation catalogue;
- stable public Rust SDK; or
- Project Memory/provenance domain semantics.

## Related authority

- [ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md)
- [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md)
- [Diagnostics contract](diagnostics-contract.md)
- [Validation engine](validation-engine.md)
- Issues #10, #17, #27, #28, #93, #94, #95, #104
