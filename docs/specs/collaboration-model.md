# Collaboration Model Specification

Decision state: Mixed — current merge behavior, the deterministic Semantic
Conflict v1 protocol, ADR-0029 history boundary, ADR-0030 canonical delta
evidence, ADR-0032 execution/transition taxonomy, and ADR-0033 snapshot-first
history profiles are Accepted; broader collaboration remains Open Question.

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md),
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md), execution and
retained-transition taxonomy defined by
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
snapshot-first history/checkpoint guarantees defined by
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md),
and deterministic conflict evidence specified by
[`conflict-resolution.md`](conflict-resolution.md).

## Principle

Collaboration currently starts from semantic state, typed intent, deterministic
change evidence, and deterministic conflict evidence rather than raw file edits.

## Operations

Intent remains in typed Command or ordered AtomicBatch:

- create entity
- update field
- change formula
- modify relationship
- add document block where a supported semantic contract defines one

ADR-0030 Semantic Delta is derived direct-state comparison evidence, not one of
these operations, an apply language, or a retained event.

ADR-0031 Semantic Conflict is deterministic three-way reconciliation evidence,
not a mutation program, validation diagnostic, retained event, or Git marker.

## Merge Model

Accepted semantic merge includes:

- equal and single-sided direct facts merging automatically;
- independent semantic-field updates composing;
- three closed structural conflict kinds: `concurrent_addition`,
  `delete_modify`, and `concurrent_change`;
- conflict targeting by typed stable target plus direct semantic facet;
- deterministic logical conflict identity and canonical ordering;
- rename continuity under stable semantic identity;
- post-merge validation/calculation failure remaining separate from structural
  conflicts; and
- no partial candidate publication when structural conflicts exist.

The merge contract covers document title, schemas and schema fields, entity
membership, references, and stored values through typed three-way reconciliation.
Issue #223 realizes the accepted v1 logical protocol in the production
merge/workspace boundary. Its concrete Rust DTO and CLI rendering remain
replaceable implementation evidence rather than a stabilized codec, wire, or SDK
contract.

Example:

Two designers modifying different fields of the same enemy should merge
automatically.

Two designers changing the same stored balance value to different values should
produce one deterministic `concurrent_change` conflict on that stable semantic
field target.

A structurally conflict-free merge that creates an invalid reference or formula
state is rejected by ordinary semantic finalization; it does not invent another
conflict kind.

## Accepted taxonomy and bounded history

- operation/revision/optional-event taxonomy is fixed by
  [ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)
- optional history declares snapshot-only, retained-evidence, or verified-tail
  capability under
  [ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
- a verified tail requires one complete checkpoint, one contiguous supported
  version-pinned replay segment, and canonical equality with the exact later
  authoritative snapshot
- retained transitions and Semantic Delta remain evidence rather than the
  replay program; replay-capable history additionally retains deterministic
  version-pinned `Command | AtomicBatch` input and required semantic resources
- compaction, redaction, snapshot/history failure, forward-only undo, and
  optional many-to-many Git association preserve explicit coverage and
  snapshot-first authority under ADR-0033
- concrete history DTOs, storage, checkpoint/replay engines, retention tooling,
  and Git adapters remain separately owned implementation work
- offline causality and selectively justified CRDT/OT boundaries (#50)
- cross-version migration/branch behavior (#47)
- broader multi-effect transaction/rollback and team recovery policy (#11)
- exact commitment bytes, signatures, and trust semantics (#53)
- interactive conflict-resolution UX and broader review workflow
- realtime collaboration transport and service topology

## Future

Possible implementation foundations include:

- optional retained evidence or a verified checkpoint tail under ADR-0033;
- selectively justified CRDT/OT adapters for named structures;
- Git adapters and review workflows; and
- AI assistance that explains or proposes conflict resolution while deterministic
  conflict evidence remains authoritative.
