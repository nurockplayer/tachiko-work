# Distributed Collaboration Architecture

Decision state: Mixed — ADR-0011 three-way merge, ADR-0031 deterministic
Semantic Conflict v1 evidence, ADR-0029 current-state/history boundary, and
ADR-0030 canonical delta evidence Accepted; broader collaboration mechanics
remain Open Question.

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md),
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md) and deterministic
conflict evidence specified by
[`../specs/conflict-resolution.md`](../specs/conflict-resolution.md).

## Principle

Collaboration is built on semantic state, typed operations, deterministic change
evidence, and deterministic conflict evidence rather than shared mutable files.

Traditional workflow:

User A edits file -> User B edits file -> merge conflict.

Tachiko Work workflow:

User actions become typed operations against a semantic model. Branch/revision
states reconcile through the accepted semantic merge contract; conflict evidence
uses typed stable targets rather than storage/Git paths.

Current v0.1 behavior:

- branch-based collaboration uses deterministic semantic three-way merge on
  admitted Tachiko documents;
- current production conflicts expose typed base/ours/theirs payloads through a
  provisional path-oriented address;
- ADR-0031 now defines the accepted `tachiko.semantic-conflict/v1` logical
  protocol that future production DTO/codec/runtime work must realize;
- no realtime/collaborative cursor model is implemented.

## Goals

- Real-time collaboration (future)
- Offline editing with future adapter support
- Deterministic synchronization (future)
- Conflict awareness through accepted semantic evidence
- Git compatibility without Git authority

## Boundary

```text
User Action
    |
Command or ordered AtomicBatch
    |
revision-safe semantic publication
    |
authoritative current state + complete snapshot
    |
semantic delta / three-way conflict evidence
    |
optional history / collaboration / Git adapters
```

Semantic Delta and Semantic Conflict are evidence contracts, not another Execute
language. A conflict-free merge candidate still passes the ordinary validation
and calculation authorities before publication; semantic-finalization failure is
not converted into a structural conflict kind.

## Future Direction

Future adapters may use bounded retained-history or selectively justified
CRDT/OT techniques, but they may not make replay authoritative, replace a
complete standalone snapshot, or use Git as semantic identity. ADR-0030 resolves
Issue #45 without making delta an operation or event. The ADR-0031 decision
resolves Issue #46's deterministic structural conflict evidence without selecting
production DTO/transport/runtime realization. Issues #47–#50 own the remaining
cross-version, operation/revision/event, history/checkpoint/Git-association, and
causality/CRDT mechanics.
