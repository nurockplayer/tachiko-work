# Tachiko Work direct `.ro` JSON v2

Decision state: Mixed. Stable identity, representation-boundary, canonical
semantic-preservation, and Number invariants are Accepted under ADR-0015,
ADR-0017, and ADR-0018. Exact direct-ro/v2 wire and resource-profile mechanics
are Provisional Milestone 02 choices.

Implementation state: Implemented by the stable-identity transition in #70

Representation namespace: `direct-ro/v2`

## Purpose

Version 2 is the identity-aware successor to frozen `legacy-direct-ro/v1`. It
persists opaque stable semantic IDs separately from mutable human keys and
stores formula/reference targets by stable ID. It is not `.roproj/v2`, a global
format version, or the separately Accepted
`tachiko.portable-package/v1` profile.

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

The Provisional Milestone 02 `direct-ro/v2` profile uses these values. They are
representation mechanisms subordinate to the Accepted storage and semantic
contracts in ADR-0017 and ADR-0018:

| Resource | Limit |
| --- | ---: |
| Normal direct-JSON complete input | 8 MiB (`8,388,608` bytes) |
| One RFC 8259 number token | 256 bytes |
| One bound formula AST | 256 nodes |
| One bound formula root-to-leaf path | 64 nodes |

The shared complete-input limit is Stage 0: it applies before UTF-8 validation
or any JSON/version scan and also covers legacy v1 and untrusted version
envelopes entering the current normal direct-JSON reader. Exactly-at-limit
input is admitted; one byte more is `storage.resource_limit` before any latent
format/version failure.

For admitted input that selects v2, the 256-byte number-token limit remains
v2-specific and is applied before the DTO converts a decimal token to semantic
`Number`. A token-limit failure is not numeric overflow or underflow.

The 8 MiB value is a Provisional normal representation-profile mechanism, not
a semantic document, product, `.roproj`, package/export, or UI maximum. Future
versions or an explicit legacy import/migration capability may adopt another
accepted finite profile; no unbounded normal-reader bypass is allowed.

Formula node/depth limits are checked iteratively after DTO decoding and before
recursive migration, semantic conversion, validation, or writing. The exact
limit is admitted; an over-limit v2 formula is an invalid representation and an
over-limit legacy formula fails migration without producing a candidate.

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

Field-type discriminators are `number`, `text`, `boolean`, `date`, and
`reference`. `date` is a date-only proleptic Gregorian value in the canonical
`YYYY-MM-DD` spelling; it has no time, timezone, epoch, or Excel serial
meaning.
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
discriminators: `number`, `text`, `boolean`, `date`, `reference`, and
`formula`. A Date value stores the canonical `YYYY-MM-DD` string:

```json
{
  "kind": "date",
  "value": "2024-02-29"
}
```
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

This profile does not define the separately Accepted `.roproj/v1` tree or wire
contract (ADR-0023), portable package v1 (ADR-0025 and
[`portable-package-v1.md`](portable-package-v1.md)), cross-document references,
a public SDK ABI, or UUID as semantic ID meaning. Final broad golden/negative
corpus closure remains #40.
