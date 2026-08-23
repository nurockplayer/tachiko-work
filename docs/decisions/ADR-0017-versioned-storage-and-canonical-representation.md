# ADR-0017: Versioned storage DTOs, explicit migration, and canonical representation

## Status

Accepted

Decision issues: #25, #37, #38, #96

## Context

Tachiko Work already has three Accepted boundaries that constrain persistence:

- ADR-0003: the semantic model is architectural truth, `.roproj` is the canonical editable/Git-native materialization, and `.ro` is a derived portable artifact;
- ADR-0015: durable semantic identity is stable and opaque, while human keys, names, paths, presentation, and storage layout are not identity;
- ADR-0016: storage depends on semantic contracts and remains a sibling boundary to the shared application layer; Rust `pub` fields and `serde` derives are not automatically public wire contracts.

The v0.1 implementation is useful evidence but does not yet satisfy that boundary completely. `crates/storage` owns a `DocumentV1` wrapper and a `format_version`, yet its DTO still embeds `semantic-core` identifiers, schemas, entities, values, and expression serialization. Changes to semantic Rust structures can therefore change persisted behavior accidentally.

Milestone 02 also needs deterministic Git-friendly bytes, explicit unsupported-version behavior, and an identity migration away from the v0.1 name-like identifiers without allowing the serializer to decide unresolved numeric semantics.

Research for #25/#37/#38 compared RFC 8259/I-JSON, RFC 8785 JCS, Unicode normalization, RFC 9562 UUIDs, versioned-storage precedents, and schema-evolution behavior in mature systems. The conclusion is to promote the persistence boundary and compatibility invariants while keeping representation-specific mechanics replaceable and versioned.

## Decision

### 1. Semantic types are not persisted DTOs

Every supported durable representation version MUST have a storage-owned, version-specific DTO schema.

Semantic-core Rust structures, module layout, field order, enum tagging, `serde` attributes, and collection implementations MUST NOT become the durable wire schema merely because they are convenient to serialize.

Conversion between storage DTOs and semantic types is explicit. A version DTO may encode the same concepts as semantic-core, but its representation remains owned by storage and versioned independently.

Historical DTO definitions are immutable compatibility contracts once published. A semantic-core refactor must not retroactively change the decoder for an existing representation version.

### 2. Version selection precedes semantic decoding

A persisted representation MUST identify the representation contract required to decode it before its semantic body is interpreted.

Milestone 02 uses one representation-local `format_version` dimension unless a later Accepted decision demonstrates an independent lifecycle that requires another version axis.

`format_version` is not the semantic model version, application release version, or future `.ro` package/container profile version.

Version namespaces are representation-local. The existing direct `.ro` JSON v1 compatibility profile and a future `.roproj` v1 profile are not the same protocol merely because both may use the integer `1` inside their own representation context.

### 2.1. Finite direct-JSON admission precedes version inspection

The current normal direct-JSON reader MUST apply a finite complete-input byte
admission before UTF-8 validation or any complete-input JSON scan. An input
over that profile limit fails with representation-local `ResourceLimit` before
any latent UTF-8, JSON syntax, duplicate-member, missing-version,
malformed-version, or unsupported-version failure can be observed.

Milestone 02 sets the Provisional normal direct-JSON profile limit to exactly
8 MiB (`8 * 1024 * 1024` bytes). Exactly 8 MiB is admitted; one byte more is
rejected. This normal profile covers legacy direct-JSON v1, direct-JSON v2,
missing or malformed versions, and unsupported future versions entering the
current reader. Admitted input retains the existing strict precedence and
version dispatch. The v2 256-byte number-token limit remains a separate,
subordinate version-profile mechanism.

```text
InvalidUtf8
→ InvalidJson
→ DuplicateMember
→ VersionMissing / VersionMalformed
→ UnsupportedVersion
→ selected-profile subordinate ResourceLimit
```

The 8 MiB value is not a semantic document limit, product limit, Number rule,
identity rule, migration invariant, `.roproj` constraint, package/export
constraint, or UI constraint. A future explicit legacy import or migration
operation MAY define a different finite, caller/host-owned admission profile
when concrete compatibility evidence requires it. Such an operation must be
an explicit capability boundary and must not silently become normal open/read
policy. An unbounded bypass such as `--no-limit`, `usize::MAX`, or equivalent
is forbidden.

### 3. Unsupported or ambiguous representation semantics fail closed

Unknown/newer required semantics MUST NOT be guessed, silently discarded, or reinterpreted.

After direct-JSON admission, a reader may perform generic UTF-8/JSON structural checks and the minimum version-envelope inspection needed to identify an unsupported representation. It MUST NOT decode the unsupported semantic body, canonical-rewrite it, migrate it, or mutate durable state.

