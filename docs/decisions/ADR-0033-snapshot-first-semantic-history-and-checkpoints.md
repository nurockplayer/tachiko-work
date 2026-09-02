# ADR-0033: Snapshot-first semantic history and checkpoints

## Status

Accepted

Decision issue: [#49](https://github.com/nurockplayer/tachiko-work/issues/49)

Specified by:
[Semantic Operation Log Model](../specs/operation-log-model.md) and
[Event Sourcing Model](../specs/event-sourcing-model.md)

Related authority:
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md),
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md),
[ADR-0030](ADR-0030-canonical-semantic-delta.md), and
[ADR-0032](ADR-0032-semantic-execution-and-transition-taxonomy.md)

Successor resolution:
[ADR-0034](ADR-0034-team-workspace-policy-and-recovery-boundary.md) resolves
Issue #11's deferred multi-document, cross-effect, transaction, and team-
recovery boundary. The Issue #11 references below record ownership when this
ADR was accepted; they no longer identify an open decision.

## Context

ADR-0029 makes the semantic `Document` authoritative for current meaning and
requires complete standalone snapshots that do not depend on retained history,
replay, Git, a server, or a historical interpreter. ADR-0032 separately fixes
the Execute-attempt, `NoChange`, revision-occurrence, retained-transition, and
receipt taxonomy. ADR-0030 makes Semantic Delta direct A-to-B evidence rather
than a mutation language, while ADR-0026 requires minimum security/provenance
evidence independently of optional general history.

Those decisions deliberately left history capability levels, checkpoints,
bounded segments, replay verification, compaction, partial-failure recovery,
and Git association unresolved. Without explicit guarantees, the mere presence
of log-like files could be mistaken for complete or replayable history, and a
replay failure could be mistaken for authority to replace a valid current
snapshot.

This decision fixes the smallest snapshot-first logical contract for those
capabilities. It does not choose production DTOs, codecs, storage layouts,
checkpoint writers, replay engines, Git adapters, transaction infrastructure,
or a new `.ro` or `.roproj` format version.

## Decision

### 1. General history declares one explicit capability profile

The v1 logical capability levels are:

- **Snapshot-only:** a complete validated canonical snapshot represents current
  meaning, with no general semantic-history guarantee.
- **Retained evidence:** zero or more immutable retained transitions, receipts,
  or related evidence may be available, but the retained set is explicitly
  incomplete and non-replayable.
- **Verified tail:** one complete checkpoint plus one contiguous, supported,
  version-pinned history segment reaches one exact later authoritative snapshot,
  and deterministic replay has been verified by canonical snapshot equality.

Guarantees MUST be declared by profile and coverage, not inferred from files,
record counts, filenames, timestamps, Git history, or implementation topology.
A consumer requiring a stronger profile fails closed or reports a capability
downgrade when only a weaker profile is available. The authoritative snapshot
remains usable when its independent admission succeeds.

V1 defines no unqualified `full history` profile. A future complete-from-origin
claim would first need to define the exact origin and every allowed import,
merge, migration, compaction, redaction, and causal boundary with no undisclosed
gap.

### 2. A checkpoint is a distinct immutable logical identity

A logical checkpoint binds:

- one continuing `DocumentId`;
- one complete validated canonical snapshot and its snapshot commitment;
- the semantic and representation contract versions required to read it;
- the history-profile contract/version;
- an explicit history-coverage declaration; and
- the declared history-segment and independently meaningful evidence
  commitments included by that profile.

`CheckpointRef` identifies one immutable logical checkpoint occurrence. It is
distinct from `DocumentId`, snapshot/content identity,
`RevisionOccurrenceRef`, retained-transition identity, receipt identity,
history-segment identity, representation identity, and every Git identity.
Equivalent snapshot content may participate in distinct checkpoints with
different coverage or commitments.

No Git identity, path, timestamp, provider/model identity, or human key becomes
semantic, revision, checkpoint, or history identity. Those incidental values
cannot establish equality, continuity, coverage, or order for these namespaces.

A standalone checkpoint MUST resolve its complete snapshot without unavailable
Git, network, server, or host-local state. Physical deduplication is permitted
when that logical guarantee remains true. Reopening a checkpoint creates a new
live runtime occurrence and MUST NOT resurrect a prior internal revision token
or imply continuity of its old revision context.

### 3. Retained history is composed of bounded immutable segments

Each history segment is scoped to exactly one `DocumentId` and one owning
history/revision context. It declares:

- an exact start checkpoint or explicit boundary;
- an exact end;
- segment-local record order;
- the contract versions required to interpret its records;
- continuity and coverage; and
- every disclosed gap or closed predecessor boundary.

When a segment starts from a checkpoint, its start boundary MUST bind the
checkpoint's exact snapshot commitment to the first replay record's exact
semantic base and `before` state. That binding establishes a new segment-start
occurrence in the owning context; it does not resurrect an old runtime revision
token. A mismatched or unprovable start binding is a gap, not a contiguous tail.

