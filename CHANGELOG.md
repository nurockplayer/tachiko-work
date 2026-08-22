# Changelog

All notable changes to Tachiko Work are documented in this file.

## Unreleased

### Changed

- Hardened legacy direct `.ro` v1 persistence with storage-owned historical
  DTOs, strict UTF-8/JSON/version handling, recursive closed-world decoding,
  and specification-ordered canonical output while preserving valid v1 bytes.
- `tachiko-storage::FormatError` now adds distinct UTF-8, JSON-duplicate,
  version-envelope, and representation failures. Existing `Json`,
  `UnsupportedVersion`, and `InvalidDocument` shapes remain available, but
  downstream exhaustive matches must account for the new variants.

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
