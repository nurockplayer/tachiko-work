# ADR-0011: Semantic three-way merge

## Status

Accepted

## Context

The first usable product can create, explain, edit, validate, calculate, export,
and semantically compare game-balance documents. External teams still have to
resolve concurrent `.ro` changes as raw JSON, even when two branches changed
independent semantic fields.

The game-development wedge requires semantic merge as a core workflow capability.
ADR-0005 is the current authority for that wedge; ADR-0002 is retained as its
superseded historical predecessor.

The earlier Developer MVP work deferred deeper merge behavior until the semantic
model and user workflow were validated; that condition was met by the verified
`393bc69` product checkpoint.

## Alternatives considered

### Implement `.roproj` first

A directory representation can reduce textual conflicts, but it cannot decide
whether two changes have compatible meaning. ADR-0003 is Accepted and defines
`.roproj` as the target canonical editable/source representation, but `.roproj`
materialization is not implemented in the v0.1 workflow. Adding a second physical
representation before proving merge semantics would widen storage without
closing the collaboration loop.

### Configure a textual Git merge driver

Canonical JSON makes textual merge deterministic, but line-level merge cannot
understand typed fields, formulas, references, or post-merge calculation. It can
silently produce semantically invalid documents.

### Add model-level three-way merge

Merge the common ancestor, current branch, and other branch at semantic field
boundaries, then validate and calculate the result before persistence. This is
the selected approach.

## Decision

Add a UI-independent `tachiko-merge-engine` crate and a safe CLI command:

```text
tachiko merge BASE OURS THEIRS --output MERGED.ro
```

The merge engine applies the standard three-way rule at each semantic unit:

- identical branch values are accepted;
- a value changed on only one branch is accepted;
- different changes to the same unit produce a typed conflict;
- independent field changes inside the same existing schema or entity merge;
- delete-versus-modify and different concurrent additions conflict.

Merge covers document identity/title, schema additions/removals and field
definitions, entity additions/removals and schema membership, and stored or
computed field values. Conflict order follows stable semantic paths in the
current implemented contract.

A conflict is an expected structured outcome, not a partially-written document.
A conflict-free candidate must pass semantic validation and complete formula
calculation. The CLI creates a new output exclusively and never overwrites an
input or existing file.

The initial release does not mutate Git configuration or implement a Git merge
driver. The model-level API is the prerequisite for that later adapter.

Broader protocol questions such as versioned machine-readable deltas, durable
conflict identity, future `.roproj` layout interactions, and cross-version merge
remain separate hardening work rather than being implied by this ADR.

## Consequences

Positive:

- independent game-balance changes can merge without raw JSON conflict work;
- conflicts name the exact semantic path and preserve base/ours/theirs values;
- invalid combined meaning is rejected before a file enters the repository;
- future CLI, graphical, Git-driver, and AI adapters share one merge contract.

Negative:

- the initial CLI requires explicit base/ours/theirs files;
- conflicts do not yet have an interactive resolver;
- adding the same new entity differently on both branches conflicts even when
  its fields might be mechanically combinable, because there is no shared
  intent to justify that merge.
