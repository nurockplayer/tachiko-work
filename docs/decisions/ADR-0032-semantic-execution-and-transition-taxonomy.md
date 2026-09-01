# ADR-0032: Semantic execution and retained-transition taxonomy

## Status

Accepted

Decision issue: [#48](https://github.com/nurockplayer/tachiko-work/issues/48)

Specified by: [Semantic API Specification](../specs/semantic-api.md)

Related authority:
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0024](ADR-0024-revision-pinned-semantic-patch.md),
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md),
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md),
[ADR-0030](ADR-0030-canonical-semantic-delta.md), and
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md)

## Context

The existing Accepted contracts already separate typed semantic intent,
revision-pinned proposals, authorization and receipts, authoritative current
state, direct-state delta evidence, and merge-conflict evidence. Later history
and collaboration work still needs one unambiguous vocabulary connecting those
contracts without turning a request, diff, receipt, or retained log into a
second mutation or state authority.

In particular, an Execute request can fail, be denied, become stale, produce no
semantic change, publish a change, or publish a change before later reporting
fails. Treating all of those cases as an "operation," "transaction," or
"event" would obscure whether a revision occurrence actually exists.

This ADR fixes only the logical taxonomy. It does not select public DTOs,
encodings, storage, replay, checkpoint, causality, transaction, or Git-mapping
mechanics.

## Decision

### 1. Command and AtomicBatch remain the normative intent vocabulary

ADR-0020 `Command | AtomicBatch` remains the complete normative semantic intent
and ordered all-or-nothing publication vocabulary. ADR-0024 `SemanticPatch`
remains an immutable proposal occurrence around exactly that intent.

At this layer, **operation** is only an umbrella or conversational word for
Semantic API activity. It does not name a public `Operation` value, DTO, base
class, mutation body, or additional apply language. ADR-0026
`operation-family` remains the capability-addressing dimension for recognized
Query and Command families; it does not imply a general `Operation` object.

