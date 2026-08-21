# ADR-0013: Stable semantic identity and typed references

Status: Proposed

Decision issue: #21

## Context

Tachiko Work requires semantic objects to survive rename, move, view changes, storage-layout changes, schema evolution, Git branches, semantic diff/merge, and future AI/client operations without changing identity merely because a human-readable name or physical representation changed.

The Developer MVP currently uses transparent string newtypes (`DocumentId`, `SchemaId`, `EntityId`, `FieldId`) whose values are also the human-readable identifiers. `FieldRef` stores an entity ID and field ID directly. As a result, renaming an entity changes its ID and requires rewriting stored entity references and formula references.

That was a useful v0.1 implementation shortcut. It must not become the durable semantic contract.

Milestone 02 therefore needs to separate:

- durable semantic identity;
- human-readable addressing/naming;
- physical storage/layout;
- derived runtime indexes.

## Decision

### 1. Mutable semantic objects use opaque stable surrogate IDs

Independently addressable mutable semantic objects use opaque stable IDs that are independent of:

- human-readable names or keys;
- UI coordinates;
- storage paths;
- physical serialization layout;
- current object content.

Normal object creation uses RFC 9562 UUID version 7.

UUID values are treated as opaque identity. Tachiko Work MUST NOT infer semantic creation time, business ordering, or authority from the UUIDv7 timestamp bits.

The Rust semantic layer SHOULD retain nominal newtypes such as `DocumentId`, `SchemaId`, `EntityId`, and `FieldId`, backed by a UUID value rather than an unconstrained string.

### 2. Human-readable keys are separate mutable semantic properties

Objects that need ergonomic human addressing have a mutable human-readable key/name/label separate from their stable ID.

Conceptually:

```text
EntityId  = opaque stable identity
EntityKey = iron_sword

FieldId   = opaque stable identity
FieldKey  = damage
```

Human-facing paths such as:

```text
iron_sword.damage
```

are addresses resolved through keys. They are not durable identity.

Renaming `iron_sword` to `moonblade` changes the key while retaining the same `EntityId`.

### 3. Stable-ID scope follows independent semantic addressability

Milestone 02 requires stable IDs for:

- Document, as the current semantic aggregate root;
- Schema;
- Field definition;
- Entity.

The same policy applies when future independently addressable objects are introduced, including likely Block, View, and Asset objects.

A separate `FormulaId` is NOT introduced while a formula is simply the value/definition of a field. The `FieldId` is the semantic anchor. A future independently reusable formula object may receive its own ID if real use cases require it.

Operation/change identity remains owned by the mutation/history work rather than this ADR.

This ADR does not introduce a separate `ProjectId` solely because `.roproj` exists. `.roproj` is a representation. A distinct Project/Workspace semantic container should be added only when a real multi-document aggregate requires one.

### 4. The semantic graph uses typed stores and typed relationships

Tachiko Work does not adopt a universal untyped `Node/Edge` graph as the core representation.

The semantic model uses typed stores and explicit typed relationships.

Conceptually:

