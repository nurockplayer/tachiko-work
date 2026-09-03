# ADR-0011: Semantic three-way merge

## Status

Accepted, with the DocumentId admission boundary amended by
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md).

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

The original v0.1 implementation applied that rule to document identity/title,
schema additions/removals and field definitions, entity additions/removals and
schema membership, and stored or computed field values. ADR-0031 subsequently
narrows exactly one part of that surface: `DocumentId` is now the continuing
semantic identity that admits one three-way reconciliation and is **not** a
mergeable direct facet. `base`, `left`, and `right` MUST therefore carry the same
`DocumentId`; a different-Document input is a contract/admission failure rather
than a one-sided identity change or `SemanticConflict`.

This amendment supersedes only ADR-0011's original treatment of `Document.id` as
an ordinary three-way-selected unit. Document title remains a direct merge facet,
and the other merge laws in this ADR remain Accepted. The current merge-engine
code that still applies ordinary three-way selection to `Document.id` is
implementation lag to be removed by the separately Ready ADR-0031 production
realization work; this authority change does not modify runtime code in place.

Conflict order follows stable semantic paths in the original implemented
contract. ADR-0031 separately replaces path-oriented conflict identity as public
protocol authority with typed stable targets, direct facets, and deterministic
Semantic Conflict v1 ordering; the current path form remains implementation
evidence until production realization catches up.

A conflict is an expected structured outcome, not a partially-written document.
A conflict-free candidate must pass semantic validation and complete formula
calculation. The CLI creates a new output exclusively and never overwrites an
input or existing file.

The initial release does not mutate Git configuration or implement a Git merge
driver. The model-level API is the prerequisite for that later adapter.

Broader protocol questions such as machine-readable delta transport, concrete
conflict DTO/codec mapping, future `.roproj` layout interactions, and
cross-version merge remain separate hardening or implementation work rather than
being implied by this ADR. ADR-0030 owns the Accepted canonical direct-state
Semantic Delta boundary; ADR-0031 owns the Accepted logical Semantic Conflict v1
boundary.

## Consequences

Positive:

- independent game-balance changes can merge without raw JSON conflict work;
- semantic conflicts preserve deterministic base/left/right evidence while
  ADR-0031 supplies stable typed conflict identity independent of paths;
- invalid combined meaning is rejected before a file enters the repository;
- future CLI, graphical, Git-driver, and AI adapters share one merge contract;
- a merge can no longer silently turn one semantic document identity into
  another, aligning merge continuity with ADR-0015 and ADR-0030.

Negative:

- the initial CLI requires explicit base/ours/theirs files;
- conflicts do not yet have an interactive resolver;
- adding the same new entity differently on both branches conflicts even when
  its fields might be mechanically combinable, because there is no shared
  intent to justify that merge;
- current production merge-engine behavior for changed `Document.id` is
  temporarily behind the Accepted ADR-0031 boundary until separate
  implementation work lands.
