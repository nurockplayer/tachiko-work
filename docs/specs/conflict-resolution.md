# Conflict Resolution Model

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

## Goals

- explain conflicts
- preserve intent
- avoid silent data loss
- provide AI assistance
