# Tachiko Work `.roproj/v2` wire DTO specification

Decision state: Accepted target under [ADR-0037](../decisions/ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md)

Implementation state: Not implemented. This versioned DTO contract grants no
production storage, runtime, CLI, Designer, API, or package implementation
authority.

Representation namespace: `.roproj/v2`

Authority: [ADR-0037](../decisions/ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md),
constrained by ADR-0015, ADR-0017, ADR-0018, ADR-0019, ADR-0023, and ADR-0036

Physical tree authority: [`.roproj/v2` layout](roproj-layout-v2.md)

## Purpose

This specification owns the complete logical wire DTOs and canonical JSON
spelling for `.roproj/v2`. The layout specification owns the fixed directory
tree, filenames, entity-shard placement, and allowed physical input forms.
Neither paths nor record positions are semantic identity.

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

## Representation ownership

`.roproj/v2` is its own representation namespace. Its `format_version` value
`2` does not select `legacy-direct-ro/v1`, `direct-ro/v2`, a semantic-model
version, or the distinct Accepted `tachiko.portable-package/v1` profile.

These DTOs are owned by `.roproj/v2`. They are independent of:

- semantic-core Rust structs and enum layouts;
- Rust field declaration order, `serde` derives, attributes, defaults, and
  collection implementations;
- the `direct-ro/v2` DTOs, even where this version deliberately uses the same
  logical tag spellings; and
- the JavaScript probe DTOs in the Issue #41 research record.

A change to one of those implementations or representations MUST NOT change
`.roproj/v2` bytes. An incompatible change to the DTOs below requires another
`.roproj` representation version and an explicit migration.

## Closed-world and presence rules

Every member listed for a DTO below is required, non-null, and always emitted.
No member is optional, nullable, defaulted, or an extension point. A reader
MUST NOT synthesize an omitted member from a language or serializer default.
Unknown members are rejected at every recursive object depth. Unknown tags are
rejected rather than ignored, preserved, or guessed.

`required: false` is still emitted on every field definition. It controls
schema-instance conformance; it is not a wire-omission instruction.

Empty arrays and objects are emitted and accepted where the semantic state
permits them. In particular:

- a document with no schemas uses `[]` in `schemas.json`;
- a document with no definitions uses `[]` in `definitions.json`;
- a schema with no fields uses `"fields": []`;
- an entity's `fields` member always exists and may be `{}`; and
- a field declared with `required: false` and having no entity value is absent
  from the entity's `fields` object. There is no `null` or placeholder entry.

## Common JSON rules

All JSON values in `.roproj/v2` adopt the Tachiko
[canonical JSON profile](canonical-json-profile.md), with the version-owned
rules in this specification taking precedence where this specification is more
specific.

Canonical JSON uses UTF-8 without a BOM, preserves decoded Unicode scalar
sequences without normalization or case folding, and uses the profile's
deterministic string escaping. Lone surrogates and other invalid Unicode input
are rejected. Duplicate object member names are rejected at every depth after
JSON escape decoding, so `"a"` and `"\u0061"` are duplicates.

`manifest.json`, `schemas.json`, and `definitions.json` use two ASCII spaces per indentation level,
LF line endings, no trailing spaces or tabs, and exactly one final LF. Each
entity record is one compact JSON object with no structural whitespace,
followed by one LF. A nonempty entity shard therefore ends in exactly one LF;
an empty shard is exactly zero bytes. Blank JSONL records are invalid.

For the three pretty JSON files, `render(value, depth)` is the complete v2
renderer; it uses the canonical primitive token, string escaping, and declared
member order, with one indentation level equal to two ASCII spaces:

- a primitive token and an empty object or array render on the current line as
  that token, `{}`, or `[]`;
- a nonempty array renders `[` followed by LF, then each element on its own
  line as `depth + 1` indentation levels followed by
  `render(element, depth + 1)`, a comma after every element except the last,
  and LF; it then renders `depth` indentation levels followed by `]`;
