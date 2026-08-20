# ADR-0007: AI Semantic Interaction Model

## Status
Accepted

## Context

AI systems should understand and manipulate the underlying meaning of work rather than automate clicks against traditional interfaces.

## Decision

AI interacts through the Tachiko Work semantic layer.

MVP permissions:

- read: allowed
- analysis: allowed
- explanation: allowed
- suggestions: allowed
- direct mutation: requires explicit approval

## Consequences

The AI API should expose semantic operations, document structure, formulas, and impact analysis.

Autonomous agents and unrestricted editing are deferred until permission, safety, and review workflows are mature.
