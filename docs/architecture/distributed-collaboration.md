# Distributed Collaboration Architecture

## Principle

Collaboration is built on semantic operations, not shared mutable files.

Traditional workflow:

User A edits file -> User B edits file -> merge conflict.

Tachiko Work workflow:

User actions become typed operations against a semantic model.

## Goals

- Real-time collaboration
- Offline editing
- Deterministic synchronization
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
