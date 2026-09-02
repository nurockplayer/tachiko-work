# ADR-0029: Current-state authority and optional semantic history

## Status

Accepted

Decision issue: [#12](https://github.com/nurockplayer/tachiko-work/issues/12)

Related authority: [ADR-0003](ADR-0003-ro-and-roproj-representation.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](ADR-0022-resident-semantic-runtime-and-host-boundary.md),
[ADR-0024](ADR-0024-revision-pinned-semantic-patch.md),
[ADR-0025](ADR-0025-portable-package-v1.md), and
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md)

Subsequent authority:
[ADR-0030](ADR-0030-canonical-semantic-delta.md) resolves the deferred
machine-readable Semantic Delta contract, and
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md) resolves the deferred
deterministic semantic conflict-object contract.
[ADR-0032](ADR-0032-semantic-execution-and-transition-taxonomy.md) resolves the
deferred execution/revision/optional-event taxonomy,
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md) resolves
the deferred snapshot-first history/checkpoint guarantees, and
[ADR-0034](ADR-0034-team-workspace-policy-and-recovery-boundary.md) resolves the
deferred cross-effect recovery boundary. None changes this current-state/
history boundary.

## Context

Tachiko Work needs semantic revisions, meaningful review, and optional Git
integration without making Git or a retained event stream prerequisites for a
standalone semantic workflow. Earlier architecture and specification notes
preserved operation logs, event sourcing, CRDT/OT, checkpoints, and replay as
possibilities, but did not decide which layer owns current meaning.

Current implementation evidence supports a complete materialized semantic
state, revision-safe publication, deterministic semantic diff and merge, and
standalone save/open behavior. It does not prove durable revision identity,
retained semantic history, checkpoint, replay, CRDT, or Git-mapping mechanics.

This ADR accepts only the smallest authority boundary needed to keep those
future capabilities composable without creating a second semantic authority.

## Decision

### 1. Current semantic state is authoritative and snapshots are complete

The Tachiko semantic `Document` is authoritative for current meaning. A
complete canonical snapshot MUST remain sufficient to open and use that
meaning without Git, a server, a retained operation/event log, or replay.

General history, collaboration metadata, checkpoints, and Git associations
remain outside `Document`. Omitting those optional artifacts MUST NOT change
the current semantic meaning represented by a complete snapshot.

### 2. Commands publish revision occurrences

ADR-0020's typed `Command` and ordered `AtomicBatch` remain the accepted
semantic intent vocabulary. One gated execution against one exact semantic
base may install at most one all-or-nothing semantic state. Batch members do
not create intermediate revisions.

An actual semantic state installation creates a new semantic **revision
occurrence**. A failure or denial before installation creates none. A failure
reported after installation does not erase the installed occurrence and must
not be represented as though no state change occurred.

A revision occurrence is exact concurrency and proposal-binding identity. It
is distinct from content identity, snapshot or checkpoint identity, and Git
identity; content-equivalent occurrences may still be distinct. Current
internal revision tokens MUST NOT be inferred to be durable or global.

This decision does not define public revision encoding, parent/DAG shape,
retry or semantic-no-op rules, a retained event DTO, or a replay vocabulary.

### 3. General semantic history is optional

Retained semantic transition history may add audit, recovery,
synchronization, or collaboration guarantees, but it MUST NOT become a second
source of semantic truth. A complete snapshot does not require retained
history for reconstruction or admission.

Semantic diff or delta is derived evidence between states. It is not silently
a mutation program, retained event, replay protocol, or alternate authority.

General history optionality does not weaken ADR-0026. Proposal,
authorization, approval, consumption/replay-protection, and execution
provenance required by that ADR remain mandatory where applicable and remain
outside the semantic `Document`. Their durable storage, retention, redaction,
and recovery contract is deferred.

### 4. Event sourcing is not the core persistence model

Tachiko Work does not adopt event sourcing as its core persistence model. A
retained event stream is not the system of record, and snapshots are not mere
replay optimizations.

A future optional history profile may use event-sourcing techniques for a
declared audit, verification, or reconstruction guarantee. Doing so requires
separate Accepted contracts for event semantics, versioning, retention,
replay, compaction, failure recovery, and snapshot equality; it does not amend
the authority boundary above.

