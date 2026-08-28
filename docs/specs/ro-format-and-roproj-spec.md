# .ro and .roproj Format Direction

## Purpose

Tachiko Work requires representations that work for both ordinary file handling and Git-native collaboration.

ADR-0003 is Accepted and defines the target relationship:

- `.roproj` is the canonical editable/source materialization.
- `.ro` is the portable single-file artifact.
- the semantic model remains authoritative over both.

## Current implementation

The shipped CLI continues to use deterministic direct `.ro` files for ordinary
creation, authoring, validation, calculation, semantic diff/merge, and runtime
export workflows.

It also implements explicit standalone `.roproj/v1` operations: materializing
a direct `.ro` document to a distinct absent canonical tree, validating an
exact canonical tree, and canonicalizing the Accepted bounded non-canonical
family to a distinct absent output. These operations preserve their source and
do not require Git.

This implementation state does not supersede ADR-0003. ADR-0025 now fixes the
deterministic portable-package v1 and integrity contract over `.roproj/v1`;
Issue #3 implements its exact packaged `.ro` codec, bounded host pack/unpack,
read-only comparison, and CLI workflow. Optional Git/CI integration remains
Issue `#44`.

## `.ro`

The `.ro` filename currently carries the implemented direct-JSON v0.1
representation. It is also the Provisional filename for the separately
Accepted and implemented `tachiko.portable-package/v1` representation.
Content framing and representation-local version dispatch, not the extension,
distinguish them.

Current uses:

- direct CLI persistence
- sharing and transport
- deterministic review snapshots
- backup/archive
- runtime export source

Users should not need to understand its internal tagged JSON structure for ordinary workflows.

## `.roproj`

Accepted canonical Git working/source representation. ADR-0023 now fixes the
`.roproj/v1` physical and wire contract. Issue #123 implements the production
pure reader/writer codec plus native materialize, canonical-only validate, and
explicit bounded canonicalize operations. Issue #3 implements package
pack/unpack and comparison over that exact tree.

Representation properties:

- deterministic and canonical
- UTF-8 where textual
- human-readable where practical
- diff friendly
- merge friendly
- branch/PR friendly
- CI and semantic-tooling friendly

The canonical v1 tree is exactly:

```text
Project.roproj/
├── manifest.json
├── schemas.json
└── entities/
    ├── 0.jsonl
    ├── ...
    └── f.jsonl
```

All 16 lowercase entity shards exist even when empty. Entity placement and
ordering derive from the complete stable `EntityId`; no path, shard, or line
number is semantic identity. Bound formulas stay inline with their owning
entity field. Assets, views, tests, caches, and generated `.ro` artifacts are
outside the v1 canonical tree.

[`roproj-layout-v1.md`](roproj-layout-v1.md) defines the exact tree and
canonicalization boundary. [`roproj-format.md`](roproj-format.md) defines the
complete version-owned DTO contract. Later versions may change representation
layout through explicit migration without changing semantic identity.

## Portable package v1

ADR-0025 and [`portable-package-v1.md`](portable-package-v1.md) define the
portable artifact as one deterministic 19-entry, store-only ZIP32 envelope
containing `package.json` and the exact 18 `.roproj/v1` files under `payload/`.
The package manifest adds only representation dispatch and a path-separated
SHA-256 payload root; it does not define another semantic schema.

Pack and unpack preserve every payload path and byte exactly. A verified
package that disagrees with canonical tracked `.roproj` source reports a
source mismatch without mutating either side; the tracked source remains
authoritative in that working context. The packaged `.ro` ZIP codec and CLI
pack/unpack/compare workflow are implemented by #3; optional Git/CI integration
remains owned by #44.

## Canonical principle

Neither physical format owns meaning. The semantic model is authoritative, while ADR-0003 establishes `.roproj` as the canonical editable materialization and `.ro` as the portable artifact.

## Migration and interoperability

Legacy formats such as DOCX and XLSX belong at adapters/system boundaries. CSV, JSON, Markdown, OpenDocument, and other existing standards should be reused where useful rather than forcing legacy behavior into the semantic core.

The core must not inherit historical compatibility accidents.
