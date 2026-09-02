# Semantic Diff Specification

Decision state: The logical canonical Semantic Delta contract is Accepted under
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md). Stable-ID
continuity and bound-formula comparison follow
[ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md) and
[ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md).
The current `diff-engine` Rust surface and rendered output remain an implemented
Provisional baseline rather than the protocol DTO.

Authority: [ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md)

Decision issue: [#45](https://github.com/nurockplayer/tachiko-work/issues/45)

## Problem

Traditional spreadsheet diff compares files. Humans and independent tools need
to understand semantic change without treating representation paths as meaning.

## Goal

Tachiko Work provides deterministic direct-state semantic evidence instead of
raw text changes or another mutation language.

## Canonical Semantic Delta v1

The logical contract identifier is exactly `tachiko.semantic-delta/v1`.

A canonical delta has three logical parts:

- the contract identifier;
- the continuing `DocumentId` that scopes the comparison; and
- the canonically ordered sequence of direct change facts.

This is a logical DTO contract. It does not select Rust or Serde layout, JSON or
protobuf bytes, IPC, WASM ABI, network transport, or public SDK spelling.

### Admission and equality

Inputs A and B MUST be admitted semantic `Document` states interpreted under
the same supported semantic contract. Their `DocumentId` values MUST be equal.
A consumer MUST reject a different-Document comparison as not being a canonical
revision delta; it MUST NOT emit a document-ID-change fact.

Canonical delta equality consists of the supported contract identifier, the
scope `DocumentId`, and the canonical direct-fact sequence. Runtime revision
occurrences and provenance are not part of this equality. An empty sequence is
the canonical delta for equal direct semantic state under the same scope and
contract.

### Stable targets

Every fact uses a closed typed target:

| Subject | Logical target |
| --- | --- |
| Document | `DocumentId` |
| Schema | `SchemaId` |
| Schema field | `(SchemaId, FieldId)` |
| Entity | `EntityId` |
| Stored entity field | `(EntityId, SchemaId, FieldId)` |

Targets never use `.roproj` paths, JSON Pointers, serialized positions, human
keys, presentation coordinates, or Git identifiers. The `SchemaId` in a stored
entity-field target is the schema applicable to that field occurrence in the
compared state.

### Closed direct-change vocabulary

The v1 change kinds and their complete payload meaning are:

| Subject rank | Change rank | Change kind | Required direct payload |
| ---: | ---: | --- | --- |
| 0 | 0 | `document_title_changed` | `before` and `after` title |
| 1 | 0 | `schema_created` | complete schema definition: key and all `(FieldId, field definition)` entries |
| 1 | 1 | `schema_deleted` | complete prior schema definition: key and all `(FieldId, field definition)` entries |
| 1 | 2 | `schema_key_changed` | `before` and `after` schema key |
| 2 | 0 | `schema_field_created` | complete field definition: key, field type, and requiredness |
| 2 | 1 | `schema_field_deleted` | complete prior field definition: key, field type, and requiredness |
| 2 | 2 | `schema_field_key_changed` | `before` and `after` field key |
| 2 | 3 | `schema_field_type_changed` | `before` and `after` field type |
| 2 | 4 | `schema_field_requiredness_changed` | `before` and `after` requiredness |
| 3 | 0 | `entity_created` | complete entity state: key, `SchemaId`, and all `(FieldId, typed stored value)` entries |
| 3 | 1 | `entity_deleted` | complete prior entity state: key, `SchemaId`, and all `(FieldId, typed stored value)` entries |
| 3 | 2 | `entity_key_changed` | `before` and `after` entity key |
| 3 | 3 | `entity_schema_changed` | `before` and `after` `SchemaId` |
| 4 | 0 | `entity_field_value_created` | typed `after` stored value |
| 4 | 1 | `entity_field_value_changed` | typed `before` and `after` stored values |
| 4 | 2 | `entity_field_value_cleared` | typed `before` stored value |

The target carries the subject's stable ID, so complete definitions do not
repeat an independently mutable ID property. A field definition's field type
includes any typed stable target it owns, such as the `SchemaId` of a Reference
field. Stored values preserve their admitted semantic type; formulas compare as
bound semantic expressions, not authoring text.

This vocabulary is closed. A normative consumer that does not support the
contract identifier or any encountered change kind MUST fail closed rather than
skip or guess it.

### Non-overlap and continuity rules

- Schema creation or deletion emits exactly one complete schema fact and no
  child schema-field facts for that schema.
- Entity creation or deletion emits exactly one complete entity fact and no
  stored entity-field facts for that entity.
- For a continuing schema field, key, type, and requiredness are independent
  direct properties. Emit one fact for each changed property; do not also emit a
  whole-definition change.
- For a continuing stable subject, mutable key changes preserve continuity and
  MUST NOT become delete/create pairs.
- Replacement under a different stable ID is deletion plus creation. Stable IDs
  themselves are never mutable facts.
- For a continuing entity that changes schema, emit
  `entity_schema_changed`. Compare stored values using schema-qualified targets.
  The same `FieldId` text under the old and new `SchemaId` denotes two targets,
  so an applicable old value clears and an applicable new value creates rather
  than becoming one cross-schema value change.
- Emit no duplicate fact with the same logical target and change kind.

### Canonical ordering

Sort the direct facts lexicographically by this logical tuple:

```text
(subject rank, typed target IDs, change rank)
```

Typed target IDs appear in the tuple in the order shown in the stable-target
table. For every typed stable ID, `tachiko.semantic-delta/v1` compares the exact
logical identifier Unicode scalar sequence lexicographically by scalar value,
with shorter equal-prefix sequences first and with no normalization, case
folding, locale, or interpretation of identifier contents. This one order is
part of the v1 logical contract and is independent of transport or storage
encoding. Complete schema and entity definitions order their field entries by
the same `FieldId` rule.

No other value participates in ordering. In particular, ordering MUST NOT use
filesystem paths, serialized member order, locale, mutable human keys,
insertion or hash iteration order, Git coordinates, or runtime occurrence
order. Canonical ordering carries no execution semantics.

### Direct facts versus derived evidence

Only stored/direct semantic state belongs in the canonical sequence. The
following are separate derived review, analysis, or observation evidence:

- calculated formula results and formula impact;
- dependency causes;
- validation diagnostics or gate outcomes;
- risk or conflict classification;
- localization and rendered prose;
- runtime before/after revision occurrences;
- author, timestamp, approval, source label, or Git provenance.

An outer host or review record may bind such evidence to a canonical delta.
Those fields do not change canonical delta equality. The current
`diff-engine::SemanticChange::FormulaImpact` variant is useful implementation
evidence but is not a v1 direct change kind.

Before-values are comparison evidence. They are not authorization,
optimistic-concurrency predicates, JSON Patch `test` operations, or an
`apply_if` language. Exact-base stale protection remains owned by
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).

