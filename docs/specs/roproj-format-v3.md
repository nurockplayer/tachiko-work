# Tachiko Work `.roproj/v3` wire DTO specification

Decision state: Accepted target under [ADR-0041](../decisions/ADR-0041-bounded-durable-field-constraints.md).

Implementation state: The storage codec, exact canonical-tree admission,
semantic-core validation, and explicit v1/v2 conversion are implemented by #449.
Native read, publish, and migration are implemented for canonical directories.
Formula calculation admission, Designer selection/save, WASM integration, and
portable-package support are not implemented by this storage slice.

Editable-directory namespace: `.roproj`; format version: `3`

Physical tree authority: [`roproj-layout-v3.md`](roproj-layout-v3.md)

## Scope and closed boundary

This is the complete v3 DTO authority for the `.roproj` editable directory. V3
uses the exact v2 canonical tree and all v2 DTO rules except the bounded changes
listed here. The incorporated v2 sections are the closed-world/presence rules,
Common JSON rules, Stable ID tokens, Entity JSONL record, Value, Bound
Expression, definitions, decode/conversion/validation contract, and normative
byte rules in the accepted [v2 specification](roproj-format-v2.md), at the
ADR-0041 decision boundary. This incorporation is a fixed normative snapshot;
future edits to v2 do not alter v3. An incompatible change requires v4.

V3's only DTO changes are: manifest version `3`; the five-member
`FieldDefinition`; the added `date` FieldType; the added `date` Value; and the
constraint union below. All v2 members, tags, requiredness, ordering,
unknown-member rejection, ID ordering, formula rules, definition rules, and
canonical JSON/JSONL bytes remain exactly as incorporated. No extension member,
nullable member, sidecar, or alternate alias exists.

## Manifest and tree envelope

`manifest.json` has fixed member order `format`, `format_version`, `document`:

```json
{
  "format": "tachiko.roproj",
  "format_version": 3,
  "document": {
    "id": "opaque-document-id",
    "title": "Balance"
  }
}
```

`format` is exactly `tachiko.roproj`; `format_version` is the lexical JSON
integer `3`; `document` has fixed member order `id`, `title`. Version dispatch
happens before any other file or record is decoded. A missing, malformed,
unsupported, or non-lexical version fails closed.

## Schema and field definition

`schemas.json` is the v2 array of Schema records, ordered by stable Schema ID.
A Schema has fixed member order `id`, `key`, `fields`; fields are ordered by
stable Field ID. V3's complete FieldDefinition has fixed member order
`id`, `key`, `field_type`, `required`, `constraint`:

```json
{
  "id": "field-damage",
  "key": "damage",
  "field_type": {"type": "number"},
  "required": false,
  "constraint": {"type": "number_inclusive_range", "min": 1, "max": 5}
}
```

All five members are required, non-null, and emitted, including `required:false`
and `{"type":"none"}`. Field IDs are unique within the owning schema.

### FieldType

The complete v3 tag set is `number`, `text`, `boolean`, `date`, and
`reference`. Scalar shapes have the sole member `type`; a reference has the
incorporated v2 member order `type`, `schema`:

```json
{"type":"number"}
{"type":"text"}
{"type":"boolean"}
{"type":"date"}
{"type":"reference","schema":"opaque-target-schema-id"}
```

`date` is the existing date-only proleptic Gregorian scalar. Its canonical
value is `YYYY-MM-DD`, year 0001 through 9999 inclusive, with no time, timezone,
epoch, DateTime, or date arithmetic meaning.

### Constraint

The complete closed union and member orders are:

```json
{"type":"none"}
{"type":"text_literal_set","values":["alpha","beta"]}
{"type":"number_inclusive_range","min":1,"max":5}
```

`none` has only `type`. `text_literal_set` has fixed member order `type`,
`values`; `values` is nonempty and contains 1–256 distinct strings, each at
most 1024 UTF-8 bytes, with aggregate decoded UTF-8 length at most 65536 bytes
before JSON escaping. Values are sorted by unsigned UTF-8 bytes. Empty strings
are valid. Duplicate values, noncanonical order, normalization, folding,
trimming, or repair are invalid.