Missing, malformed, and unsupported versions are distinct storage failures.

Opening or reading a supported historical version MUST NOT implicitly rewrite or upgrade it. Durable migration is an explicit operation.

### 4. Supported versions are closed-world unless an extension area is explicitly versioned

Within a recognized representation version, unknown fields MUST be rejected unless that version explicitly defines an opaque extension area together with preservation, collision, canonicalization, and migration semantics.

Generic `ignore unknown fields` behavior is not forward compatibility when the reader cannot losslessly preserve and re-emit the unknown semantics.

Duplicate JSON member names are invalid at every object depth. Duplicate comparison occurs after JSON escape decoding, so names such as `"a"` and `"\u0061"` collide.

Semantic duplicate identities in record collections are a separate DTO/semantic uniqueness failure, not a JSON duplicate-member failure.

### 5. Each representation version defines one canonical emission

For a given representation version and accepted semantic state, canonical writing MUST be deterministic across supported platforms and independent of:

- insertion or hash-map iteration order;
- filesystem enumeration;
- mutable human keys, names, paths, or presentation coordinates;
- locale;
- wall clock;
- thread completion order;
- dependency-library output accidents not included in the normative version contract.

Unordered semantic collections are canonically ordered by stable semantic identity or another explicitly Accepted stable semantic key. Ordered semantic sequences preserve semantic order and MUST NOT be re-sorted as a serialization convenience.

Each version-specific DTO defines normative record-member ordering and required/optional/default/null behavior. Rust declaration order or serializer defaults are not the contract unless the version specification explicitly adopts that ordering.

### 6. Canonical text preserves user strings

Textual persisted representations use valid UTF-8.

Storage MUST preserve the decoded Unicode scalar sequence and MUST NOT apply NFC, NFD, NFKC, NFKD, confusable folding, case folding, or another semantic normalization implicitly.

String escaping may use a deterministic version-defined spelling, but escaping is representation only; it must not change decoded string meaning.

If a future semantic rule defines normalization/equality for a particular key or field, that rule belongs to the semantic/validation contract or an explicit migration, not to universal storage canonicalization.

### 7. Serialization does not define numeric meaning

The persistence layer MUST NOT infer semantic numeric equivalence from Rust `f64`, JavaScript `Number`, a serializer's shortest-float algorithm, or another host/runtime convenience.

#24 owns the durable numeric/formula semantic contract, including `-0`, exact integer/decimal boundaries, overflow, rounding, and other numeric edge meaning.

A representation version may define canonical numeric spelling only for numeric semantics already Accepted by the owning semantic contract. Until #24 resolves those edge semantics, numeric canonicalization remains deliberately incomplete rather than being decided accidentally by storage.

### 8. Migrations are explicit, version-labelled, deterministic where source identity permits, and testable

Historical representation changes are modeled as explicit migration edges rather than by deserializing every historical format directly into whatever the current semantic Rust structures happen to be.

The default topology is version-labelled DTO-to-DTO migration, normally through adjacent versions:

```text
V1 DTO -> V2 DTO -> ... -> current DTO -> semantic model
```

An optimized direct migration may be added only when fixtures prove it is observationally equivalent to the composed path for the supported input class.

Tachiko Work does not create a permanent second "storage semantic model" as a universal migration IR. The semantic model remains architectural truth; historical DTOs and migrations remain representation history.

Legacy v0.1 name-like identifiers MUST NOT be silently reinterpreted as ADR-0015 surrogate identities. The migration establishes new stable identities, rewrites typed references consistently, and preserves the legacy names as human-facing addresses where applicable.

Migration identity generation MAY use a deterministic namespace-based mechanism such as RFC 9562 UUIDv5 when the source address is stable enough for the migration scope. Such a generator is a migration mechanism, not semantic identity meaning and not the normal creation policy.

### 9. Migration and write correctness are atomic at the representation boundary

A migration/save candidate must be fully decoded, migrated, structurally checked, converted, semantically validated as required by the operation, and canonically encoded before replacing durable source state.

Failure MUST leave the previously durable source observably intact.

The exact native filesystem, browser transaction, temporary-file, rename, fsync, IndexedDB, IPC, or WASM mechanism remains owned by #26. This ADR accepts the correctness requirement, not a host implementation.

## Milestone 02 provisional mechanisms

The following are practical Milestone 02 mechanisms rather than timeless ecosystem invariants:

