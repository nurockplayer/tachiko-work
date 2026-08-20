# Schema System Specification

## Principle

Tachiko Work treats structured data as typed semantic objects rather than unvalidated cells.

## Schema Responsibilities

Schemas define:

- field names
- types
- constraints
- references
- computed fields
- validation rules

## Example

```
Enemy
├── id: EnemyId
├── hp: Health
├── attack: Damage
├── speed: Speed
└── drops: ItemReference[]
```

## Benefits

Typed schemas enable:

- validation
- autocomplete
- AI understanding
- safer refactoring
- dependency analysis
- engine integration

## References

References should be semantic relationships, not strings.

A missing referenced object should be detectable before runtime.
