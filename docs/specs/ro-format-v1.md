# Tachiko Work .ro Format v1

Decision state: Normative legacy compatibility / migration profile under ADR-0017; not the target long-term editable representation

Implementation state: Implemented immutable compatibility reader/writer and
migration source; the current semantic writer emits `direct-ro/v2`

## Authority note

This document freezes the historical direct `.ro` JSON version-1 behavior as a compatibility and migration source profile.

Its purpose is to preserve what the v0.1 reader/writer meant so later semantic-core and storage refactors cannot silently reinterpret old files. It is not authority for future `.roproj` layout, future `.ro` package/container design, or future semantic identity/numeric choices.

ADR-0017 requires complete storage-owned historical DTOs and explicit
migration. The implementation now satisfies that boundary: v1 decoding and
historical canonicalization never depend on current semantic-core Serde layouts.
This is not permission for the legacy wire contract to evolve.

ADR-0017 also places this profile behind the current normal direct-JSON
Stage-0 admission. Exactly 8 MiB is admitted and one byte more is rejected
before UTF-8/JSON inspection. This is an intentional normal-reader resource
boundary, not a change to the v1 wire meaning or a permanent semantic/document
maximum.

ADR-0003 remains the accepted representation direction: `.roproj` is the target canonical editable/source materialization and `.ro` becomes a derived portable artifact. The current direct `.ro` JSON persistence path is an implementation stage and a distinct representation/version namespace.

## Purpose

The version-1 direct `.ro` profile is the single-file JSON representation shipped by the Developer MVP.

It is retained so existing files can be decoded and migrated deterministically. It is not the primary semantic model.

## Historical goals

- Portable sharing
- Deterministic serialization
- Cross-platform compatibility
- Executable evidence for the semantic/storage boundary

## Accepted relationship with .roproj

`.ro` and `.roproj` represent the same logical semantic work under ADR-0003, but their physical/version namespaces are distinct.

- direct `.ro` v1 is an immutable legacy compatibility profile;
- future `.roproj` is the canonical editable/Git-native representation;
- future `.ro` packaging is owned by #43 and must not be inferred from this direct JSON v1 profile.

A future `.roproj` version `1` would not mean the same wire schema as direct `.ro` JSON `format_version: 1`.

## Complete version-1 wire schema

This section is the normative structural definition of the legacy direct-`.ro/v1` DTO. A storage implementation must own equivalent version-specific DTO types rather than reconstructing this contract from current `semantic-core` derives.

### General rules

- The root value is a JSON object.
- Every member listed below is required and non-null.
- The hardened compatibility decoder rejects unlisted members recursively. Historical parser permissiveness is not permission to add meaning to v1.
- JSON object member order does not affect decoding. The canonical v1 writer emits fixed members in the order shown below.
- Map-shaped stores are JSON objects whose member names are legacy identifier strings.
- Legacy identifiers use `[a-z0-9][a-z0-9_-]*`.
- Schema, entity, and field maps are emitted in ascending lexicographic order of their ASCII identifier bytes.
- Schema map member names must equal the nested schema `id`.
- Entity map member names must equal the nested entity `id`.
- Entity field-map member names must resolve to fields declared by the referenced schema.
- Strings are decoded as Unicode strings and preserved without normalization.
- Numeric values are finite historical `f64` values. Non-finite values are invalid. This legacy fact does not decide #24's future numeric semantic contract.
- The normal reader applies the shared Provisional 8 MiB direct-JSON input
  envelope before these structural rules. A future explicit legacy import or
  migration profile may use another finite bound; no unbounded bypass is part
  of this compatibility contract.

### `DocumentV1`

Canonical fixed-member order:

```json
{
  "format_version": 1,
  "id": "legacy-document-id",
  "title": "Document title",
  "schemas": {},
  "entities": {}
}
```

