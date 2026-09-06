# ADR-0037: `.roproj/v2` persistence for keyed grouped sums

## Status

Accepted

Decision issue: [#327](https://github.com/nurockplayer/tachiko-work/issues/327)

Specified by: [`.roproj/v2` wire DTOs](../specs/roproj-format-v2.md),
[`.roproj/v2` layout](../specs/roproj-layout-v2.md), and the
[Storage Versioning and Migration Contract](../specs/storage-versioning-and-migration.md)

Related authority: ADR-0015, ADR-0017, ADR-0018, ADR-0019, ADR-0023,
ADR-0025, and ADR-0036

## Context

ADR-0036 admits exactly one durable saved semantic family,
`KeyedGroupedSumDefinition`. Its durable meaning is stable-ID-bound structure,
not an evaluated result. ADR-0017 requires a version-owned DTO and explicit
migration for new durable meaning. ADR-0023 freezes `.roproj/v1` as one closed
eighteen-file tree and its DTOs reject unknown members and categories.

Adding a definition member, cache, or file to v1 would silently change a
published representation. Reusing a view, formula, Analysis Query, or package
payload would also either change unrelated authority or make derived output
durable truth. The narrow decision required is therefore a new editable
representation version, not a production implementation.

## Decision

### 1. `.roproj/v2` is the sole selected editable representation for this family

`.roproj/v2` is a new version in the existing `.roproj` representation
namespace. Its manifest uses the existing fixed member order and exact
`"tachiko.roproj"` format string with lexical `format_version: 2`.

It has an exact closed nineteen-file canonical tree: the v1 `manifest.json`,
`schemas.json`, and sixteen entity shards, plus root-level `definitions.json`.
The new file is required even when empty, when it is
exactly the canonical bytes `[]\n`. No path, root name, file order, entity
shard, line number, key, label, or evaluated output is identity.

The v2 version-owned manifest, schema, entity, value, and expression DTOs are
fully redeclared in the v2 DTO specification. They are byte-compatible with
v1 only where that v2 specification says so; they do not alias semantic-core,
direct-JSON, or v1 implementation types. V2 adds only the closed definition
DTO described below. It does not select new Number, formula, query, delta,
merge, host durability, or package semantics.

### 2. The definition DTO is closed and only records durable meaning

`definitions.json` is one two-space-indented JSON array. Entries sort
by decoded opaque definition ID in unsigned UTF-8 byte lexicographic order.
IDs are nonempty opaque tokens and unique within the array; they are a fifth
typed stable-ID class and are not interchangeable with document, schema, field,
or entity IDs.

Every entry has this exact fixed-member shape:

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

The seven target members are typed stable schema/field references. The v2 file
has one record type only and is not an extension bag. Any future definition
family or incompatible meaning requires another `.roproj` version. Conversion must prove that
the two schemas and their declared fields exist and satisfy ADR-0036's exact
Orders/Products Text/Number roles. Rebinding by key, label, path, or a later
human rename is forbidden.

The DTO has no operation discriminator or parameters, labels, target-language syntax, evaluated
group map, candidate set, diagnostic, cache, revision/currentness evidence,
result-profile identity, export value, or optional extension area. All unknown,
duplicate, missing, wrong-type, duplicate-ID, unresolved-reference,
or invalid-role input fails closed before semantic publication.

### 3. Reading, saving, and migration are explicit and fail closed

Version dispatch occurs from the manifest before any v1/v2 body is decoded. A
v1-only reader rejects v2 at the manifest and must not inspect or reinterpret
the body. A reader supporting both versions selects the exact decoder; future,
missing, malformed, or unsupported versions fail closed without decoding,
canonicalizing, migrating, or rewriting their body.

The only newly accepted migration edge is explicit deterministic DTO-to-DTO
`.roproj/v1 -> .roproj/v2`:

```text
exact canonical v1 schemas + 16 entity-shard bytes --preserve exactly-->
fresh canonical v2 manifest (same document values, version 2) + those bodies
  + definitions.json = []\n
```

It accepts only an exact canonical v1 source and writes a complete v2 candidate
only to a distinct absent destination after strict source decode, conversion,
applicable validation, v2 canonical encoding,
and complete candidate preparation. It does not mutate the v1 source. Opening,
reading, or saving a definition-free v1 project does not silently upgrade it.
Creating a definition for a v1 project first requires this explicit migration;
a v1 writer must reject a definition-bearing semantic state rather than omit or
down-convert it. A v2 writer emits v2.

Every v2 migration/save candidate must complete strict DTO validation,
definition-ID uniqueness, stable-target resolution and role validation,
applicable ADR-0019 semantic validation, and canonical encoding before durable
publication. Failure leaves the source observably intact and does not expose a
partial destination. The native/browser transaction, no-replace primitive,
recovery, locking, and `fsync` mechanisms remain separately host-owned.

### 4. Reopen recomputes; portable package v1 refuses v2 truthfully

Save/reopen preserves only the definition. Any evaluation is recomputed from
the current accepted snapshot under ADR-0036. A cache or result represented in
the v2 canonical tree is forbidden and rejected; a retained non-current result
must never become truth merely because it was previously evaluated.

`tachiko.portable-package/v1` remains an exact wrapper over the eighteen-path
`.roproj/v1` payload. Its pack operation refuses a v2 source before package
construction; it must not drop `definitions.json`, migrate/down-convert,
or claim package v1 preserves v2 semantics. Package v2 is neither selected nor
implemented by this decision.

ADR-0036's existing whole-request fail-closed `semantic-delta/v1` and current
merge/conflict-v1 definition boundary remains unchanged. This decision adds no
delta, merge, public API, frontend, storage runtime, or Designer implementation.

## Required pressure cases

Future production implementation and conformance work must independently show:

- a definition-free v1 tree remains readable and byte-unchanged by ordinary
  open/read/save;
- explicit v1→v2 migration is deterministic, produces the exact empty
  `definitions.json`, and leaves its source intact;
- one admitted definition has deterministic v2 bytes despite construction/map
  order, and create/update/delete retain its stable definition identity across
  save/reopen;
- mutable schema/field labels and view/storage presentation changes leave the
  definition's stable bindings and canonical definition bytes unchanged;
- evaluated cache/result/revision bytes are absent from canonical output and
  rejected when supplied as closed-world DTO data;
- reopen recomputes the ADR-0036 result from the saved definition and current
  source values, never from a prior evaluated value;
- duplicate definition IDs, escaped-equivalent duplicate member names,
  unknown/malformed members, missing targets, wrong schema/field roles,
  and invalid semantic candidates fail closed before publication;
- v1-only readers reject v2 at manifest dispatch before body interpretation;
- a current v1/v2 reader rejects a future v3 manifest before it interprets a
  body that contains unknown definition data;
- repeated migration of byte-identical canonical v1 input produces
  byte-identical v2 output, and a failing migration candidate never clobbers
  either source or destination;
- a v1 definition-bearing write and package-v1 pack of a v2 tree fail
  truthfully as unsupported representation/payload version without omitting
  data; and
- the existing delta/merge v1 whole-request failure remains in force for every
  definition create, durable update, and delete.

## Consequences

Positive:

- v1 remains an immutable readable historical contract;
- v2 makes the one admitted saved-live family reviewable as canonical text;
- migration, identity, cache/currentness, and package boundaries are explicit;
- a later production lane receives a stable persistence target without gaining
  any authority to broaden ADR-0036.

Costs:

- v2 must maintain a complete version-owned DTO and conformance corpus even
  where v1 shapes are identical;
- users must explicitly migrate before adding a definition; and
- portable distribution waits for a separately Accepted package version.

## Rejected alternatives

- **Add a field or definitions file to v1:** rejected because ADR-0023 freezes
  its closed DTO/tree.
- **Persist evaluated groups or cache:** rejected because ADR-0036 makes them
  derived, revision-scoped, non-authoritative output.
- **Store definitions as formulas, views, or Analysis Query results:** rejected
  because none owns the accepted live definition family.
- **Silently migrate on open/save or down-convert on write/package:** rejected
  because ADR-0017 requires explicit, non-lossy migration and fail-closed
  unsupported semantics.
- **Package v2 using package v1:** rejected because package v1's exact payload
  profile covers only the v1 eighteen-file representation.

## Related

- [#327](https://github.com/nurockplayer/tachiko-work/issues/327)
- [#293](https://github.com/nurockplayer/tachiko-work/issues/293) — remains not
  Ready; Steward re-runs decomposition only after this authority is merged
- [ADR-0036](ADR-0036-bounded-keyed-lookup-and-grouped-sum.md)
