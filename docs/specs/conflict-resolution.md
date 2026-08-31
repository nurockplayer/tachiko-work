# Conflict Resolution Model

Decision state: The implemented three-way merge contract follows
[ADR-0011](../decisions/ADR-0011-semantic-three-way-merge.md). Canonical direct
state delta evidence follows
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md). The remaining
versioned conflict-object protocol is owned by
[#46](https://github.com/nurockplayer/tachiko-work/issues/46).

## Principle

Conflicts are resolved at the semantic level, not by comparing text files.

The v0.1 merge behavior:

- unchanged values from both sides pass through
- single-sided changes are accepted
- conflicting changes return typed conflict objects (base/ours/theirs)
- merged outputs must validate and calculate before persistence

## Example

Independent changes:

```
Dragon.hp
8000 -> 9000

Dragon.attack
420 -> 380
```

can merge automatically.

Conflicting changes:

```
Dragon.hp
8000 -> 9000

Dragon.hp
8000 -> 10000
```

require review.

Current implementation notes:

- path-order is stable and deterministic.
- typed payload preserves semantic context; no partial documents are emitted on conflict.

Here `path` is the current internal typed semantic conflict address, not a
filesystem path or JSON Pointer and not canonical Semantic Delta ordering.
Merge may consume delta as comparison evidence, but delta is not an apply
language or mutation authority.

## Goals

- explain conflicts
- preserve intent
- avoid silent data loss
- provide AI assistance
