# Event Sourcing Model

Decision state: Mixed — core event sourcing Rejected by ADR-0029; ADR-0032
semantic-event meaning Accepted; optional history techniques Open Question

Implementation state: Not implemented

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)
and
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)

Decision owner for optional history mechanics: #49

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

## Optional-profile hypothesis

A future optional history profile may use event-sourcing techniques for one
declared verification or reconstruction guarantee:

```text
Declared complete checkpoint
      |
      v
Complete retained semantic-transition tail
      |
      v
Verified state equal to the authoritative snapshot
```

Such a profile must define its contract/version, retained-transition identity,
completeness boundary, storage, retention, replay/verification, compaction,
migration, failure recovery, and snapshot-equality rules. It remains an
optional evidence/recovery profile and cannot reverse current-state authority.

Potential benefits to investigate include semantic history, collaboration,
debugging, auditability, and reproducible verification. They do not establish
those guarantees by themselves.

## Relationship with Git

Git may retain repository history while an optional Tachiko history profile
retains domain-level transition evidence that raw Git cannot express directly.
Neither history is semantic state authority. A Git commit, tree, blob, ref,
repository, or host is not revision-occurrence or semantic-event identity.

Issue #49 owns optional Git association together with history profiles,
checkpoints, replay/verification, compaction, retention/redaction, and crash
recovery. Issue #50 owns offline causality and selective CRDT/OT mechanics.

## Constraints already accepted

Any future profile must preserve these constraints:

- complete current semantic state remains usable without replay or retained
  history;
- pre-publication failure and `NoChange` create no semantic event;
- a post-install failure does not erase the installed revision occurrence;
- retained transitions and security/provenance receipts remain distinct;
- Semantic Delta remains evidence, not an apply language;
- Git remains optional and non-semantic; and
- collaboration convenience must not silently discard disputed human intent.