- a nonempty object follows the same line and comma rules with `{` and `}`;
  each member line is `depth + 1` indentation levels, the canonical JSON
  string token for the member name, the two bytes `: `, and
  `render(member_value, depth + 1)`; and
- no other structural whitespace or blank line is emitted.

Each pretty file body is `render(root, 0)` followed by exactly one LF. Thus a
nested nonempty container's opening delimiter remains on its member or element
line, while its children and closing delimiter use the recursive indentation
above. This v2 rule is not a serializer pretty-printer default or an adoption
of the v1 tree; it merely uses the same fully specified rendering algorithm.

Canonical fixed-member order is declared for every object below. Arrays and
maps described as ID-ordered compare the decoded opaque ID strings after UTF-8
encoding, lexicographically as unsigned byte sequences. Ordering never uses a
human key, path, locale, insertion order, filesystem order, hash-map order, or
JSON source escape spelling.

Number values adopt ADR-0018's finite IEEE 754 binary64 semantics and the
canonical JSON profile's RFC 8785/ECMAScript shortest-roundtrip number token.
Both IEEE zero encodings have semantic value positive zero and emit as `0`.
NaN and infinities are not JSON Numbers or semantic Numbers. For an admitted
JSON number token, decoding treats the token as an exact decimal, rounds to
binary64 with round-to-nearest, ties-to-even, rejects a result that is
infinite, accepts finite subnormals and underflow to zero, and normalizes zero.
This reuse of the number primitive does not make `.roproj/v2` JCS.

## Stable ID tokens

`DocumentId`, `SchemaId`, `FieldId`, `EntityId`, and `KeyedGroupedSumDefinitionId` are encoded as JSON
strings. Every stable ID token MUST be nonempty. Otherwise, its decoded Unicode
scalar sequence is opaque to storage:

- UUID, UUIDv7, prefix, timestamp, or other generator syntax is not required;
- no business meaning may be inferred from its spelling;
- no normalization, folding, parsing, or regeneration is performed; and
- an ID remains independent of mutable keys, content, array position, shard,
  filename, and directory path.

ID types are not interchangeable merely because all five use strings. Every
location below declares the ID kind it contains.

## `manifest.json`

The manifest is the only `.roproj/v2` version envelope. Its complete canonical
shape and fixed member order are `format`, `format_version`, then `document`:

```json
{
  "format": "tachiko.roproj",
  "format_version": 2,
  "document": {
    "id": "opaque-document-id",
    "title": "Balance"
  }
}
```

The `document` object's fixed member order is `id`, then `title`.

- `format` MUST be the exact JSON string `"tachiko.roproj"`.
- `format_version` MUST be the lexical JSON integer token `2`. Alternate
  numeric spellings such as `2.0` and `2e0` are malformed versions, not v2.
- `document.id` is a `DocumentId`.
- `document.title` is the document title string.

No schema/entity counts, shard inventory, digest, timestamp, tool version,
path, or generated `.ro` metadata member exists in the v2 manifest.

Version selection occurs from this exact envelope before `schemas.json`, any
entity record, or `definitions.json` receives DTO or semantic
interpretation. A missing, malformed,
or unsupported envelope fails closed. An unsupported version's remaining tree
MUST NOT be semantically decoded, canonicalized, migrated, or rewritten.

## `schemas.json`

The root value is an array of `Schema` records. It is never a keyed object.
Records are in strictly increasing `Schema.id` order. Equal IDs are duplicates;
decreasing IDs are non-canonical ordering failures.

### `Schema`

The complete fixed member order is `id`, `key`, then `fields`:

```json
{
  "id": "opaque-schema-id",
  "key": "weapon",
  "fields": []
}
```

- `id` is a `SchemaId`.
- `key` is the mutable human-facing schema key.
- `fields` is an array of `FieldDefinition` records in strictly increasing
  `FieldDefinition.id` order.

Schema IDs MUST be unique across `schemas.json`. Field IDs MUST be unique
within their owning schema. Equal IDs in the same collection are
duplicate-identity failures; decreasing IDs are ordering failures.

