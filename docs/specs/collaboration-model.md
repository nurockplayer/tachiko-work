# Collaboration Model Specification

Decision state: Mixed — current merge behavior, the deterministic Semantic
Conflict v1 protocol, ADR-0029 history boundary, ADR-0030 canonical delta
evidence, and ADR-0032 execution/transition taxonomy are Accepted; broader
collaboration remains Open Question.

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md),
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md), execution and
retained-transition taxonomy defined by
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
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

## Accepted taxonomy and still-open mechanics

- operation/revision/optional-event taxonomy is fixed by
  [ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)
- optional retained history, checkpoints, replay, compaction, and Git association
  (#49)
- offline causality and selectively justified CRDT/OT boundaries (#50)
- cross-version migration/branch behavior (#47)
- interactive conflict-resolution UX and broader review workflow
- realtime collaboration transport and service topology

## Future

Possible foundations include:

- optional retained semantic transition history;
- selectively justified CRDT/OT adapters for named structures;
- Git adapters and review workflows; and
- AI assistance that explains or proposes conflict resolution while deterministic
  conflict evidence remains authoritative.
