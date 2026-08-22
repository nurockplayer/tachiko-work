# `.ro` Format Documentation Entry Point

This file is retained as a compatibility/navigation entry point for older links.

It does **not** define an independent `.ro` contract. Use the documents below according to the question you are answering.

## Current sources

- [`ro-format-and-roproj-spec.md`](ro-format-and-roproj-spec.md) — Accepted representation direction under ADR-0003: `.roproj` is the target canonical editable/source materialization, `.ro` is the portable artifact, and the semantic model is authoritative over both.
- [`../decisions/ADR-0017-versioned-storage-and-canonical-representation.md`](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md) — Accepted architecture for version-specific storage DTOs, fail-closed version dispatch, explicit migration, and canonical representation.
- [`storage-versioning-and-migration.md`](storage-versioning-and-migration.md) — representation namespaces, version-envelope behavior, complete DTO ownership, and migration requirements.
- [`canonical-json-profile.md`](canonical-json-profile.md) — deterministic JSON, Unicode, ordering, member-emission, and whitespace rules whose non-numeric portions are settled for Milestone 02.
- [`ro-format-v1.md`](ro-format-v1.md) — immutable normative legacy compatibility/migration profile for the direct `.ro` JSON bytes shipped by the v0.1 Developer MVP.
- [`roproj-format.md`](roproj-format.md) and [`roproj-layout-v1.md`](roproj-layout-v1.md) — target/project representation material; `.roproj` is not yet implemented and #41 owns its physical layout.
- [`../decisions/ADR-0003-ro-and-roproj-representation.md`](../decisions/ADR-0003-ro-and-roproj-representation.md) — Accepted architectural authority for the long-term representation relationship.

## Current implementation state

The v0.1 CLI still directly persists deterministic `legacy-direct-ro/v1` files for validation, calculation, semantic diff/merge, authoring, and export.

ADR-0017 is Accepted, but its implementation migration has not landed yet. The current writer therefore remains v1 while the repository deliberately replaces semantic-core-coupled serialization with storage-owned historical DTOs and explicit conversion.

The direct `.ro` v1 profile is stable only as **legacy compatibility input**. It is not the future `.roproj` editable format, the future `.ro` package/container profile, or authority for future identity and numeric semantics.

## Work ownership

- #25, #37, and #38 are completed Decision Issues.
- #74 is the implementation parent for ADR-0017.
- #40 owns executable golden and negative conformance evidence.
- #70 owns ADR-0015 stable-identity migration integration.
- #24 retains numeric semantic ownership.
- #41 retains `.roproj` physical-layout ownership.
- #43 retains future `.ro` package/container/integrity ownership.

For the project-wide authority model, see [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md) and [`../governance/canonical-reconciliation-register.md`](../governance/canonical-reconciliation-register.md).
