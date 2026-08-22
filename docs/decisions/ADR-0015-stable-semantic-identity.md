# ADR-0015: Stable semantic identity and mutable human keys

## Status

Accepted

## Context

Tachiko Work requires semantic objects to retain meaning while presentation,
storage layout, and human-facing names evolve. The Product Constitution and
Design Principles already establish that presentation coordinates, storage
paths, and physical serialization must not silently become durable semantic
identity.

The v0.1 implementation does not yet satisfy that requirement. `DocumentId`,
`SchemaId`, `EntityId`, and `FieldId` are currently string-backed identifiers,
and ADR-0013 records the implemented entity lifecycle in which `rename` changes
an entity's intrinsic identifier and rewrites every typed reference that names
it.

That behavior is valid implementation history, but it conflates two concepts:

- durable semantic identity;
- mutable human-readable naming/addressing.

Milestone 02 hardening needs to separate those concepts before persistence,
formula binding, diff/merge behavior, and public APIs make the conflation costly
to reverse.

This ADR resolves the architectural identity contract. It does not freeze the
final identifier-generation algorithm or storage encoding.

## Alternatives considered

### Keep names as durable identity

This preserves the current implementation and makes authoring tokens convenient,
but rename becomes identity replacement. Every rename requires reference
rewrites, merge semantics become noisier, and storage paths or human naming
conventions can leak into semantic meaning.

Rejected.

### Use content-derived identity

Content hashes are excellent for immutable snapshots, integrity, caches, and
content-addressed assets. They are a poor identity for mutable semantic objects
because legitimate content edits necessarily change the hash.

Rejected for mutable semantic objects.

### Use a universal untyped node/edge graph

A generic graph would make every object look structurally uniform, but it would
weaken domain guarantees and move semantic typing into conventions layered on
top of an untyped substrate.

Rejected for the Milestone 02 core model.

### Freeze UUIDv7 as the permanent semantic contract

UUIDv7 is a strong current implementation choice: standardized, decentralized,
widely supported, and suitable for native/WASM generation. However, the project
constitution explicitly prefers freezing semantic invariants while keeping
replaceable mechanisms replaceable unless stronger evidence requires a public
ecosystem commitment.

Rejected as a permanent semantic invariant. UUIDv7 remains the preferred
provisional generation strategy for normal Milestone 02 object creation.

## Decision

### 1. Durable semantic identity is opaque and stable

Independently addressable mutable semantic objects use typed, opaque surrogate
identities that are independent of:

- human-readable names or keys;
- UI coordinates or view layout;
- `.roproj` paths or physical file placement;
- serialization order;
- mutable object content.

Identity MUST survive rename, move, view changes, and storage-layout changes.
Application logic MUST treat IDs as opaque values rather than deriving business
meaning from their representation.

### 2. Human-facing keys and labels are mutable addresses

Human-readable keys, names, and labels are distinct from durable identity.

Examples:

```text
EntityId  = opaque stable identity
EntityKey = iron_sword

FieldId   = opaque stable identity
FieldKey  = damage
```

CLI, formula source text, importers, and future graphical interfaces MAY expose
human-friendly paths such as `iron_sword.damage`. Binding resolves those
addresses to typed stable IDs. Bound semantic references store IDs, not mutable
keys.

Renaming a key or label MUST NOT change the object's durable semantic identity.

### 3. Milestone 02 stable-ID scope

The current semantic aggregate requires stable IDs for:

- Document;
- Schema;
- Field definition;
- Entity.

The same rule applies when future Block, View, Asset, or other first-class
objects become independently referenceable, movable, mergeable, or expected to
survive revisions.

Do not introduce `ProjectId` merely because `.roproj` exists. `.roproj` is a
physical/source materialization, not proof that a separate semantic workspace
aggregate is required.

Do not introduce a separate `FormulaId` while a formula remains the definition
or value anchored by an existing field. Add one only if formulas later become
independently addressable first-class objects.

Operation/change identity remains owned by the mutation/history work rather than
this ADR.

### 4. The semantic graph uses typed stores and typed relationships

The core model uses explicit typed stores and domain relationships rather than a
universal untyped `Node` / `Edge` container.

Conceptually:

```text
Document
├── schemas: SchemaId -> Schema
├── entities: EntityId -> Entity
└── future typed stores only when real use cases require them

Schema
└── fields: FieldId -> FieldDefinition

Entity
├── schema: SchemaId
└── values: FieldId -> Value

Reference value
└── target: EntityId

Bound formula reference
└── entity: EntityId + field: FieldId
```

Containment and hierarchy are represented by explicit typed fields. Cross-object
semantic references use typed stable IDs. Ordered collections are semantic only
when order itself has meaning; unordered stores MUST NOT inherit insertion,
filesystem, locale, or hash iteration order as semantic state.

