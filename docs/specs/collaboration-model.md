# Collaboration Model Specification

## Principle

Collaboration should operate on semantic changes, not raw file edits.

## Operations

A change should describe intent:

- create entity
- update field
- change formula
- modify relationship
- add document block

## Merge Model

Semantic merge should understand:

- independent field changes
- conflicting intent
- dependency impact

Example:

Two designers modifying different fields of the same enemy should merge automatically.

Two designers changing the same balance value should create a meaningful conflict.

## Future

Possible foundations:

- operation log
- CRDT
- Git integration
- review workflow
