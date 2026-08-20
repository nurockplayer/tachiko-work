# ADR-0006: MVP Interface Strategy

## Status
Accepted

## Context

Building a spreadsheet-like interface too early risks turning Tachiko Work into an incremental Excel clone and delaying validation of its unique capabilities.

## Decision

The MVP primary interface is:

- CLI
- semantic document model
- machine-readable operations

A graphical workspace is a later layer built on top of the semantic core.

## Consequences

The implementation can validate the data model, Git workflow, and AI interaction before investing in complex UI.

Future interfaces must consume the same semantic model instead of introducing separate application state.
