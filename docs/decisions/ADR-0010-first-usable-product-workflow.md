# ADR-0010: First usable product workflow

## Status

Accepted

## Context

The developer MVP proves the semantic architecture, but it does not yet provide
a complete first-user journey. `tachiko init` creates an empty document, CLI
help does not explain what commands do, and users must understand the `.ro` wire
format before they can reach calculation, semantic diff, or explanation.

ADR-0008 and ADR-0009 make usability refinement the current milestone. They
explicitly prioritize a workflow a technical designer can understand without
learning the internal architecture.

## Alternatives considered

### Build a web or spreadsheet-style UI now

This would improve approachability, but it would also introduce rendering,
application state, packaging, and interaction infrastructure before the
authoring workflow is stable. It is deferred.

### Keep `.ro` hand-authored and improve documentation only

Documentation cannot make a verbose tagged expression tree into a safe editing
experience. This would leave the largest first-user obstacle intact.

### Add a guided semantic CLI workflow

This closes the creation-to-result loop using the existing semantic core and
produces operations a later UI can reuse. This is the selected approach.

## Decision

The first usable product remains CLI-first and adds a reusable
`tachiko-workflow` crate for user-facing semantic operations.

The product workflow is:

1. `tachiko init balance.ro` creates a meaningful game-balance starter by
   default. `--template empty` remains available for advanced users.
2. `tachiko show balance.ro` reveals entities, stable field paths, inputs,
   references, formulas, and calculated values.
3. `tachiko explain balance.ro iron_sword.dps` explains dependencies and impact.
4. `tachiko set balance.ro iron_sword.damage 45 --output buffed.ro` parses the
   value from schema type, validates all references/formulas, writes a new
   canonical document, and prints the semantic impact.
5. Existing `validate`, `calculate`, `diff`, and `export` complete the review and
   integration workflow.

`set` never overwrites its input or an existing output. Formula fields cannot be
replaced through scalar `set`; users edit their dependencies instead. A future
formula-authoring command will require a separate design.

Human invocation of `set` is explicit mutation approval and therefore preserves
ADR-0007. AI suggestions remain inert until a caller chooses an approved write
path.

The checked-in example workflow becomes a CI contract. Project-directory and
`.roproj` decisions remain deferred until ADR-0003 is resolved.

## Consequences

Positive:

- A first user reaches a meaningful computed result without editing wire JSON.
- Stable field paths make CLI, Git review, documentation, and future UI concepts
  consistent.
- Mutation stays typed, validated, deterministic, and reviewable.
- The workflow crate provides a UI-independent product boundary.

Negative:

- The built-in starter is intentionally opinionated toward game balance.
- Schema and formula authoring remain advanced workflows for this milestone.
- Output-copy editing is safer but less direct than in-place editing.
