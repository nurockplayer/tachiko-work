# ADR-0009: Developer MVP Validation and Next Phase Boundary

## Status

Accepted

This record was originally added with the duplicate number ADR-0008. It was
renumbered to ADR-0009 when the conflict was discovered; its decision and
accepted status are unchanged.

## Context

Tachiko Work has completed the first developer MVP vertical slice.

The MVP validates the core product hypothesis:

> Work data can be represented as a semantic, computational model understandable by humans, Git, and AI.

The implementation now demonstrates:

- semantic documents
- typed data and schemas
- references
- deterministic serialization
- formula computation
- dependency validation
- semantic diff
- CLI workflow
- AI-readable semantic operations
- game balance example workflow

The project must now avoid expanding into feature accumulation before validating usability.

## Decision

The next phase should prioritize turning the developer MVP into a usable product experience rather than immediately expanding the platform surface.

The next milestone is:

> A technical designer can understand, create, modify, and review a Tachiko Work project without needing to understand the internal architecture.

Priority areas:

1. Improve developer/user workflow.
2. Stabilize public APIs and migration boundaries.
3. Add CI regression protection.
4. Add user-facing interaction layers only after semantic foundations remain stable.

## Not the immediate focus

- Spreadsheet clone UI
- Full Office replacement
- Realtime collaboration
- Cloud platform
- Broad compatibility layers

## Future implementation decisions should preserve

- semantic model as source of truth
- Git-native workflow
- AI operating on semantic data rather than UI automation
- deterministic behavior

## Consequences

The developer MVP is considered a successful architectural validation point.

Future work should optimize for adoption and usability while protecting the semantic core.