### `FieldDefinition`

The complete fixed member order is `id`, `key`, `field_type`, then `required`:

```json
{
  "id": "opaque-field-id",
  "key": "damage",
  "field_type": {
    "type": "number"
  },
  "required": false
}
```

- `id` is a `FieldId`.
- `key` is the mutable human-facing field key.
- `field_type` is exactly one `FieldType` object.
- `required` is a JSON Boolean and is always present, including when false.

### `FieldType`

The complete tag set is `number`, `text`, `boolean`, and `reference`.

The scalar field-type objects have the sole member `type`:

```json
{"type":"number"}
{"type":"text"}
{"type":"boolean"}
```

The code blocks above illustrate logical object shapes compactly. Inside the
pretty `schemas.json` file, they use the file's normal indentation.

The reference field-type object's fixed member order is `type`, then `schema`:

```json
{
  "type": "reference",
  "schema": "opaque-target-schema-id"
}
```

`schema` is the target `SchemaId`. It is required only in the sense that it is
a required member of the `reference` variant; because every variant member is
required, there is no optional wire member. Scalar variants containing
`schema`, or a reference variant lacking it, are invalid.

## Entity JSONL record

Each JSONL value is one `Entity` record. Its complete fixed member order is
`id`, `key`, `schema`, then `fields`:

```json
{
  "id": "opaque-entity-id",
  "key": "iron_sword",
  "schema": "opaque-schema-id",
  "fields": {}
}
```

The code block is expanded for readability; canonical entity shard bytes use
the compact one-line form specified below.

- `id` is an `EntityId`.
- `key` is the mutable human-facing entity key.
- `schema` is the entity's `SchemaId`.
- `fields` is a required JSON object whose member names are stable `FieldId`
  tokens and whose member values are `Value` objects.

The `fields` member names are in strictly increasing unsigned-UTF-8 order.
Every member name MUST identify a field declared by the entity's schema.
Missing fields declared with `required: false` are absent. A field declared
with `required: true` MUST have a member. Empty `fields` is valid when all
fields are optional or the schema has no fields.

Entity IDs MUST be unique across every entity record in every shard. Entity
record ordering and shard placement are specified by the
[physical layout](roproj-layout-v2.md); neither is identity.

## `Value`

Every `Value` is an adjacently tagged object with fixed member order `kind`,
then `value`. The complete tag set and payload types are:

| `kind` | `value` payload |
| --- | --- |
| `number` | semantic `Number` encoded as a JSON number token |
| `text` | JSON string |
| `boolean` | JSON Boolean |
| `reference` | target `EntityId` string |
| `formula` | inline bound `Expression` object |

Complete scalar shapes are:

```json
{"kind":"number","value":40}
{"kind":"text","value":"Iron Sword"}
{"kind":"boolean","value":true}
{"kind":"reference","value":"opaque-target-entity-id"}
```

The formula shape is:

```json
{
  "kind": "formula",
  "value": {
    "op": "number",
    "args": 40
  }
}
```

The expanded formula block illustrates the logical shape. Within an entity
JSONL record every nested object is compact.

Reference values store stable identity, not an entity key. Formula values are
inline in the owning entity field. `.roproj/v2` defines no formula source-text
member and no separate `FormulaId`, formula record, or formula file.

## Bound `Expression`

Every expression is an adjacently tagged object with fixed member order `op`,
then `args`. The complete operator set is `number`, `reference`, `add`,
`subtract`, `multiply`, `divide`, `minimum`, and `maximum`.

### Number expression

For `op: "number"`, `args` is a semantic `Number` JSON token:

```json
{
  "op": "number",
  "args": 2
}
```

### Reference expression

For `op: "reference"`, `args` is an object with fixed member order `entity`,
then `field`:

```json
{
  "op": "reference",
  "args": {
    "entity": "opaque-target-entity-id",
    "field": "opaque-target-field-id"
  }
}
```