### 5. Universal CRDT/OT is not adopted

Tachiko Work does not adopt a universal CRDT or operational-transformation
model for semantic state. Convergence alone does not establish semantic
validity or preserve disputed human intent.

Selective CRDT or OT techniques remain eligible only for explicitly named
datatypes or collaboration capabilities after their invariants, conflict
visibility, causality, compaction, resynchronization, and validation behavior
are accepted. Any resulting semantic state must pass normal Tachiko
admission, authorization, validation, and publication.

### 6. Git remains optional and non-semantic

Git remains an optional transport, review, retention, and provenance adapter.
A semantic revision or standalone saved state MUST be able to exist before,
between, or entirely without Git commits. Git commits, refs, repositories,
and hosts MUST NOT become semantic identity.

Git history rewrite, squash, rebase, host migration, or absence may change
repository evidence without automatically changing semantic meaning. This ADR
defines no semantic-revision-to-Git-commit mapping, association cardinality,
checkpoint cadence, signing, or trust policy.

## Deferred decisions

This ADR authorizes no production implementation. Its machine-readable semantic
delta deferral in [Issue #45](https://github.com/nurockplayer/tachiko-work/issues/45)
is resolved by ADR-0030, and its deterministic merge/conflict-object deferral in
[Issue #46](https://github.com/nurockplayer/tachiko-work/issues/46) is resolved by
ADR-0031. Its command/operation/transaction/event taxonomy deferral in
[Issue #48](https://github.com/nurockplayer/tachiko-work/issues/48) is resolved
by ADR-0032. Issue #49's logical history-profile, checkpoint, replay,
compaction, retention, recovery, and Git-association boundary is resolved by
ADR-0033, while ADR-0034 resolves the cross-effect recovery boundary. Those
Issues retain historical provenance, and concrete mechanisms remain Deferred.
The following decision work remains open:

- [Issue #47](https://github.com/nurockplayer/tachiko-work/issues/47):
  compatibility migration and cross-version branch behavior;
- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50): offline
  causality, selective CRDT/OT boundaries, and resynchronization.

Production realization of ADR-0030 Semantic Delta or ADR-0031 Semantic Conflict
or the logical ADR-0033/ADR-0034 guarantees is not implicitly authorized merely
because those contracts are Accepted; each concrete mechanism requires
separately Ready implementation work.

Concrete public revision/event/receipt DTOs, checkpoint formats or cadence,
history storage, replay/upcasters, causal clocks, Git mapping, and
collaboration/server topology are not selected here. ADR-0032 fixes their
logical taxonomy and identity separation, ADR-0033 fixes bounded snapshot-first
history guarantees, and ADR-0034 fixes truthful cross-effect recovery.

## Consequences

- Standalone snapshots remain independently usable and authoritative for
  current meaning.
- Revision-safe semantic publication can evolve without committing Tachiko to
  event-sourced storage.
- Optional audit, recovery, synchronization, and Git adapters must declare
  their guarantees without becoming semantic authorities.
- ADR-0026 security/provenance obligations survive even when general semantic
  history is omitted.
- Later collaboration work must justify selective algorithms against Tachiko's
  semantic invariants instead of assuming universal CRDT/OT convergence.

## Rejected alternatives

- **An authoritative retained event stream with snapshots as cache:** rejected
  because it reverses the standalone snapshot boundary and prematurely freezes
  permanent event, replay, migration, and retention semantics.
- **Universal CRDT/OT semantic state:** rejected because convergence does not
  by itself preserve validation invariants or explicit human intent.
- **Git commits as semantic revision or checkpoint identity:** rejected
  because Git is optional and its evidence can be rewritten, migrated, or
  absent without changing semantic meaning.

## Related

- [Issue #12](https://github.com/nurockplayer/tachiko-work/issues/12)
- [Semantic operation log model](../specs/operation-log-model.md)
- [Event sourcing model](../specs/event-sourcing-model.md)
- [Collaboration model](../specs/collaboration-model.md)
- [Distributed collaboration architecture](../architecture/distributed-collaboration.md)
- [Git-native workflow](../architecture/git-native-workflow.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
- [ADR-0032 semantic execution and retained-transition taxonomy](ADR-0032-semantic-execution-and-transition-taxonomy.md)
