# ADR-0044: Saved-definition facts in semantic delta/conflict v2

## Status

Accepted

Decision issue: [#531](https://github.com/nurockplayer/tachiko-work/issues/531)

Amends: [ADR-0041](ADR-0041-bounded-durable-field-constraints.md) for the
current pre-release logical delta v2 vocabulary;
[ADR-0030](ADR-0030-canonical-semantic-delta.md) only through that v2
successor; and [ADR-0036](ADR-0036-bounded-keyed-lookup-and-grouped-sum.md)
only for its requirement for future definition-delta authority. Frozen delta
and conflict v1 remain unchanged. ADR-0031's conflict vocabulary is not widened
by this decision; the unresolved definition-conflict choice is recorded below.

Specified by: [Semantic Diff Specification](../specs/semantic-diff-spec.md),
[Conflict Resolution Specification](../specs/conflict-resolution.md), and
[Bounded Keyed Lookup and Grouped-Sum Definition Specification](../specs/bounded-keyed-lookup-and-grouped-sum.md).

## Context and baseline evidence

A name-bearing occurrence must select `tachiko.semantic-delta/v2` under
[ADR-0043](ADR-0043-native-display-names-separate-from-keys.md), because
frozen v1 cannot represent names. ADR-0041 similarly requires v2 for constrained
state. V2 currently refuses saved-definition changes, so adding a display name
blocks the otherwise supported saved grouped-summary create/update journey.

The inspected baseline is `b1210053657c114bd65914f1728612bd1f7b5509`.
It does **not** contain saved-definition delta facts or conflict facets in v1:

- [`diff-engine::diff`](../../crates/diff-engine/src/lib.rs) refuses changed
  definition maps; [`canonical_delta`](../../crates/diff-engine/src/canonical_delta.rs)
  does so for v2 as well.
- [`merge-engine::merge`](../../crates/merge-engine/src/lib.rs) and
  [`merge_v2`](../../crates/merge-engine/src/conflict_v2.rs) refuse changed
  definition maps. Their target/facet vocabularies contain no saved definition.
- The v1 [`PatchLifecycle::finalize_evaluation`](../../crates/workspace-engine/src/patch_lifecycle.rs)
  admits validated definition-only commands/batches with an empty receipt
  projection; mixed batches with a definition change are refused. This
  publication path is not a definition delta or conflict protocol.
- The [v1 diff refusal tests](../../crates/diff-engine/tests/keyed_grouped_sum_refusal.rs)
  and [v1 merge refusal tests](../../crates/merge-engine/tests/keyed_grouped_sum_refusal.rs)
  explicitly cover create, in-place update, and delete. ADR-0036 §5 and
  ADR-0037 §4 require those v1 refusals.

Accordingly, this decision preserves the existing saved-definition **meaning
and command behavior**, rather than claiming to copy nonexistent v1 facts.
The new evidence vocabulary is explicitly selected here. It does not revise
lookup, grouped reduction, validation, authorization, or persistence semantics.

## Decision

### 1. One existing saved definition is a direct-state subject

The only added subject family is ADR-0036's `KeyedGroupedSumDefinition`.
Its logical target is the existing opaque typed
`KeyedGroupedSumDefinitionId`, scoped by the delta's continuing `DocumentId`.
It is distinct from SchemaId, FieldId, and EntityId, even when their decoded
strings happen to be equal. Paths, labels, keys, evaluated groups, and storage
positions never identify this subject.

The complete direct payload has exactly the existing bindings:

- Orders: `SchemaId`, local lookup-key `FieldId`, local quantity `FieldId`;
- Products: `SchemaId`, remote key `FieldId`, category `FieldId`, price `FieldId`.

Each FieldId retains its role and owning binding's SchemaId. The fixed
`lookup-exact-text -> price * quantity -> group by category -> SUM` meaning is
inherent in this sole family, not a new mutable operation/version parameter.
The target carries the definition ID; the payload has no independently mutable
ID. Payload equality is exact equality of all seven typed role bindings under
ADR-0036/0037. A change to any binding is a durable definition change; a label
or key edit of a bound subject leaves these bindings unchanged.

This logical payload does not copy a storage DTO or stabilize Rust, Serde,
wire, SDK, or byte spelling. It excludes evaluated values, diagnostics,
dependencies, caches, result-profile identity, revisions, and provenance.

### 2. Append complete-definition facts to delta v2

Retain the identifier `tachiko.semantic-delta/v2` and explicitly amend its
current pre-release closed vocabulary. Append saved definition **subject rank
5**, after stored entity fields (rank 4). All earlier subject/change ranks,
including ADR-0043 names and ADR-0041 constraints, stay fixed.

| Subject rank | Change rank | Change kind | Required direct payload |
| ---: | ---: | --- | --- |
| 5 | 0 | `keyed_grouped_sum_definition_created` | complete typed `after` definition |
| 5 | 1 | `keyed_grouped_sum_definition_deleted` | complete typed `before` definition |
| 5 | 2 | `keyed_grouped_sum_definition_changed` | complete typed `before` and `after` definitions |

Comparison uses the union of stable definition IDs in A and B. Absent-to-present
emits creation; present-to-absent emits deletion; unequal present-to-present
emits one complete-definition change. Equal definitions emit no definition
fact. An in-place change preserves its ID and is never delete/create.
Replacement under different IDs is deletion plus creation, even for identical
bindings. Multiple binding edits at one continuing ID produce exactly one
change fact, with no per-binding child facts or overlapping replacement fact.

A fact's logical identity is its contract, continuing DocumentId, typed
definition target, change kind, and canonical before/after direct facts.
The target/change-kind coordinate is unique within one delta; payloads remain
part of fact and delta equality. These are comparison facts, not commands,
approval, optimistic-concurrency predicates, or an executable patch sequence.

For every compared state, an existing definition contributes a **PRESENT**
complete payload. Explicit outer absence means that this definition subject is
missing. Creation has absent before/PRESENT after; deletion has PRESENT
before/absent after; change has two PRESENT unequal payloads. Transport omission,
null, empty bindings, or an empty evaluated result must not stand for absence.
This is the same outer presence distinction as ADR-0043: an existing nameless
Schema/FieldDefinition contributes a PRESENT optional-name value, not an absent
subject. Definitions themselves gain no optional name or optional binding.

Sort by `(subject rank, typed target IDs, change rank)`. Here the sole target
ID component is KeyedGroupedSumDefinitionId, compared by exact Unicode scalar
lexicographic order, shorter equal-prefix first, with no normalization, case
folding, locale, or interpretation of ID contents. Do not sort by binding
values, names, human keys, or paths. Definitions are document-owned subjects;
schema/entity parent suppression does not suppress their separate facts when
bound schemas or fields also change.

### 3. Preserve admission, fail-closed behavior, and the conflict boundary

Both delta inputs must be admitted same-Document states under the same supported
contract, including complete definition-ID/binding/role validation and the
existing semantic/formula gates. Unknown contracts, subjects, change kinds,
malformed payloads, invalid bindings, or unsupported required meaning fail the
whole request. An implementation or consumer lacking these facts must refuse a
definition-changing v2 comparison, never omit the definition, return a partial
delta, or substitute the v1 empty receipt projection.

Frozen `tachiko.semantic-delta/v1` and `tachiko.semantic-conflict/v1` retain
their existing definition-change refusal and their name/constraint boundaries.
This decision adds no definition conflict facet or three-way selection law:
v1 has none to mirror. `tachiko.semantic-conflict/v2` therefore continues to
refuse a base/left/right occurrence whose definition maps differ, including
one-sided changes, equal concurrent additions, and matching deletions. It
returns neither partial conflicts nor a candidate with omitted or silently
selected definitions. Unchanged definition maps remain subject to existing
input and candidate validation. Lead decision (#531): this refusal is the
accepted behavior for this slice. If definition-changing v2 merge is needed
later, the selected direction is one atomic complete-definition `subject`
facet; independently reconciled role-binding facets are rejected as
speculative. That extension needs its own authority amendment before
implementation is Ready.

### 4. Restore definition publication on admitted v2 occurrences

After realization, named and constrained occurrences using v2 support the
existing definition create/change/remove commands exactly as admitted nameless
v1 occurrences do: same stable identity/bindings, validation, exact-base atomic
publication, capability and approval rules. V2 returns complete direct evidence
instead of treating a definition change as unsupported or empty. Unchanged
definitions survive label edits and round trips without spurious definition
facts. General command/batch admission is unchanged; this is no blanket waiver
of existing mixed-batch or per-command gates.

Constraint admission under [#452](https://github.com/nurockplayer/tachiko-work/issues/452)
remains out of scope. Supporting definition evidence for already admitted
constrained states does not admit constraints into a runtime that currently
refuses them. No storage DTO, `.roproj` version, package profile, migration,
formula language, definition family, evaluation rule, or public wire is added.

## Pressure cases and delivery boundary

The #531 implementation child must independently demonstrate:

- create, in-place one/multiple-binding change, deletion, equal no-change,
  different-ID replacement, and multiple-ID stable ordering;
- complete PRESENT/absent payloads, exact role bindings, rank 5 and ranks
  0/1/2, and unchanged earlier name/constraint ordering;
- the ordinary admitted named create/change/remove journey and equivalent
  nameless command outcomes, plus engine-level admitted constrained cases
  without enabling #452 constraint admission;
- unchanged definitions during key/name edits, complete evidence in the v2
  publication path, and unchanged stale/invalid/unauthorized atomic refusal;
- invalid/missing/retyped bindings and unsupported facts fail closed;
  frozen v1 and definition-changing conflict-v2 requests retain their refusals.

Authority only: this PR changes no source, tests, scripts, CI, or fixtures.
Implementation belongs to **the #531 implementation child**, which the Work
lead creates and separately Readies after authority integration and the required
acceptance/decomposition handoff. #531 owns this authority PR, not a claim of
delivered behavior. The Work lead (Opus 5.5) owns independent review, conflict
disposition, integration, and the merge-time changelog entry. This writer does
not merge or resolve threads. Sheet re-pin and consumer qualification remain
Sheet-owned. Documentary consistency, local links/anchors, and diff checks
are this PR's validation; executable acceptance belongs to implementation.

## Lead decisions on the intake questions

1. **Baseline confirmed.** v1 has no saved-definition delta facts or conflict
   facets; a definition-only publication uses an empty receipt projection, and
   delta/merge refuse definition changes. This ADR gives v2 complete definition
   facts rather than copying v1's empty shortcut, and assumes or backfills no
   v1 vocabulary.
2. **Conflict v2 keeps refusing definition-changing merges** in this slice
   (section 3). Any future extension uses one atomic complete-definition
   `subject` facet; independent role-binding facets are rejected. Sheet has no
   merge flow, so the refusal costs no current user value.
