# `.ro` Format Documentation Entry Point

This file is retained as a compatibility/navigation entry point for older links.

It does **not** define an independent `.ro` contract. Use the documents below according to the question you are answering.

## Current sources

- [`ro-format-and-roproj-spec.md`](ro-format-and-roproj-spec.md) — Accepted representation direction under ADR-0003: `.roproj` is the target canonical editable/source materialization, `.ro` is the portable artifact, and the semantic model is authoritative over both.
- [`../decisions/ADR-0017-versioned-storage-and-canonical-representation.md`](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md) — Accepted architecture for version-specific storage DTOs, fail-closed version dispatch, explicit migration, and canonical representation.
- [`storage-versioning-and-migration.md`](storage-versioning-and-migration.md) — representation namespaces, version-envelope behavior, complete DTO ownership, and migration requirements.
- [`canonical-json-profile.md`](canonical-json-profile.md) — deterministic JSON, Unicode, ordering, member-emission, and whitespace rules whose non-numeric portions are settled for Milestone 02.
- [`ro-format-v1.md`](ro-format-v1.md) — immutable normative legacy compatibility/migration profile for the direct `.ro` JSON bytes shipped by the v0.1 Developer MVP.
- [`ro-format-v2.md`](ro-format-v2.md) — normative current identity-aware direct `.ro` JSON representation.
- [`roproj-format.md`](roproj-format.md) and [`roproj-layout-v1.md`](roproj-layout-v1.md) — Accepted `.roproj/v1` version-owned DTO and physical-tree contracts under ADR-0023; production materialization is not yet implemented.
- [`../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md`](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md) — Accepted authority for the exact v1 canonical tree, entity placement, canonical JSON/JSONL boundary, and path nonidentity.
- [`portable-package-v1.md`](portable-package-v1.md) and [`../decisions/ADR-0025-portable-package-v1.md`](../decisions/ADR-0025-portable-package-v1.md) — Accepted authority for the exact portable-package v1 envelope, payload root, round trips, and tracked-source conflict behavior; production codecs and CLI are not implemented.
- [`../decisions/ADR-0003-ro-and-roproj-representation.md`](../decisions/ADR-0003-ro-and-roproj-representation.md) — Accepted architectural authority for the long-term representation relationship.

## Current implementation state

The CLI reads frozen `legacy-direct-ro/v1` through storage-owned DTOs and a
strict decoder, then migrates it deterministically in memory for the requested
operation without rewriting the source. New or explicitly saved semantic
documents use canonical `direct-ro/v2`, which preserves opaque stable IDs,
mutable human keys, bound references, and ADR-0018 Number semantics losslessly.

The direct `.ro` v1 profile is stable only as **legacy compatibility input**.
It is not the separate `.roproj/v1` editable format, the Accepted
`tachiko.portable-package/v1` container profile, or authority for future
identity and numeric semantics.

## Work ownership

- #25, #37, and #38 are completed Decision Issues.
- #74 is the implementation parent for ADR-0017.
- #40 owns executable golden and negative conformance evidence.
- #70 owns ADR-0015 stable-identity migration integration.
- ADR-0018 is the Accepted numeric/formula authority; #24 is closed.
- ADR-0023 and the two `.roproj/v1` specifications resolve #41's durable physical-layout decision; a later implementation issue must own the production codec.
- ADR-0025 and `portable-package-v1.md` resolve #43's durable package,
  integrity, round-trip, and conflict decision; #3 remains the production
  codec/CLI implementation issue.

For the project-wide authority model, see [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and [`../governance/canonical-reconciliation-register.md`](../governance/canonical-reconciliation-register.md).