| Member | Type | Meaning |
| --- | --- | --- |
| `format_version` | JSON lexical integer | Exactly `1`. |
| `id` | legacy document identifier string | Historical document address. |
| `title` | string | Historical document title. |
| `schemas` | object from legacy schema ID to `SchemaV1` | Schema store. |
| `entities` | object from legacy entity ID to `EntityV1` | Entity store. |

### `SchemaV1`

Canonical fixed-member order:

```json
{
  "id": "legacy-schema-id",
  "fields": {}
}
```

| Member | Type |
| --- | --- |
| `id` | legacy schema identifier string |
| `fields` | object from legacy field ID to `FieldDefinitionV1` |

A field definition has no separate nested `id`; its legacy field identity/address is the `fields` object member name.

### `FieldDefinitionV1`

Canonical fixed-member order:

```json
{
  "field_type": {
    "type": "text"
  },
  "required": true
}
```

| Member | Type |
| --- | --- |
| `field_type` | `FieldTypeV1` |
| `required` | boolean |

### `FieldTypeV1`

The `type` discriminator is required and uses the following exact spellings.

```json
{"type": "number"}
```

```json
{"type": "text"}
```

```json
{"type": "boolean"}
```

Reference field types use canonical member order `type`, then `schema`:

```json
{
  "type": "reference",
  "schema": "legacy-schema-id"
}
```

No other v1 field-type discriminator is defined.

### `EntityV1`

Canonical fixed-member order:

```json
{
  "id": "legacy-entity-id",
  "schema": "legacy-schema-id",
  "fields": {}
}
```

| Member | Type |
| --- | --- |
| `id` | legacy entity identifier string |
| `schema` | legacy schema identifier string |
| `fields` | object from legacy field ID to `ValueV1` |

### `ValueV1`

`ValueV1` is an adjacently tagged object. Canonical fixed-member order is `kind`, then `value`.

Number:

```json
{
  "kind": "number",
  "value": 1.0
}
```

Text:

```json
{
  "kind": "text",
  "value": "text"
}
```

Boolean:

```json
{
  "kind": "boolean",
  "value": true
}
```

Entity reference:

```json
{
  "kind": "reference",
  "value": "legacy-entity-id"
}
```

Formula:

```json
{
  "kind": "formula",
  "value": {
    "op": "number",
    "args": 1.0
  }
}
```

No other v1 value discriminator is defined.

### `ExpressionV1`

`ExpressionV1` is an adjacently tagged object. Canonical fixed-member order is `op`, then `args`.

Numeric literal:

```json
{
  "op": "number",
  "args": 1.0
}
```

Field reference:

```json
{
  "op": "reference",
  "args": {
    "entity": "legacy-entity-id",
    "field": "legacy-field-id"
  }
}
```

`FieldRefV1` uses canonical member order `entity`, then `field`.

Binary expressions use an `args` object with canonical member order `left`, then `right`:

```json
{
  "op": "add",
  "args": {
    "left": {
      "op": "number",
      "args": 1.0
    },
    "right": {
      "op": "number",
      "args": 2.0
    }
  }
}
```

The exact v1 binary operator spellings are:

- `add`
- `subtract`
- `multiply`
- `divide`
- `minimum`
- `maximum`

No other v1 expression discriminator is defined.

## Complete typed-ID occurrence inventory

A legacy identity migration must account for every location below. Building maps for only top-level objects is insufficient.

| Wire location | Legacy ID kind | Migration requirement |
| --- | --- | --- |
| `DocumentV1.id` | document | Establish the new document ID and preserve the legacy value as an address where the hardened model provides one. |
| `DocumentV1.schemas` member name | schema | Rewrite the schema-store key. |
| `SchemaV1.id` | schema | Rewrite consistently with its store key. |
| `SchemaV1.fields` member name | field | Rewrite the field-definition key. |
| `FieldTypeV1.schema` for `type: reference` | schema | Rewrite the target schema reference. |
| `DocumentV1.entities` member name | entity | Rewrite the entity-store key. |
| `EntityV1.id` | entity | Rewrite consistently with its store key. |
| `EntityV1.schema` | schema | Rewrite the entity's schema relationship. |
| `EntityV1.fields` member name | field | Rewrite each stored-value field key through the referenced schema's field mapping. |
| `ValueV1.value` for `kind: reference` | entity | Rewrite the target entity reference. |
| `ExpressionV1.args.entity` for `op: reference` | entity | Rewrite the bound formula entity reference. |
| `ExpressionV1.args.field` for `op: reference` | field | Rewrite the bound formula field reference in the target entity's schema scope. |