`entity` is an `EntityId` and `field` is a `FieldId`. Together they are the
bound stable field reference. Mutable `[entity-key.field-key]` authoring text
is a projection and is not persisted.

### Binary expressions

For each of `add`, `subtract`, `multiply`, `divide`, `minimum`, and `maximum`,
`args` is an object with fixed member order `left`, then `right`. Both members
contain one recursive `Expression`:

```json
{
  "op": "add",
  "args": {
    "left": {
      "op": "number",
      "args": 2
    },
    "right": {
      "op": "reference",
      "args": {
        "entity": "opaque-target-entity-id",
        "field": "opaque-target-field-id"
      }
    }
  }
}
```

An operator with the wrong `args` JSON type or members is invalid. Unary,
variadic, empty-array, null, or reordered binary argument shapes do not exist
in v2. Unknown members or operators are rejected at any recursive depth.

These spellings deliberately match the current logical vocabulary of
`direct-ro/v2`, but they are redeclared here in full and do not import or alias
that representation's DTO contract.

## `definitions.json`

The root value is an array of `KeyedGroupedSumDefinition` records. It is never
a keyed object. Records sort in strictly increasing decoded
`KeyedGroupedSumDefinitionId` order by unsigned UTF-8 bytes. Equal IDs are a
duplicate-identity failure; decreasing IDs are a canonical-ordering failure.
The empty array is the complete canonical representation of no definitions.

Each entry has exactly the fixed member order `id`, `orders`, then `products`:

```json
{
  "id": "opaque-definition-id",
  "orders": {
    "schema": "opaque-orders-schema-id",
    "lookup_key_field": "opaque-orders-key-field-id",
    "quantity_field": "opaque-orders-quantity-field-id"
  },
  "products": {
    "schema": "opaque-products-schema-id",
    "key_field": "opaque-products-key-field-id",
    "category_field": "opaque-products-category-field-id",
    "price_field": "opaque-products-price-field-id"
  }
}
```

- `id` is a nonempty `KeyedGroupedSumDefinitionId`.
- `orders` has fixed member order `schema`, `lookup_key_field`,
  `quantity_field`. `schema` is a `SchemaId`; both remaining values are
  `FieldId`s in that schema.
- `products` has fixed member order `schema`, `key_field`, `category_field`,
  `price_field`. `schema` is a `SchemaId`; all remaining values are `FieldId`s
  in that schema.

V2 has this sole definition record type; no kind discriminator, extension
area, other definition family, or operation parameter exists. The IDs bind
exactly the ADR-0036 Orders and Products roles. Conversion proves that the
Orders lookup key and Products key/category are Text fields, and that the
Orders quantity and Products price are Number fields. This DTO never stores
keys, labels, paths, source syntax, evaluated groups, candidate sets,
diagnostics, cache values, source revision/currentness, result-profile identity,
or export output. Such input is unknown closed-world data and is rejected.

## Decode, conversion, and validation contract

A `.roproj/v2` DTO is not itself a semantic `Document`. After the version has
been selected and all physical files have been admitted under the layout
contract, a conforming implementation performs the following logical stages:

1. Decode every JSON value with duplicate-member and recursive unknown-member
   rejection, exact tag/payload checking, required-member checking, and stable
   ID token checking.
2. Prove cross-record ID uniqueness and classify canonical ID ordering, entity
   record ordering, definition ordering, and placement under the physical-layout
   contract.
3. Resolve every stored stable-ID relationship and convert the complete
   version-owned DTO graph explicitly into one semantic candidate. No Rust
   struct deserialization shortcut, default insertion, key-based rebinding, or
   partial conversion is conforming.
4. Apply the operation's authoritative semantic validation gate under
   ADR-0019, including ADR-0018 formula structural, graph, and numeric rules.

At minimum, conversion and validation reject:

- an empty stable ID token, a duplicate schema/field/entity/definition ID, or use of an ID
  string in the wrong typed location;
- an out-of-order schema array, field array, entity sequence, or entity
  `fields` object when validating an already-canonical tree;