**Transaction** is likewise non-normative here. `AtomicBatch` is the only
Accepted semantic all-or-nothing publication unit. Multi-document, host,
external-effect, durability, rollback, and recovery transactions remain with
[Issue #11](https://github.com/nurockplayer/tachiko-work/issues/11).

### 2. An Execute attempt may publish zero or one semantic revision

One admitted Execute attempt evaluates one `Command` or ordered `AtomicBatch`
against one exact current semantic base and may install at most one final
semantic state. Batch members never create intermediate revision occurrences.

Denial, unsupported contract, stale base, admission or construction failure,
semantic-precondition failure, conflict, authoritative gate rejection, or
finalization failure before installation publishes no semantic state and
creates no semantic revision occurrence.

If authoritative evaluation reaches a candidate semantically equal to its
exact current base, the semantic result is **`NoChange`**:

- no new semantic state is installed;
- no semantic revision occurrence is claimed;
- no retained semantic transition/event is created; and
- ADR-0026 no-publication and Approval-consumption laws remain unchanged.

An implementation may update private attempt, cache, metric, or bookkeeping
state while reporting `NoChange`. Such a private token is not semantic state or
a semantic revision occurrence.

An actual non-no-op installation creates exactly one semantic revision
occurrence. A post-install verification, receipt persistence, response, or
reporting failure does not erase that occurrence and must not be represented as
pre-publication failure or `NoChange`. Where ADR-0026 Approval applies,
publication and Approval consumption have already occurred atomically.

### 3. RevisionOccurrenceRef is opaque and context-scoped

A logical **`RevisionOccurrenceRef`** identifies one actual semantic state
occurrence for one continuing `DocumentId` inside one owning revision
context/domain. Equality is defined only when that owning context/domain and
`DocumentId` are the same. The reference MUST be sufficient there to
distinguish occurrences, including content-equivalent occurrences.

A `RevisionOccurrenceRef`:

- implies neither global uniqueness nor ordering;
- is not semantic snapshot content or a content hash;
- is not a Semantic Delta, SemanticPatch, retained transition, checkpoint,
  receipt, timestamp, storage path, or Git identity; and
- does not acquire parent, DAG, or causal-clock meaning merely by existing.

A concrete mapping may carry the owning context/domain and `DocumentId` inside
the reference or establish them unambiguously outside it. This ADR fixes the
logical equality boundary, not that representation. Current internal
`resident/N`-style values may implement private runtime equality but MUST NOT
be exposed or persisted as globally meaningful identity by inference.

Parent/history structure belongs to
[Issue #49](https://github.com/nurockplayer/tachiko-work/issues/49). Offline
causal metadata, DAG/clock mechanics, and selective CRDT/OT belong to
[Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50).

### 4. Retry and idempotency are attempt-level concerns

A retry is another request to obtain or reconcile an Execute outcome. It does
not reuse or create semantic occurrence identity merely because its payload is
the same.

ADR-0024 proposal occurrence identity is not an idempotency key. Every retry
that reaches authoritative Execute rechecks the exact current base,
authorization, Approval where applicable, and semantic gates. After one
revision-pinned SemanticPatch publishes, its old base is stale and the proposal
cannot silently publish again. Re-proposal against a new base creates a new
proposal occurrence.

Repeating a direct Command is a new Execute attempt. If it now evaluates to the
current semantic state, its result is `NoChange`, not another occurrence of the
prior publication. Adapter or transport deduplication may later map repeated
delivery to one previously established attempt outcome, but its key and policy
do not become proposal, revision, retained-transition, or semantic-content
identity. When a caller cannot determine whether installation occurred, it
must reconcile authoritative state rather than assume that blind retry is
semantically idempotent.

### 5. A retained semantic transition/event is optional publication evidence

There is one optional retained concept: a **retained semantic transition**.
When the phrase **semantic event** is used, it means this same concept, not a
second event type.

A retained semantic transition is immutable evidence that one actual non-no-op
semantic publication occurred. It relates:

- the continuing `DocumentId`;
- the exact before and after `RevisionOccurrenceRef` values; and
- the canonical ADR-0030 direct A-to-B Semantic Delta evidence under its
  supported logical contract.

It is not a Command, AtomicBatch, SemanticPatch, Execute input, mutation
program, authoritative snapshot, checkpoint, or replay requirement. It does
not assert that the retained intent caused the transition merely because an
adapter stores both nearby.

Pre-publication failure and `NoChange` create no retained semantic transition.
They may produce separately authorized attempt, denial, security, or audit
evidence. General retained-transition history remains optional under ADR-0029;
current semantic state and complete snapshots remain authoritative without it.

### 6. Security/provenance receipts and retained transitions are distinct

An ADR-0026 security/provenance receipt proves the authorization, Approval,
principal, policy, gate, and publication facts required by that security
contract. A retained semantic transition proves the semantic before-to-after
publication relationship defined above.

Neither guarantee implies the other. Required security/provenance evidence must
survive when general semantic history is disabled. Conversely, retaining a
semantic transition does not prove authorization or Approval. A future history
profile may place both in one storage envelope only if each contract remains
independently identifiable and valid.

Durable denial/audit evidence is not a semantic event. Optional general-history
retention must not become mandatory merely because security evidence is
required.

### 7. Contract versions and identity namespaces remain separate

Any future public retained-transition contract MUST declare its own logical
contract/version, fail closed when unsupported, and use an opaque retained-
transition occurrence identity. This ADR does not mint that version identifier,
choose its encoding, or define a production DTO.

The following identities and version axes remain distinct and MUST NOT be
silently substituted for one another:

- semantic object identity and snapshot/content identity;
- revision-context/domain and `RevisionOccurrenceRef` identity;
- retained semantic-transition identity and contract version;
- SemanticPatch proposal identity and Semantic API compatibility version;
- security/provenance receipt and authorization-policy identity/version;
- checkpoint identity and history-profile version;
- representation, storage, runtime/session, and transport versions; and
- Git commit, tree, blob, ref, repository, and host identity.

No retained-transition identity may be derived by requirement from a Git SHA,
storage path, timestamp, provider/model identity, human key, Semantic Delta
content, or snapshot content.

### 8. Mechanics remain separately owned

This decision authorizes no production implementation.

- [Issue #49](https://github.com/nurockplayer/tachiko-work/issues/49) owns
  retained-history profiles, durable storage, checkpointing, replay and
  verification, compaction, retention/redaction, crash recovery, and optional
  Git association.
- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50) owns
  offline parent/causal metadata, DAG/clock mechanics, resynchronization, and
  selective CRDT/OT.
- [Issue #11](https://github.com/nurockplayer/tachiko-work/issues/11) owns
  broader team policy, multi-document or host transaction semantics,
  external-effect recovery, and stronger administration/review rules.

Public Rust types, wire DTOs, persisted history/event storage, replay,
checkpoints, event sourcing, transaction infrastructure, CRDT/OT, and the exact
receipt/transition storage envelope remain outside this ADR.

## Consequences

- A request, attempt, proposal, publication, revision occurrence, delta,
  receipt, and retained transition can no longer be conflated.
- `NoChange` has an explicit non-publication meaning without forcing private
  runtime bookkeeping to stand still.
- Later history work has a minimum occurrence reference and transition meaning
  without inheriting current internal revision tokens or creating global order.
- Security evidence can remain mandatory where required while general semantic
  history remains optional.
- A post-install failure must preserve the truth that publication occurred,
  even when later evidence or reporting is incomplete.

## Rejected alternatives

- **A public `Operation` DTO parallel to Command:** rejected because it would
  duplicate the Accepted mutation vocabulary.
- **Treat AtomicBatch as a general transaction protocol:** rejected because
  semantic atomic publication does not define host, durability, rollback, or
  distributed transaction semantics.
- **Create a revision for every admitted attempt or no-op:** rejected because
  attempts and private bookkeeping are not semantic state occurrences.
- **Use proposal identity as retry idempotency:** rejected because proposal
  occurrence identity binds immutable review, not repeated-delivery semantics.
- **Treat Command or Semantic Delta as the retained event:** rejected because
  intent and direct-state evidence have different meanings and neither proves
  that publication occurred.
- **Make receipts and semantic events interchangeable:** rejected because
  security/provenance and semantic-transition guarantees remain independently
  required or optional.
- **Adopt an authoritative event stream:** rejected by ADR-0029's complete
  current-state snapshot boundary.
- **Use Git, content, timestamp, path, or provider identity for revision/event
  identity:** rejected because those namespaces have different equality and
  lifecycle rules.

## Related

- [Issue #48](https://github.com/nurockplayer/tachiko-work/issues/48)
- [Semantic API Specification](../specs/semantic-api.md)
- [Semantic operation log model](../specs/operation-log-model.md)
- [Event sourcing model](../specs/event-sourcing-model.md)
- [Semantic authorization](../specs/semantic-authorization.md)
- [Semantic Diff Specification](../specs/semantic-diff-spec.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
