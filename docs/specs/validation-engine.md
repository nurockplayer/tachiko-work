# Validation Engine Specification

## Purpose

The validation engine ensures semantic documents remain correct.

## Validation Categories

### Identity and Address Validation

Checks:

- nonempty opaque stable IDs and store-key/nested-ID coherence
- valid mutable schema/entity/field key grammar
- deterministic duplicate-key diagnostics separate from stable-ID failures
- bound references resolving to the intended stable object and numeric field

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