Within a contiguous transition range, each retained transition's `before`
occurrence MUST match the preceding transition's `after` occurrence in that
same context. A missing, unsupported, corrupt, redacted, or mismatched required
record breaks continuity and therefore the stronger capability claim.

Segment-local order does not create global time, a universal revision order,
multi-parent causality, or a universal DAG. Offline parent/branch identity,
logical clocks, resynchronization, and selective CRDT/OT remain with
[Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50).

### 4. Replay input is distinct from retained transition evidence

ADR-0032's retained semantic transition remains immutable evidence that one
actual non-no-op publication occurred. Its canonical Semantic Delta remains
direct A-to-B evidence. Neither is the replay program.

A replay-capable segment additionally retains sufficient deterministic,
version-pinned replay input, normally the exact accepted
`Command | AtomicBatch`, together with every required semantic configuration or
resource and the recorded outcome. Replay input remains distinct from retained-
transition identity and from the authorization or provenance meaning of an
ADR-0026 receipt.

Imports, semantic or representation migrations, merge/rebaseline boundaries,
and other transitions that cannot be represented faithfully through the
supported intent contract MUST establish a new verified checkpoint or explicit
boundary. They MUST NOT be disguised as replayable Commands.

### 5. Replay verifies a tail; it does not admit current state

A verified-tail claim has exactly this logical shape:

```text
complete validated checkpoint
+ complete contiguous supported replay tail
-> reconstructed candidate
-> canonical equality with the recorded authoritative snapshot
```

Replay MUST be deterministic and side-effect free. It MUST NOT invoke an LLM,
network call, wall clock, random source, Git operation, or external effect.
Before the first replay step, the exact checkpoint snapshot MUST satisfy the
segment-start binding. After every step, the reconstructed outcome and canonical
before-to-after evidence MUST match the recorded outcome and retained transition
before the candidate becomes the next exact replay base. Final snapshot equality
is an additional end-to-end check, not a substitute for start or per-step
verification.

Missing or corrupt records, unsupported contracts, discontinuity,
non-deterministic dependencies, invalid transitions, or canonical snapshot
mismatch fail the replay/history capability closed. Such failure MUST NOT
reinterpret, silently repair, advance, or replace an independently valid
authoritative snapshot. Replay is optional verification or recovery evidence,
not current-state admission and not a second source of semantic truth.

### 6. Snapshot and history admission report separate results

Snapshot admission and optional history admission are independent. A valid
current snapshot may open while history is absent, incomplete, unsupported,
corrupt, redacted, discontinuous, or divergent. A workflow that requires
verified history or required provenance MUST fail closed or explicitly
downgrade that capability; it MUST NOT fabricate a transition, receipt,
authorization fact, replay input, or `NoChange` outcome.

A concrete profile MUST detect and report at least:

- authoritative snapshot ahead of retained history;
- retained evidence ahead of the last durable snapshot or checkpoint;
- missing required receipt or evidence;
- unsupported, corrupt, redacted, or incomplete history; and
- snapshot/history mismatch.

Snapshot-ahead creates an explicit history gap unless trustworthy real evidence
is recovered. History-ahead MUST NOT automatically advance authoritative
current state. It may remain truthful evidence that an earlier live publication
occurred without a corresponding durable checkpoint. A failure after semantic
publication MUST preserve the known truth that publication occurred.

