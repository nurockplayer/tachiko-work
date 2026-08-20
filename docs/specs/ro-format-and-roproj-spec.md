# .ro and .roproj Format Direction

## Purpose

Tachiko Work requires representations that work for both ordinary file handling and Git-native collaboration.

ADR-0003 is Accepted and defines the target relationship:

- `.roproj` is the canonical editable/source materialization.
- `.ro` is the portable single-file artifact.
- the semantic model remains authoritative over both.

## Current v0.1 implementation

The shipped CLI currently persists and operates on deterministic `.ro` files only.

`Project.ro` is used by the current creation, authoring, validation, calculation, semantic diff/merge, and runtime export workflows.

This implementation state does not supersede ADR-0003. `.roproj` materialization and deterministic pack/unpack are future implementation work.

## `.ro`

Implemented v0.1 representation.

Current uses:

- direct CLI persistence
- sharing and transport
- deterministic review snapshots
- backup/archive
- runtime export source

Users should not need to understand its internal tagged JSON structure for ordinary workflows.

## `.roproj`

Accepted target Git working/source representation, not yet implemented.

Target properties:

- deterministic and canonical
- UTF-8 where textual
- human-readable where practical
- diff friendly
- merge friendly
- branch/PR friendly
- CI and semantic-tooling friendly

Illustrative layout:

```text
Project.roproj/
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

## Canonical principle

Neither physical format owns meaning. The semantic model is authoritative, while ADR-0003 establishes `.roproj` as the canonical editable materialization and `.ro` as the portable artifact.

## Migration and interoperability

Legacy formats such as DOCX and XLSX belong at adapters/system boundaries. CSV, JSON, Markdown, OpenDocument, and other existing standards should be reused where useful rather than forcing legacy behavior into the semantic core.

The core must not inherit historical compatibility accidents.
