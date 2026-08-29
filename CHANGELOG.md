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
- First-party semantic validation now returns one deterministic
  `ValidationReport` with symbolic meaning, machine severity, stable-ID
  subjects and related facts, opaque validator provenance, and
  presentation-only paths/messages. Workspace validation is shared across
  queries, mutations, and merge finalization while authoring projection remains
  an explicit operation gate.

### Added

- The provisional resident workspace session now exposes occurrence-and-
  revision-pinned, stable-ID-selective entity and field projections without
  returning the whole semantic `Document`. Stored literals, bound formula
  definitions, calculated outcomes, stable-subject diagnostics, and mutable
  human addresses remain distinct. Each exact guarded publication derives
  occurrence/revision-paired stale entity and field projections plus
  deterministic downstream calculated projections from fresh full-oracle
  dependency facts, including renames and output-equal dependents. Clients can
  discard stale revision-keyed caches without recreating semantic meaning.
  Native/WASM conformance covers the same bounded query/mutation corpus; no
  retained cache, UI, persistence, or public wire DTO is introduced.
- A provisional `tachiko-workspace-engine` resident workspace session now owns
  one authoritative semantic `Document` occurrence and an internal monotonic
  `SemanticRevision`. Validation and calculation queries are revision-pinned
  without publication; approved `SemanticPatch` execution installs only
  through the existing guarded `SemanticPublicationAuthority`, advances once,
  and rejects stale, failed, unauthorized, or wrong-occurrence attempts without
  changing state. Full snapshots remain explicit detached exports, trusted time
  stays host-supplied, and the production session has matching native/WASM and
  `ai-api` composition evidence without defining a public wire/session DTO.
- A provisional provider-neutral bounded semantic Analysis Query now runs
  through the shared workspace authority and structured CLI. It implements
  stable-ID schema selection and optional narrowing, typed AND predicates,
  zero/one non-Formula grouping field, exact membership and Count, Number
  Min/Max, bounded per-member Number observations, same-definition paired
  exact contexts, and reproducible lineage. Formula-backed values reuse the
  ADR-0018 calculation oracle; candidate-domain and complete-result disclosure
  reuse ADR-0026 Grants and deny the whole assertion rather than aggregating a
  visible subset. Results remain ephemeral, native/WASM conformance is
  exercised, and no public wire/SDK, persistence, Sum/Mean, ranking,
  statistics, or runtime/history contract is introduced.
- A provisional provider-neutral M04 vertical slice now exposes structured
  formula-reasoning and exact-snapshot read-only Number-override scenario
  Queries through the workspace and CLI. Typed formula updates now bind their
  complete stable-reference meaning before entering the existing
  SemanticPatch Propose/Execute/Approval lifecycle. The implementation reuses
  the authoritative calculation, dependency, validation, diff, authorization,
  and publication paths; scenarios remain transient and non-publishing, and
  native/WASM conformance covers all three operations without defining a
  public wire or SDK contract.
- The Game Dev Alpha acceptance journey now starts from the durable Moonfall
  canonical `.roproj`, edits it semantically without touching internal files,
  materializes a distinct accepted tree, proves deterministic review,
  calculation, validation, and export, and rejects an invalid formula input
  locally. The same fixture now drives the optional ordinary-Git branch/CI
  proof, including localized raw diff, semantic parity, package consistency,
  and fail-closed invalid-state validation.
- A provisional provider-facing `tachiko-ai-api::security_boundary` now keeps
  system/developer/user instructions, trusted semantic metadata, and untrusted
  document/import/plugin/model content explicitly separated. Untrusted typed
  Propose/Execute requests receive effective identity and time only from a
  trusted host context, must resolve to an active Delegated lifecycle principal,
  and cannot reuse a Human session principal to avoid Approval. They delegate to
  the workspace lifecycle; raw semantic or storage mutation and persistence/
  filesystem/network/process/Git/plugin/deployment/credential effects are
  rejected with stable machine codes.
  Model explanations and validation claims remain inert evidence. Concrete
  public authentication/session/revision/transport mechanics remain deferred,
  and actual host/plugin capability mechanisms remain separately owned.
- A provisional provider-neutral SemanticPatch lifecycle in
  `tachiko-workspace-engine` now evaluates stable-ID typed field-value Commands
  and ordered AtomicBatch proposals through the shared validation, formula, and
  semantic-diff path. It derives relational scoped capabilities, gates review
  evidence with independent Query authority, binds finite one-shot Human
  Approval to the exact proposal/base/executor/policy context, publishes only
  through an opaque revision compare-and-publish seam with a fresh trusted-time
  authorization callback, consumes Approval with successful publication,
  verifies installed state, and retains disclosure-safe execution receipts.
  Issue #93 now supplies the provisional resident session/revision mechanics;
  the provider-facing hostile boundary composes this lifecycle through
  `tachiko-ai-api`, while actual external-effect capabilities remain separate
  host/plugin work.
- An optional provider-neutral Git/CI adapter now keeps canonical `.roproj/v1`
  members as LF text, accepts exact project trees in existing read-only
  semantic commands, and composes canonical/workspace validation with
  read-only generated-package consistency checks. The executable journey uses
  ordinary Git without a host API or semantic Git identity.
- Accepted a semantic authorization contract for scoped Grants, trusted
  authorization-footprint derivation, and exact finite Human Approval with
  at-most-once successful publication. Issue #29 now provides the provisional
  in-process lifecycle implementation, and #30 adds the provisional hostile-
  client adapter plus safe denial projection. Public authorization/wire DTOs
  Public resident revision/session transport mechanics remain deferred; Issue
  #93 supplies the current provisional in-process implementation.
- A provider-free, read-only Semantic Analyst slice now exposes deterministic
  document inspection, formula and dependency analysis, semantic change and
  affected-area analysis, and validation findings through shared Rust queries
  and structured CLI JSON. Results carry caller-owned source-state labels and
  document identity without introducing mutation or revision semantics.
- An Accepted `tachiko.portable-package/v1` specification and executable
  evidence fix one deterministic ZIP32 envelope over exact `.roproj/v1`
  bytes, a path-separated SHA-256 payload root, exact byte round trips, and
  tracked-source conflict behavior. Production storage now implements the
  bounded exact-byte codec, fail-closed content framing, atomic no-replace
  pack/unpack, and read-only source comparison. `tachiko roproj pack`,
  `unpack`, and `compare-package` compose those operations with workspace
  validation; fixed package evidence agrees on native and WASM.
- Production `tachiko-storage` now encodes and decodes the canonical 18-file
  `.roproj/v1` tree. Explicit `tachiko roproj materialize`, `validate`, and
  `canonicalize` commands provide a standalone host workflow with staged,
  source-preserving, no-clobber publication, while fixed exact-tree evidence
  agrees byte-for-byte on native and `wasm32-unknown-unknown`.
- An executed production-semantic conformance corpus compares normalized
  values, complete formula failures, stable diagnostic observations,
  dependency/SCC results, and stable formula projection byte-for-byte on native
  and `wasm32-unknown-unknown` builds.

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
