# Validation Engine Specification

Decision state: Mixed. Stable-ID, entity-reference, and bound-formula checks
follow [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md) and
[ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md);
the remaining validation categories are an implemented Provisional baseline.
See the [canonical reconciliation register](../governance/canonical-reconciliation-register.md).

## Purpose

The validation engine ensures semantic documents remain correct.

## Validation Categories

### Identity and Address Validation

Checks:

- nonempty opaque stable IDs and store-key/nested-ID coherence
- valid mutable schema/entity/field key grammar
- deterministic duplicate-key diagnostics separate from stable-ID failures
- stored entity references (`Value::Reference(EntityId)`) resolving to the
  intended entity
- bound formula references (`Expression::Reference(EntityId + FieldId)`)
  resolving to the intended entity and schema-numeric field

### Schema Validation

Checks:

- required fields
- type correctness
- allowed values

### Reference Validation

Checks:

- missing references
- circular dependencies
- unused objects

### Formula Validation

Checks:

- invalid expressions
- dependency failures
- impossible states

### Domain Validation

Examples:

- game balance ranges
- economy constraints
- business rules

## CI Integration

Validation should run in:

- local editor
- CLI
- GitHub Actions

## Goal

Move quality assurance from manual review into deterministic verification.
