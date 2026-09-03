# Conflict Resolution Specification

Decision state: Normative Accepted logical conflict contract under
[ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md), preserving
ADR-0011's merge laws except for the explicit ADR-0031 amendment that makes
`DocumentId` same-Document admission/continuity identity rather than a mergeable
facet, and preserving the direct-state evidence boundary accepted by
[ADR-0030](../decisions/ADR-0030-canonical-semantic-delta.md).

Authority: [ADR-0031](../decisions/ADR-0031-semantic-merge-conflict-protocol.md)

Decision issue: [#46](https://github.com/nurockplayer/tachiko-work/issues/46)

Issue #223 makes the current `merge-engine` Rust conflict shape implementation
evidence for this logical contract: same-Document admission and typed
target/facet/kind/fact semantics are realized in the production merge/workspace
boundary. The concrete Rust/CLI shape is not a stabilized codec, wire, or public
SDK contract and must not be treated as permanent public meaning.

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
same supported semantic contract. Before structural reconciliation, each input
MUST already have passed ADR-0019 full semantic validation and ADR-0018 complete
formula calculation for the merge-input role. An unfinalized or invalid input is
an admission/contract failure and returns neither a Semantic Conflict set nor a
candidate under v1.

All three inputs MUST also have the same `DocumentId`. A different-Document input
is an admission/contract failure, not a conflict and not a one-sided identity
change. Input finalization does not pre-judge cross-branch interaction: the
conflict-free combined candidate is finalized again as specified below.

This same-Document rule is the explicit ADR-0031 amendment to ADR-0011's
original v0.1 merge surface, which treated `Document.id` as an ordinary
three-way-selected unit. Document title and the remaining ADR-0011 merge laws
are unchanged. The pre-#223 merge-engine behavior that selected `Document.id`
is historical implementation lag; #223 removes that selection at the production
merge/workspace boundary without stabilizing a codec, wire, or SDK shape.

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

An Entity that names a missing Schema or contains a stored `FieldId` that its
named Schema does not declare cannot pass the merge-input finalization gate. If
such stale or invalid membership is encountered, admission fails before
structural reconciliation; no canonical conflict facts, conflict set, or
candidate are produced. For admitted inputs, complete-entity membership remains
derived from the Entity's own semantic `fields` map rather than re-projected from
Schema iteration, so fact membership and ordering have one deterministic source.

For a continuing Entity, each state qualifies every present stored entry by that
state's stored Entity `SchemaId`, producing the target
`(EntityId, state.Entity.SchemaId, FieldId)`. Compare the union of those qualified
targets across `base / left / right`. For any target in the union, a state
contributes `absent` when its Entity has a different `SchemaId` or its own fields
map has no such `FieldId`. Consequently, a schema-membership change clears facts
under the old `SchemaId` and creates facts under the new `SchemaId`; identical
`FieldId` text under two Schema IDs denotes two distinct targets. Entity
create/delete parent conflicts still suppress these child facts as defined
below.

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
and complete formula calculation required by ADR-0011, ADR-0019, and ADR-0018,
even though every input passed that gate individually. Validation or calculation
failure caused by the combined candidate blocks publication and returns the
existing semantic diagnostic/calculation evidence. It MUST NOT create a Semantic
Conflict, conflict identity, or fourth conflict kind.

This separation applies to schema/data interactions, stale or invalid references,
formula failures, and other cross-fact incompatibilities that cannot be known by
examining one directly disputed facet in isolation.

### Normative logical fixtures

These fixtures are part of the logical v1 conformance contract. They describe
semantic facts, not a concrete serialization or production DTO. Unless a fixture
says otherwise, reconciliation uses conflict contract
`tachiko.semantic-conflict/v1`; `base / left / right` are admitted under one same
supported semantic contract, share `DocumentId = d:arena`, and differ only in the
facts shown. Each input has individually passed full semantic validation and
complete formula calculation. `Number`, `Text`, `Reference`, and `Formula` below
are typed semantic values; formula references are bound stable-ID expressions.

1. **Independent edits.** Base has
   `(e:goblin, s:unit, f:hp) = Number(180)` and
   `(e:goblin, s:unit, f:attack) = Number(18)`. Left changes only `f:hp` to
   `Number(210)`; right changes only `f:attack` to `Number(21)`. Structural
   reconciliation returns a candidate containing both changes and no conflict.
2. **Same-final-value edit.** Base has
   `(e:goblin, s:unit, f:hp) = Number(180)`; both sides change it to
   `Number(210)`. Structural reconciliation returns that value in the candidate
   and no conflict.
3. **Same-fact conflict.** With the same base fact, left changes it to
   `Number(210)` and right to `Number(240)`. The result is exactly one conflict:
   target `(e:goblin, s:unit, f:hp)`, facet `stored_value`, kind
   `concurrent_change`, facts `Number(180) / Number(210) / Number(240)`.
4. **Delete/update with parent suppression.** Base contains Entity `e:goblin`
   with key `goblin`, schema `s:unit`, and `f:hp = Number(180)`. Left deletes the
   Entity; right changes its key to `goblin_elite`. The result is exactly one
   Entity `e:goblin` / `subject` / `delete_modify` conflict with complete base
   and right Entity facts and an absent left fact. No child `key` or stored-value
   conflict is emitted.
5. **Incompatible concurrent addition.** Schema `s:boss` is absent in base. Left
   adds it with key `boss` and required Number field `f:hp`; right adds the same
   stable Schema target with key `boss` and required Text field `f:hp`. The
   result is exactly one Schema `s:boss` / `subject` /
   `concurrent_addition` conflict carrying both complete definitions.
6. **Rename continuity.** Base Entity `e:goblin` has key `goblin`; left changes
   the key to `goblin_elite` and right to `goblin_veteran`. The result is one
   Entity `e:goblin` / `key` / `concurrent_change` conflict. It is not a
   delete/create pair.
7. **Schema/data finalization failure.** Base Schema `s:unit` has no `f:armor`
   and Entity `e:goblin` is absent. Left adds required Number field `f:armor` to
   `s:unit`; right adds `e:goblin` under `s:unit` without an `f:armor` stored
   value. The direct facts do not conflict, so structural reconciliation returns
   a candidate. Full schema-instance validation rejects that candidate with the
   existing ADR-0019 evidence; no Semantic Conflict is emitted.
8. **Reference finalization failure.** Base contains Entities `e:source`,
   `e:old`, and `e:target`, with stored target
   `(e:source, s:unit, f:target) = Reference(e:old)`. Left changes that value to
   `Reference(e:target)`; right deletes `e:target`. Each direct change is on a
   different target, so structural reconciliation returns a candidate.
   Relationship validation rejects the dangling stable-ID reference; no Semantic
   Conflict is emitted.
9. **Bound-formula conflict.** Base stored target
   `(e:goblin, s:unit, f:power)` is
   `Formula(Add(Ref(e:goblin, f:hp), Number(1)))`; left uses `Number(2)` and right
   uses `Number(3)` in the otherwise identical bound expression. The result is
   one schema-qualified `stored_value` / `concurrent_change` conflict whose facts
   are those bound expressions, regardless of authoring spelling.
10. **Post-merge formula failure.** Base has Number values at
    `(e:goblin, s:unit, f:a)` and `(e:goblin, s:unit, f:b)`. Left changes only
    `f:a` to `Formula(Add(Ref(e:goblin, f:b), Number(1)))`; right changes only
    `f:b` to `Formula(Add(Ref(e:goblin, f:a), Number(1)))`. Each branch is
    acyclic, and the direct targets do not conflict. The merged candidate has a
    cycle and is rejected by the ADR-0018/ADR-0019 formula oracle; no Semantic
    Conflict is emitted.
11. **Schema-membership qualification.** Schemas `s:unit` and `s:boss` both
    declare Number field `f:hp`. Base Entity `e:goblin` has
    `SchemaId = s:unit` and `f:hp = Number(180)`. Left changes membership to
    `s:boss` while retaining `f:hp = Number(180)` in its Entity fields map; right
    keeps `s:unit` and changes `f:hp` to `Number(210)`. The one-sided `schema`
    facet change can merge, but the old target `(e:goblin, s:unit, f:hp)` has
    facts `Number(180) / absent / Number(210)` and therefore emits one
    `stored_value` / `delete_modify` conflict. The new target
    `(e:goblin, s:boss, f:hp)` is a distinct one-sided addition, not the same
    field occurrence.
12. **Canonical order.** When one reconciliation also produces conflicts on
    Document `d:arena` / `title`; Schemas `s:alpha` / `key` and `s:unit` / `key`;
    schema field `(s:unit, f:hp)` / `key` and then `field_type`; Entity
    `e:goblin` / `key`; and stored field `(e:goblin, s:unit, f:hp)` /
    `stored_value`, the sequence is exactly the order just listed. Subject rank,
    stable-ID order, and facet rank determine it; input collection or insertion
    order cannot change it.
13. **Admission and compatibility failure.** If any input is unfinalized or
    invalid, any input uses a different `DocumentId`, or a consumer encounters an
    unsupported conflict contract, target/facet combination, or kind, processing
    fails closed and returns no Semantic Conflict set or candidate under v1.

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

The production merge/workspace boundary realizes this logical v1 contract under
[Issue #223](https://github.com/nurockplayer/tachiko-work/issues/223). It enforces
same-Document finalized-input admission, removes three-way selection of
`Document.id`, returns the typed target/facet/kind/fact conflict object in
canonical semantic order, qualifies stored fields by each Entity state's stored
`SchemaId`, preserves parent-child suppression, and executes all 13 normative
logical fixtures above. Candidate validation and complete calculation remain
workspace finalization evidence rather than another conflict kind.

The concrete Rust DTO and CLI rendering remain implementation-level. This
realization does not select or stabilize a serialization codec, WASM/public
transport, network/SDK shape, hash/UUID identity, storage format, resolver UI,
or Git merge driver.

Issue [#47](https://github.com/nurockplayer/tachiko-work/issues/47) retains
cross-version migration work.
[ADR-0035](../decisions/ADR-0035-collaboration-causality-and-selective-convergence-boundary.md)
resolves causality/selective-convergence boundaries while preserving ordinary
Semantic Conflict for structured meaning; concrete collaboration mechanics
remain separately owned.
[ADR-0033](../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
fixes the bounded logical history/checkpoint/Git-association contract; concrete
implementations require separately Ready work.
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)
separately fixes operation/revision/event taxonomy without changing this
conflict contract.

## Goals

- explain conflicts from deterministic semantic evidence;
- preserve stable semantic intent and rename continuity;
- avoid silent data loss and catch-all conflict taxonomies;
- let CLI, GUI, Git, and AI projections share one logical contract; and
- keep mutation, validation, history, transport, and Git authority separate.
