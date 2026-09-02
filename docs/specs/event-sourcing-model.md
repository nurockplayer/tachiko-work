# Event Sourcing Model

Decision state: Mixed — core event sourcing Rejected by ADR-0029; ADR-0032
semantic-event meaning and ADR-0033 bounded snapshot-first history techniques
Accepted; concrete implementations Deferred

Implementation state: Not implemented

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)
and
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
with optional history profiles defined by
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)

Decision provenance: [#49](https://github.com/nurockplayer/tachiko-work/issues/49)

## Authority note

Event sourcing is not Tachiko Work's core persistence model. Current semantic
state and complete standalone snapshots are authoritative. A retained event
stream is not the system of record, and a snapshot is not merely a replay
optimization.

When Tachiko documentation uses **semantic event**, it means ADR-0032's one
optional **retained semantic transition** concept: immutable evidence that one
actual non-no-op semantic publication occurred, relating the exact before and
after revision occurrences and canonical ADR-0030 A-to-B Semantic Delta
evidence.

A semantic event is not:

- a Command, AtomicBatch, SemanticPatch, Execute request, or mutation program;
- a pre-publication attempt, denial, stale result, conflict, or `NoChange`;
- authoritative current state or a mandatory replay input;
- an ADR-0026 security/provenance receipt; or
- checkpoint, content, Git, timestamp, path, provider, or human identity.

General retention is optional. Required ADR-0026 security/provenance evidence
survives independently when no semantic-event history is retained.

## Accepted optional profiles

ADR-0033 defines three explicit logical capability levels: snapshot-only,
retained evidence, and verified tail. Only the verified-tail profile makes a
replay claim, and that claim has this shape:

```text
complete validated checkpoint
+ complete contiguous supported replay tail
-> reconstructed candidate
-> canonical equality with the recorded authoritative snapshot
```

A retained semantic transition and its Semantic Delta remain publication
evidence, not replay instructions. A replay-capable tail additionally retains
sufficient deterministic, version-pinned replay input, normally the exact
accepted `Command | AtomicBatch`, required semantic configuration/resources,
and the recorded outcome. Imports, migrations, merges/rebaselines, and other
unsupported intent boundaries begin a new verified checkpoint or disclosed
boundary instead of being represented as synthetic Commands.

Replay is deterministic and side-effect free. Missing, corrupt, unsupported,
non-deterministic, discontinuous, or mismatching history fails the history
capability closed without replacing or reinterpreting an independently valid
authoritative snapshot. V1 defines no unqualified `full history` profile.

Physical repack preserves logical records and coverage. Retention compaction or
redaction first establishes a verified complete checkpoint, then mints new
history/checkpoint identity and discloses the new boundary. Snapshot/history
partial failures are reported truthfully; repair recovers real evidence or
declares a new boundary and never manufactures continuity.

## Relationship with Git

Git may retain repository history while an optional Tachiko history profile
retains domain-level transition evidence that raw Git cannot express directly.
Neither history is semantic state authority. A Git commit, tree, blob, ref,
repository, or host is not revision-occurrence or semantic-event identity.

ADR-0033 defines Git association as optional immutable evidence with many-to-
many cardinality between Tachiko checkpoint/history commitments and Git
commits/repositories. Mutable refs are locators. Rebase, squash, recommit,
mirroring, or migration creates new association evidence rather than silently
retargeting an existing association. Exact integrity bytes, signatures, and
trust remain with #53; concrete Git adapters require separately Ready work.

Issue #50 owns offline causality and selective CRDT/OT mechanics.

## Constraints already accepted

Any future profile must preserve these constraints:

- complete current semantic state remains usable without replay or retained
  history;
- pre-publication failure and `NoChange` create no semantic event;
- a post-install failure does not erase the installed revision occurrence;
- retained transitions and security/provenance receipts remain distinct;
- replay input remains distinct from retained transition and delta evidence;
- Semantic Delta remains evidence, not an apply language;
- Git remains optional and non-semantic; and
- collaboration convenience must not silently discard disputed human intent.

Concrete retained-history DTOs, codecs, storage, checkpoint/replay engines,
retention tooling, and Git adapters are not implemented or authorized by this
specification.
