# Semantic Operation Log Model

Decision state: Mixed — ADR-0029 history boundary, ADR-0032 transition
taxonomy, ADR-0033 snapshot-first retained-history profiles, ADR-0034
cross-effect recovery boundary, and ADR-0035 causal-evidence separation
Accepted; concrete DTO, wire, storage, causal, and operational mechanics
Deferred

Implementation state: No first-class persisted semantic operation/history log

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)
and
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
with history profiles, checkpoints, and replay verification defined by
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
and cross-effect recovery constrained by
[ADR-0034](../decisions/ADR-0034-team-workspace-policy-and-recovery-boundary.md),
with collaboration causal evidence separated by
[ADR-0035](../decisions/ADR-0035-collaboration-causality-and-selective-convergence-boundary.md)

Decision provenance: [#49](https://github.com/nurockplayer/tachiko-work/issues/49)

## Overview

Meaningful changes are requested through ADR-0020 typed `Command | AtomicBatch`
and may be proposed through an ADR-0024 SemanticPatch. `Operation` is only an
umbrella/conversational word at this layer. It does not name another mutation
DTO or a persistable apply language, and `transaction` does not extend
AtomicBatch into host or distributed transaction semantics.

An Execute attempt publishes zero or one semantic state installation. Only an
actual non-no-op installation creates a semantic revision occurrence.
Pre-publication failure and `NoChange` create none.

ADR-0029 makes any general retained history optional and non-authoritative. A
complete snapshot remains sufficient to open and use current semantic meaning
without an operation log, retained transition stream, Git, checkpoint, or
replay.

## Reconciled vocabulary

```text
Command | AtomicBatch
    -> optional SemanticPatch proposal
    -> gated Execute attempt
    -> zero or one semantic state installation
       -> if no installation: failure or NoChange; no revision/event
       -> if installed: revision occurrence
          + canonical A-to-B Semantic Delta evidence
          + required security/provenance receipt where ADR-0026 applies
          + optional retained semantic transition/event
```

The concepts are not interchangeable:

- `Command | AtomicBatch` is typed semantic intent.
- SemanticPatch is an immutable exact-base proposal occurrence.
- Execute attempt is a request to evaluate and possibly publish that intent.
- `NoChange` is a non-publication outcome.
- `RevisionOccurrenceRef` is opaque occurrence identity scoped to one owning
  revision context/domain and continuing `DocumentId`.
- Semantic Delta is canonical direct A-to-B state evidence, not intent or a
  mutation program.
- A retained semantic transition, also called a semantic event, is optional
  immutable evidence of one actual non-no-op publication.
- An ADR-0026 receipt is independent security/provenance evidence and remains
  required where that authority applies even if general history is disabled.

Current runtime Commands and internal revision tokens are implementation
evidence. They are not a canonical persisted log, globally meaningful revision
identity, or a public retained-transition DTO.

## Accepted history capability profiles

History guarantees are declared explicitly rather than inferred from retained
files:

- **Snapshot-only** provides no general semantic-history guarantee.
- **Retained evidence** may preserve immutable transitions, receipts, or related
  evidence, but declares the set incomplete and non-replayable.
- **Verified tail** binds one complete checkpoint to one contiguous, supported,
  version-pinned replay segment ending at an exact later authoritative snapshot,
  verified by canonical snapshot equality.

V1 defines no unqualified `full history` profile. Imports, merges, migrations,
redaction, compaction, and future causal branches require explicit boundaries
and gaps rather than an undefined completeness claim.

## Checkpoints and bounded segments

A checkpoint has an immutable logical identity distinct from revision,
snapshot/content, transition, receipt, segment, representation, and Git
identities. It binds one `DocumentId`, a complete validated canonical snapshot,
required semantic and representation versions, the history-profile version,
explicit coverage, and declared included segment/evidence commitments. A
standalone checkpoint resolves its complete snapshot without unavailable Git,
network, server, or host-local state. Reopening it creates a new live runtime
occurrence rather than reviving an old internal revision token.

History is retained in immutable bounded segments scoped to one `DocumentId`
and one owning history/revision context. Each segment declares an exact start
checkpoint or boundary, exact end, segment-local order, required contract
versions, continuity, coverage, and gaps. Each transition's `before` occurrence
matches the preceding `after` occurrence within a contiguous range. Segment
order implies neither global time nor multi-parent causality. A checkpoint-start
boundary binds the checkpoint snapshot commitment to the first replay record's
exact base and `before` state; a mismatch is a gap.

## Replay and equality verification

A retained transition and its Semantic Delta are publication evidence, not the
replay program. A replay-capable segment additionally retains the exact
deterministic, version-pinned replay input, normally the accepted
`Command | AtomicBatch`, plus every required semantic configuration/resource
and the recorded outcome. Replay verifies the start binding and each
reconstructed outcome/transition before advancing to the next exact base;
endpoint equality cannot replace those checks.

Replay runs only from a complete checkpoint through a complete contiguous
supported tail. It is side-effect free and must not use an LLM, network, wall
clock, random source, Git operation, or external effect. Canonical equality with
the recorded authoritative snapshot verifies the claim. Missing, corrupt,
unsupported, non-deterministic, discontinuous, or mismatching history fails the
history capability closed without replacing or reinterpreting a valid snapshot.

Imports, migrations, merge/rebaseline boundaries, and other changes that cannot
be expressed faithfully through the supported intent contract establish a new
verified checkpoint or explicit boundary rather than synthetic Commands.

## Retention, failure, and repair

A physical repack may change representation identity while preserving every
logical record, order, commitment, and coverage guarantee. Retention compaction
that discards, coalesces, redacts, or closes history first verifies a complete
checkpoint, then mints new checkpoint/history identity and declares the new
coverage boundary. Privacy policy may sever predecessor links, but general
history retention cannot waive ADR-0026 minimum provenance or replay-protection
obligations.

Snapshot and history admission are separate. A profile reports snapshot-ahead,
history-ahead, missing required evidence, unsupported/corrupt/redacted/incomplete
history, and snapshot/history mismatch. History-ahead never advances current
state automatically; snapshot-ahead is an explicit gap unless real evidence is
recovered. Repair recovers genuine records or establishes a new boundary and
never fabricates history.

Undo/revert moves forward through a newly authorized `Command | AtomicBatch`
against the current base. Prior-equivalent content is a new revision occurrence;
history is not erased or rewound.

## Logical commitments and deferred implementation

ADR-0033 distinguishes snapshot, history-segment, and checkpoint commitment
scopes without selecting canonical bytes, digest/signature algorithms, or trust
semantics; those remain with #53. Git association is optional many-to-many
immutable evidence and never supplies semantic, checkpoint, or history identity.
Every checkpoint, transition, replay input, and history profile pins its
interpretation contracts; unsupported versions fail the capability closed and
a new verified checkpoint may explicitly close an older replay range.

Under ADR-0034, multi-document work is orchestration over separate exact-base,
authorized publications. Semantic publication, host persistence, required
security/provenance evidence, optional history/checkpoints, Git, external
effects, and collaboration coordination retain separate truthful outcomes. A
later failure does not erase an installed revision; recovery reconciles current
snapshots plus genuine evidence and moves forward through a new authorized
command when semantic compensation is needed. An uncertain external outcome is
reconciled before retry. External correlation, delivery deduplication, and
idempotency MUST NOT be derived by requirement from semantic proposal,
revision, snapshot, transition, receipt, checkpoint, or Git identity; the
authoritative effect boundary owns its separately Deferred mechanism.

Concrete public DTOs, wire mappings, codecs, storage layouts, checkpoint/replay
engines, retention tooling, Git adapters, cross-effect coordinators, and
external-effect protocols require separately Ready implementation work.
ADR-0035 resolves the logical offline-causality, resynchronization, and
selective-convergence boundary while concrete clocks, DAGs, engines, and
CRDT/OT mechanisms remain separately owned. Issue #47 owns cross-version branch
migration behavior.
