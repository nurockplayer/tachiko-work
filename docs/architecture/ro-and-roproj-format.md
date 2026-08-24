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

It reads frozen direct JSON v1 through an explicit deterministic in-memory
migration and writes canonical identity-aware direct JSON v2. Stable IDs,
mutable keys, bound references, Unicode scalar sequences, and ADR-0018 Number
meaning survive canonical v2 round trips. Original numeric lexemes and the IEEE
negative-zero bit are not separate semantic meaning. Merely reading a legacy
file never rewrites it.
This path is used by the current CLI, semantic diff/merge, validation, formula
authoring, and product smoke journeys.

This is a transitional implementation state, not a reversal of ADR-0003.
ADR-0023 fixes the `.roproj/v1` durable representation contract, but its
production materializer and the deterministic `.roproj` ↔ `.ro` pack/unpack
path are not yet implemented.

## Target Git working representation

```text
project.roproj/
├── manifest.json
├── schemas.json
└── entities/
    ├── 0.jsonl
    ├── ...
    └── f.jsonl
```

The 16 entity shards are fixed and always present. Their names and record
placement are version-scoped materialization, never semantic identity. Bound
formulas remain inline; assets, shared views, semantic tests, caches, and
generated `.ro` remain outside the v1 canonical tree. The exact normative
layout and DTO contracts live in
[`roproj-layout-v1.md`](../specs/roproj-layout-v1.md) and
[`roproj-format.md`](../specs/roproj-format.md).

## Rule

- Current product behavior must document `.ro` as the implemented v0.1 persistence format.
- Architecture documents must document `.roproj` as the Accepted canonical editable target under ADR-0003.
- `.roproj/v1` documents must follow ADR-0023's Accepted physical and wire contract without treating paths, shard names, or line numbers as semantic identity.
- `.ro` packaging sophistication must not block semantic-core or user-workflow validation.
- The system does not yet provide deterministic `.ro` ↔ `.roproj` conversion.