`number_inclusive_range` has fixed member order `type`, `min`, `max`. Both
members are finite Accepted binary64 values with normalized zero, and
`min <= max`. NaN and infinity are invalid. Constraint pairing is intrinsic:
Text accepts `none` or `text_literal_set`, Number accepts `none` or
`number_inclusive_range`, and Boolean/Date/Reference accept only `none`.

Unknown constraint tags or members, missing members, nulls, duplicate object
members, or a mismatched constraint/type pair fail closed. An authoring writer
may sort an admitted literal set into canonical unsigned UTF-8 order, while a
decoder rejects duplicate or noncanonical input without repairing it.

## Entity values and formulas

The Entity JSONL record retains the incorporated v2 fixed member order
`id`, `key`, `schema`, `fields`, stable-ID field map, and canonical shard rules.
The v3 Value tag set is the incorporated v2 set plus `date`:

```json
{"kind":"date","value":"2024-02-29"}
```

Date values must be valid proleptic Gregorian dates from `0001-01-01` through
`9999-12-31`, in exact zero-padded `YYYY-MM-DD` form. This preserves the
existing direct-ro/v2 Date meaning and introduces no new formula operator or
Date constraint. Existing number, text, boolean, reference, and formula values
retain their v2 shapes and semantics. Formula expressions and keyed-grouped-sum
definitions are unchanged and remain version-owned by the incorporated v2
sections.

An optional stored value remains absent from the entity `fields` object; no
constraint creates a null placeholder. A present value must match its field
type and satisfy its constraint. Requiredness remains a separate field member.
A Number formula is checked against `number_inclusive_range` only after a
successful complete calculation; failed/unavailable calculation suppresses
that dependent range check and reports the existing calculation outcome.

## Canonical bytes and layout reference

V3 uses the incorporated v2 canonical JSON profile: UTF-8 without BOM, decoded
Unicode preserved without normalization, duplicate names rejected after decode,
fixed member order, two-space pretty JSON with one final LF for manifest,
schemas, and definitions, compact one-record JSONL with one LF for nonempty
shards, and zero-byte empty shards. Collections use unsigned UTF-8 stable-ID
order; text constraint values use the same order. The exact nineteen-file tree
and SHA-256 first-hex-nibble entity placement are in
[`roproj-layout-v3.md`](roproj-layout-v3.md).

## Decode, migration, and semantic admission

A v3 reader dispatches from the manifest, rejects unsupported/unknown structure,
decodes the exact ordered nineteen-file tree, checks constraint pairing and
parameter limits, checks Date syntax/range, and runs the shared semantic-core
declaration, relationship, direct-value, and formula-structure oracle. It never
sorts or repairs malformed input. Complete formula evaluation and calculated
result-range enforcement remain at the workspace/application admission gate;
the storage codec does not claim that gate. A writer emits v3 only from a
storage-validated semantic state and the native host writes a complete canonical
candidate atomically to an absent destination.

Explicit v2→v3 migration preserves stable IDs, keys, types, requiredness, values,
formulas, definitions, and document meaning, adding `none` to every field. It
must decode, admit, convert, validate, canonicalize, and prepare the complete
candidate before publication. V1 uses the existing explicit v1→v2 edge first.
Older formats, v1 portable packages, and package-v2 requests that cannot
represent this tree fail with a truthful unsupported representation/version
outcome. Existing Date projects held in the private `TWDPROJ2` host envelope
(whose payload is direct-ro/v2) enter v3 only through an explicit user-selected
conversion/export; ordinary private read/save does not silently change format.
The private-project conversion path remains separately owned application work.

## Explicitly outside v3

This contract does not alter `.roproj/v1`, `.roproj/v2`, direct-ro/v2,
portable-package/v1, or frozen semantic delta/conflict v1. It does not define
portable-package/v2, a public API/SDK/transport, a ConstraintId, a generic
constraint DSL, UI enforcement, automatic upgrades, or implementation status.
