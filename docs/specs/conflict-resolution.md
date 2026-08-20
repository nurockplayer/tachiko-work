# Conflict Resolution Model

## Principle

Conflicts should be resolved at the semantic level, not by comparing text files.

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

## Goals

- explain conflicts
- preserve intent
- avoid silent data loss
- provide AI assistance
