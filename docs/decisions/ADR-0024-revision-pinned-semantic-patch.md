# ADR-0024: Revision-pinned SemanticPatch proposal envelope

## Status

Accepted

Decision issue: [#27](https://github.com/nurockplayer/tachiko-work/issues/27)

Specified by: [`semantic-api.md`](../specs/semantic-api.md)

Related authority: ADR-0007, ADR-0015 through ADR-0023

## Context

ADR-0020 establishes one transport-neutral Semantic API for first-party
clients. It already defines typed semantic `Command`, `Propose`, `Execute`, and
ordered all-or-nothing `AtomicBatch` meaning. ADR-0007 establishes that AI has
no intrinsic authority and that current MVP AI-originated canonical mutation
requires explicit approval. ADR-0022 places authoritative interactive state in
the shared Rust runtime while deliberately deferring concrete revision/session
mechanics to later implementation work.

Issue #27 asks for the smallest durable proposal unit that can bind review to
one exact semantic change and one semantic base without creating a raw-file
mutation path or a second operation vocabulary. Its earlier illustrative
`operations[]`, `preconditions[]`, intent, and provenance shape predates
ADR-0020 and is not authority over that Accepted command model.

Current implementation provides useful but incomplete evidence. The
provider-free AI adapter can return an inert validated one-field `Suggestion`,
and workspace-engine can form immutable one-operation candidate documents and
semantic diffs. Neither surface has proposal occurrence identity, Semantic API
contract binding, revision pinning, general Command coverage, or AtomicBatch
support. Those Rust types are not public protocol DTOs.

The durable decision must therefore bind proposal identity and review to exact
semantic meaning while freezing less than a wire, digest, authorization, or
runtime protocol.

## Decision

### 1. SemanticPatch is an envelope around existing Propose meaning

Conceptually:

```text
SemanticPatch
- proposal occurrence identity
- Semantic API compatibility contract
- semantic base reference
- change: Command | AtomicBatch
```

`SemanticPatch` is the immutable, revision-bound proposal envelope around
ADR-0020 `Propose(Command | AtomicBatch)`.

It belongs to the existing Semantic API/application boundary. It does not add a
semantic mutation primitive, operation catalogue, scripting language, or
parallel AI-only policy path.

A proposal contains exactly one typed `Command` or one ordered `AtomicBatch`.
It does not itself become a `Command`. Any later authorized execution consumes
the same command meaning through ADR-0020; it does not “apply a patch” through
another mutation vocabulary.

The conceptual labels above do not freeze Rust structs, field names, enum
shapes, Serde attributes, JSON, IPC, FFI, WASM ABI, network DTOs, or a public
SDK.

### 2. Every reviewable proposal has opaque occurrence identity

Every reviewable proposal MUST have an opaque proposal occurrence identity.

The same proposal identity MUST NOT refer to different proposal contents. Once
that identity is issued, the complete proposal record is immutable.

Changing any identity-defining content requires a new proposal identity,
including:

- semantic base;
- Semantic API compatibility contract;
- single-command versus batch body;
- command contents;
- command order;
- stable targets;
- typed values;
- bound formulas;
- generated semantic IDs; or
- any immutable annotation stored inside the proposal.

Two independently issued proposals MAY have identical exact-change semantics
and different occurrence identities. Proposal identity is therefore not
semantic equivalence, a content digest, or an idempotency guarantee by itself.

Proposal identity is separate from ADR-0015 semantic object identity. It does
not identify a Document, Schema, Field, Entity, or future semantic object and
does not grant stable semantic continuity to an authoring fragment.

The proposal-ID encoding, generator, issuer, namespace, collision handling,
and retry/idempotency mechanism remain Provisional.

### 3. Exact-change binding is representation-neutral

For a proposal `P`, the exact semantic change is bound by this law:

```text
ExactChangeBinding(P) =
    Semantic API compatibility contract
  + semantic base reference
  + body kind
  + complete typed command semantics
  + command order for AtomicBatch
```

Complete typed command semantics include every semantic input that can affect
candidate construction. This includes stable targets, typed operands, bound
formulas, generated semantic IDs, and any command-owned semantic precondition.

Generated semantic IDs needed by the change MUST be resolved and included
before proposal identity is issued. Execution cannot allocate a different ID
while claiming to execute the same proposal. Formula authoring that requires
parse/bind/type construction likewise reaches its accepted bound semantic form
before the reviewable exact change is fixed.

`ExactChangeBinding` does not depend on:

- Rust memory or field layout;
- Serde or transport shape;
- transport bytes or JSON formatting;
- UI coordinates or provider metadata;
- rendered diff prose or other presentation;
- storage paths, shard placement, or `.roproj` bytes; or
- a Git object identifier.

This ADR defines a logical binding law, not a byte canonicalization protocol.
It selects no hash algorithm, canonical proposal-byte format, signature, MAC,
or digest protocol. Those integrity and approval-binding mechanisms remain
owned by #28.

Proposal occurrence identity alone MUST NOT be treated as cryptographic proof
that a received record has the expected exact-change binding.

### 4. SemanticPatch uses the Semantic API compatibility axis

`SemanticPatch` is part of the Semantic API contract and MUST NOT introduce an
independent patch-operation version axis.

Every durable or transported proposal MUST carry or unambiguously derive the
Semantic API compatibility contract under which its body is interpreted. A
consumer that does not support that contract MUST reject the proposal before
semantic candidate construction rather than reinterpret unknown command
meaning.

Changing the bound Semantic API contract creates a new proposal occurrence,
even when an adapter believes the change is equivalent. Translation between
Semantic API versions is explicit proposal reconstruction and produces a new
proposal identity.

The exact compatibility identifier, negotiation mechanism, support window,
and transport encoding remain Provisional under ADR-0020's compatibility laws.
Representation format, transport, crate/package, diagnostic-provider, and
runtime revision versions remain separate axes.

### 5. The base identifies one exact semantic context revision

Every proposal is bound to one exact semantic base. The base reference MUST be
sufficient in its owning runtime/context to distinguish the semantic context
and revision against which the change was proposed.

Base equality is a semantic concurrency precondition. It means exact semantic
revision identity, not equality of semantic content. A proposal matches only
the same semantic revision occurrence against which it was formed. Any
intervening canonical semantic publication makes the proposal stale, including
an unrelated semantic change. If later canonical state becomes semantically
equivalent to the original base, that does not restore the original revision
identity or make the old proposal current again.

Base equality is not defined by `.roproj` bytes, a storage path, file
modification time, UI state, provider metadata, or a Git commit, even when a
host records such values as related provenance.

The base reference is representation-neutral at this contract layer. The
concrete revision token type, generator, monotonicity mechanism, session scope,
cross-process portability, persistence, and concurrency algorithm remain
Provisional and are owned by #93 and runtime/host work.

### 6. A base mismatch is stale and fails closed

Before re-evaluating, authorizing, or executing an existing proposal against a
current semantic context, the trusted application/runtime boundary MUST compare
that context with the proposal's base using the owning revision contract.

If they do not match, the proposal is `Stale`. Stale handling MUST:

- fail before constructing or publishing a new candidate against the changed
  base;
- publish no semantic state;
- perform no implicit rebase, merge, retargeting, or best-effort replay; and
- leave the immutable proposal record unchanged.

A client may explicitly form a new proposal against the newer base. That work
re-runs command construction/binding and authoritative Propose evaluation and
receives a new proposal identity. Semantic diff or merge may help a user
understand the change, but neither silently changes the original proposal's
base or body.

The exact stale result DTO, retry UX, revision lookup, and lifecycle state
machine remain #29/#93 implementation work.

### 7. Preconditions stay typed and operation-owned

`SemanticPatch` does not add a generic `preconditions[]` expression language.

The semantic base is the proposal-level optimistic-concurrency precondition.
Any additional semantic precondition belongs to the typed `Command` whose
meaning requires it and is part of `ExactChangeBinding`.

Authorization, approval, expiry, revocation, replay policy, durable-write
availability, and external-effect permission are enforcement conditions, not
patch-operation preconditions and not command semantics.

This prevents JSON Pointer predicates, storage checks, UI-coordinate tests,
provider claims, or arbitrary scripts from becoming a second semantic
precondition vocabulary.

### 8. AtomicBatch order and publication laws are preserved

For an `AtomicBatch` body, command order is identity-defining and exact-change
binding. Reordering commands creates a different proposal.

ADR-0020 remains atomicity authority:

- commands are evaluated in order against one proposal base to form one final
  candidate transition;
- no failed prefix becomes canonical state; and
- final semantic publication is all-or-nothing.

The envelope does not define nested batches, transactions, temporary-object
handle syntax, partial application, rollback sessions, or an operation log.

### 9. Proposal evidence is derived and non-authoritative

Authoritative Propose evaluation may produce a semantic candidate, semantic
diff, validation report, gate outcome, calculated impact, or other
machine-readable review evidence according to the relevant operation.

That evidence is derived from the bound base, exact command semantics, and
Accepted semantic engines. It does not replace `ExactChangeBinding` and does
not become mutation authority.

In particular:

- rendered diff prose is presentation, not approval identity;
- a semantic diff is evidence about base-to-candidate meaning, not a mutation
  program;
- validation success does not grant authorization;
- an invalid or gate-rejected semantic candidate publishes nothing; and
- later approval or execution MUST NOT trust stale client-rendered evidence in
  place of authoritative base and gate checks.

A malformed request may fail before a reviewable proposal exists. Once a
proposal identity has been issued, later validation, review, rejection, or
stale outcomes do not mutate that proposal record. Exact result/container and
proposal-issuance sequencing remain Provisional.

### 10. Proposal, authorization, and effects remain separate

Creating, receiving, inspecting, or validating a `SemanticPatch`:

- does not publish canonical semantic state;
- does not grant execute authority or approval;
- does not write `.roproj` or another durable representation;
- does not grant filesystem, network, process, Git, plugin, deployment, or
  other host capability; and
- does not create an operation log or event-sourcing requirement.

ADR-0007 remains AI authority. #28 owns principals, capabilities, grants,
approval, minimum provenance, expiry/replay/revocation, and the structural or
cryptographic mechanism that binds approval to the exact proposal. #29 owns
the preview/validate/approve/apply/verify lifecycle implementation. #93 owns
the resident revision/session implementation needed to realize stale checks.

Human intent, explanations, provider/model metadata, and provenance are
advisory or authorization/audit context rather than command meaning unless an
Accepted typed Command explicitly makes an input semantic. If an implementation
stores an annotation inside the immutable proposal record, changing it requires
a new proposal identity. Mutable discussion/review state belongs outside the
proposal record.

### 11. Storage, Git, history, and runtime remain neighboring boundaries

`SemanticPatch` is not persisted canonical semantic state and is not a
`.roproj` mutation format. A host may durably retain proposal records for review
or audit under a future profile, but that persistence does not make them part
of the semantic Document or authorize materialization.

After an authorized successful Execute, storage/host code may materialize the
resulting semantic snapshot under its separate authority. A proposal does not
contain or approve `.roproj` byte edits.

A Git commit, branch, tree, or blob may be associated with a proposal as
workflow provenance. It is not the SemanticPatch base or exact-change binding
unless a future explicit adapter contract defines a lossless mapping while
preserving this ADR's semantic laws.

Operation history, event sourcing, undo, collaboration, and persisted proposal
repositories remain separately owned. This ADR creates no requirement to use
any of them.

## Required conformance scenarios

Future #29 implementation and transport mappings must preserve at least these
representation-neutral scenarios. This ADR defines the expected semantic
observations, not concrete fixture bytes or Rust APIs.

1. **One-field update** — one typed field-update Command names stable
   `EntityId + FieldId`, carries the typed value, is bound to one base, produces
   review evidence without publication, and cannot retain its proposal ID if
   the value or target changes.
2. **Multi-entity update** — one ordered AtomicBatch changes fields on multiple
   stable entities, produces one final candidate/diff, and publishes no prefix.
   Reversing command order creates a different proposal.
3. **Formula update** — the proposal binds the complete typed bound formula and
   its stable references. Source spelling or rendered human addresses cannot
   substitute for that bound meaning.
4. **Stale base** — a proposal created against revision `R` is rejected as
   stale when the current semantic context is not `R`, before candidate
   construction and without mutation or implicit rebase.
5. **Invalid command** — a missing stable target, wrong typed operand, failed
   bound-formula admission, or equivalent typed command failure produces no
   published semantic state.
6. **Individually valid, invalid batch** — two formula changes that are each
   valid against the base but form a cycle together are rejected by the final
   authoritative batch gate, with no partial publication.
7. **Proposal contents change after review** — changing any identity-defining
   proposal content cannot retain the reviewed proposal identity. The changed
   proposal must be issued as a new proposal occurrence before review or later
   processing can refer to it.
8. **Canonical semantic state changes after proposal creation** — any
   intervening canonical semantic publication makes the old proposal stale,
   including an unrelated change and a later return to semantically equivalent
   content. Cached review evidence for the old proposal is historical only.

Additional conformance must cover unsupported Semantic API compatibility,
generated-ID binding, and native/WASM equivalent Stable outcomes where the same
capability is exposed.

## Deliberately Provisional or Deferred

This ADR does not freeze:

- Rust, Serde, JSON, IPC, FFI, WASM ABI, network, or SDK types;
- exact conceptual field spelling or tagged-union layout;
- proposal-ID encoding, generator, issuer, namespace, or idempotency protocol;
- Semantic API compatibility identifier/negotiation encoding;
- semantic revision token representation or runtime concurrency algorithm;
- hash, digest, signature, MAC, canonical proposal bytes, or approval token;
- capability identifiers, principals, grants, approval, provenance, expiry,
  replay, or revocation (#28);
- lifecycle state names, preview DTOs, apply/verify results, or execution
  orchestration (#29);
- session handles, resident revision implementation, persistence, or recovery
  (#93 and host work);
- intra-batch temporary-object handles;
- durable proposal-store or operation-history format;
- Git integration; or
- a new crate or public API surface.

## Rejected alternatives

### Define a new patch operation vocabulary

Rejected. ADR-0020 already owns typed Command and AtomicBatch meaning. A second
vocabulary would duplicate semantic policy and capability classification.

### Use JSON Patch, JSON Pointer, storage paths, or array indexes

Rejected. Those address representation layout rather than stable semantic
identity and would allow storage mechanics to become the mutation contract.

### Make SemanticPatch itself a Command

Rejected. It would create recursive or parallel mutation semantics and blur the
difference between proposed occurrence identity and semantic intent.

### Use rendered semantic diff as the exact change

Rejected. Diff is derived review evidence. Presentation and projection may
evolve without changing command meaning.

### Use `.roproj` bytes or Git objects as the universal base

Rejected. Representation and workflow identities are neighboring concerns and
cannot replace the semantic runtime/context revision contract.

### Make proposal ID a selected content hash now

Rejected. Occurrence identity is required, but digest canonicalization,
integrity, and approval binding belong to #28 and must not freeze a wire format
through this ADR.

### Permit mutation under the same proposal ID

Rejected. Review and approval cannot bind to an exact change if the base, body,
order, targets, values, generated IDs, formulas, or immutable annotations may
change in place.

## Consequences

Positive:

- AI, GUI, CLI, plugins, and automation can refer to one provider-neutral
  reviewable proposal occurrence;
- stale semantic bases fail closed without making Git or storage bytes the
  concurrency protocol;
- exact review/approval can bind to typed semantic meaning independently of
  transport representation;
- SemanticPatch reuses the Accepted Command/AtomicBatch vocabulary and
  capability boundary; and
- #28/#29/#93 receive explicit seams without their mechanisms being
  pre-implemented here.

Costs:

- proposal construction must resolve every identity-defining semantic input,
  including generated IDs and bound formulas, before issuing identity;
- rebasing or changing a proposal always creates a new occurrence identity;
- durable/transport mappings must preserve unsupported-contract and
  ID/content-mismatch rejection; and
- the current one-field AI `Suggestion` and snapshot-style workspace-engine
  surface do not yet implement this contract.

## Required follow-up

- Reconcile `semantic-api.md`, `ai-agent-api.md`, `semantic-diff-spec.md`,
  crate/AI architecture, ADR/spec indexes, and the canonical reconciliation
  register.
- #28 defines capability, approval, minimum provenance, integrity/digest, and
  stale/replay authorization mechanics against `ExactChangeBinding`.
- #29 implements preview, validation, approval, atomic execution, and
  verification without inventing another mutation model.
- #93 supplies the concrete resident semantic revision/session mechanism while
  preserving this representation-neutral base law.
- Production code, public wire DTOs, and transport/SDK stabilization remain
  separate implementation/decision work.

## Related

- Product Constitution §§2.2, 2.5 through 2.7, 6, 7
- Design Principles §§3, 7 through 10, 12
- ADR-0007
- ADR-0015
- ADR-0018
- ADR-0019
- ADR-0020
- ADR-0021
- ADR-0022
- ADR-0023
- Issues #27, #28, #29, #93
