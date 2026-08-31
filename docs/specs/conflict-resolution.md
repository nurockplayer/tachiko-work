# Conflict Resolution Specification

Decision state: Normative Accepted logical conflict contract under
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), preserving
ADR-0011's merge laws except for the explicit ADR-0031 amendment that makes
`DocumentId` same-Document admission/continuity identity rather than a mergeable
facet, and preserving the direct-state evidence boundary accepted by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md).

Authority: [ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md)

Decision issue: [#46](https://github.com/nurockplayer/tachiko-work/issues/46)

The current `merge-engine` Rust conflict shape is implementation evidence. Its
path-oriented address, concrete enum/serialization shape, and legacy
three-way-selection of `Document.id` are not this protocol DTO and must not be
treated as permanent public meaning.

## Principle

Conflicts are deterministic semantic reconciliation evidence. They are not text
merge markers, mutation programs, validation diagnostics, or Git authority.

## Semantic Conflict v1

The logical contract identifier is exactly `tachiko.semantic-conflict/v1`.

A canonical conflict-set result is scoped by one continuing `DocumentId` and
contains one canonically ordered **non-empty** sequence of semantic conflict
objects. An empty conflict sequence is not a structural merge result;
conflict-free reconciliation returns the candidate semantic state instead. This
is a logical DTO contract. It does not select Rust or Serde layout, JSON or
protobuf bytes, IPC, WASM ABI, network transport, UUID/hash spelling, or public
SDK shape.

### Admission

`base`, `left`, and `right` MUST be admitted semantic `Document` states under the
same supported semantic contract. All three MUST have the same `DocumentId`.
A different-Document input is an admission/contract failure, not a conflict and
not a one-sided identity change.

This same-Document rule is the explicit ADR-0031 amendment to ADR-0011's
original v0.1 merge surface, which treated `Document.id` as an ordinary
three-way-selected unit. Document title and the remaining ADR-0011 merge laws
are unchanged. Until separate production realization work lands, the current
merge-engine behavior that still selects `Document.id` is implementation lag.

The existing CLI's `ours` and `theirs` terminology maps to logical `left` and
`right`. Those presentation labels are not conflict identity.

Structural reconciliation otherwise follows ADR-0011:

- equal branch facts pass through;
- a fact changed only on one side is accepted;
- equal changes on both sides are accepted;
- non-equivalent competing changes produce a conflict;
- independent direct facts may merge; and
- no partial candidate is published when conflicts exist.

### Stable targets and closed direct facets

Each conflict uses one typed stable target plus one direct facet:

| Subject rank | Subject | Logical target | Allowed facets and facet rank |
| ---: | --- | --- | --- |
| 0 | Document | `DocumentId` | `title = 0` |
| 1 | Schema | `SchemaId` | `subject = 0`, `key = 1` |
| 2 | Schema field | `(SchemaId, FieldId)` | `subject = 0`, `key = 1`, `field_type = 2`, `requiredness = 3` |
| 3 | Entity | `EntityId` | `subject = 0`, `key = 1`, `schema = 2` |
| 4 | Stored entity field | `(EntityId, SchemaId, FieldId)` | `stored_value = 0` |

The target families and subject ranks are the same as
`tachiko.semantic-delta/v1`. Stable IDs use that contract's exact logical Unicode
scalar ordering. The Document target's `DocumentId` identifies the continuing
Document whose `title` may conflict; `DocumentId` itself is not a direct conflict
facet.

The `subject` facet is used only where complete direct subject state is needed
for create/delete-level conflict evidence. A complete schema subject contains
its key and all `(FieldId, field definition)` entries in stable-ID order. A
complete schema-field subject contains key, field type, and requiredness. A
complete entity subject contains its key, its stored `SchemaId`, and **every**
`(FieldId, typed stored value)` entry present in that Entity's own semantic
`fields` map, ordered by the same `FieldId` rule. Membership is defined by the
Entity state itself; it MUST NOT be filtered through the currently resolved
Schema's field declarations.

If an admitted Entity names a missing Schema or contains a stored `FieldId` that
the named Schema does not declare, the complete entity subject still preserves
that `SchemaId` and stored field entry as direct comparison evidence. Such stale
or invalid membership is handled by the existing validation/finalization
authority after structural reconciliation; it does not make canonical conflict
facts implementation-dependent.

Stored values preserve admitted semantic type. References use stable IDs.
Formulas use bound semantic expressions rather than authoring text.

Targets and facets MUST NOT use `.roproj` paths, JSON Pointers, serialized
positions, mutable human keys as identity, UI coordinates, Git identifiers, or
runtime occurrence IDs.

### Closed conflict-kind vocabulary

The v1 conflict kinds are exactly:

| Conflict rank | Kind | Required relation among canonical facts |
| ---: | --- | --- |
| 0 | `concurrent_addition` | base is absent; left and right are present and non-equivalent |
| 1 | `delete_modify` | base is present; one side is absent; the other is present and non-equivalent to base |
| 2 | `concurrent_change` | base, left, and right are present; both sides differ from base and from each other |

No other structural conflict kind exists in v1.

Equal concurrent additions are accepted. Matching deletions are accepted.
Same-final-value changes and one-sided changes are accepted. A consumer MUST NOT
invent a catch-all conflict merely because a conflict-free merged candidate later
fails validation or formula calculation.

### Parent-child non-overlap

A parent subject create/delete conflict carries complete direct subject state and
suppresses redundant child conflicts beneath that subject.

Examples:

- deleting a schema on one side while the other side changes that schema or one
  of its fields produces one schema `subject` / `delete_modify` conflict;
- deleting an entity on one side while the other changes its key, schema, or
  stored values produces one entity `subject` / `delete_modify` conflict;
- concurrently adding the same previously absent schema/entity target with
  different complete direct state produces one parent `subject` /
  `concurrent_addition` conflict.

For continuing subjects without a parent create/delete conflict, independent
facets remain independently conflictable. An absent-versus-change transition for
one stored value is `delete_modify` on the schema-qualified `stored_value` facet.

### Canonical conflict facts

Every conflict carries canonical `base`, `left`, and `right` facts. Each fact is
logically one of:

- explicit `absent`; or
- the typed direct semantic value required by the target/facet.

Absence is semantic comparison evidence, not `null`, zero, false, an empty
string, or a missing transport member. Concrete encodings must preserve this
distinction unambiguously.

Human explanations, localized text, current paths, authors, timestamps,
approvals, Git coordinates, review metadata, and runtime revision occurrences
may accompany a conflict in outer evidence. They do not participate in canonical
conflict equality.

### Deterministic conflict identity and equality

Canonical conflict identity is the logical composite:

```text
(
  contract = tachiko.semantic-conflict/v1,
  DocumentId,
  typed target,
  direct facet,
  conflict kind,
  canonical base fact,
  canonical left fact,
  canonical right fact
)
```

Equivalent admitted `base / left / right` states under the same contract produce
the same logical conflict identity. A concrete implementation MAY hash or encode
this composite, but the hash algorithm, UUID choice, textual rendering, DTO
member names, and transport bytes are not semantic identity.

A normative consumer that does not support the contract identifier, a target
family, an allowed target/facet combination, or a conflict kind MUST fail closed
rather than skip, guess, or reinterpret it.

### Canonical ordering

Sort conflict objects lexicographically by:

```text
(subject rank, typed target IDs, facet rank, conflict-kind rank)
```

Typed target IDs use the exact `tachiko.semantic-delta/v1` stable-ID ordering:
lexicographic Unicode scalar value, shorter equal-prefix sequence first, with no
normalization, case folding, locale, or interpretation of identifier contents.

At most one conflict exists for a logical `(target, facet, kind)` after
parent-child suppression. Canonical ordering MUST NOT use mutable values,
filesystem paths, serialized order, locale, human keys as identity, insertion or
hash iteration order, Git coordinates, or runtime occurrence order. Ordering has
no execution semantics.

### Rename continuity and replacement

A mutable key change on a continuing stable target preserves identity. It may
produce a `key` / `concurrent_change` conflict when both sides choose different
keys, but it MUST NOT become delete/create merely because display names differ.

Replacement under a different stable ID is deletion plus creation according to
the ordinary merge semantics. Stable IDs themselves are never mutable facets.

### Relationship to Semantic Delta

Canonical `base -> left` and `base -> right` Semantic Deltas are deterministic
branch-change evidence and may help an implementation locate candidate disputed
targets. A conflict object does not embed or require full delta envelopes, and
its equality does not include delta transport/provenance.

Semantic Delta remains direct-state evidence, not Execute input. Conflict objects
are reconciliation evidence, not mutation input. `Command | AtomicBatch` and
SemanticPatch remain the mutation/proposal vocabulary.

### Structural result versus semantic finalization

Structural reconciliation has two possible logical results:

1. one canonically ordered non-empty conflict set; or
2. one conflict-free candidate semantic state.

A conflict-free candidate MUST then pass the existing full semantic validation
and complete formula calculation required by ADR-0011, ADR-0019, and ADR-0018.
Validation or calculation failure blocks publication and returns the existing
semantic diagnostic/calculation evidence. It MUST NOT create a Semantic Conflict,
conflict identity, or fourth conflict kind.

This separation applies to schema/data interactions, stale or invalid references,
formula failures, and other cross-fact incompatibilities that cannot be known by
examining one directly disputed facet in isolation.

### Git integration

Git is an optional adapter. A merge driver or review surface may consume the
semantic merge result and project ordinary files, text markers, comments, or UX
for humans. Git refs, SHAs, branches, repositories, paths, and textual conflict
markers MUST NOT enter semantic conflict identity or override semantic merge
results.

## Human-readable projection

A client may render a conflict using current keys or localized prose, for
example:

```text
Goblin.hp
base: 180
left: 210
right: 240
```

That rendering is derived presentation. Canonical identity remains the typed
stable target, `stored_value` facet, `concurrent_change` kind, and canonical
base/left/right facts.

## Implementation status and follow-up

The current `merge-engine` demonstrates deterministic three-way selection,
typed base/ours/theirs payload, stable-ID-aware semantic behavior inherited from
the current model, and no partial output on conflict. Its current `path` address,
concrete Rust conflict shape, and three-way selection of `Document.id` are
Provisional implementation evidence and do not satisfy the complete logical v1
protocol above by themselves.

This authority/specification does not authorize production DTO, codec, CLI
output, WASM/public transport, or merge-engine changes. After this authority is
merged, a separate Ready implementation Issue must own production realization,
including same-Document admission enforcement, removal of mergeable
`Document.id`, and executable fixtures for the accepted protocol.

Issues [#47](https://github.com/nurockplayer/tachiko-work/issues/47),
[#48](https://github.com/nurockplayer/tachiko-work/issues/48),
[#49](https://github.com/nurockplayer/tachiko-work/issues/49), and
[#50](https://github.com/nurockplayer/tachiko-work/issues/50) retain cross-version
migration, operation/revision/event taxonomy, history/checkpoint/Git association,
and causality/CRDT work respectively.

## Goals

- explain conflicts from deterministic semantic evidence;
- preserve stable semantic intent and rename continuity;
- avoid silent data loss and catch-all conflict taxonomies;
- let CLI, GUI, Git, and AI projections share one logical contract; and
- keep mutation, validation, history, transport, and Git authority separate.
