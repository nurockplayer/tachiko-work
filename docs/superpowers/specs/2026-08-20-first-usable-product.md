# First Usable Product Design

## Product outcome

A game developer can install Tachiko Work, create a meaningful project, discover
what it contains, understand a formula, make a safe typed balance change, review
its calculated impact, validate it, and export evaluated data without reading
the `.ro` specification.

## First-run journey

```text
tachiko init moonfall.ro
tachiko show moonfall.ro
tachiko explain moonfall.ro iron_sword.dps
tachiko set moonfall.ro iron_sword.damage 45 --output moonfall-buffed.ro
tachiko diff moonfall.ro moonfall-buffed.ro
tachiko validate moonfall-buffed.ro
tachiko export moonfall-buffed.ro moonfall.json
```

`init` prints a compact starter summary and the next two commands; it does not
duplicate the full overview in first-run output. `show` prints every entity with
its stable identifier and fields, distinguishing input values, typed references,
and calculated formulas. Typed references name their target schema. `explain`
works for both an input and a formula: inputs
list affected formulas; formulas list their expression and direct dependencies.

`set` accepts exactly one `entity.field` path and one scalar value. The field's
schema determines number, text, boolean, or reference parsing. It rejects
formula targets, missing paths, invalid booleans/numbers, broken references,
formula failures, no-op changes, input/output path equality, and existing output
files. On success it prints the same semantic diff users will review in Git.

## Architecture

`tachiko-workflow` sits above semantic-core, formula-engine, and diff-engine.
It owns the opinionated starter template and structured product operations, but
no filesystem or terminal behavior. The CLI remains a thin adapter that
loads/saves canonical documents and renders workflow results. `ai-api` remains
a parallel semantic adapter with its stricter AI permission contract.

Core semantic and storage crates remain unaware of templates and CLI concepts.
A future graphical interface may consume the workflow result structures without
reimplementing parsing, dependency explanations, or edit validation.

## Error and safety model

- Semantic identifiers use `[a-z0-9][a-z0-9_-]*`; `.` remains an unambiguous
  stable field-path separator.
- Creation and edit outputs use exclusive-create storage; no implicit overwrite.
- An edit is applied to a clone, fully validated, and calculated before it can be
  returned for persistence.
- Scalar `set` refuses computed fields so a user cannot accidentally erase a
  formula.
- Error messages name the semantic field path and expected type.
- All output collections use semantic identifier order.

## Demo and CI contract

The built-in game-balance starter must serialize byte-for-byte to the checked-in
`examples/game-balance/game-balance.ro`. CI runs formatting, clippy, all tests,
and the complete first-run journey. The semantic diff must show the direct sword
damage change and affected DPS.

## Deferred

- In-place editing and undo history
- Formula/schema authoring commands
- `.roproj` or project-directory packaging
- Graphical/TUI editing
- Engine-specific plugins
- Autonomous AI mutation
