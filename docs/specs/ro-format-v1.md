# Tachiko Work .ro Format v1

Decision state: Normative legacy compatibility / migration profile under ADR-0017; not the target long-term editable representation

Implementation state: Implemented in the v0.1 Developer MVP and still the current direct `.ro` writer until the storage migration lands

## Authority note

This document freezes the historical direct `.ro` JSON version-1 behavior as a compatibility and migration source profile.

Its purpose is now to preserve what the v0.1 reader/writer meant so later semantic-core and storage refactors cannot silently reinterpret old files. It is not authority for future `.roproj` layout, future `.ro` package/container design, or future semantic identity/numeric choices.

ADR-0017 requires complete storage-owned historical DTOs and explicit migration. The current implementation still embeds semantic-core serialization types; that is implementation debt to remove, not permission for this legacy wire contract to evolve with semantic Rust structs.

ADR-0003 remains the accepted representation direction: `.roproj` is the target canonical editable/source materialization and `.ro` becomes a derived portable artifact. The current direct `.ro` JSON persistence path is an implementation stage and a distinct representation/version namespace.

## Purpose

The version-1 direct `.ro` profile is the single-file JSON representation shipped by the Developer MVP.

It is retained so existing files can be decoded and migrated deterministically. It is not the primary semantic model.

## Historical goals

- Portable sharing
- Deterministic serialization
- Cross-platform compatibility
- Executable evidence for the semantic/storage boundary

## Accepted relationship with .roproj

`.ro` and `.roproj` represent the same logical semantic work under ADR-0003, but their physical/version namespaces are distinct.

- current direct `.ro` v1 is a legacy/current implementation profile;
- future `.roproj` is the canonical editable/Git-native representation;
- future `.ro` packaging is owned by #43 and must not be inferred from this direct JSON v1 profile.

A future `.roproj` version `1` would not mean the same wire schema as direct `.ro` JSON `format_version: 1`.

## Version-1 historical encoding

Version 1 is a UTF-8 JSON document. The shipped writer emits the top-level members in this order:

```text
format_version
id
title
schemas
entities
```

Schema, entity, and field maps are ordered lexicographically by their serialized v0.1 identifiers. The shipped writer uses two-space indentation and exactly one trailing LF.

Current v0.1 document, schema, entity, and field identifiers use the grammar:

```text
[a-z0-9][a-z0-9_-]*
```

The v0.1 CLI uses `entity.field` authoring paths and the legacy representation stores those name-like identifiers as if they were durable IDs.

ADR-0015 supersedes that identity interpretation for the hardened semantic model. Migration MUST treat these values as legacy source addresses and establish new stable semantic IDs explicitly; old v1 bytes must never be silently reclassified as though those strings had always been surrogate IDs.

## Historical reader behavior

The v0.1 implementation:

- requires `format_version: 1`;
- rejects unsupported versions rather than guessing later schemas;
- rejects malformed JSON;
- rejects unknown top-level document fields;
- validates the decoded semantic document.

ADR-0017 and `storage-versioning-and-migration.md` strengthen the future reader boundary with strict duplicate-member detection, recursive version-specific DTO ownership, representation-local version dispatch, and explicit migration. Those new safeguards do not retroactively change the historical v1 byte meaning; they constrain the hardened compatibility decoder around it.

## Compatibility and migration rule

The v1 wire contract is immutable compatibility input.

- semantic-core struct/Serde changes must not alter v1 decoding;
- a hardened reader must decode v1 through storage-owned legacy DTOs;
- reading/opening v1 must not implicitly rewrite durable source;
- migration to an identity-aware representation is explicit and deterministic where the source address permits;
- ambiguous or malformed legacy data must fail rather than be guessed;
- v1 numeric behavior remains historical implementation compatibility and does not decide #24's future numeric semantic contract.

If the direct `.ro` JSON representation evolves incompatibly before `.roproj` replaces it as the canonical working source, it must use a new version in the direct-`.ro` representation namespace. The next available value is `2`; this does not reserve `.roproj` version `2`.

## Current follow-up

- ADR-0017 — accepted storage/version/canonical boundary
- #25 — storage-owned DTO and migration implementation
- #37 — hardened version-envelope state machine
- #38 — canonical JSON/profile implementation
- #40 — golden and negative fixtures
- #70 — ADR-0015 identity migration integration
- #24 — future numeric semantics and final numeric canonical vectors
- #41 — `.roproj` physical layout
- #43 — future `.ro` package profile
