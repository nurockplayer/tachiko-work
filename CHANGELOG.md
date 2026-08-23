# Changelog

All notable changes to Tachiko Work are documented in this file.

## Unreleased

### Changed

- Semantic document, schema, field, and entity identity is now opaque and
  stable, while mutable human keys remain the authoring address. Renames retain
  bound references/formulas and diff/merge continuity instead of rewriting or
  replacing the object.
- Valid legacy direct `.ro` v1 documents now migrate deterministically in
  memory to stable IDs. Explicit saves emit lossless `direct-ro/v2`; merely
  opening v1 never rewrites durable bytes.
- The normal direct-JSON reader now applies a shared Provisional 8 MiB Stage-0
  envelope to legacy v1, v2, and untrusted version envelopes before UTF-8/JSON
  inspection. This intentionally rejects otherwise-valid legacy v1 input over
  8 MiB; the value is a representation profile, not a semantic or product
  maximum.
- Direct `.ro` v2 adopts finite normalized binary64 semantics, ECMAScript
  shortest-roundtrip numeric spelling, stable-ID collection order, and bounded
  number/formula resources. Runtime export correspondingly advances to
  `runtime-export/v2`.
- Hardened legacy direct `.ro` v1 persistence with storage-owned historical
  DTOs, strict UTF-8/JSON/version handling, recursive closed-world decoding,
  and specification-ordered canonical output while preserving admitted v1
  wire meaning and canonical bytes.
- `tachiko-storage::FormatError` now adds distinct UTF-8, JSON-duplicate,
  version-envelope, and representation failures. Existing `Json`,
  `UnsupportedVersion`, and `InvalidDocument` shapes remain available, but
  downstream exhaustive matches must account for the new variants.

### Added

- An executed production-semantic conformance corpus compares normalized
  values, failures, dependency/cycle results, and stable formula projection
  byte-for-byte on native and `wasm32-unknown-unknown` builds.

## [0.1.0] - 2026-08-20

### Added

- Typed semantic documents with schemas, entities, scalar values, references,
  validation, and canonical versioned `.ro` serialization.
- Deterministic formula evaluation with dependency tracking, cycle detection,
  semantic diffing, and derived-impact reporting.
- Typed three-way semantic merge with structured conflicts and validation of
  successful merge candidates.
- A complete game-balance CLI workflow for initializing, browsing, explaining,
  editing, validating, calculating, diffing, merging, and exporting documents.
- Validated entity duplication, relationship-safe rename, and non-cascading
  removal with actionable dependent field paths.
- Bounded formula parsing and canonical formatting plus validated formula
  authoring through the workflow and CLI.
- Evaluated runtime JSON export that preserves semantic document and entity
  identity.
- Read-only AI-oriented structure, formula, and impact APIs plus validated,
  approval-required scalar and bounded formula suggestions.
- A checked-in Moonfall game-balance example and executable first-user,
  collaboration, entity-lifecycle, and formula-authoring smoke tests.
- Reproducible checksummed native archives for four targets, including the
  project license texts and a deterministic audited notice for every locked
  all-target CLI dependency.
- A release-equivalent local gate that selects stable Rust for ordinary builds
  and independently proves compatibility with exact Rust 1.85.0.

[0.1.0]: https://github.com/nurockplayer/tachiko-work/releases/tag/v0.1.0
