# Distributed Collaboration Architecture

Decision state: Mixed — ADR-0029 layer boundary and ADR-0030 canonical delta
evidence Accepted; collaboration mechanics Open Question

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md) and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md)

## Principle

Collaboration is built on semantic operations, not shared mutable files.

Traditional workflow:

User A edits file -> User B edits file -> merge conflict.

Tachiko Work workflow:

User actions become typed operations against a semantic model.

Current v0.1 behavior:

- branch-based collaboration uses deterministic semantic three-way merge on `.ro` documents.
- conflicts are returned with typed path-level payloads.
- no realtime/collaborative cursor model is implemented.

## Goals

- Real-time collaboration (future)
- Offline editing with future adapter support
- Deterministic synchronization (future)
- Conflict awareness
- Git compatibility

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
optional history / collaboration / Git adapters
```

## Future Direction

Future adapters may use bounded retained-history or selectively justified
CRDT/OT techniques, but they may not make replay authoritative, replace a
complete standalone snapshot, or use Git as semantic identity. ADR-0030 resolves
#45 without making delta an operation or event. #46 and Issues #48–#50 own the
remaining deferred mechanics.
