# Collaboration Model Specification

Decision state: Mixed — current merge, ADR-0029 history boundary, and ADR-0030
canonical delta evidence Accepted; broader collaboration Open Question

Authority: [ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md) and
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md),
with canonical direct-state delta evidence defined by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md)

## Principle

Collaboration currently starts from semantic changes, not raw file edits.

## Operations

Intent remains in typed Command or ordered AtomicBatch:

- create entity
- update field
- change formula
- modify relationship
- add document block

ADR-0030 Semantic Delta is derived direct-state comparison evidence, not one of
these operations, an apply language, or a retained event.

## Merge Model

Implemented semantic merge:

- independent field updates
- typed field-level conflicts
- dependent and merged candidate validation

The merge contract also covers schema, entity membership, and references through
typed three-way reconciliation.

Future work:

- operation-log or realtime adapters
- interactive conflict resolvers
- branch-aware user identity and comments

Example:

Two designers modifying different fields of the same enemy should merge automatically.

Two designers changing the same balance value should create a meaningful conflict.

## Future

Possible foundations:

- optional retained semantic transition history
- selectively justified CRDT/OT adapters
- Git integration
- review workflow
