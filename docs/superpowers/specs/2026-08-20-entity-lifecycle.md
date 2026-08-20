# Semantic Entity Lifecycle Design

## Objective

Let a game designer expand and reorganize a balance roster through semantic,
reviewable commands instead of canonical JSON edits.

## Product workflow

The cohesive CLI surface is `tachiko entity` with `duplicate`, `rename`, and
`remove` subcommands. Each command reads one canonical document and must create
a distinct, previously absent output document.

A first-user roster journey can:

1. duplicate `iron_sword` as `steel_sword`;
2. use the existing `set` command to change its name and tuning inputs;
3. rename it to `moonblade` without breaking its self-referential DPS formula;
4. explain or export the renamed formula result;
5. receive actionable dependent paths when attempting to remove a referenced
   entity; and
6. remove an unreferenced entity safely.

## Workflow contract

All lifecycle functions return the existing `EditPreview { document, diff }`.
They do not perform filesystem I/O.

### Duplicate

`duplicate_entity(document, source, target)`:

- requires a present source, an absent target, and a valid target identifier;
- clones the entity, changes its intrinsic ID, and inserts it at the target key;
- recursively rebases formula `Expression::Reference` nodes whose entity is the
  source to the target;
- preserves stored `Value::Reference` relationships and formula references to
  every other entity;
- validates, calculates, and diffs the candidate.

### Rename

`rename_entity(document, source, target)`:

- rejects a no-op before checking occupancy;
- requires a present source, an absent target, and a valid target identifier;
- moves the entity to the new key and changes its intrinsic ID;
- rewrites all `Value::Reference(source)` values and all recursively nested
  formula field references to use the target;
- validates, calculates, and diffs the candidate.

### Remove

`remove_entity(document, entity)`:

- requires a present entity;
- scans every other entity for stored and formula references to the target;
- reports one stable, sorted `FieldRef` per dependent field when blocked;
- ignores references owned by the entity being removed;
- removes, validates, calculates, and diffs an unreferenced candidate.

### Errors

The workflow adds explicit variants for invalid identifiers, occupied targets,
no-op renames, and referenced removals. Existing missing-entity, validation,
calculation, and diff errors remain shared. Error rendering must name the
offending entity and, for removal, every dependent field path.

Identifier checking is a semantic-core contract. The existing lowercase ASCII
grammar is exposed as a small public predicate so lifecycle callers cannot drift
from document validation.

## CLI and persistence

`tachiko entity` is a nested Clap subcommand so entity authoring remains
discoverable without occupying ambiguous top-level verbs. Each operation:

- rejects identical input/output paths;
- loads through `tachiko-storage`;
- invokes exactly one workflow operation;
- serializes canonical output;
- uses exclusive creation and preserves existing files on every failure; and
- prints the semantic preview, the written path, and a useful next command.

## Verification

- Semantic-core unit coverage locks the public identifier predicate to document
  validation behavior.
- Workflow tests cover successful and failing duplicate, rename, and remove
  paths, including every expression node shape, external references, preserved
  calculations, stable dependent ordering, and self-reference removal.
- CLI process tests cover help, success, same-path refusal, occupied output,
  actionable dependency errors, and no-write failure behavior.
- `scripts/entity-lifecycle-smoke.sh` executes the complete game-balance journey
  against the compiled CLI and is required by CI and `release-check.sh`.
- Completion requires formatting, warnings-as-errors Clippy, all workspace
  tests, warning-free docs, exact Rust 1.85 checking, all Cargo packages, all
  product smokes, release archive verification, and independent review.

## Non-goals

- Schema, field, or formula authoring.
- Cascading deletion.
- In-place editing.
- Fuzzy identifier or free-text rewriting.
- `.roproj`, graphical UI, or Office-compatible behavior.