- JSON remains the textual representation technology for the current hardening path;
- a required positive integer `format_version` is sufficient for the current representation envelope;
- the current direct `.ro` JSON v1 profile is frozen as legacy compatibility input; if the direct `.ro` JSON representation evolves incompatibly before `.roproj` becomes the working source, `2` is the next version in that direct-`.ro` namespace;
- future `.roproj` uses its own representation/version namespace and is not forced to begin at `2`;
- stable IDs are encoded as opaque textual tokens in the current JSON profile; the generic storage boundary does not require the token to be UUIDv7;
- deterministic namespace UUIDs are the preferred legacy-identity migration mechanism when source identity permits;
- exact whitespace, escaping, member order, and other textual profile mechanics are version-specific normative rules rather than cross-version semantic invariants.
- the normal direct-JSON complete-input limit is 8 MiB, shared by legacy v1 and
  v2 reads before UTF-8/JSON inspection; it is a replaceable profile mechanism,
  not a semantic or product maximum;
- a future larger legacy compatibility/import budget, if justified, is a
  separate explicit finite profile rather than an unlimited normal-reader
  escape hatch.

## Rejected alternatives

### Directly serialize semantic-core structs

Rejected because it makes Rust/Serde evolution a public format change and violates the storage/domain boundary accepted by ADR-0016.

### Full RFC 8785 JCS as the editable canonical source representation

Rejected for this role. JCS is an excellent canonicalization reference for hashing/signing use cases, but its no-whitespace output, global UTF-16 property sorting, and binary64/ECMAScript number serialization do not match Tachiko Work's Git-readable source requirements and would pre-decide #24 numeric semantics.

Tachiko Work may reuse JCS/I-JSON primitive rules where they fit, especially duplicate rejection, deterministic string escaping, valid Unicode requirements, and preservation without Unicode normalization.

### Ignore unknown fields as generic forward compatibility

Rejected unless a version explicitly defines lossless preservation semantics. Silent read/rewrite data loss conflicts with user ownership and #37's fail-closed requirement.

### SemVer, model/container/schema version matrices, or capability registries now

Rejected as speculative machinery. Additional version dimensions require independent lifecycle evidence. #43 owns future `.ro` package/container profile decisions.

### Implicit migration on open/save

Rejected. Reading and durable storage migration are separate operations.

### Permanent migration IR

Rejected because it would become a second semantic model that also requires versioning.

### Sort canonical output by human key/name/path

Rejected because rename or layout changes would create unrelated persistence churn and would violate ADR-0015's identity/address separation.

### Universal Unicode normalization in storage

Rejected because normalization changes character-sequence equivalence and therefore semantic data.

## Consequences

Positive:

- semantic-core can evolve without silently rewriting historical wire semantics;
- compatibility behavior becomes explicit and testable;
- future/newer data fails safely instead of being partially understood and rewritten;
- deterministic Git bytes are driven by semantic identity rather than runtime accidents;
- ADR-0015 identity migration can be reproducible across branches/hosts where legacy source identity is stable;
- `.roproj` layout and `.ro` package work retain their own authority without being pre-designed here.

Costs:

- storage must own complete historical DTO types instead of reusing semantic structs everywhere;
- strict readers reject some malformed/ambiguous files that permissive parsers might previously have collapsed silently;
- the identity migration produces an intentional one-time representation diff;
- versioned DTOs and migration fixtures create maintenance obligations;
- numeric golden vectors remain partial until #24 is accepted.
- ordinary Milestone 02 reading intentionally rejects legacy direct-JSON v1
  inputs larger than 8 MiB; a future explicit compatibility/import operation
  may admit such input only through another accepted finite profile.

## Required follow-up

- #25: implement the version-specific storage DTO/conversion/migration boundary.
- #37: implement the representation-local version envelope and unsupported-version state machine.
- #38: implement the version-defined canonical JSON profile and deterministic ordering contract.
- #40: add golden/negative fixtures for duplicate keys, version errors, Unicode preservation, ordering, round trips, and migration determinism; numeric edge vectors wait for #24.
- #70: integrate ADR-0015 identity migration through the accepted storage migration boundary.
- #24: decide numeric semantics before final numeric canonical vectors are frozen.
- #41: define `.roproj` physical layout/sharding without making paths identity.
- #43: define the future `.ro` portable package/profile and integrity rules.
- #23: define the broader diagnostic envelope; storage owns only format/migration failure meaning required by its contract.
- #26: define host-specific durability/runtime mechanisms.
- #96: implement the normal direct-JSON Stage-0 admission profile and its
  native/WASM conformance corpus.

## Related

- ADR-0003
- ADR-0015
- ADR-0016
- Issues #25, #37, #38, #40, #70, #96
- RFC 8259 / RFC 7493
- RFC 8785
- RFC 9562