## Human-readable projection

Traditional diff:

```diff
- goblin,180,18,1.4
+ goblin,210,21,1.4
```

Semantic projection:

```text
Goblin

HP
180 -> 210 (+16.7%)

Attack
18 -> 21 (+16.7%)
```

Rendered summaries may use current keys, percentages, localization, and
calculated impact. They are projections, not canonical machine evidence.

## Relationship to SemanticPatch and history

For an ADR-0024 SemanticPatch, semantic delta is derived review evidence from
the bound semantic base to the candidate produced by the exact typed Command or
ordered AtomicBatch.

Semantic delta is not:

- the proposal's operation vocabulary or occurrence identity;
- `ExactChangeBinding` or a substitute for the bound Command/AtomicBatch;
- an approval, authorization, precondition, or gate decision;
- a `.roproj`, JSON Pointer, storage-path, or Git-byte mutation program; or
- retained history, an event, or replay input.

Rendered summaries MUST NOT be used as the exact change to which approval or
execution binds. A stale proposal is not implicitly rebased by computing a new
delta; re-proposal against a new base creates a new proposal identity and new
derived evidence. Current state and complete snapshots remain authoritative
under [ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md).

## Git integration

Semantic delta can inform pull requests, reviews, merge decisions, release
notes, and AI summaries. Git identity and coordinates remain outer provenance,
not semantic targets or canonical delta equality.

## Implementation status and follow-up

The current `diff-engine` demonstrates typed state comparison, deterministic ID
iteration, stable rename continuity, and a distinction between stored change
and formula impact. It does not yet implement the complete public logical DTO
above, and this specification does not authorize that production change.

Tracking issue: a separately Ready implementation issue is required for a
concrete DTO or transport mapping. [Issue #46](https://github.com/nurockplayer/tachiko-work/issues/46)
may consume canonical delta as merge/conflict evidence without treating it as
an apply language.
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)
consumes canonical delta as retained-transition evidence without turning it
into an event or apply language.
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
keeps delta as evidence rather than replay input while fixing bounded optional
history and Git-association guarantees. Issue
[#50](https://github.com/nurockplayer/tachiko-work/issues/50) retains
causality/CRDT decisions; concrete public history/Git implementations require
separately Ready work.

## Principle

Git stores changes. Tachiko Work explains meaning.
