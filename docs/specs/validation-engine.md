# Validation Engine Specification

## Purpose

The validation engine ensures semantic documents remain correct.

## Validation Categories

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
