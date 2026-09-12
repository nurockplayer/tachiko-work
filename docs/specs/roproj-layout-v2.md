# Tachiko Work `.roproj/v2` layout and canonicalizer

Decision state: **Accepted**

Authority: [ADR-0037](../decisions/ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md),
[Issue #327](https://github.com/nurockplayer/tachiko-work/issues/327)

Implementation state: Implemented for the storage-owned canonical codec and
the bounded Designer save/reopen journey. This contract does not authorize a
portable-package v2, a public SDK/wire contract, or broader host/CLI behavior.

## Scope

This specification owns the `.roproj/v2` path vocabulary, canonical tree,
entity-shard placement, physical JSON/JSONL rules, and the bounded
non-canonical input family accepted by a canonicalizer. It does not define the
exact manifest, schema, entity, value, or formula DTO shapes. Those are
version-owned storage contracts specified by
[`roproj-format-v2.md`](roproj-format-v2.md), consistent with ADR-0017.

The semantic document remains the meaning authority. `.roproj` is its
canonical editable materialization under ADR-0003; generated `.ro` is a
separate derived portable artifact.

## Canonical tree

Every canonical `.roproj/v2` tree contains exactly these 19 regular files at
the shown paths. Its root contains only `manifest.json`, `schemas.json`,
`definitions.json`, and the required `entities/` directory; that directory contains exactly the 16
files shown and no subdirectories:

```text
project.roproj/
├── manifest.json
├── schemas.json
├── definitions.json
└── entities/
    ├── 0.jsonl
    ├── 1.jsonl
    ├── 2.jsonl
    ├── 3.jsonl
    ├── 4.jsonl
    ├── 5.jsonl
    ├── 6.jsonl
    ├── 7.jsonl
    ├── 8.jsonl
    ├── 9.jsonl
    ├── a.jsonl
    ├── b.jsonl
    ├── c.jsonl
    ├── d.jsonl
    ├── e.jsonl
    └── f.jsonl
```

The root name and host path are not semantic data. Every shard file exists;
when it has no records it is exactly zero bytes. `definitions.json` always
exists; an empty canonical definition collection is exactly `[]\n`. Canonical v2 has no other
root children, no nested canonical entity directories, and no optional shard
inventory.

`manifest.json` is the only version envelope. Its version dispatch completes
before a reader decodes `schemas.json`, `definitions.json`, or any entity record. A missing,
malformed, or unsupported manifest version fails closed; it is not decoded as
semantic data, normalized, migrated, or rewritten.

## Canonical placement and ordering

For a decoded stable `EntityId` string `id`, compute:

```text
bucket = first lowercase hexadecimal nibble of SHA-256(UTF-8(id))
path   = entities/<bucket>.jsonl
```

`UTF-8(id)` encodes the exact decoded Unicode scalar sequence. Implementations
MUST NOT normalize, case-fold, hash raw JSON spelling, or derive placement
from a human key. SHA-256 is only the v2 placement function: it is neither
semantic identity nor an integrity, revision, content-address, or security
claim.

Each shard sorts records by the complete decoded `EntityId` using unsigned
UTF-8 byte lexicographic order. The same unsigned-byte full-ID order is used
for unordered schema and field collections. No sort uses a hash prefix,
locale, path, mutable name/key, or construction order. Semantically ordered
sequences retain their semantic order.

## Identity and category boundaries

The stable ID inside a decoded DTO is the identity authority. Paths, root
names, shard names, filenames, placement hashes, line numbers, record order,
and whitespace MUST NOT identify, rename, or retarget an object. A record in a
wrong input shard is a layout defect, not an object move; canonicalization
returns it to its required shard.

Formulas are inline in their owning entity field value. `.roproj/v2` has no
`formulas/` directory and does not create a path-derived `FormulaId`.

Assets, views, semantic tests, local caches/indexes, and generated `.ro` are
outside the v2 canonical tree. Consequently, canonical materialization rejects
`assets/`, `views/`, `tests/`, `formulas/`, cache directories, and every other
unrecognized child. This does not assign their future semantics or locations.

## Canonical JSON and JSONL bytes

All values use the canonical JSON profile and member order of the applicable
`.roproj/v2` DTO contract. Canonical bytes are UTF-8 without BOM, use LF only,
preserve decoded Unicode scalar sequences without normalization/case folding,
and reject duplicate members after JSON escape decoding. The closed-world v2
DTO rejects unknown members recursively.

- `manifest.json`, `schemas.json`, and `definitions.json` are two-space-indented canonical JSON and
  end with exactly one LF.
- An entity record is one compact canonical JSON object on one JSONL line.
- A nonempty shard has one record per line and ends with exactly one LF.
- A canonical empty shard is zero bytes.
- Blank JSONL records are invalid; canonical records have no trailing
  whitespace.

DTO-level requirements, field values, formula encodings, tags, required and
optional fields, null/default/omission behavior, and record member order are
deliberately specified in `roproj-format-v2.md`; this layout specification does
not duplicate or replace that version-owned boundary.

## Bounded non-canonical input and canonicalization

A `.roproj/v2` canonicalizer accepts a bounded non-canonical input family, not
arbitrary directory discovery:

1. `manifest.json`, `schemas.json`, `definitions.json`, and an ordinary `entities/` directory are
   required at their exact locations.
2. After manifest-first v2 selection, `manifest.json`, `schemas.json`, and
   `definitions.json` may use non-canonical structural whitespace,
   object-member order, legal string/token spelling, and schema/field/definition
   stable-ID order. Required members,
   the lexical version token, duplicates, unknown members, and DTO meaning
   remain strict.
3. Regular `*.jsonl` files beneath `entities/` may have non-canonical names,
   nesting, shard placement, record order, object-member order, legal
   string/token spelling, or RFC 8259 JSON whitespace other than LF within a
   physical record. Each record is exactly one JSON object terminated by one
   LF. A physical LF cannot occur within the record; blank records, multiple
   values on one record, and every other inter-record byte are invalid.
4. Missing canonical empty shards and extra empty JSONL input files are
   admissible.
5. Ordinary directories below `entities/` are admitted only as ancestors of
   at least one accepted regular `*.jsonl` file. Empty directories and
   directories without such a descendant are rejected.
6. Symlinks, non-regular non-directory entries, unknown top-level children,
   and non-JSONL files below `entities/` are rejected rather than followed or
   ignored.
7. Duplicate JSON members, blank records, unknown DTO members, duplicate
   schema IDs, duplicate field IDs within an owning schema, duplicate entity
   IDs across all entity inputs, duplicate definition IDs, and invalid DTO or semantic content fail
   closed. Equal strings used for different declared ID types are not a
   cross-type duplicate.

Canonicalization proceeds in this order:

```text
manifest-first version dispatch
  -> strict version-owned DTO decode
  -> SchemaId uniqueness across schemas
  -> FieldId uniqueness within each owning schema
  -> EntityId uniqueness across all entity inputs
  -> KeyedGroupedSumDefinitionId uniqueness across definitions
  -> semantic aggregate conversion
  -> applicable Accepted validation gate
  -> fresh canonical v2 tree
```

Opening or inspecting accepted non-canonical input does not itself authorize a
durable rewrite. Whether ordinary open may admit this bounded family or only an
explicit canonicalize/import operation may do so remains deferred.

## Deferred work

This specification does not define production codec implementation, resource
or error profiles, exact error precedence, host atomic save/locking/recovery,
or symlink-race defenses. Portable-package and integrity rules are separately
Accepted by ADR-0025 and
[`portable-package-v1.md`](portable-package-v1.md), while their production
implementation exists for package v1's v1-only payload. This specification defers Git/CI
integration (#44), semantic delta (#45), semantic merge (#46), future
assets/views/tests semantics, schema sharding, and a different fanout or
adaptive sharding strategy. Such changes require their own Accepted decision
or a later representation version.

## Evidence

V2 retains the fixed 16-way entity-shard placement chosen by the v1 research;
the only v2 physical addition is the fixed root `definitions.json` DTO. The
new file is necessary to persist ADR-0036's sole admitted definition family;
it is not evidence for generic definitions or another category.
