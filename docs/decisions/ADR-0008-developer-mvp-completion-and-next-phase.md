# ADR-0008: Developer MVP completion and next phase boundary

## Status

Accepted

## Context

Tachiko Work has completed the first developer MVP vertical slice.

The MVP was intended to prove that work data can become a semantic computational model understood by humans, Git, and AI rather than a traditional Office document.

The implemented slice validates:

- semantic documents
- typed data and schemas
- references
- deterministic serialization
- formula computation
- dependency tracking
- semantic diff
- CLI workflow
- AI-readable semantic access
- game balance example workflow

## Decision

The developer MVP is considered complete enough to move from architecture validation into product usability refinement.

The next phase should optimize for:

1. making the existing workflow understandable to a real technical designer;
2. stabilizing public APIs and compatibility boundaries;
3. improving examples and onboarding;
4. preserving the semantic-first architecture.

Future work should not immediately expand into Office compatibility, spreadsheet UI, or collaboration infrastructure.

## Next phase priorities

Recommended order:

1. Add CI protection for semantic behavior and example workflows.
2. Stabilize public crate APIs and migration fixtures.
3. Resolve future project/container format decisions such as `.roproj`.
4. Expand formula capabilities required by real game balance workflows.
5. Add richer AI interfaces while maintaining explicit permission boundaries.

## Consequences

Positive:

- Tachiko Work has a concrete developer-facing milestone.
- Future implementation can be evaluated against a working reference.
- Human product decisions can be made using a real artifact rather than only architecture documents.

Negative:

- Some architectural questions remain intentionally unresolved.
- User-facing GUI and broader adoption workflows are postponed.