```text
Document
├── schemas: SchemaId -> Schema
├── entities: EntityId -> Entity
└── future typed stores only when required

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

Containment and hierarchy are explicit typed relationships. Ordered sequences are stored only when order itself is semantic. Unordered semantic collections MUST NOT derive meaning from Rust insertion/iteration order.

### 5. Typed references bind to stable IDs, not names

Durable semantic references use typed stable IDs.

At minimum:

- entity relationships target `EntityId`;
- bound formula field references target `EntityId + FieldId`;
- schema membership targets `SchemaId`;
- field/value association targets `FieldId`.

Human-readable source syntax may continue to use keys. Parsing/binding resolves those keys into stable IDs before the semantic reference enters canonical bound state.

Display keys/names may accompany diagnostics or presentation, but they are not reference authority.

### 6. Rename, move, delete, and replacement semantics

- Renaming a key/name/label does not change stable ID.
- Moving or reordering an object does not change stable ID.
- Changing view/layout/storage location does not change stable ID.
- Renaming a schema or field key does not change its stable ID.
- Deletion with inbound references is rejected unless the same explicit atomic change also removes or retargets those references according to the applicable transaction/validation contract.
- A materially new semantic object MUST NOT silently reuse the ID of an object it replaces merely because it occupies the same name/path/position.
- A dangling reference remains diagnosable by target ID. The system MUST NOT silently retarget it to a different object that happens to reuse the old human-readable name.

### 7. Derived indexes and caches are runtime state

Persist source semantic relationships, not rebuildable indexes.

The following are runtime-only unless a later ADR establishes a demonstrated need otherwise:

- human key -> ID lookup indexes;
- reverse-reference indexes;
- formula dependency/reverse-dependency indexes;
- schema-membership indexes derivable from entities;
- calculated-value caches;
- storage-path indexes;
- other indexes that can be deterministically rebuilt from canonical semantic state.

Derived indexes MUST NOT become competing sources of truth.

### 8. Cross-document references are deferred

The current reference scope is one Document aggregate.

Globally unique UUID-backed IDs keep future cross-document addressing possible, but this ADR does not freeze workspace identifiers, URI syntax, remote reference behavior, or cross-document resolution policy before a real use case requires them.

### 9. Legacy/import identity is a migration concern

Normal newly authored mutable objects use UUIDv7.

Import/migration adapters MAY use deterministic namespace-based UUID generation when a stable external source identity genuinely exists or when a deterministic bridge is required for legacy name-identified Tachiko data.

Name-based UUIDs are not the normal long-lived primary-key policy. The exact Developer-MVP-v1 to hardened-format migration/version contract belongs to #25/#37/#38.

### 10. Content hashes have a separate role

Content-derived identifiers remain useful for:

- integrity roots;
- immutable snapshots;
- caches;
- assets where content identity is desired;
- package/conformance evidence.

They are not the identity of normal mutable semantic entities because changing content would necessarily change the identifier.

## Why UUIDv7

RFC 9562 standardizes UUIDv7 as a 128-bit Unix-time-based UUID designed for distributed generation and opaque byte sorting. It provides a widely implemented interoperability contract without requiring a central allocator.

ULID offers similar 128-bit time-sortable behavior and a shorter Crockford Base32 text representation, but internal semantic IDs are not intended as ordinary user-authored tokens. UUIDv7 has stronger standards/ecosystem interoperability and direct mature Rust support.

The current Rust `uuid` crate supports UUIDv7, serde, native/WASM targets, and Rust 1.85, matching Tachiko Work's current minimum supported Rust version.

## Alternatives rejected

### Human-readable names as identity

Rejected because rename changes identity and forces reference rewrites. The current v0.1 behavior demonstrates this coupling.

### Storage paths / JSON pointers / UI coordinates

Rejected because move, reorder, layout, or representation changes would alter identity.

### Content-addressed identity

Rejected for mutable semantic objects because any content edit changes the identifier. Content addressing remains useful for immutable/integrity concerns.

### Name-based UUIDs as normal identity

Rejected because mutable natural keys eventually change. RFC 9562 explicitly cautions against name-based UUID natural keys for primary-key-style identity.

### ULID as canonical identity

Rejected because its main advantage here is a shorter textual representation. Tachiko Work prefers the IETF-standard UUIDv7 ecosystem while keeping internal IDs opaque and human keys separate.

### TypeID as canonical identity

Not required. Tachiko already uses nominal Rust ID types, so type safety does not require encoding the type prefix into the canonical identifier. A future presentation layer may choose a typed display convention without changing semantic identity.

### Generic untyped graph core

Rejected because Tachiko Work needs domain guarantees around schema, fields, references, formulas, validation, and semantic operations rather than an RDF-like universal `Node/Edge` kernel.

## Consequences

Positive:

- rename/move/view/storage changes preserve semantic identity;
- bound references no longer require rewriting merely because a display key changes;
- diff/merge can reason about stable objects rather than name/path coincidence;
- future `.roproj` layout can evolve independently of identity;
- current and future clients can share typed identity without UI coordinates;
- offline/distributed object creation requires no central allocator;
- derived indexes remain replaceable implementation detail.

Costs:

- persisted objects need both opaque ID and human-readable key/name where applicable;
- raw files contain less immediately readable internal identity;
- the current v0.1 name-as-ID format requires an explicit migration/compatibility bridge;
- formula parsing must distinguish authoring names from bound semantic references;
- semantic APIs must stop treating `entity.field` display paths as permanent identity.

## Follow-up work

- #25/#37/#38: storage DTO, version envelope, canonical UUID encoding, and v0.1 migration bridge.
- #24: formula source binding from human keys to `EntityId + FieldId`.
- #23: structural versus diagnostic policy for dangling/wrong-type references.
- #20: crate ownership for semantic ID/reference types versus generation/storage concerns.
- #40: golden fixtures proving rename/move/layout invariance and deterministic serialization.
- #26: native/WASM generation parity and host capability placement.

## References

- RFC 9562: https://www.rfc-editor.org/rfc/rfc9562.html
- ULID specification: https://github.com/ulid/spec
- TypeID: https://github.com/jetify-com/typeid
- Git object model: https://git-scm.com/docs/git
- IPFS content addressing: https://docs.ipfs.tech/concepts/content-addressing/
- Rust `uuid` crate: https://docs.rs/uuid/latest/uuid/
