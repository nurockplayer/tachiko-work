# .roproj Format Specification (Accepted Direction, Not Yet Implemented)

## Purpose

ADR-0003 is Accepted and defines `.roproj` as Tachiko Work's canonical editable/source representation for Git-native work.

The current v0.1 CLI does not yet implement `.roproj`; it uses deterministic `.ro` files as a transitional persistence path while the semantic model and authoring workflows stabilize.

## Target goals

`.roproj` must provide:

- deterministic serialization
- human-readable changes where practical
- UTF-8 textual materialization
- diff-friendly structure
- merge-friendly stable identifiers
- branch/PR-friendly workflows
- CI and semantic-tooling integration

## Illustrative layout

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

The exact physical layout remains evolvable until implementation.

## Git workflow

Current v0.1 users already exercise branch/edit/semantic-merge/review/validate flows through `.ro` files.

The target ADR-0003 workflow moves the canonical editable materialization to `.roproj`, with `.ro` generated as a portable artifact rather than maintained as an independent source of truth.

## Canonical ordering

Materialization must define stable ordering, normalized formatting, and deterministic output so equivalent semantic documents produce equivalent Git history.

## Design principle

Git remains a storage and collaboration protocol, not the user interface. Users edit through Tachiko Work semantic operations; Git stores reviewable semantic history.
