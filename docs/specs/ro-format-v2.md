# Tachiko Work direct `.ro` JSON v2

Decision state: Normative current direct-`.ro` representation under ADR-0015,
ADR-0017, and ADR-0018

Implementation state: Implemented by the stable-identity transition in #70

Representation namespace: `direct-ro/v2`

## Purpose

Version 2 is the identity-aware successor to frozen `legacy-direct-ro/v1`. It
persists opaque stable semantic IDs separately from mutable human keys and
stores formula/reference targets by stable ID. It is not `.roproj/v2`, a global
format version, or the future packaged `.ro` profile.

## Reader and writer contract

- `format_version` is the lexical JSON integer `2`.
- The reader accepts valid non-canonical JSON spellings but rejects duplicate
  members after escape decoding, recursively unknown members, malformed or
  unsupported versions, incoherent ID stores, and invalid semantic graphs.
- The canonical writer emits UTF-8, two-space indentation, LF line endings, no
  trailing whitespace, and exactly one final LF.
- Fixed record members use the order documented below.
- Schema, field, and entity stores are ordered by opaque stable-ID token using
  unsigned UTF-8 byte order. Mutable keys never determine collection order.
- Strings preserve their decoded Unicode scalar sequence. The reader and writer
  perform no Unicode normalization or case folding.
- Stable-ID tokens are opaque nonempty strings. Generic decoding does not
  require UUID syntax and does not expose UUID timestamp bits as meaning.

## Resource admission and Number conversion

The Provisional `direct-ro/v2` profile limits are:

| Resource | Limit |
| --- | ---: |
| Complete UTF-8 input | 8 MiB (`8,388,608` bytes) |
| One RFC 8259 number token | 256 bytes |

After strict JSON/version inspection selects v2, both limits are applied before
the v2 DTO converts a decimal token to semantic `Number`. Exactly-at-limit input
is admitted; one byte more is `storage.resource_limit`, not a numeric overflow
or underflow.

Every admitted number converts to nearest finite IEEE 754 binary64 using
round-to-nearest, ties-to-even. Infinity is invalid, finite subnormals and
correctly rounded underflow are valid, and either zero sign becomes semantic
positive zero. Canonical output uses the RFC 8785 §3.2.2.3 / ECMAScript
`Number::toString` shortest-roundtrip token only; this does not make the full
document JCS.

## Complete wire records

### Document

Fixed member order is `format_version`, `id`, `title`, `schemas`, `entities`:

```json
{
  "format_version": 2,
  "id": "opaque-document-id",
  "title": "Balance",
  "schemas": {},
  "entities": {}
}
```

### Schema and field definition

Schema member order is `id`, `key`, `fields`. The schema store member name must
equal nested `id`.

```json
{
  "id": "opaque-schema-id",
  "key": "weapons",
  "fields": {}
}
```

Field-definition member order is `id`, `key`, `field_type`, `required`. The
field store member name must equal nested `id`.

```json
{
  "id": "opaque-field-id",
  "key": "damage",
  "field_type": {
    "type": "number"
  },
  "required": true
}
```

Field-type discriminators are `number`, `text`, `boolean`, and `reference`.
Reference member order is `type`, then `schema`; `schema` stores the target
stable `SchemaId`:

```json
{
  "type": "reference",
  "schema": "opaque-target-schema-id"
}
```

### Entity

Entity member order is `id`, `key`, `schema`, `fields`. The entity store member
name must equal nested `id`; `schema` stores a stable `SchemaId`; field-store
member names are stable `FieldId` tokens declared by that schema.

```json
{
  "id": "opaque-entity-id",
  "key": "iron_sword",
  "schema": "opaque-schema-id",
  "fields": {}
}
```

### Values

Values retain v1's adjacent `kind`, `value` record order and exact
discriminators: `number`, `text`, `boolean`, `reference`, and `formula`.
Reference values store a stable target `EntityId`:

```json
{
  "kind": "reference",
  "value": "opaque-target-entity-id"
}
```

### Bound expressions

Expressions retain adjacent `op`, `args` order. Operators are `number`,
`reference`, `add`, `subtract`, `multiply`, `divide`, `minimum`, and `maximum`.
Binary `args` member order is `left`, then `right`.

A formula reference stores stable IDs, not the human `[entity.field]` address:

```json
{
  "op": "reference",
  "args": {
    "entity": "opaque-target-entity-id",
    "field": "opaque-target-field-id"
  }
}
```

Human source is projected on demand by resolving those IDs through current
entity and field keys and proving the address round-trips to the same IDs.

## Legacy migration

`legacy-direct-ro/v1` remains immutable compatibility input. Reading a v1 file
may create the explicit version-labelled v2 candidate in memory for the current
operation, but never rewrites the durable source. Any subsequently saved
semantic result is fully validated and emitted as v2.

The deterministic UUIDv5 namespace, byte construction, schema-scoped field
mapping, golden vectors, and complete 12-location rewrite inventory are frozen
in [`storage-versioning-and-migration.md`](storage-versioning-and-migration.md)
and [`ro-format-v1.md`](ro-format-v1.md).

## Boundaries

This profile does not define `.roproj` layout (#41), future `.ro` packaging
(#43), cross-document references, a public SDK ABI, or UUID as semantic ID
meaning. Final broad golden/negative corpus closure remains #40.
