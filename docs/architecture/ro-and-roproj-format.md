# .ro and .roproj Format Concept

## Goal

Support users and Git workflows from one semantic model.

## Architectural target

ADR-0003 is Accepted and defines two roles:

- `project.roproj/` is the canonical editable/source representation optimized for Git, branches, CI, CLI tooling, and semantic operations.
- `project.ro` is the portable single-file artifact optimized for sharing, transport, archival, and ordinary open/save workflows.

The semantic model remains the architectural source of truth. Physical representations must preserve equivalent semantic meaning.

## Current implementation

The current CLI retains the single-file direct `.ro` persistence path:

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

Issue #123 also implements the independent production `.roproj/v1` pure codec
and the native standalone host workflow. `tachiko roproj materialize` converts
an explicit direct `.ro` input to a distinct absent canonical tree;
`tachiko roproj validate` is canonical-only; and `tachiko roproj canonicalize`
admits only the Accepted bounded family and writes a distinct absent canonical
output. These operations preserve their source and do not require Git.

This is a transitional implementation state, not a reversal of ADR-0003.
ADR-0023 fixes the `.roproj/v1` durable representation contract. ADR-0025
fixes the deterministic portable-package v1 envelope and integrity root over
that exact tree. The packaged `.ro` ZIP codec and CLI pack/unpack are not yet
implemented under #3. Optional Git/CI integration remains #44, and hostile
container/security plus broader race/durability work retain their existing
owners and Deferred status.

## Canonical Git working representation

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

## Target portable representation

Portable package v1 is a derived 19-entry, store-only ZIP32 envelope:

```text
project.ro
├── package.json
└── payload/              # exact canonical .roproj/v1 files
```

`package.json` selects `tachiko.portable-package/v1`, claims the
`tachiko.roproj/v1` payload, and records a path-separated SHA-256 root over all
18 exact payload files. The package adds no semantic DTO. Its fixed metadata,
entry order, lossless pack/unpack laws, content framing, and tracked-source
conflict behavior are normative in
[`portable-package-v1.md`](../specs/portable-package-v1.md).

When a verified package and a canonical tracked `.roproj` have different
payload roots, the tracked tree remains authoritative. Neither side is
automatically overwritten, synchronized, or merged.

## Rule

- Current product behavior must document direct `.ro` as the implemented ordinary persistence format and `.roproj/v1` as an implemented explicit canonical materialization/validation/canonicalization path.
- Architecture documents must document `.roproj` as the Accepted canonical editable target under ADR-0003.
- `.roproj/v1` documents must follow ADR-0023's Accepted physical and wire contract without treating paths, shard names, or line numbers as semantic identity.
- Portable package v1 implementations must consume ADR-0025's exact envelope and integrity contract without introducing another semantic schema.
- `.ro` packaging sophistication must not block semantic-core or user-workflow validation.
- The system provides explicit deterministic direct `.ro` → `.roproj/v1`
  materialization but does not yet provide packaged `.ro` ZIP pack/unpack or
  implicit ordinary-open conversion.
