# ADR-0031: Deterministic semantic merge conflict protocol

## Status

Accepted. This ADR amends only ADR-0011's original treatment of `DocumentId` as
a mergeable three-way-selected unit; the remaining ADR-0011 merge laws remain
Accepted.

Decision issue: [#46](https://github.com/nurockplayer/tachiko-work/issues/46)

Specified by: [Conflict Resolution Specification](../specs/conflict-resolution.md)

Related authority: [ADR-0011](ADR-0011-semantic-three-way-merge.md),
[ADR-0015](ADR-0015-stable-semantic-identity.md),
[ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md),
[ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md), and
[ADR-0030](ADR-0030-canonical-semantic-delta.md)

## Context

ADR-0011 already accepts deterministic model-level three-way merge over
`base / ours / theirs`, including same-final-value and one-sided merge,
independent semantic-field composition, delete/modify conflicts, incompatible
concurrent additions, and validation/calculation of a conflict-free candidate.
The original v0.1 merge surface also applied ordinary three-way selection to
`Document.id`. The current merge engine proves that historical behavior, but its
path-oriented conflict address and mergeable-DocumentId treatment are
implementation evidence rather than the final protocol boundary.

ADR-0015 and ADR-0030 now establish stable document identity and require direct
A-to-B Semantic Delta comparison to stay within one continuing `DocumentId`.
Human authority for Issue #46 approved the same continuity rule for three-way
reconciliation: different-Document inputs are outside one merge occurrence,
not a semantic disagreement within it. This ADR therefore makes that narrow
amendment to ADR-0011 explicitly rather than silently pretending the older
`document identity/title` wording never included identity.

ADR-0030 also provides the typed stable target and direct-state vocabulary needed
to harden the remaining machine conflict contract without turning storage paths,
Git coordinates, or another patch language into semantic authority. ADR-0019 and
ADR-0018 already own post-reconciliation semantic validation and formula
correctness.

The remaining M06 requirement is otherwise narrower than redesigning merge:
define a versioned, deterministic conflict object that independent clients can
compare, order, explain, and project while keeping mutation, validation,
history, and Git boundaries separate.

## Decision

### 1. Merge remains state reconciliation, not mutation

`base`, `left`, and `right` are admitted semantic states for one continuing
`DocumentId` under the same supported semantic contract. Each input MUST already
have passed ADR-0019 full semantic validation and ADR-0018 complete formula
calculation for the merge-input role. An unfinalized or invalid input fails
admission before structural reconciliation and produces neither a conflict set
nor a candidate.

All three inputs MUST carry that same `DocumentId`. A different-Document input is
an admission/contract failure, not a semantic conflict and not a one-sided
identity change. Making the existing merge-input operation gate explicit does
not pre-judge the combined candidate and does not create another amendment to
ADR-0011's semantic merge laws.

This supersedes only ADR-0011's original treatment of `Document.id` as an
ordinary mergeable semantic unit. Document title and all other ADR-0011 merge
facets/laws remain Accepted. The current production merge-engine behavior that
still three-way-selects `Document.id` is implementation lag and must be removed
by the separately Ready ADR-0031 production realization work; this authority PR
does not alter runtime code.

`Command | AtomicBatch` and `SemanticPatch` remain the mutation/proposal
authority. Canonical Semantic Delta may be derived for `base -> left` and
`base -> right` as deterministic branch-change evidence, but a delta is never
applied as a merge program and is not required inside each conflict object.

The existing CLI names `ours` and `theirs` are presentation aliases for the
logical `left` and `right` inputs; those words do not enter semantic identity.

### 2. Conflict target is typed stable target plus one direct facet

The v1 logical contract reuses ADR-0030 target families:

- document: `DocumentId`;
- schema: `SchemaId`;
- schema field: `(SchemaId, FieldId)`;
- entity: `EntityId`; and
- stored entity field: `(EntityId, SchemaId, FieldId)`.

The document target uses the continuing `DocumentId` only to identify the
Document whose direct `title` facet may conflict; `DocumentId` itself is never a
conflict facet.

A target is paired with one closed direct facet that identifies the disputed
semantic fact. Facets are limited to document title; complete schema subject or
schema key; complete schema-field subject, key, field type, or requiredness;
complete entity subject, key, or schema membership; and stored entity-field
value.

The `subject` facet carries complete direct state for create/delete-level
conflicts. Parent subject conflicts suppress redundant child conflicts, matching
ADR-0030's non-overlap rule. Human keys, current internal paths, filesystem/JSON
locations, UI coordinates, Git identity, and provenance are projections only.

### 3. The structural conflict-kind vocabulary is closed

Semantic Conflict v1 has exactly three structural conflict kinds:

- `concurrent_addition`: the base fact is absent and both sides add
  non-equivalent direct state;
- `delete_modify`: the base fact is present, one side makes it absent, and the
  other side changes it to non-equivalent direct state; and
- `concurrent_change`: the same continuing base fact changes on both sides to
  non-equivalent direct values.

Equal concurrent additions, same-final-value changes, one-sided changes, and
matching deletions are not conflicts. A parent create/delete conflict carries
complete direct subject state and suppresses overlapping child conflicts.

Cross-fact incompatibility that structurally reconciles is not a fourth
catch-all conflict kind. It proceeds to the existing validation/calculation
oracle.

### 4. Conflict payload is semantic evidence

Each conflict carries its typed target, direct facet, conflict kind, and
canonical `base / left / right` facts. Each fact is logically either explicit
absence or the typed direct semantic value appropriate to the facet. Complete
subject facts contain their complete direct state in stable-ID order.

References use stable IDs. Formula values use bound semantic expressions rather
than authoring text. Human explanation, localized text, paths, authors,
timestamps, Git coordinates, review metadata, and runtime occurrence identity
stay outside conflict equality.

### 5. Conflict identity is a logical composite before any encoding

The logical contract identifier is `tachiko.semantic-conflict/v1`.

Conflict equality and deterministic identity are the logical composite:

```text
(
  conflict-contract,
  DocumentId,
  typed target,
  direct facet,
  conflict kind,
  canonical base fact,
  canonical left fact,
  canonical right fact
)
```

This decision does not select a UUID, hash, digest, string format, Rust enum,
Serde layout, JSON bytes, protobuf shape, IPC schema, WASM ABI, or public SDK.
A concrete DTO may encode or hash the logical key later, but equivalent admitted
states under the same contract must yield the same logical conflict identity.
Unsupported contract versions, target/facet combinations, or conflict kinds
fail closed.

### 6. Canonical order follows semantic target order

Conflicts are canonically ordered by:

```text
(subject rank, typed target IDs, facet rank, conflict-kind rank)
```

Subject rank and typed stable-ID ordering are exactly ADR-0030's Semantic Delta
v1 order. Facet ranks are closed per target family:

- Document: `title = 0`;
- Schema: `subject = 0`, `key = 1`;
- Schema field: `subject = 0`, `key = 1`, `field_type = 2`,
  `requiredness = 3`;
- Entity: `subject = 0`, `key = 1`, `schema = 2`;
- Stored entity field: `stored_value = 0`.

Conflict-kind ranks are `concurrent_addition = 0`, `delete_modify = 1`, and
`concurrent_change = 2`.

At most one conflict exists for one logical target/facet/kind after parent-child
suppression. Ordering never uses mutable values, paths, locale, insertion/hash
iteration, Git coordinates, or runtime occurrence and has no execution meaning.

### 7. Post-merge semantic failure is a separate outcome

Structural reconciliation first yields either a canonical conflict set or one
candidate semantic state. Although each input passed its merge-input gate
individually, a conflict-free combined candidate then passes ADR-0019 full
validation and ADR-0018 complete calculation again.

Failure there returns the existing semantic diagnostic/calculation evidence and
blocks publication. It does not manufacture a `SemanticConflict`, add a conflict
kind, or change conflict identity. Schema/data incompatibility, stale
references, formula failure, and other cross-fact invalidity therefore remain
owned by their existing semantic authorities.

### 8. Git remains an adapter

A Git merge driver or review surface may consume the semantic merge result and
project files, markers, or UX around it. Git refs, SHAs, paths, repository
identity, branch names, and textual conflict markers never enter semantic
conflict identity or authority.

## Consequences

- Human, CLI, GUI, Git, and AI clients can share one deterministic logical
  conflict contract while rendering different explanations.
- Three-way reconciliation is now explicitly scoped to one continuing
  `DocumentId`, matching ADR-0015/ADR-0030 continuity; a changed document ID is
  rejected as different-document input rather than merged or conflicted.
- Stable-ID rename continuity remains distinct from delete/create replacement.
- Canonical delta and canonical conflict evidence compose without turning either
  into mutation input.
- Validation/calculation stays single-sourced instead of being duplicated inside
  merge conflict classification.
- Current path-oriented merge-engine conflict output and current
  three-way-selection of `Document.id` are explicit implementation lag, not
  protocol authority.
- Production conflict DTO/codec/runtime changes, including the DocumentId
  admission correction, require a separately Ready implementation Issue after
  this authority lands.
- ADR-0032 defines operation/revision/optional-event taxonomy without reopening
  merge conflict meaning; #49/#50 retain history/checkpoint and causality/CRDT
  work.

## Rejected alternatives

- **Path/JSON/Git coordinates as conflict identity:** rejected because
  representation and repository coordinates are not semantic identity.
- **Merging divergent `DocumentId` values:** rejected by the Human-approved v1
  continuity boundary; a merge reconciles states of one continuing semantic
  document rather than choosing which document identity survives.
- **Semantic Delta as an apply/merge program:** rejected because ADR-0030 is
  derived evidence and Command/SemanticPatch already own mutation intent.
- **Open-ended conflict kinds:** rejected because validation/calculation already
  owns cross-fact semantic invalidity and a catch-all kind would duplicate that
  authority.
- **Hash/UUID formatting as semantic meaning:** rejected; concrete encoding is
  replaceable beneath the logical identity contract.
- **Last-writer-wins for disputed semantic intent:** rejected unless a future
  Accepted decision explicitly defines it for a named semantic type.
- **Git conflict markers as semantic authority:** rejected; they remain adapter
  presentation only.

## Related

- [Issue #46](https://github.com/nurockplayer/tachiko-work/issues/46)
- [ADR-0011 semantic three-way merge](ADR-0011-semantic-three-way-merge.md)
- [ADR-0030 canonical Semantic Delta](ADR-0030-canonical-semantic-delta.md)
- [Conflict Resolution Specification](../specs/conflict-resolution.md)
- [Issue #47](https://github.com/nurockplayer/tachiko-work/issues/47)
- [Issue #48](https://github.com/nurockplayer/tachiko-work/issues/48)
- [ADR-0032 semantic execution and retained-transition taxonomy](ADR-0032-semantic-execution-and-transition-taxonomy.md)
- [Issue #49](https://github.com/nurockplayer/tachiko-work/issues/49)
- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50)
