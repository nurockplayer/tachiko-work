# Event Sourcing Model

Decision state: Mixed — core event sourcing Rejected by ADR-0029; optional techniques Open Question

Implementation state: Not implemented in v0.1

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)

Decision owner for optional history mechanics: #49

## Authority note

Event sourcing is not Tachiko Work's core persistence model.

ADR-0029 accepts current semantic state and complete standalone snapshots as
authoritative. A retained event stream is not the system of record, and a
snapshot is not merely a replay optimization. Whether an optional history
profile should use event-sourcing techniques for a declared bounded guarantee
remains unresolved.

This document preserves the conceptual model and benefits worth evaluating. It must not be used as authority to make the semantic core or `.ro` / `.roproj` persistence depend on replaying an event stream.

## Optional-profile hypothesis

An optional Tachiko history profile may make one state transition explainable
as the result of applying retained semantic events from a declared complete
checkpoint:

```text
Declared complete checkpoint
      |
      v
Complete retained event tail
      |
      v
Verified state equal to the authoritative snapshot
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

Those histories must remain optional and non-authoritative. Their retention,
checkpoint, compaction, and reconstruction guarantees are Open Questions owned
by #49.

## Constraints already accepted

Any future history design must preserve these existing constraints:

- current semantic state remains meaningful independently of raw storage representation;
- Git is storage/collaboration infrastructure, not the semantic model;
- event sourcing and CRDT are not current MVP dependencies;
- history machinery must not silently redefine document meaning;
- collaboration convenience must not require silent loss of disputed human intent.
