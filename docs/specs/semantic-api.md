# Semantic API Specification

Decision state: Mixed. The first-class boundary and semantic laws are Accepted
under [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md).
The immutable, revision-pinned SemanticPatch proposal contract and exact-change
binding law are Accepted under
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).
The authorization, stable-ID scope, trusted footprint, exact Human Approval,
and provenance laws that consume this operation/proposal meaning are Accepted
under [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md).
Runtime ownership, resident interactive topology, host separation, explicit
snapshot boundaries, and native/WASM semantic parity are Accepted under
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).
Exact Rust APIs, complete operation catalogue, wire schemas, transports,
proposal/revision encodings, session mechanics, and several result/projection
shapes remain Provisional or Deferred as marked below.

Implementation state: partially implemented through `workspace-engine` as the
shared first-party application authority. Current Rust functions and result
structures are implementation evidence, not the versioned public product
contract. Current operations remain substantially snapshot-style; the resident
runtime/session implementation is later work under #93–#95. No current Rust
type implements the general SemanticPatch envelope or AtomicBatch contract.

Decision issues: [#10](https://github.com/nurockplayer/tachiko-work/issues/10),
[#27](https://github.com/nurockplayer/tachiko-work/issues/27),
[#28](https://github.com/nurockplayer/tachiko-work/issues/28)

## Purpose

Define the smallest transport-neutral semantic command/query/result contract
that GUI, CLI, AI, automation, and future first-party clients must share,
including the representation-neutral proposal contract that binds review to one
exact semantic change and semantic base.

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
| Revision-pinned SemanticPatch     |
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
ADR-0015 says they are document-local. ADR-0024 requires every reviewable
proposal to bind one exact semantic context revision, while ADR-0022 accepts a
resident shared Rust runtime as the preferred interactive topology. The exact
session handle, revision/precondition representation, concurrency/conflict
policy, cancellation, and runtime state-installation mechanics remain Deferred
to #93 and related runtime work.

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

### SemanticPatch proposal envelope

The reviewable output/input contract around Propose is conceptually:

```text
SemanticPatch
- proposal occurrence identity
- Semantic API compatibility contract
- semantic base reference
- change: Command | AtomicBatch
```

These are logical contract elements, not frozen source or wire field names.

A SemanticPatch:

- belongs to the same Semantic API/application boundary as Query, Command,
  Propose, and Execute;
- contains exactly one typed Command or one ordered AtomicBatch;
- does not itself become a Command;
- introduces no patch-operation vocabulary or independent patch version;
- publishes no semantic state;
- grants no authorization or approval; and
- performs no `.roproj`, filesystem, Git, network, or other host write.

The exact API call sequence and result container used to issue a proposal are
Provisional. A conforming mapping MUST preserve the logical envelope and the
laws below whether it models proposal construction and evaluation as one call
or as admission followed by evaluation.

### Proposal occurrence identity and immutability

Every reviewable proposal MUST have an opaque proposal occurrence identity.
The same identity MUST NOT refer to different proposal contents. Once issued,
the complete proposal record is immutable.

Changing any of the following requires a new proposal identity:

- Semantic API compatibility contract;
- semantic base;
- single-command versus AtomicBatch body;
- command content or batch order;
- stable targets or typed values;
- bound formulas;
- generated semantic IDs; or
- an immutable annotation stored inside the proposal record.

Two proposal occurrences MAY have identical semantic contents and different
identities. Proposal identity is not semantic object identity under ADR-0015,
not proof of content integrity, and not a content-equivalence or idempotency
claim.

Proposal-ID spelling, generation, issuer, namespace, collision handling, and
transport encoding remain Provisional.

### Exact-change binding

For proposal `P`, exact semantic review binds this logical value:

```text
ExactChangeBinding(P) =
    Semantic API compatibility contract
  + semantic base reference
  + body kind
  + complete typed command semantics
  + command order for AtomicBatch
```

Complete typed command semantics include every semantic input that can affect
candidate construction, including stable targets, typed operands,
command-owned semantic preconditions, bound formulas, and generated semantic
IDs.

Generated IDs required by the change MUST be fixed before proposal identity is
issued. A formula update MUST bind its accepted typed formula meaning and stable
references before the exact reviewable change is fixed. A later execution
cannot generate different IDs or rebind formula source while claiming to
execute the same proposal.

`ExactChangeBinding` is representation-neutral. It does not depend on Rust
layout, Serde shape, transport bytes, JSON formatting, UI coordinates,
provider metadata, rendered diff prose, storage paths, `.roproj` bytes, or Git
objects.

This specification selects no canonical proposal bytes, hash, digest,
signature, or MAC. ADR-0026 requires trusted structural verification of this
complete binding for exact Approval while deliberately deferring any canonical
bytes, digest, signature, MAC, or portable token. Proposal identity by itself
MUST NOT be treated as cryptographic proof of the expected binding.

### Semantic API compatibility binding

Every durable or transported proposal MUST carry or unambiguously derive the
Semantic API compatibility contract used to interpret its body.

A consumer that does not support that contract MUST reject before semantic
candidate construction. It MUST NOT reinterpret unknown command semantics or
fall back to representation CRUD.

SemanticPatch introduces no independent patch-operation version axis. An
explicit translation to another Semantic API compatibility contract forms a
new proposal and receives a new proposal identity, even when an adapter judges
the result equivalent.

The compatibility identifier and negotiation/encoding mechanism remain
Provisional under ADR-0020. Representation, transport, crate/package,
diagnostic-provider, and runtime revision versions remain separate axes.

### Semantic base and stale behavior

Every proposal binds one exact semantic base reference. The reference MUST be
sufficient under the owning context/runtime contract to distinguish the
semantic context and revision against which Propose was evaluated.

Base equality is a semantic optimistic-concurrency precondition. It means exact
semantic revision identity, not equality of semantic content. A proposal
matches only the same semantic revision occurrence against which it was formed.
Any intervening canonical semantic publication makes the proposal stale,
including an unrelated semantic change. Later canonical state that is
semantically equivalent to the original base does not restore the original
revision identity or make the old proposal current again.

Base equality is not defined by `.roproj` bytes, paths, timestamps, UI state,
provider metadata, or Git objects.

Before an existing proposal is re-evaluated, authorized, or executed against a
current semantic context, the trusted application/runtime boundary MUST compare
that context with the proposal base. A mismatch is `Stale` and MUST:

- fail before constructing or publishing a candidate against the changed base;
- publish no semantic state;
- perform no implicit rebase, merge, retarget, or best-effort replay; and
- leave the immutable proposal unchanged.

Re-proposing against a newer base re-runs command construction/binding and
authoritative Propose evaluation and receives a new proposal identity. Exact
revision-token types, equality mechanics, session scope, persistence,
concurrency algorithms, and stale-result DTOs remain #93/#29 work.

### Preconditions

SemanticPatch defines no generic `preconditions[]` language.

The semantic base is the envelope-level concurrency precondition. Any
additional semantic precondition belongs to the typed Command whose meaning
requires it and is included in `ExactChangeBinding`.

Authorization, approval, expiry, replay/revocation policy, durable-write
availability, and external-effect permission remain separate enforcement
conditions under ADR-0026 and the relevant host authority. JSON Pointer
predicates, storage checks, UI-coordinate tests, provider claims, and arbitrary
scripts do not become semantic preconditions.

### Proposal evidence

Propose may return a candidate, semantic diff, validation report, gate outcome,
calculated impact, or other operation-specific review evidence. These are
derived observations over the bound base and exact typed change.

Derived evidence does not replace `ExactChangeBinding`, grant authorization,
or become a mutation program. Rendered diff prose is presentation. A semantic
diff explains base-to-candidate meaning. Validation success does not grant
permission. A malformed request may fail before a reviewable proposal exists;
an invalid or gate-rejected candidate publishes nothing.

The trusted semantic/application boundary derives ADR-0026
`AuthorizationFootprint` from typed operation meaning and relevant
base/candidate relationships. Its disclosure scope includes every subject
revealed by preview, diff, dependencies, impact, and diagnostics. Propose
authority does not grant arbitrary Query authority: evidence outside live Query
scope MUST be denied or safely reduced. The client cannot authoritatively
declare its own footprint.

Once proposal identity is issued, validation, review, rejection, or stale
outcomes do not mutate the proposal record. A later execution must perform the
authoritative base, authorization, and gate checks required for the state it
actually acts on rather than trust stale client-rendered evidence.

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

ADR-0024 fixes proposal occurrence immutability, exact-change binding, Semantic
API contract binding, semantic-base pinning, and fail-closed stale meaning.
ADR-0026 fixes structural exact Approval, live authorization, and
consume-with-successful-publication laws without selecting a public token or
DTO. Proposal-ID/revision encoding, Approval lifecycle DTOs, and concrete
session/commit mechanics remain Provisional or Deferred to #29/#93.

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
- durable proposal-store or approval lifecycle mechanics; or
- intra-batch temporary-object handle syntax.

ADR-0022 likewise does not turn semantic atomicity into a specific runtime
commit/swap/locking/cloning algorithm.

## Result contract

The Semantic API result must preserve operation-specific semantic meaning.
There is no requirement that all operations return one universal response bag.

A conforming client must be able to distinguish, where applicable:

1. completed semantic operation results;
2. failure before a new admissible semantic candidate exists;
3. unsupported Semantic API compatibility or proposal identity/content
   mismatch;
4. stale semantic base;
5. semantic precondition/inapplicability failure;
6. rejection by the authoritative operation gate, including relevant validation
   and gate facts; and
7. typed operation-specific outcomes such as merge conflict/reconciliation
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

### Proposal contract failure

A durable/transported proposal whose Semantic API compatibility contract is not
supported fails before candidate construction. Reuse of one proposal identity
with different contents is rejected rather than treated as a replacement.

Exact error codes, integrity verification, digest, and transport behavior remain
Provisional or #28 work.

### Stale base

An otherwise supported immutable proposal whose semantic base does not equal
the current context revision is stale. It fails before candidate construction
against the changed base and publishes nothing. Stale is distinct from semantic
command inapplicability and gate rejection.

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

The following minimum authority dimensions are distinct:

- Query;
- Propose by mutation class;
- Execute by mutation class; and
- Approve by mutation class.

Granting one MUST NOT imply another. Value, Formula, Structure, Schema, and
Destructive mutation authority likewise do not imply one another.

ADR-0026 and [`semantic-authorization.md`](semantic-authorization.md) define
the representation-neutral Principal, Grant, stable-ID semantic scope,
`AuthorizationFootprint`, exact Human Approval, expiry/replay/revocation,
minimum provenance, and external-effect separation laws that consume these
operations. Exact capability strings, DTOs, storage, result codes, canonical
bytes, digest/token profiles, and wire security mechanisms remain
Provisional/Deferred.

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

## SemanticPatch conformance scenarios

Future implementation and transport mappings MUST preserve these logical
fixtures without requiring common bytes or Rust types:

1. one stable-ID-targeted typed field update remains inert under Propose and
   changes identity if its target or value changes;
2. one ordered multi-entity AtomicBatch forms one candidate and publishes no
   prefix, while reordering creates a different proposal;
3. a formula update binds its complete typed bound AST and stable references,
   not only source spelling or rendered addresses;
4. a proposal created against revision `R` fails stale against any current base
   other than `R`, before candidate construction and without implicit rebase;
5. an invalid typed command produces no published semantic state; and
6. two formula updates that are individually valid against the base but create
   a cycle together fail the final batch gate with no partial publication.

Conformance also covers unsupported Semantic API compatibility, reuse of one
proposal identity with different content, generated-ID binding, and equivalent
Stable native/WASM outcomes where the same capability is exposed. Concrete
fixtures and implementation belong to #29/#93.

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
| SemanticPatch as immutable envelope around `Propose(Command | AtomicBatch)` | Accepted under ADR-0024 |
| Opaque proposal occurrence identity and complete-record immutability | Accepted under ADR-0024 |
| Representation-neutral `ExactChangeBinding` law | Accepted under ADR-0024 |
| Semantic API compatibility binding with no independent patch-operation version | Accepted under ADR-0024 |
| Exact semantic-base pinning and fail-closed stale behavior | Accepted under ADR-0024 |
| Proposal-ID, revision-token, and transport encoding | Provisional / #93 |
| Hash/digest/signature/MAC/canonical proposal bytes | Deferred under ADR-0026 |
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
| Capability/scope/Grant/Approval/provenance meaning | Accepted under ADR-0026 |
| Exact authorization identifiers/DTOs/storage/wire representation | Provisional / Deferred |
| Semantic API version independent from storage/crate/transport versions | Accepted |
| Published diagnostic code meaning not silently reusable | Accepted |
| Complete externally Stable operation catalogue | Provisional, promote operation-by-operation |
| Exact semantic result tagged union / field spelling | Provisional |
| Exact effect/diff projection shape | Provisional |
| Revision/concurrency/precondition token | #93 / Provisional |
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
or transport topology is not independent semantic authority. A mapping of
SemanticPatch also preserves ADR-0024 occurrence immutability, exact-change and
compatibility binding, base equality, and stale failure without making its
transport bytes the semantic proposal.

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
- proposal-ID/revision-token field encoding;
- canonical proposal bytes, hash, digest, signature, or MAC;
- exact Approval/capability/Grant/provenance/expiry/replay/revocation DTO or
  wire format;
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
- [ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md)
- [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md)
- [Semantic authorization](semantic-authorization.md)
- [Diagnostics contract](diagnostics-contract.md)
- [Validation engine](validation-engine.md)
- Issues #10, #17, #27, #28, #29, #93, #94, #95, #104
