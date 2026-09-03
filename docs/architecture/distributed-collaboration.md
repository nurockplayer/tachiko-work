# Distributed Collaboration Architecture

Decision state: Mixed — ADR-0011 three-way merge, ADR-0031 deterministic
Semantic Conflict v1 evidence, ADR-0029 current-state/history boundary,
ADR-0030 canonical delta evidence, ADR-0032 execution/transition taxonomy,
ADR-0033 snapshot-first history profiles, ADR-0034 team-policy/recovery
boundary, and ADR-0035 causality/selective-convergence boundary Accepted;
concrete collaboration mechanics remain Deferred.

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md),
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md), execution and
retained-transition taxonomy defined by
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md),
snapshot-first history/checkpoint guarantees defined by
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md),
team policy and cross-effect recovery constrained by
[ADR-0034](../decisions/ADR-0034-team-workspace-policy-and-recovery-boundary.md),
collaboration causality and selective convergence constrained by
[ADR-0035](../decisions/ADR-0035-collaboration-causality-and-selective-convergence-boundary.md),
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

Under ADR-0034, multi-document workflows are orchestration over separately
exact-base, authorized document publications. Partial success is explicit, and
semantic publication, persistence, required provenance, optional history, Git,
external effects, and coordination retain separate truthful outcomes. This
boundary does not select distributed transaction or coordinator mechanics.

Under ADR-0035, collaboration causal evidence remains outside semantic state,
authorization, revision identity, wall-clock authority, and Git identity. A
declared collaboration scope distinguishes known causal succession, concurrency,
and broken or unknown continuity without selecting a concrete clock or DAG
encoding. When continuity cannot be proved, the complete admitted current
snapshot is the full-resynchronization root; the gap downgrades collaboration
continuity without invalidating the snapshot or fabricating history.

Automatic convergence is selective. Presence and transient UI state may use
convergent techniques without semantic publication. No text or ordered semantic
datatype is selected yet. Structured semantic fields, schemas, formulas,
references, and disputed intent retain ordinary semantic merge/conflict and
validation. Authorization/Approval remains outside semantic state and merge,
does not converge, and stays subject to live host-authority checks. Every
collaboration result that changes semantic meaning still passes the exact-base,
authorization, validation, and publication boundary.

## Future Direction

Future adapters may use bounded retained-history or separately Accepted,
explicitly named text or ordered CRDT/OT datatypes, but they may not make replay
authoritative,
replace a complete standalone snapshot, use Git as semantic or causal identity,
or apply generic convergence to structured semantic disputes. ADR-0030 resolves
Issue #45 without making delta an operation or event. ADR-0031 resolves
Issue #46's deterministic structural conflict evidence; Issue #223 realizes its
logical production merge boundary without selecting a stable codec, transport,
wire, or SDK contract. ADR-0032 resolves Issue #48's
operation/revision/optional-event taxonomy without selecting history mechanics.
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
resolves Issue #49's snapshot-first logical history/checkpoint/Git-association
boundary without selecting concrete implementations.
[ADR-0034](../decisions/ADR-0034-team-workspace-policy-and-recovery-boundary.md)
resolves Issue #11's multi-document, cross-effect recovery, and team-policy
boundary without selecting a coordinator or runtime implementation.
[ADR-0035](../decisions/ADR-0035-collaboration-causality-and-selective-convergence-boundary.md)
resolves Issue #50's logical causality and selective-convergence boundary
without selecting clocks, DTOs, topology, text/ordered CRDT/OT datatypes or
libraries, compaction, or runtime implementation. Issue #47 retains cross-
version behavior; concrete history and collaboration engines and adapters
require separately Ready work.