### 5. Persist semantic source relationships, not derived indexes

Canonical semantic state persists the source relationships needed to reconstruct
meaning.

The following are runtime-derived indexes/caches and MUST NOT become competing
sources of truth merely for convenience:

- human key -> ID lookup indexes;
- reverse-reference indexes;
- formula dependency/reverse-dependency indexes;
- schema-membership indexes derivable from canonical entities;
- calculation caches/materialized results unless separately defined as semantic
  state;
- storage-path indexes.

Derived indexes must be rebuildable deterministically from canonical semantic
state.

### 6. Rename, move, replacement, and delete semantics

- Renaming a human key/label leaves the stable ID unchanged.
- Moving, reordering, or changing a view/storage layout leaves stable IDs
  unchanged.
- Renaming schemas or fields leaves their stable IDs unchanged.
- Replacing an object with materially new semantic meaning creates a new ID
  rather than recycling the old identity.
- Deleting an object with inbound references is rejected unless the same explicit
  atomic semantic operation removes or retargets those references.
- Dangling references, when representable by a higher-level editing/validation
  policy, remain diagnosable by target ID and MUST NOT silently retarget by a
  matching human name.

The exact structural-rejection versus temporary-diagnostic policy remains owned
by the validation architecture work.

### 7. Reference scope remains document-local for v1

Milestone 02 does not freeze cross-document addressing syntax or workspace
identity semantics. Semantic references resolve within the current Document
aggregate.

Using globally collision-resistant IDs for normal creation keeps a future
cross-document path open without committing the core to that feature now.

### 8. ID generation is a replaceable mechanism

For normal Milestone 02 object creation, the preferred provisional generator is
RFC 9562 UUIDv7 because it provides standardized 128-bit identifiers,
decentralized/offline creation, broad ecosystem support, and practical
native/WASM implementations.

However:

- UUIDv7 is NOT part of the permanent semantic meaning of an object;
- application logic MUST NOT interpret UUIDv7 timestamp bits as semantic
  creation-time data;
- ID construction MUST remain behind an explicit creation/generation seam rather
  than spreading UUID-specific behavior across semantic APIs;
- storage/versioning work owns the canonical persisted encoding;
- import and migration adapters MAY use deterministic namespace-based IDs when a
  stable source identity genuinely exists and the migration contract defines it.

ULID, TypeID, or another future generator therefore does not require changing
the semantic identity model, only the generation/encoding boundary if the
project later has evidence to switch.

## Relationship to ADR-0013

ADR-0013 remains authoritative historical evidence for the implemented v0.1
entity lifecycle, including preview-first immutable mutation and safe reference
validation.

This ADR supersedes only the parts of ADR-0013 that treat a human-facing entity
identifier as both name and durable identity. In the hardened model, entity
rename changes the human key/name while preserving `EntityId`; bound references
therefore do not need identity rewrites merely because a key was renamed.

The implementation must migrate deliberately. Current v0.1 behavior must not be
silently reinterpreted as if old string identifiers had always been surrogate
IDs.

## Consequences

Positive:

- Rename no longer creates semantic identity churn.
- Formula binding, references, semantic diff, and merge can reason about object
  continuity independently of display names and storage layout.
- `.roproj` can evolve its physical layout without changing object identity.
- Future clients and AI can use stable semantic references while presenting
  human-friendly addresses.
- The project gains a durable identity invariant without freezing a specific ID
  generator forever.

Negative / migration cost:

- Current v0.1 name-like IDs and rename workflows require an explicit migration.
- Human key lookup becomes a separate concern and requires uniqueness/diagnostic
  rules where ergonomic authoring depends on keys.
- Existing tests and fixtures that equate rename with ID replacement must be
  updated or retained as historical-format migration fixtures.

## Required follow-up

- ADR-0016: keep typed IDs/references in the lowest semantic layer while keeping
  ID generation and storage-specific encoding behind the workspace-engine/host
  and storage seams rather than leaking through unrelated crates.
- #23: define structural rejection versus temporary diagnostics for dangling or
  wrong-type references.
- #24: bind formula source addresses to stable `EntityId + FieldId` references.
- #25/#37/#38: define versioned DTO migration and canonical persisted encoding
  for stable IDs without inheriting current Rust struct serialization as the
  public format contract.
- #40: add fixtures proving rename/move/storage-layout invariance and migration
  behavior.

## Related

- Product Constitution §§2.2, 2.5, 2.7, 6
- Design Principles §§3, 7, 9, 12
- `docs/architecture/semantic-core-rationale.md` §§2, 3, 7, 8, 12
- ADR-0001
- ADR-0003
- ADR-0013
- Issues #20, #21, #23, #24, #25, #37, #38, #40