- a reference field type whose target schema does not exist;
- an entity whose schema does not exist;
- an undeclared entity field or a missing required entity field;
- a value whose kind does not match its declared field type: `number` or
  `formula` for a Number field, `text` for Text, `boolean` for Boolean, and
  `reference` for Reference;
- a reference value whose entity target does not exist or whose target entity
  does not belong to the schema declared by the reference field type;
- a formula reference whose entity or field does not exist, whose field does
  not belong to the referenced entity's schema, or whose target is not numeric;
- a bound formula that violates Accepted structural, dependency, cycle, or
  evaluation rules; and
- a definition with an absent target, field belonging to the wrong schema,
  non-Text/non-Number required role, or another failure of ADR-0036's admitted
  stable bindings; and
- any other intrinsic or schema-instance failure defined by the Accepted
  semantic contracts.

Human keys never repair or retarget a failing stable-ID reference. Unknown or
wrong-type data never becomes a partial semantic document. Failure leaves the
previously durable source unchanged; the production durability mechanism is
outside this specification.

The canonical writer accepts only a semantic state that satisfies the write
operation's validation gate, converts it explicitly to these version-owned
DTOs, orders every collection as specified, and emits the one canonical byte
tree. A semantic-core refactor or dependency-library update that changes the
bytes below is an implementation failure unless a new representation version
has been accepted.

Ordering or placement outside the canonical rules is a representation defect,
not a semantic mutation. The canonicalizer defined by the physical-layout
contract MAY admit only that contract's bounded non-canonical ordering and
placement family, then sort and place records after strict DTO decoding,
uniqueness proof, semantic conversion, and validation. Such accepted input is
not a canonical tree before fresh emission. This exception does not permit the
canonicalizer to repair duplicates, unknown members/tags, bad references,
wrong types, invalid definition bindings, or invalid semantic content.

The failure descriptions in this section specify required rejection classes,
not public diagnostic-code spellings or a total precedence among independent
failures. `.roproj` resource admission and its error precedence remain outside
v2's Accepted contract.

## Normative golden bytes

The following are wire conformance vectors. Text between each nonempty code
block's fences is encoded as UTF-8 exactly, including the single LF immediately
before the closing fence. There is no BOM, CR, trailing whitespace, or extra
final LF.

### Empty document

`manifest.json` is exactly 121 bytes:

```json
{
  "format": "tachiko.roproj",
  "format_version": 2,
  "document": {
    "id": "doc-empty",
    "title": "Empty"
  }
}
```

`schemas.json` is exactly three bytes, `0x5b 0x5d 0x0a`:

```json
[]
```

Every entity shard defined by the physical layout is a zero-byte file. A
zero-byte shard has no BOM, JSON value, blank line, or LF.

`definitions.json` is exactly three bytes, `0x5b 0x5d 0x0a`:

```json
[]
```

### Nonempty keyed grouped-sum definition

This canonical `definitions.json` vector has one record and one final LF. It
is semantically valid with an Orders schema `orders-schema` declaring
`orders-key` as Text and `orders-quantity` as Number, and a Products schema
`products-schema` declaring `products-key` and `products-category` as Text and
`products-price` as Number. Those referenced schema/field records remain
ordinary v2-owned `schemas.json` DTOs and are not a second definition shape.

```json
[
  {
    "id": "definition-orders-by-category",
    "orders": {
      "schema": "orders-schema",
      "lookup_key_field": "orders-key",
      "quantity_field": "orders-quantity"
    },
    "products": {
      "schema": "products-schema",
      "key_field": "products-key",
      "category_field": "products-category",
      "price_field": "products-price"
    }
  }
]
```

The member and nested-member order above is mandatory. A collection with two
or more records sorts by definition ID, not construction order, map insertion,
schema key, label, path, or evaluated result. The vector contains no kind,
cache, result, revision, or currentness member; adding one is a closed-world
failure, not a new interpretation.

### Nonempty schemas

