# ADR-0004: MVP Boundary

## Status
Accepted

## Context

Tachiko Work has a long-term vision of becoming a semantic workspace beyond traditional Office tools. The first implementation must prove the core hypothesis without attempting to rebuild existing productivity suites.

## Decision

The first MVP focuses on proving:

> Work data can be represented as a semantic, computational model understandable by humans, Git, and AI.

MVP includes:

- semantic documents
- versioned `.ro` foundation
- schema validation
- formula computation
- semantic diff
- CLI workflow
- game balance example
- AI semantic read/query capability

MVP explicitly excludes:

- spreadsheet UI
- Word/Office compatibility
- realtime collaboration
- cloud SaaS platform
- enterprise permissions

## Consequences

Implementation prioritizes semantic capability over familiar Office features.

Future UI and collaboration layers must build on the semantic model rather than redefine it.