Any implementation extension that discovers another typed-ID occurrence in the frozen v1 DTO must treat omission as a migration defect, not as permission to preserve a legacy address as a new stable ID.

## Canonical version-1 emission

The shipped canonical writer uses:

- UTF-8 without a BOM;
- two ASCII spaces per indentation level;
- LF structural newlines;
- no trailing whitespace;
- exactly one final LF;
- the fixed-member order documented above;
- lexicographically ordered legacy-ID maps.

Equivalent valid v1 semantic documents constructed in different insertion orders therefore produce identical canonical bytes.

For the legacy profile, numeric spelling is historical compatibility behavior. A hardened v1 writer must reproduce the committed canonical v1 fixture bytes and must not let a newer semantic-core derive or serializer upgrade silently alter them. #40 owns expansion of the executable fixture corpus; #24 owns future numeric meaning and final future-format numeric vectors.

Current canonical fixture anchors include:

- `examples/game-balance/game-balance.ro`
- `examples/game-balance/buffed-sword.ro`
- `crates/storage/tests/ro_format.rs`

The examples are representative, not an exhaustive list of optional variants. The complete discriminator and member definitions in this document remain normative even when a particular example does not exercise them.

## Historical and hardened reader behavior

The v0.1 implementation:

- requires `format_version: 1`;
- rejects unsupported versions rather than guessing later schemas;
- rejects malformed JSON;
- rejects unknown top-level document fields;
- validates the decoded semantic document.

ADR-0017 and `storage-versioning-and-migration.md` strengthen the compatibility reader boundary with:

- shared normal-profile admission before UTF-8 or JSON scanning;
- duplicate-member detection at every depth, including escaped-equivalent names;
- recursive version-specific DTO ownership;
- recursive unknown-member rejection;
- representation-local version dispatch;
- explicit migration.

Those safeguards constrain the hardened compatibility decoder. They do not
assign new meaning to bytes that were valid under the frozen v1 schema. The
normal profile intentionally declines otherwise-valid v1 input above 8 MiB;
that resource policy is separate from historical wire meaning.

## Compatibility and migration rule

The v1 wire contract is immutable compatibility input.

- semantic-core struct/Serde changes must not alter v1 decoding;
- a hardened reader must decode v1 through storage-owned legacy DTOs;
- reading/opening v1 must not implicitly rewrite durable source;
- migration to an identity-aware representation is explicit and deterministic where the source address permits;
- all typed-ID occurrences in the inventory above must be rewritten consistently;
- ambiguous, duplicate, mismatched, or unresolvable legacy data must fail rather than be guessed;
- v1 numeric behavior remains historical implementation compatibility and does not decide #24's future numeric semantic contract.
- ordinary reading, migration-in-memory, and legacy canonicalization share the
  finite normal direct-JSON admission profile;
- a future larger compatibility/import operation, if accepted, must expose a
  separate explicit finite profile rather than an unbounded or silent normal
  reader bypass.

If the direct `.ro` JSON representation evolves incompatibly before `.roproj` replaces it as the canonical working source, it must use a new version in the direct-`.ro` representation namespace. The next available value is `2`; this does not reserve `.roproj` version `2`.

## Current follow-up

- ADR-0017 — accepted storage/version/canonical boundary
- #74 — implementation parent for the versioned storage and canonical JSON pipeline
- #40 — golden and negative fixtures
- #70 — ADR-0015 identity migration integration
- #24 — future numeric semantics and final numeric canonical vectors
- #41 — `.roproj` physical layout
- #43 — future `.ro` package profile
