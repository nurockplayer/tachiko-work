# ADR-0030: Canonical semantic delta as direct-state evidence

## Status

Accepted

Decision issue: [#45](https://github.com/nurockplayer/tachiko-work/issues/45)

Specified by: [Semantic Diff Specification](../specs/semantic-diff-spec.md)

Related authority: [ADR-0015](ADR-0015-stable-semantic-identity.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0024](ADR-0024-revision-pinned-semantic-patch.md), and
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md)

## Context

Tachiko Work needs deterministic machine-readable evidence for semantic review,
merge, release notes, and future collaboration. The existing `diff-engine`
proves useful implementation properties, but its Rust enum mixes direct stored
changes with calculated impact and is not a public protocol DTO.

The protocol must preserve stable semantic identity and remain independent of
storage paths and presentation. It must also compose with the existing mutation
authority: ADR-0020 owns typed `Command | AtomicBatch`, ADR-0024 owns the exact-
base `SemanticPatch` proposal envelope, and ADR-0029 keeps current state and
complete snapshots authoritative.

## Decision

### 1. Canonical Semantic Delta is derived evidence

A canonical Semantic Delta is deterministic direct-state comparison evidence
derived from admitted semantic states A and B. It is not Execute input, a patch
body, a retained event, a replay protocol, or an alternate source of semantic
truth.

Canonical mutation continues to use ADR-0020 Command/Propose/Execute authority.
SemanticPatch continues to bind one exact base to one Command or ordered
AtomicBatch under ADR-0024. A consumer may verify, project, or transform delta
evidence, but it MUST NOT treat the delta as authorization to mutate state.

### 2. One delta compares one continuing Document

The canonical revision-delta profile requires
`before.document_id == after.document_id`. Different `DocumentId` values denote
different semantic aggregates and MUST NOT produce one canonical revision
delta. Cross-document comparison may exist as separate analysis or
presentation.

`DocumentId` scopes the delta; it is not a mutable change fact. Subjects use
typed stable identities:

- schema: `SchemaId`;
- schema field: `(SchemaId, FieldId)`;
- entity: `EntityId`; and
- stored entity field: `(EntityId, SchemaId, FieldId)`.

Representation paths, serialized member positions, human keys, labels, and UI
coordinates MUST NOT identify a canonical delta subject. A key change on a
continuing stable subject is a value change, while replacement with a different
stable identity is removal plus creation.

### 3. The canonical body contains only direct semantic facts

The logical `tachiko.semantic-delta/v1` contract is closed and contains only
the direct state facts defined by the Semantic Diff Specification. Creation or
deletion of a schema or entity carries that subject's complete direct state and
suppresses overlapping child facts. Continuing subjects use non-overlapping
facts for each independently changed direct property.

Stored entity-field facts retain their applicable `SchemaId`. If a continuing
entity changes schema, an identical-looking `FieldId` under the old and new
schemas is not one field target; applicable stored values are cleared and
created under their separately qualified targets.

Stable IDs themselves are never mutable facts.

### 4. Canonical order is semantic, deterministic, and non-executable

Canonical change order is the closed subject-kind, typed-target, and change-kind
order defined by the specification. Complete definitions embedded in create or
delete facts use the same stable-ID collection order.

Ordering MUST NOT depend on filesystem paths, serialized member order, locale,
mutable human keys, insertion order, hash iteration, Git coordinates, or
runtime occurrence order. It carries no mutation semantics. Reordering a
transport container cannot turn the facts into an operation sequence.

Equivalent admitted A-to-B semantic states under the same supported contract
therefore produce the same logical canonical delta.

### 5. Derived impact and observation context stay outside delta equality

Calculated formula impact, dependency causes, validation results, risk
classification, localization, and rendered prose are derived review or
analysis projections. They may accompany a delta but MUST NOT enter its
canonical direct-state body or equality.

Runtime revision occurrences, authors, timestamps, Git identifiers, source
labels, approvals, and other provenance belong to an outer observation or
evidence context. Two independent occurrence pairs with equivalent admitted A
and B semantic states still produce the same canonical delta.

Before-values in delta facts are evidence, not optimistic-concurrency or
authorization predicates. Generic tests, hashes, JSON Pointer predicates,
scripts, and `apply_if` conditions do not belong in Semantic Delta.

### 6. Version the logical contract and fail closed

Normative consumers MUST recognize the exact logical contract identifier and
every change kind. An unsupported contract identifier or change kind fails
closed; it MUST NOT be ignored, guessed, or reinterpreted.

This decision fixes logical meaning and ordering, not a Rust enum, Serde
spelling, JSON bytes, protobuf schema, IPC protocol, WASM ABI, network
transport, or public SDK. Any future concrete mapping must preserve the logical
contract and separately declare its own compatibility and resource limits.

## Consequences

- Review, merge, release-note, and collaboration projections can share one
  deterministic direct-state evidence contract.
- Rename continuity and schema-qualified field identity remain stable across
  representation changes.
- Formula and validation impact can evolve without changing canonical delta
  equality.
- Issue #46 can define deterministic conflict-object protocol behavior without
  treating delta as an apply language.
- ADR-0032 defines operation/revision/event taxonomy without making delta an
  operation or event; ADR-0033 resolves history/Git guarantees and ADR-0035
  resolves causality/selective-convergence without changing delta.
- Production DTO and codec work requires a separately Ready implementation
  issue.

## Rejected alternatives

- **RFC 6902 JSON Patch / RFC 6901 JSON Pointer as the canonical delta:**
  rejected because representation paths and an ordered executable operation
  language would become semantic protocol concepts.
- **RFC 7396 JSON Merge Patch as the canonical delta:** rejected because JSON
  representation structure and merge-patch semantics do not express Tachiko's
  typed stable subject continuity.
- **The current `diff-engine::SemanticChange` enum as the protocol DTO:**
  rejected because current implementation shape is evidence, mixes direct and
  derived facts, and lacks parts of the accepted logical contract.
- **Occurrence/provenance fields inside canonical equality:** rejected because
  occurrence identity is distinct from semantic state content.
- **A generic precondition or apply language:** rejected because it would
  duplicate SemanticPatch and Command authority.

JSON patch standards remain available to adapters whose genuine boundary is a
JSON resource. They do not define Tachiko semantic identity or mutation
authority.

## Related

- [Issue #45](https://github.com/nurockplayer/tachiko-work/issues/45)
- [Issue #46](https://github.com/nurockplayer/tachiko-work/issues/46)
- [Issue #48](https://github.com/nurockplayer/tachiko-work/issues/48)
- [ADR-0032 semantic execution and retained-transition taxonomy](ADR-0032-semantic-execution-and-transition-taxonomy.md)
- [Issue #49](https://github.com/nurockplayer/tachiko-work/issues/49)
- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50)
- [ADR-0035 collaboration causality and selective convergence](ADR-0035-collaboration-causality-and-selective-convergence-boundary.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
