# Schema System Specification

Decision state: Mixed. Stable identity and reference-address statements are
Accepted under [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md);
the remaining detailed schema behavior is an implemented Provisional baseline.
See the [canonical reconciliation register](../governance/canonical-reconciliation-register.md).

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

The implemented Milestone 02 schema model separates opaque `SchemaId` and
`FieldId` identities from mutable `SchemaKey` and `FieldKey` authoring
addresses. Reference field types store the target stable `SchemaId`; human keys
are resolved through deterministic derived indexes and may be renamed without
retargeting existing relationships.
