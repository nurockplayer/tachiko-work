# Semantic Diff Specification

Decision state: Mixed. Stable-ID continuity and bound-formula comparison follow
[ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md) and
[ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md);
the remaining detailed diff surface is an implemented Provisional baseline.
See the [canonical reconciliation register](../governance/canonical-reconciliation-register.md).

## Problem

Traditional spreadsheet diff compares files.

Humans need to understand meaning.

## Goal

Tachiko Work provides semantic changes instead of raw text changes.

## Example

Traditional diff:

```diff
- goblin,180,18,1.4
+ goblin,210,21,1.4
```

Semantic diff:

```text
Goblin

HP
180 -> 210 (+16.7%)

Attack
18 -> 21 (+16.7%)
```

## Required Features

- field-level changes
- type awareness
- calculated impact
- dependency tracing
- human-readable summaries
- continuity by stable schema/entity/field ID
- explicit mutable-key changes rather than rename-as-delete-plus-add

Bound references and formulas compare by stable IDs and normalized semantic
values. Renaming a current human key does not rewrite the formula definition or
create a false relationship change; rendered summaries project current keys for
human review.

## Git Integration

Semantic diff should power:

- pull requests
- reviews
- merge decisions
- release notes
- AI summaries

## Principle

Git stores changes.

Tachiko Work explains meaning.
