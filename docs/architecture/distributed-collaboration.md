# Distributed Collaboration Architecture

Decision state: Mixed — ADR-0011 three-way merge, ADR-0031 deterministic
Semantic Conflict v1 evidence, ADR-0029 current-state/history boundary, and
ADR-0030 canonical delta evidence, and ADR-0032 execution/transition taxonomy
Accepted; broader collaboration mechanics remain Open Question.

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md),
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md), execution and
retained-transition taxonomy defined by
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
and deterministic conflict evidence specified by
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
- Issue #223 realizes the accepted `tachiko.semantic-conflict/v1` logical
  protocol in the production merge/workspace boundary with typed stable targets,
  direct facets, canonical facts, and deterministic ordering;
- the concrete Rust DTO and CLI rendering remain implementation-level rather
  than a stabilized codec, transport, wire, or SDK contract;
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
Issue #45 without making delta an operation or event. ADR-0031 resolves
Issue #46's deterministic structural conflict evidence; Issue #223 realizes its
logical production merge boundary without selecting a stable codec, transport,
wire, or SDK contract. ADR-0032 resolves Issue #48's
operation/revision/optional-event taxonomy without selecting history mechanics.
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
resolves Issue #49's snapshot-first logical history/checkpoint/Git-association
boundary without selecting concrete implementations. Issues #47 and #50 own
the remaining cross-version and causality/CRDT mechanics; concrete history
engines and adapters require separately Ready work.