Repair may recover genuine records or establish a new declared checkpoint and
coverage boundary. It MUST NOT synthesize false history. Semantic publication,
checkpoint persistence, retained-transition persistence, receipt persistence,
and Git commit remain separate effects. Broader multi-effect transaction,
rollback, and recovery policy was assigned to
[Issue #11](https://github.com/nurockplayer/tachiko-work/issues/11) and is now
resolved at the logical boundary by ADR-0034.

### 7. Repack and retention compaction have different semantics

A **physical repack** preserves every logical record, order, commitment,
continuity fact, and coverage declaration. It may change representation
identity without changing logical checkpoint/history identity or guarantees.

A **retention compaction** discards, coalesces, redacts, or closes prior history.
Before claiming a new retained-history boundary, it MUST establish and verify a
complete checkpoint. It then mints new checkpoint/history identity and
explicitly discloses the new coverage boundary and gaps. It MUST NOT continue
claiming discarded material as complete or replayable.

Privacy policy may intentionally sever predecessor links. Optional general-
history compaction never waives ADR-0026 minimum provenance, Approval-
consumption, or replay-protection obligations. If required security evidence is
removed below its accepted minimum, the artifact loses that security guarantee
even though an independently valid semantic snapshot remains meaningful.

### 8. Undo and revert move forward

Undo or revert is a new authorized `Command | AtomicBatch` evaluated against
the exact current base through ordinary admission, authorization, validation,
and publication. Publishing content equivalent to an earlier snapshot creates
a distinct new revision occurrence and, when retained history is enabled, a
new retained transition.

History is never erased, rewound, or retargeted by undo. Historical checkout is
a read/view or candidate source until it re-enters the normal publication
boundary.

### 9. Commitment scopes are logical and separate

This decision defines three logical commitment scopes:

- a **snapshot commitment** covers current semantic snapshot content;
- a **history-segment commitment** covers ordered immutable records, required
  contract versions, continuity, and declared coverage/gaps; and
- a **checkpoint commitment** covers `DocumentId`, the complete snapshot
  commitment, required contract versions, the history profile/coverage, and
  declared included segment or evidence commitments.

This ADR does not choose canonical commitment bytes, digest or signature
algorithms, key/trust policy, or portable proof formats. Those integrity and
trust semantics must coordinate with
[Issue #53](https://github.com/nurockplayer/tachiko-work/issues/53).
A history root alone does not prove authorization.

### 10. Git association is optional immutable evidence

Git association has many-to-many cardinality. One checkpoint or history
commitment may appear in zero, one, or many commits or repositories, while one
Git commit may contain zero, one, or many Tachiko checkpoints plus unrelated
files.

An association record identifies the Git object plus the repository and hash-
algorithm context needed to interpret it. Mutable refs are locators, not
identity. Rebase, squash, recommit, mirroring, or repository migration creates
new association evidence; an existing association MUST NOT be silently
retargeted.

Git absence or unreachable historical commits do not invalidate semantic state,
checkpoint identity, or a self-contained checkpoint. Bytes obtained from Git
re-enter ordinary Tachiko admission and validation. Git identity never becomes
semantic, revision, checkpoint, history, or commitment identity.

### 11. Versions close unsupported replay ranges explicitly

Every checkpoint, retained transition, replay input, and history profile pins
the contracts required to interpret its guarantee. Unsupported versions fail
the relevant history or replay capability closed. Old commands MUST NOT be
silently reinterpreted under current semantics.

A verified complete checkpoint under a new supported contract may explicitly
close an older replay range and begin a new one. Cross-version branch migration,
write-version pinning, and merge behavior remain with
[Issue #47](https://github.com/nurockplayer/tachiko-work/issues/47).

## Consequences

- Standalone current-state artifacts remain snapshot-complete and usable when
  general history is absent or unusable.
- A history consumer can distinguish incomplete evidence from a replay-verified
  contiguous tail without trusting storage shape or filenames.
- Checkpoint, history, revision, receipt, commitment, representation, and Git
  identities remain independently meaningful.
- Replay verifies recorded current-state equality but never becomes mutation or
  admission authority.
- Compaction, redaction, partial failure, and repair preserve truthful coverage
  rather than manufacturing continuity.
- Concrete profiles and implementations require separately Ready work.

## Rejected alternatives

- **An unqualified full-history profile:** rejected because imports, merges,
  migrations, redaction, compaction, and future causal branches make the claim
  undefined without an exact origin and disclosed boundaries.
- **Retained transitions or Semantic Delta as replay programs:** rejected because
  publication evidence and direct-state evidence are not accepted intent.
- **Replay output as current-state authority:** rejected because it would reverse
  ADR-0029's snapshot authority and turn history failure into silent state
  replacement.
- **Checkpoint identity as a revision, snapshot hash, or Git commit:** rejected
  because those identities have different equality, lifecycle, and optionality.
- **Compaction that preserves a completeness claim after discarding history:**
  rejected because coverage would become false.
- **Undo by deleting or rewinding history:** rejected because a prior-equivalent
  state is a new authorized publication occurrence.
- **One checkpoint per Git commit:** rejected because Tachiko and Git artifacts
  have genuinely many-to-many cardinality.
- **Eternal command upcasting:** rejected because unsupported historical
  semantics must fail closed and may be closed by a new verified boundary.

## Ownership boundaries

This ADR authorizes no production implementation.

- [Issue #47](https://github.com/nurockplayer/tachiko-work/issues/47) owns
  representation/semantic migration, write-version pinning, and cross-version
  branch behavior.
- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50) owns causal
  parents/DAGs, offline branch identity, logical clocks, resynchronization, and
  selective CRDT/OT.
- [Issue #11](https://github.com/nurockplayer/tachiko-work/issues/11) originally
  owned broader multi-document, host, external-effect, transaction/rollback,
  and team recovery policy; ADR-0034 now resolves the logical boundary while
  concrete mechanisms remain separately owned.
- [Issue #53](https://github.com/nurockplayer/tachiko-work/issues/53) owns exact
  integrity-root algorithms, canonical commitment bytes, signatures, and trust
  semantics.
- Separately Ready implementation Issues must own concrete DTO/wire/storage
  layouts, codecs, durable adapters, checkpoint and replay engines, retention
  policy UI, Git adapters, and operational tooling.

## Related

- [Issue #49](https://github.com/nurockplayer/tachiko-work/issues/49)
- [Semantic Operation Log Model](../specs/operation-log-model.md)
- [Event Sourcing Model](../specs/event-sourcing-model.md)
- [Collaboration Model](../specs/collaboration-model.md)
- [Git-Native Workflow](../architecture/git-native-workflow.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
