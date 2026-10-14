# ADR-0043: Native display names separate from keys

## Status

Accepted

Decision issue: [#525](https://github.com/nurockplayer/tachiko-work/issues/525)

Authority: [Work lead's nine frozen decisions and Astra investigation](https://github.com/nurockplayer/tachiko-work/issues/525#issuecomment-6174270161),
2026-10-15, applying the [founder pre-release decision](https://github.com/nurockplayer/tachiko-work/issues/488#issuecomment-6126009165).

Amends: [ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md)
only for the bounded pre-release exception; [ADR-0041](ADR-0041-bounded-durable-field-constraints.md)
for v3 schema/field shape, its incompatible-change rule, and logical v2 facts;
[ADR-0042](ADR-0042-first-public-portable-package-over-roproj-v3.md) for that
payload definition; [ADR-0030](ADR-0030-canonical-semantic-delta.md) and
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md) for the explicit
display-name evidence facet in their current v2 successors. Historical bodies
remain readable subject to their explicit amendment notes.

Specified by: [v3 DTOs](../specs/roproj-format-v3.md),
[v3 layout](../specs/roproj-layout-v3.md),
[portable package v1](../specs/portable-package-v1.md),
[storage versioning](../specs/storage-versioning-and-migration.md),
[semantic model](../specs/semantic-data-model.md),
[schema system](../specs/schema-system.md),
[semantic diff](../specs/semantic-diff-spec.md), and
[conflict resolution](../specs/conflict-resolution.md).

## Context

Native tables are Schemas and their columns are FieldDefinitions. Today their
creation names become lowercase ASCII keys, so ordinary names such as
"My Expenses", "Item Name", and "項目" are refused, and "Item" is presented
as "item". Document title is not a per-table name. Sheet must consume shared
canonical names rather than maintain a competing frontend label store.

ADR-0015 already separates stable opaque IDs from mutable keys used as human
addresses. Formulas, references, constraints, and saved definitions bind by ID;
adding a label does not require relationship rewriting. The current v3 storage
DTOs, however, are closed, reject unknown members, and admit an exact canonical
tree. Optional fields in a replacement decoder cannot make an older binary
understand new members. This is a pre-release authority supersession, not an
additive compatibility claim or a claim that implementation is complete.

## Decision

### 1. Supersede the pre-release v3 and package/v1 definitions

Retain `tachiko.roproj/v3` and `tachiko.portable-package/v1` as the single
current canonical writer and strict reader profiles. Supersede their present
schema/field definition to admit the optional member below. No v4, dual current
profile, implicit down-conversion, or new migration edge is selected.

This is a narrow exception to ADR-0017's historical DTO immutability and
ADR-0041's requirement that an incompatible amendment choose another version.
It applies only to this explicitly authorized unreleased definition and does
not create general permission to revise released contracts in place.
Closed-world, manifest-first, exact-tree admission remains mandatory.

The replacement reader accepts existing valid nameless v3 documents: absence
means key fallback. **Older binaries refuse name-bearing documents**, including
their package/v1 payloads. That limitation is accepted. Sheet pins an exact Work
SHA; rollback past its name-capable re-pin cannot reopen name-bearing documents
and must be disclosed. A Serde default in the new reader changes none of this.

Named native projects select the revised v3 profile explicitly. Canonical-tree
export and the existing explicit v3 opaque-project transfer route must preserve
names, as must portable `.ro` pack/unpack/fresh reopen. The v3 opaque transfer
uses existing `TWDPROJ1` framing; `TWDPROJ2` is the separate direct-ro/v2 route.
Writers targeting older representations, including direct-ro/v2 and
roproj/v1/v2, must refuse name-bearing state rather than discard names. This
does not introduce a new host envelope or authorize automatic format migration.

### 2. Optional durable names and effective-name fallback

Schema and FieldDefinition each gain optional `display_name`. Absence is
canonical omission, never JSON `null`. Effective name is the stored
`display_name` when present, otherwise `key`. `Document.title` remains
independent. IDs and keys retain ADR-0015 meaning.

In `schemas.json`, the normative Schema member order is `id`, `key`,
`display_name` when present, `fields`. FieldDefinition order is `id`, `key`,
`display_name` when present, `field_type`, `required`, `constraint`. The five
existing field members remain required and non-null. No other member, alias,
extension bag, or sidecar is added. Stable-ID collection order is unchanged.

### 3. Validate spelling without normalization

Authoring trims leading/trailing Unicode whitespace. The resulting name must
be nonempty, contain no control characters or line separators, and occupy at
most 4,096 UTF-8 bytes after trimming, before JSON escaping. Store the remaining
Unicode scalar sequence exactly as typed: no NFC/NFKC, case folding, width
folding, transliteration, or locale-dependent transformation.

Canonical storage contains the already-trimmed admitted spelling. Readers
reject invalid or untrimmed stored names rather than silently repairing them.
The shared Rust semantic/application authority enforces these rules; a dialog
may assist but is not the authority. Existing projection-byte/capacity gates
still reject oversized candidates atomically.

### 4. Compare effective names exactly in their owning namespace

Effective column names are unique within their Schema; effective table names
are unique within the Document. Compare exact Unicode scalar sequences after
trimming, including names obtained by key fallback. Shared Rust validation
enforces uniqueness at admission and publication, including merge candidates.

`Item` and `item` are distinct names; so are `Item` and `Ｉtem`, and composed
and decomposed Unicode spellings. `" 項目 "` and `"項目"` collide after
authoring trim. A named column `item` also collides with a nameless column whose
key is `item`. Report a collision rather than silently changing the label.

### 5. Derive or allocate keys separately

If the complete trimmed name already matches the current ASCII identifier
grammar `[A-Za-z0-9][A-Za-z0-9_-]*`, apply the existing ASCII lowercase key
derivation. Otherwise allocate `table_N` or `column_N`, where `N` is the
smallest free positive integer suffix (starting at 1) in the document's schema
key namespace or the owning schema's field key namespace, respectively.
Occupied keys are skipped. Stable ID allocation remains separate.

| Input name after trimming | Stored display name | Key result |
| --- | --- | --- |
| `My Expenses` | `My Expenses` | `table_1` if free, otherwise the smallest free `table_N` |
| `Item Name` | `Item Name` | smallest free `column_N` in its table |
| `項目` | `項目` | smallest free `column_N` in its table |
| `Amount_USD` | `Amount_USD` | `amount_usd` |
| `Item` / `item` | two distinct admitted labels | both derive candidate `item`; see the bounded open question below |

Key uniqueness remains an independent invariant; label uniqueness does not
waive it. A display rename changes neither stable ID nor allocated key. CJK
names never become transliterated or lossy ASCII slugs.

### 6. User-facing column rename changes only the label

The existing user-facing column rename becomes a revision-checked display-name
edit. It leaves FieldId, key, address, stored values, bound relationships,
formula results, and constraints unchanged. Its inverse history restores the
previous optional name, including absence, through ordinary current-base
admission and forward-only undo/redo. Stale, invalid, or rejected operations
publish nothing and preserve semantic state and history stacks.

Explicit low-level key rename remains a separate operation under ADR-0015;
this decision does not abolish mutable keys. Native creation and add-column
paths use the naming rules above. Table rename UI/API is deferred to a
separately bounded follow-up.

### 7. Add an explicit display-name delta and conflict facet

Supersede the pre-release closed logical v2 vocabularies from ADR-0041 with
these additions; frozen logical v1 remains unchanged and must refuse
name-bearing state rather than omit its meaning. No public wire/SDK encoding
is selected. All existing continuity, direct-fact, parent suppression,
canonical stable-target ordering, and fail-closed laws remain.

For `tachiko.semantic-delta/v2`, append
`schema_display_name_changed` at Schema change rank 3 and
`schema_field_display_name_changed` at schema-field change rank 6, after
constraint rank 5. Each carries typed `before` and `after` optional stored name
facts: explicit logical absence or the exact admitted string. Complete schema
and field create/delete payloads also include their optional stored names.
Names are direct semantic facts, not localized rendered prose. A label-only
change cannot be an empty delta, a key change, or a delete/create replacement.

For `tachiko.semantic-conflict/v2`, append `display_name` at Schema facet rank
2 and schema-field facet rank 5, after constraint rank 4. Complete subject
facts include optional stored names. Unequal concurrent label edits on one
stable target produce one `display_name` / `concurrent_change` conflict with
canonical `base / left / right` optional-name facts. Absent and present names
remain distinct facts even when their effective names happen to match.
Equal concurrent edits and independent key/label edits retain ordinary merge
laws; parent deletion/modification evidence includes names and suppresses
redundant children. Cross-object name collisions after structural merge are
validation failures, not a new conflict kind. Unsupported facts/facets fail
closed; no consumer may silently lose label evidence.

### 8. Project effective names while retaining addressing

Add required `display_name: string`, containing the effective name, to each
`CollectionSummary` and each projected table column. Existing `id`, `key`,
collection lookup semantics, and `FieldProjection.address` are unchanged.
Absent durable names therefore still yield a projection string equal to the
key. Presentation refresh/invalidation includes schema and field names, even
for empty tables; labels never identify operation targets.

### 9. Preserve the first-slice exclusions

CSV/XLSX label promotion to canonical names is a separate bounded slice.
Existing private interop labels are not canonical native names; do not claim
that imported labels survive CORE `.ro` until promotion is delivered.
Source worksheet/A1 resolution rules remain distinct from native comparison.
Formula authoring remains key-based and expressions remain ID-bound; spaced
or CJK label tokens and formula-syntax changes are outside this slice.

[#452](https://github.com/nurockplayer/tachiko-work/issues/452) constraint
admission remains separately owned: the current runtime refusal stays. Native
display names do not enable constraints or complete #452. CORE packages still
omit Sheet source bytes, interop/fidelity metadata, and report settings;
ADR-0042's actual attachment-loss disclosure remains required.

## Consequences and rejected alternatives

People can use their own language and retain the case they typed while stable
IDs, key-authored formulas, and existing addresses remain useful. Names become
durable reviewable semantic facts, so validators, projections, history, storage,
delta, and merge implementations must agree. Exact comparison intentionally
allows visually similar distinct names. Named `schemas.json` bytes change its
SHA-256 leaf, package root, affected CRCs, and ZIP bytes; hashes are consequences
of the definition, not a compatibility mechanism.

Rejected alternatives are an additive Serde default presented as older-binary
compatibility; v4 or another pre-release package profile; dual writers/readers
or down-conversion; normalization/folding; transliterated CJK keys; and a
frontend-owned canonical label store. Storage still preserves Unicode and
rejects unknown required semantics. Future incompatible released changes need
explicit versioned authority.

## Golden vectors and delivery boundary

Existing valid nameless v3 and current v1-over-v3 package fixtures **MUST remain
byte-identical**, including length, hashes, CRCs, and repack bytes. Absence
must not emit `null`, a fallback-name member, or unrelated golden churn.
The separate implementation slice **MUST add a new independent static named
fixture** with exact canonical `schemas.json`, package bytes/root/CRCs, and
byte-identical repack/fresh-reopen evidence. Do not regenerate expected bytes
from the candidate codec as the independent oracle. Historical superseded
v1-over-v1 package evidence keeps its ADR-0042 status.

Authority only: no code, tests, scripts, CI, or fixtures change here. Native
implementation is a separate Guarded PR under #525 after this authority merges,
with a single writer and fresh independent review. Sheet's re-pin, effective-name
rendering, dialog changes, and actual host save/reopen qualification are
Sheet-owned. Neither this decision nor Work CORE evidence completes that
consumer work. Documentary consistency, links, and diff checks validate this
PR; executable acceptance belongs to the implementation slice. The Work lead
owns independent review and integration.

## Bounded open question

The frozen decisions do not choose how to resolve a collision of an
ASCII-derived key with an existing key (for example `Item` and `item` both
derive `item` despite distinct valid labels). The implementation handoff needs
the lead's bounded disposition on refusal versus allocation in that case.
This ADR does not invent a suffix rule or silently fold display-name equality.
