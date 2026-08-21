# Event Sourcing Model

Decision state: Hypothesis / Open Question

Implementation state: Not implemented in v0.1

Decision owners: #12 and, for later snapshot/history details, #49

## Authority note

Event sourcing is not an Accepted Tachiko Work architecture decision.

The project has accepted semantic change, deterministic state, Git-native review, and meaningful history as important directions. Whether event sourcing should become a persistence model, an optional history technique, or remain unnecessary is still unresolved.

This document preserves the conceptual model and benefits worth evaluating. It must not be used as authority to make the semantic core or `.ro` / `.roproj` persistence depend on replaying an event stream.

## Hypothesis

A Tachiko Work document may be explainable as the result of applying semantic events:

```text
Initial State
      |
      v
Semantic Events
      |
      v
Current Document State
```

Potential benefits to investigate include:

- semantic history
- reproducible states
- collaboration support
- AI reasoning over changes
- debugging and auditability

## Relationship with Git

Git already records repository history.

A future Tachiko semantic history layer could record domain-level intent or applied semantic changes that raw Git history cannot express directly.

Whether those histories are authoritative, optional, derived, checkpointed, compacted, or reconstructable is an Open Question owned by #12/#49.

## Constraints already accepted

Any future history design must preserve these existing constraints:

- current semantic state remains meaningful independently of raw storage representation;
- Git is storage/collaboration infrastructure, not the semantic model;
- event sourcing and CRDT are not current MVP dependencies;
- history machinery must not silently redefine document meaning;
- collaboration convenience must not require silent loss of disputed human intent.
