# ADR-0002: Game development is the first vertical wedge

Status: Accepted

## Context

Game teams already depend on spreadsheets for balancing, but spreadsheet files do not integrate naturally with Git workflows.

Common workarounds:

- store binary spreadsheet files
- export CSV files
- manually review changes

## Decision

The first Tachiko Work product should target game data workflows.

Core capabilities:

- Git-native spreadsheets
- typed schemas
- formulas
- semantic diff
- semantic merge
- validation and CI

## Rationale

This provides a focused market where the pain is strong and the value is measurable.

The long-term platform remains broader than games.
