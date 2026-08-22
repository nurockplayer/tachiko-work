# Storage Versioning and Migration Contract

Decision state: Mixed — Accepted invariants under ADR-0017; Milestone 02 representation mechanics are Provisional where marked.

Implementation state: Implemented for frozen `legacy-direct-ro/v1`, explicit
deterministic v1→v2 migration, and canonical identity-aware `direct-ro/v2`.

Authority: ADR-0017

Implementation parent: #74

Conformance and identity integration: #40, #70

## Purpose

Define how Tachiko Work selects persisted representation versions, separates versioned storage DTOs from semantic-core, handles unsupported or malformed versions, and performs explicit migration without silently changing durable state.

This specification does not define `.roproj` filenames/sharding (#41), the future `.ro` package/container profile (#43), numeric semantics (#24), the cross-client diagnostic envelope (#23), or host-specific filesystem/browser transaction mechanisms (#26).

## Representation namespaces

Version numbers are local to a known representation profile/context.

The following are distinct namespaces:

- legacy/current direct `.ro` JSON representation;
- future `.roproj` editable materialization;
- future `.ro` portable package/container profile.

The integer `1` in two different representation namespaces does not imply the same wire schema.

The shipped v0.1 direct `.ro` JSON is frozen as the
`legacy-direct-ro/v1` compatibility profile. Its implemented incompatible
successor is `direct-ro/v2` within the same direct-JSON namespace. This does not
assign `.roproj` version `2`.

## Version envelope

### Accepted contract

Every persisted representation must identify the exact versioned decoder required before semantic-body decoding.

Milestone 02 uses one required `format_version` field for the direct JSON representation. Additional model/container/capability versions are not defined without independent lifecycle evidence.

### Provisional Milestone 02 spelling

`format_version` is a JSON lexical integer in the inclusive range `1..=u32::MAX`.

The following are malformed for the Milestone 02 direct JSON profile:

- missing field;
- string form such as `"1"`;
- fractional form such as `1.0`;
- exponent form such as `1e0`;
- zero;
- negative values;
- values greater than `u32::MAX`.

The use of `u32` is an implementation/profile mechanism, not a permanent ecosystem invariant.

## Reader pipeline

A reader processes input in this order:

```text
bytes
  ↓
UTF-8 validity
  ↓
JSON syntax + duplicate-member validation
  ↓
minimal version-envelope probe
  ↓
exact supported-version decoder selection
  ↓
version-specific storage DTO validation
  ↓
explicit representation migration/conversion as required
  ↓
current storage DTO
  ↓
explicit DTO → semantic conversion
  ↓
semantic validation required by the operation
  ↓
semantic document/state
```

The version probe must not deserialize the semantic body into current DTOs merely to discover that the version is unsupported.

## Error precedence and machine meaning

Storage-domain failures should preserve at least the following machine-distinguishable meanings:

```text
storage.invalid_utf8
storage.invalid_json
storage.duplicate_member
storage.version_missing
storage.version_malformed
storage.version_unsupported
storage.invalid_representation
storage.migration_failed
storage.invalid_semantic_document
```

Names above are the Milestone 02 recommended codes. #23 may later wrap them in a broader diagnostic envelope, but it must not erase their format/migration distinction.

Error precedence is:

```text
invalid UTF-8 / invalid JSON / duplicate member
before
missing or malformed version
before
unsupported version
before
supported-version DTO validation
before
migration/conversion failure
before
semantic validation
```

`storage.version_unsupported` should carry at least:

- `found`;
- a stable ordered list/range of supported versions for the relevant representation namespace.

It must not imply that the unsupported semantic body was understood.

## Unsupported-version behavior

For a syntactically valid, duplicate-free input with an unsupported `format_version`:

- return the unsupported-version failure;
- do not semantically decode the body;
- do not inspect version-specific title/id/schema metadata;
- do not canonicalize or rewrite;
- do not migrate;
- do not mutate durable source.

If future product requirements need preview metadata that is safe across versions, that metadata must be separately defined as part of a stable envelope contract.

## Supported-version strictness

Recognized versions are closed-world schemas unless the version explicitly defines an opaque extension area.

Unknown fields outside such an extension area are rejected recursively.

An extension area, if added in a future version, must define:

- preservation semantics;
- canonical ordering/encoding;
- name/collision rules;
- migration behavior;
- whether unknown extension payload can be safely re-emitted.

Generic ignore-and-drop behavior is forbidden.

## Duplicate JSON members

Duplicate JSON object member names are invalid at every depth.

Duplicate detection compares decoded member-name character sequences, not raw source spelling. Therefore these collide:

```json
{"a": 1, "a": 2}
```

and:

```json
{"a": 1, "\u0061": 2}
```

Duplicate semantic record IDs in arrays or other collections are a separate representation/semantic uniqueness failure.

## Version-specific DTO ownership

Each durable version owns complete storage DTO types for the wire schema it represents.

A historical DTO module must not embed semantic-core serialization types such as `Schema`, `Entity`, `Value`, `Expression`, or ID newtypes in a way that lets later semantic `serde` changes alter historical decoding.

The complete legacy direct-`.ro/v1` structural schema, enum tags, required members, fixed-member order, and typed-ID inventory are normative in `ro-format-v1.md`.

Conversion modules may depend on both storage DTOs and semantic types to perform explicit mapping.

Conceptually:

```text
storage::legacy_ro::v1::*
storage::direct_ro::v2::*
storage::migration::*
storage::conversion::*
```

The module names are illustrative and Provisional; the ownership boundary is normative.

## Migration topology

Durable migrations are explicit and version-labelled.

The default model is adjacent DTO-to-DTO migration:

```text
V1 DTO -> V2 DTO -> V3 DTO -> current DTO
```

A direct `V1 -> V3` optimization is allowed only when conformance fixtures prove the result is equivalent to the composed path for the supported inputs.

The current semantic model is not a permanent migration IR. Semantic validation may be used during migration where useful, but historical representation meaning must remain inspectable in version-labelled migration code and fixtures.

## Legacy direct `.ro` v1 identity migration

The v0.1 name-like IDs are source addresses, not the new ADR-0015 durable identities.

Migration is a two-phase graph conversion. It must finish the complete mapping before it rewrites any typed relationship.

### Phase 1: decode, validate, and map

1. Decode the complete frozen legacy DTO without inheriting current semantic serialization.
2. Validate legacy map-key/nested-ID coherence, schema membership, field membership, references, and formula references.
3. Build deterministic mappings for:
   - the document ID;
   - every schema ID;
   - every field ID in its schema scope;
   - every entity ID.
4. Reject ambiguous, duplicate, mismatched, or unresolvable legacy addresses before producing a candidate.

### Phase 2: rewrite every typed-ID occurrence

The migration must rewrite all typed-ID locations defined by the frozen v1 schema, including:

- `DocumentV1.id`;
- every `DocumentV1.schemas` member name;
- every `SchemaV1.id`;
- every `SchemaV1.fields` member name;
- every `FieldTypeV1.schema` target for reference fields;
- every `DocumentV1.entities` member name;
- every `EntityV1.id`;
- every `EntityV1.schema` relationship;
- every `EntityV1.fields` member name through the field mapping of that entity's schema;
- every entity target stored by `ValueV1` with `kind: reference`;
- every `ExpressionV1` reference `args.entity`;
- every `ExpressionV1` reference `args.field` in the target entity's schema scope.

The authoritative inventory is also recorded in `ro-format-v1.md`.

The migration must then:

1. preserve appropriate legacy names/keys as mutable human-facing addresses;
2. verify that no legacy source address remains in any typed-ID slot merely because its spelling is convenient;
3. validate the complete migrated DTO candidate;
4. convert it explicitly to the semantic model;
5. run the semantic validation required by the operation;
6. canonicalize the result before any durable commit.

Building the mapping but omitting one typed-ID occurrence is a migration failure. It must never be repaired by matching a new object through a human name after conversion.

### Provisional deterministic ID mechanism

Milestone 02 legacy migration uses RFC 9562 UUIDv5 with this fixed namespace:

```text
7a199010-e2db-5f4f-a216-07ddb708f5ef
```

That namespace is UUIDv5 in the standard URL namespace for this exact UTF-8
name:

```text
https://tachiko.work/migrations/legacy-direct-ro/v1
```

Every UUID name is the exact concatenation below encoded as UTF-8. `NUL` means
one `00` byte; no length prefix, Unicode normalization, case folding, or final
separator is added.

```text
document NUL legacy_document_id
schema   NUL legacy_document_id NUL legacy_schema_id
field    NUL legacy_document_id NUL legacy_schema_id NUL legacy_field_id
entity   NUL legacy_document_id NUL legacy_entity_id
```

Field identity is therefore schema-scoped. Initial frozen vectors are:

| Kind | Exact name with `NUL` shown textually | UUIDv5 |
| --- | --- | --- |
| document | `document NUL legacy-doc` | `1213a728-1f70-5425-a330-20a8797f5e82` |
| schema | `schema NUL legacy-doc NUL source` | `ff71fea8-d907-5234-a6be-819f6e6fdf07` |
| field | `field NUL legacy-doc NUL source NUL calc` | `32c7bf4d-e5e4-5ea0-ab43-0d42c6878cce` |
| entity | `entity NUL legacy-doc NUL source-entity` | `1832624c-a6ad-55fb-b96a-8617af123e7f` |

The namespace and input construction are frozen compatibility mechanisms, not
semantic identity meaning.

Normal object creation remains governed by ADR-0015's separate creation seam and preferred Provisional UUIDv7 generator.

Migration-generated UUIDs must not be interpreted as semantic creation time or as proof of globally unique historical lineage across unrelated legacy documents that reused the same old names. v1 references remain document-local; future cross-document identity work must not assume more than the legacy source can prove.

## No implicit upgrade

Reading/opening a supported historical representation may decode or migrate in memory for explicit inspection/operation, but it must not rewrite the source representation merely because a newer writer exists.

Durable migration is a distinct, user/operation-visible action.

## Representation-level atomicity

A migration or write is correct only if failure does not destroy or partially replace previously durable source state.

Before commit, the candidate must complete the required pipeline:

```text
decode
→ migrate/convert
→ validate
→ canonicalize
→ prepare durable result
```

The exact temporary-file/rename/fsync/browser-transaction implementation remains #26.

## Conformance requirements

#40 should include at least:

- missing version;
- string/fraction/exponent/zero/negative/out-of-range version;
- unsupported future version;
- unsupported future version with body fields unknown to the current reader;
- malformed/truncated future-looking JSON;
- top-level and nested duplicate members;
- escaped-equivalent duplicate member names;
- recursive unknown field in a supported version;
- complete legacy v1 DTO coverage for every field/value/expression discriminator;
- legacy map-key/nested-ID mismatch failures;
- deterministic document/schema/field/entity ID mappings;
- `FieldTypeV1.schema` rewrite;
- `EntityV1.schema` rewrite;
- `EntityV1.fields` key rewrite;
- entity-reference target rewrite;
- formula-reference entity and field rewrite;
- a nonempty migrated graph proving all typed relationships validate against the new IDs;
- a negative fixture that intentionally omits or corrupts each mapping class;
- ambiguous migration input failure;
- migration determinism across repeated runs;
- no partial durable output on migration failure;
- composed migration vs optimized migration equivalence if optimized edges exist.

The implementation-critical ADR-0018 numeric and exact resource-boundary
vectors are covered with v2 here; #40 owns final broad conformance closure.

## Related

- ADR-0017
- ADR-0015
- ADR-0016
- `ro-format-v1.md`
- #40, #70, #74
