# Distributed Collaboration Architecture

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

## Model

```
User Action
    |
Semantic Operation
    |
Operation Log
    |
State Reconstruction
```

## Future Direction

The system may combine event sourcing, CRDT techniques, and Git history to provide both collaboration and reproducibility.
