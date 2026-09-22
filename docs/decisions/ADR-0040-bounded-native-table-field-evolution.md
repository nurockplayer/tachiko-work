# ADR-0040: Bounded native-table field evolution

**Status:** Accepted

**Decision issue:** #392

**Related authority:** [ADR-0015](ADR-0015-stable-semantic-identity.md),
[ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md),
[ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0021](ADR-0021-progressive-semantic-strengthening.md),
[ADR-0030](ADR-0030-canonical-semantic-delta.md),
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md), and
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md).

## Context

The Driver Common Profile needs a user to evolve a native table by adding a
typed required field and by removing a field that has no surviving semantic
dependents. ADR-0021 requires schema evolution to be explicit, deterministic,
reviewable, and identity-preserving, but deliberately deferred its operation
family. This decision admits the smallest such family. It does not authorize
the broader durable-constraint and representation decisions owned separately
by #391.

## Decision

### Accepted operation meanings

The semantic authority admits exactly two field-evolution operations for one
existing schema, evaluated against one exact base revision and published as one
atomic candidate transition:

1. **Add required scalar field** creates one fresh stable `FieldId` with a
   valid human key and one of `Text`, `Number`, `Boolean`, or `Date`. The field
   is required. The candidate supplies one explicit direct stored value of that
   exact scalar type for the field in every entity already in the schema. A
   Formula or Reference value is not an initializer.
2. **Remove unreferenced scalar field** removes one existing `Text`, `Number`,
   `Boolean`, or `Date` `FieldId` and its stored values. It is permitted only
   when no surviving formula, reference, or other Accepted durable semantic
   definition depends on that `FieldId`.

Surviving schema, field, entity, relationship, and formula identities remain
unchanged. Presentation order is a projection and is never semantic identity.
Adding a field never inserts an implicit default, null, empty value, zero, or
coerced value. Removing a field never cascades, rewrites, or retargets a
dependent semantic object.

### Failure and publication laws

The operation must reject without publishing a prefix when its exact base is
stale; the target schema or field is absent; a new identity or key is invalid
or conflicts; a field type is outside this decision; an initialization is
missing or invalid; a removal has a surviving dependency; or final candidate
validation fails. Dependency analysis covers bound formulas, references, and
any other Accepted durable definition that records the field identity.

Propose and Execute retain ADR-0020's common semantic meaning, authoritative
gate, and atomic publication laws. When a Driver session exposes either
operation, one accepted operation is one user-history action. Under ADR-0033,
Undo and Redo publish the authorized inverse or forward command against the
exact current base through the ordinary gates; they do not rewind a snapshot or
revision occurrence. Rejected and no-change attempts create no user-history
action.

Concrete Rust names, transport DTO spellings, capability/authorization
footprints, diagnostics codes, and session-history storage remain Provisional.

### Representation, delta, and merge boundary

The existing version-owned representations already encode the resulting schema
and entity states. This decision selects no storage-version change, DTO
revision, or format migration. A conforming future writer must preserve the
accepted resulting values; it must not use representation compatibility as a
reason to drop them.

The Accepted semantic-delta and semantic-merge contracts already encode
schema-field creation and deletion, including their direct facts and conflict
subjects. Their evidence remains distinct from the typed command that proposes
or publishes this operation. A consumer that does not support the applicable
contract or encountered change kind must fail closed; it must not omit the
field change and report no semantic change or conflict.

### Deferred work and non-goals

This decision does not admit reference-field evolution, nullable fields,
defaults, type changes, constraint vocabulary, arbitrary batch evolution,
general migration machinery, formula rewriting, dependency repair, or a
storage-format migration. #391 retains the separate authority for durable
constraints and any `.roproj/v3` compatibility/rollback boundary.

## Consequences

An implementation remains separately unqualified until a separately scoped
delivery child of #316 receives a Ready decision with an executed,
Steward-owned acceptance seed on then-current main.
This ADR itself adds no production behavior, public wire contract, or new
format version.