This complete 1,071-byte `schemas.json` vector demonstrates stable-ID ordering,
every field type, and explicit emission of both `required: true` and
`required: false`:

```json
[
  {
    "id": "schema-character",
    "key": "character",
    "fields": [
      {
        "id": "field-active",
        "key": "active",
        "field_type": {
          "type": "boolean"
        },
        "required": false
      },
      {
        "id": "field-base",
        "key": "base",
        "field_type": {
          "type": "number"
        },
        "required": true
      },
      {
        "id": "field-name",
        "key": "name",
        "field_type": {
          "type": "text"
        },
        "required": true
      },
      {
        "id": "field-note",
        "key": "note",
        "field_type": {
          "type": "text"
        },
        "required": false
      },
      {
        "id": "field-power",
        "key": "power",
        "field_type": {
          "type": "number"
        },
        "required": false
      },
      {
        "id": "field-target",
        "key": "target",
        "field_type": {
          "type": "reference",
          "schema": "schema-character"
        },
        "required": false
      }
    ]
  }
]
```

### Compact entity line and final LF

For the schema above, the entity with ID `entity-a` belongs in the shard chosen
by the physical layout (currently `entities/6.jsonl`). Its shard content is the
following 432 bytes: one compact record plus exactly one final LF. `field-note`
is absent because it is optional; the required `fields` object itself is
present. Field members are ordered by stable `FieldId`:

```text
{"id":"entity-a","key":"hero","schema":"schema-character","fields":{"field-active":{"kind":"boolean","value":true},"field-base":{"kind":"number","value":40},"field-name":{"kind":"text","value":"Éowyn"},"field-power":{"kind":"formula","value":{"op":"add","args":{"left":{"op":"number","args":2},"right":{"op":"reference","args":{"entity":"entity-a","field":"field-base"}}}}},"field-target":{"kind":"reference","value":"entity-a"}}}
```

In the same tree, every other entity shard is exactly zero bytes. For example,
`entities/0.jsonl` has byte length `0`; it does not contain `[]`, `{}`, or LF.

## Explicitly out of scope

This specification does not define or accept:

- `.roproj` tree, file, line, string, member-count, or total-byte resource
  limits, or precedence between resource failures and other errors;
- a production reader, writer, canonicalizer, codec library, Rust type, or
  Serde configuration;
- host atomic replacement, fsync, locking, recovery, permissions, symlink-race,
  watcher, browser-storage, or other durability behavior;
- portable-package container, integrity, and pack/unpack rules, which are
  separately Accepted by ADR-0025 and
  [`portable-package-v1.md`](portable-package-v1.md);
- Git attributes, diff drivers, hooks, CI policy, repository operations, or
  generated `.ro` consistency policy;
- semantic delta, revision, or operation-log formats;
- three-way merge, conflict projection, or merge-driver behavior; or
- assets, shared views, semantic tests, caches, or other future categories.

In particular, the direct-JSON 8 MiB complete-input limit and 256-byte number
token limit do not apply to `.roproj/v2`. The probe's canonical-tree digest is
evidence only: it is not a manifest member, integrity claim, revision ID,
semantic identity, or normative `.roproj/v2` value.

## Related

- [ADR-0015: Stable semantic identity](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0017: Versioned storage DTOs](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018: Bound formulas and deterministic binary64](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019: Staged semantic validation](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0023: `.roproj/v1` canonical tree and sharding](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md)
- [ADR-0037: `.roproj/v2` keyed grouped-sum persistence](../decisions/ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md)
- [ADR-0025: Portable package v1](../decisions/ADR-0025-portable-package-v1.md)
- [`.roproj/v2` physical layout](roproj-layout-v2.md)
- [Portable package v1](portable-package-v1.md)
- [Tachiko canonical JSON profile](canonical-json-profile.md)
- [`direct-ro/v2` specification](ro-format-v2.md)
- [Issue #41 layout and sharding research](../research/2026-08-24-roproj-v1-layout-and-sharding.md)
