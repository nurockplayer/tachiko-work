# .ro and .roproj Format Concept

## Goal

Support users and Git workflows from one semantic model.

## Architectural target

ADR-0003 is Accepted and defines two roles:

- `project.roproj/` is the canonical editable/source representation optimized for Git, branches, CI, CLI tooling, and semantic operations.
- `project.ro` is the portable single-file artifact optimized for sharing, transport, archival, and ordinary open/save workflows.

The semantic model remains the architectural source of truth. Physical representations must preserve equivalent semantic meaning.

## Current v0.1 implementation

The current CLI implements only the single-file `.ro` persistence path:

```text
project.ro
```

It provides deterministic parsing/serialization and is used by the current CLI, semantic diff/merge, validation, formula authoring, and product smoke journeys.

This is a transitional implementation state, not a reversal of ADR-0003. The canonical `.roproj` editable materialization and deterministic `.roproj` ↔ `.ro` pack/unpack path are not yet implemented.

## Target Git working representation

```text
project.roproj/
├── manifest.json
├── schema.json
├── data/
│   ├── enemies.jsonl
│   └── weapons.jsonl
├── formulas/
├── views/
└── tests/
```

The exact physical layout remains evolvable until implementation, but it must satisfy ADR-0003 requirements for deterministic, Git-friendly semantic materialization.

## Rule

- Current product behavior must document `.ro` as the implemented v0.1 persistence format.
- Architecture documents must document `.roproj` as the Accepted canonical editable target under ADR-0003.
- `.ro` packaging sophistication must not block semantic-core or user-workflow validation.
- The system does not yet provide deterministic `.ro` ↔ `.roproj` conversion.
