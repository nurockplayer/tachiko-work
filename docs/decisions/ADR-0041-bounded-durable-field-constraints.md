# ADR-0041: Two bounded durable Driver field constraints

- **Status:** Accepted
- **Date:** 2026-09-22
- **Decision issue:** [#391](https://github.com/nurockplayer/tachiko-work/issues/391)
- **Amends:** ADR-0015, ADR-0018, ADR-0019, ADR-0020, ADR-0021, ADR-0023, ADR-0030, ADR-0031, ADR-0033, ADR-0037
- **Implementation state:** Decision and representation authority only. No production
  codec, runtime, API, UI, native/WASM, or package implementation is authorized by
  this ADR.

## Decision

Tachiko Work admits exactly two durable schema-field constraints for the Driver
product journey: `text_literal_set` and `number_inclusive_range`. A field has one
constraint facet. The facet is always present in the `.roproj/v3` DTO and is the
closed tagged union below:

```json
{"type":"none"}
{"type":"text_literal_set","values":["...","..."]}
{"type":"number_inclusive_range","min":1,"max":5}
```

The `none` tag is a real non-null value, not an omitted or nullable metadata
slot. A field has no `ConstraintId`, separate constraint object, extension bag,
or sidecar. Constraint state is schema meaning owned by the stable `FieldId`.
Exactly one constraint is stored for each field, and the constraint's type must
match the field's declared type:

| Field type | Allowed constraint |
| --- | --- |
| Text | `none` or `text_literal_set` |
| Number | `none` or `number_inclusive_range` |
| Boolean, Date, Reference | `none` only |

The two constrained variants are closed. Unknown tags and members fail closed.
No generic validation-rule language is introduced.

## Constraint parameters

A `text_literal_set` contains 1–256 distinct decoded Unicode strings. Every
string is at most 1024 UTF-8 bytes and the sum of all decoded UTF-8 byte lengths
is at most 65536 bytes, measured before JSON escaping. The empty string is
allowed. Values are emitted and compared in unsigned UTF-8 lexicographic order.
An authoring writer may sort an admitted set into that canonical order, but
duplicate values are rejected before emission. A decoder rejects duplicate or
noncanonical input and never sorts, deduplicates, normalizes, trims,
case-folds, locale-folds, or otherwise repairs malformed input.

A `number_inclusive_range` contains finite Accepted binary64 `min` and `max`,
with normalized zero and `min <= max`. NaN and infinities are rejected. Numeric
spelling and conversion retain ADR-0018's finite binary64 and canonical-number
rules.

Existing stored values must satisfy their field constraint. An absent optional
value remains absent; constraints do not change requiredness. A required field
still uses the existing requiredness rule. Date values retain their existing
`YYYY-MM-DD` meaning and may only use `none`.

A formula-valued Number field is checked against its range only after the
complete successful calculation produces its final finite result. A failed or
unavailable calculation suppresses that dependent range check: it does not
invent a value or replace the formula failure with a range failure. The complete
validation/calculation oracle and existing diagnostic ordering remain authoritative.

## Mutation, runtime, and projection boundary

Constraint edits are semantic commands through the current-base
Propose/Execute boundary. Review evidence includes the affected stable field and
its affected values. Publication is atomic. An invalidation or failed edit
leaves semantic state, revision occurrence, and retained history unchanged.
Undo and Redo follow ADR-0033's forward-only law: each action is a newly
authorized inverse or forward `Command | AtomicBatch` evaluated against the
exact current base through ordinary admission, authorization, validation, and
publication. An accepted prior-equivalent state is a distinct new revision
occurrence and retained transition when history is enabled; history is never
erased, rewound, or retargeted. A stale, invalid, or otherwise failed history
action publishes no semantic state, creates no revision or transition, and
preserves the current state plus the undo/redo stacks. Constraint edits do not
create a second mutation vocabulary.

The shared semantic/application authority enforces constraints at final
publication for every first-party path: native, WASM, paste, edit, formula
calculation, and merge finalization. UI controls and validation may assist with
previews and user feedback, but UI is a projection and never the sole or
authoritative enforcement point. Exact Rust/API/transport/SDK shapes remain
separately owned and unimplemented by this decision.

## `.roproj/v3` representation

`.roproj/v3` is the next closed editable-directory representation. It has the
same exact nineteen-file tree and shard placement as `.roproj/v2`:
`manifest.json`, `schemas.json`, `definitions.json`, and the sixteen fixed
`entities/*.jsonl` shards. The complete tree is specified by
[`roproj-layout-v3.md`](../specs/roproj-layout-v3.md); the DTO authority is
[`roproj-format-v3.md`](../specs/roproj-format-v3.md).

The manifest is identical to v2 except for the required lexical
`"format_version": 3`. The v3 `FieldDefinition` member order is exactly
`id`, `key`, `field_type`, `required`, `constraint`; all five members are
required, non-null, and emitted. Its `constraint` object uses exactly the three
shapes and member orders in this ADR. The v3 FieldType set adds the already
existing date scalar, `{"type":"date"}`. The Value set adds the already
existing direct-ro/v2 Date shape, `{"kind":"date","value":"YYYY-MM-DD"}`.
Date is proleptic Gregorian year 0001–9999, with no time zone, time, or new
arithmetic semantics. V3 does not add a Date constraint.

All v2 rules that v3 does not explicitly amend remain frozen by exact section
reference in `roproj-format-v3.md`: closed-world admission, canonical JSON,
stable IDs, manifest-first dispatch, schemas/entities/values/expressions,
keyed-grouped-sum definitions, entity sharding, and canonical byte rules. The
reference is to the ADR-0037 v2 contract as accepted at this decision; it is
not an alias to future edits. Any incompatible amendment requires another
representation version and ADR.

## Migration and compatibility boundaries

A supported v2-to-v3 migration decodes and admits a complete v2 source, creates
one `none` constraint for every field, preserves all stable IDs, keys, types,
requiredness, values, formulas, definitions, and document meaning, then
re-encodes one canonical v3 tree. It is explicit, deterministic, no-clobber,
and produces no partial destination. It does not infer constraints, alter
content, or silently upgrade ordinary read/read-save.

A v1 source uses the existing explicit v1-to-v2 migration first, then this
v2-to-v3 migration. Frozen v1 and v2 readers/writers remain closed and cannot
write a v3 field. A v1 portable package remains the eighteen-path v1 payload;
package-v2 is not selected here. Unsupported older representations and package
payloads fail with a truthful unsupported-representation/version outcome.

The existing `direct-ro/v2` representation namespace already admits Date. It is
distinct from the app-private `TWDPROJ2` host envelope, whose payload is
direct-ro/v2 with browser-only metadata. `TWDPROJ2` is not the direct-ro/v2 DTO,
`.roproj/v3`, `.roproj/v2`, or a portable package. A private project containing
Date in that envelope may enter v3 only through an explicit user-selected
conversion/export from an admitted semantic snapshot. Ordinary private
read/save never silently changes format. A new project may explicitly select
v3. No conversion drops Date, strips constraints, or claims portable-package
fidelity where the target cannot represent it. Implementation status of these
conversions is unimplemented and must be reported as such until separately
delivered.

## Delta v2 and conflict v2

The frozen `tachiko.semantic-delta/v1` and `tachiko.semantic-conflict/v1`
contracts remain unchanged. This ADR defines their constraint-aware successors
as logical contracts only; it does not select a wire, SDK, or Rust DTO.

### `tachiko.semantic-delta/v2`

Delta v2 retains every v1 direct target, kind, rank, continuity, suppression,
and ordering law. Complete schema and field payloads include the complete tagged
`constraint` value, including explicit `none`. For a continuing schema field,
constraint is an independent facet after requiredness. A changed constraint
emits exactly one atomic `schema_field_constraint_changed` fact at schema-field
change rank 5 with `before` and `after` tagged constraints. It never disappears,
becomes an empty delta, or becomes a whole-definition replacement. Unsupported
contract, target, or fact kinds fail closed.

### `tachiko.semantic-conflict/v2`

Conflict v2 retains every v1 admission, same-Document, structural-kind,
parent-child suppression, identity, facts, and canonical-order law. Complete
schema-field subject payloads include the tagged `constraint`, including
`none`. The schema-field facet table adds `constraint` at rank 4 after
`requiredness`; the existing v1 ranks and meanings remain fixed. Concurrent
unequal edits of one stable field's constraint produce one `constraint` facet
conflict. They do not become separate per-variant conflicts or a whole-field
replacement. Unsupported contract, target/facet, or kind fails closed.

A conflict-free merged candidate must still pass complete validation and
complete formula finalization before publication. An invalid merged candidate
is a validation/calculation failure, not a fabricated conflict object.

## Rejected alternatives

This decision rejects extending frozen v1/v2 contracts; nullable or optional
constraint metadata; ConstraintId identity; separate facets per constraint
variant; sorting or deduplicating malformed input; UI-only enforcement;
automatic migration; Date dropping; and claims of portable fidelity that the
representation cannot provide. A generic DSL, arbitrary patterns/defaults,
formula syntax change, or package-v2 is outside this decision.

## Consequences and implementation boundary

The accepted semantic and storage authority is complete enough for independent
codec, validator, delta, and conflict implementations. #448 implements the
closed core field model, the existing declaration/direct-value/complete-success
formula validation gates, and fail-closed frozen v1 writer, diff, and merge
boundaries. #318 and #368 remain later implementation work; this ADR supplies
their bounded authority and acceptance target. The `.roproj/v3` codec and
migration, v2 delta/conflict facts, UI/native save journey, native/WASM parity,
and public API remain unimplemented unless a later implementation issue records
evidence.

Documentation acceptance for this docs-only decision is consistency/link/diff
validation, exact cross-document agreement, and independent Sol exact-head
review. Executable behavior and unit-test acceptance are not applicable to this
PR; hosted and committed release gates still apply before integration.

## Related authority

- [ADR-0015](ADR-0015-stable-semantic-identity.md)
- [ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](ADR-0020-first-class-headless-semantic-api.md)
- [ADR-0021](ADR-0021-progressive-semantic-strengthening.md)
- [ADR-0023](ADR-0023-roproj-v1-canonical-tree-and-sharding.md)
- [ADR-0030](ADR-0030-canonical-semantic-delta.md)
- [ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md)
- [ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
- [ADR-0037](ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md)
- [Issue #391](https://github.com/nurockplayer/tachiko-work/issues/391)
