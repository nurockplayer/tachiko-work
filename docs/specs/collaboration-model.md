# Collaboration Model Specification

Decision state: Mixed — current merge and ADR-0029 history boundary Accepted;
broader collaboration Open Question

Authority: ADR-0011 and ADR-0029

## Principle

Collaboration currently starts from semantic changes, not raw file edits.

## Operations

A change should describe intent:

- create entity
- update field
- change formula
- modify relationship
- add document block

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
