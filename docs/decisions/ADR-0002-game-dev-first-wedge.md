# ADR-0002: Game development is the first vertical wedge

Status: Superseded by ADR-0005

## Reconciliation note

ADR-0005 restates this decision with the later commercial-wedge and target-user framing. ADR-0005 is the current authority.

This record is retained because it documents the earlier reasoning and the original capability framing.

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
