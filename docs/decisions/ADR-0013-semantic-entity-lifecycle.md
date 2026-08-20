# ADR-0013: Validated semantic entity lifecycle

## Status

Accepted

## Context

Tachiko Work can create a useful game-balance document and safely edit existing
scalar fields, but a designer cannot add a second weapon, change an entity's
stable identifier, or remove obsolete content without editing canonical JSON.
That makes the first practical use case stop at tuning the starter roster.

The semantic model already distinguishes entity identifiers, typed entity
references, and field references inside formula expressions. Lifecycle changes
must preserve those meanings rather than operating as textual search and
replace.

## Alternatives considered

### Add a generic JSON patch command

This would expose every representation detail, would not communicate semantic
intent, and would make reference and formula safety the caller's responsibility.
It conflicts with the semantic-first product boundary.

### Add schema and formula authoring at the same time

Those are important authoring capabilities, but each needs its own grammar,
validation experience, and migration contract. Coupling them to entity
lifecycle would make a focused roster-growth workflow harder to ship and use.

### Add validated entity lifecycle operations

This lets a designer grow and reorganize a roster using the current schemas and
formula structures. The operations remain reusable by later graphical and AI
interfaces. This is the selected approach.

## Decision

The workflow layer owns three semantic entity operations:

1. **Duplicate** copies an existing entity under a new valid, unused identifier.
   Formula references from the copied entity to its own fields are rebased to
   the new identifier. Stored entity relationships and formula references to
   other entities are preserved.
2. **Rename** changes an entity's map key and intrinsic identifier, then rewrites
   every typed entity reference and every formula field reference that names the
   old identifier.
3. **Remove** deletes an entity only when no other entity has a stored or formula
   reference to it. References contained by the removed entity itself disappear
   with it and do not block removal. A rejected removal reports stable, sorted
   dependent field paths.

Every operation works on an immutable input, validates and calculates the full
candidate, and produces a semantic diff before returning it. Missing sources,
invalid or occupied target identifiers, no-op renames, and referenced removals
fail before persistence.

The CLI exposes the operations beneath `tachiko entity`:

```text
tachiko entity duplicate INPUT.ro SOURCE NEW_ID --output OUTPUT.ro
tachiko entity rename INPUT.ro OLD_ID NEW_ID --output OUTPUT.ro
tachiko entity remove INPUT.ro ID --output OUTPUT.ro
```

Like `set` and `merge`, lifecycle commands exclusively create a new output and
never overwrite their input or an existing path. An explicit human invocation
is mutation approval under ADR-0007. AI systems may propose these commands, but
must not execute an unapproved write through the read-only AI API.

## Consequences

Positive:

- A designer can grow the game-balance roster without understanding `.ro` JSON.
- Rename is relationship-safe because it operates on typed references and
  formula AST nodes rather than text.
- Removal failures explain exactly what must be migrated first.
- The workflow operations are UI-independent and deterministic.

Negative:

- Duplicate retains the source's display name until the user changes it with
  `set`; this keeps the operation predictable but creates a deliberate follow-up.
- Removal does not cascade and rename does not infer new human-facing names.
- Entity lifecycle does not create schemas, fields, or new formula expressions.

## Deferred work

- Guided creation from schema defaults.
- Schema and field lifecycle.
- Formula authoring and editing syntax.
- Cascading removal or interactive reference migration.
- Rename inference inside untyped free-form text.
