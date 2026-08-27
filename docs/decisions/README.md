# Architecture Decision Records

This directory contains Tachiko Work Architecture Decision Records (ADRs).

ADRs preserve both current authority and decision history. A Superseded ADR remains in the repository so future readers can understand why the project changed direction.

For project-wide authority rules, read [`../governance/knowledge-authority.md`](../governance/knowledge-authority.md). For the current reconciliation map, read [`../governance/canonical-reconciliation-register.md`](../governance/canonical-reconciliation-register.md).

## Current ADR index

| ADR | Decision | State | Current authority note |
| --- | --- | --- | --- |
| [ADR-0001](ADR-0001-semantic-platform-not-office-clone.md) | Semantic platform, not Office clone | Accepted | Foundational architectural direction |
| [ADR-0002](ADR-0002-game-dev-first-wedge.md) | Game-development first wedge | Superseded | Replaced by ADR-0005; retained for history |
| [ADR-0003](ADR-0003-ro-and-roproj-representation.md) | `.roproj` source / `.ro` portable representation | Accepted | Long-term representation relationship; current direct `.ro` persistence is implementation state |
| [ADR-0004](ADR-0004-mvp-boundary.md) | Developer MVP boundary | Accepted, historical milestone boundary | Defines the completed Developer MVP scope |
| [ADR-0005](ADR-0005-game-development-first-wedge.md) | Game-development first commercial wedge | Accepted | Current first-wedge authority |
| [ADR-0006](ADR-0006-mvp-interface-strategy.md) | CLI-first MVP interface | Accepted | GUI remains a later projection |
| [ADR-0007](ADR-0007-ai-semantic-interaction-model.md) | AI semantic interaction model | Accepted | AI is a delegated semantic client with no intrinsic authority; MVP AI-originated canonical mutation remains approval-gated |
| [ADR-0008](ADR-0008-developer-mvp-completion-and-next-phase.md) | Developer MVP completion / next phase | Superseded | ADR-0009 is the surviving authority |
| [ADR-0009](ADR-0009-developer-mvp-validation-and-next-phase.md) | Developer MVP validation / next phase | Accepted, historical milestone boundary | Confirms Developer MVP as the completed validation point |
| [ADR-0010](ADR-0010-first-usable-product-workflow.md) | First usable product workflow | Accepted | Current CLI-first usable workflow |
| [ADR-0011](ADR-0011-semantic-three-way-merge.md) | Semantic three-way merge | Accepted for implemented merge contract | Broader collaboration/conflict semantics remain separate Open Questions |
| [ADR-0012](ADR-0012-release-distribution-contract.md) | Release distribution contract | Accepted | Tag-gated release/distribution contract |
| [ADR-0013](ADR-0013-semantic-entity-lifecycle.md) | Validated semantic entity lifecycle | Accepted for v0.1 lifecycle contract | Preview-first mutation and relationship safety remain authoritative; ADR-0015 supersedes rename-as-identity semantics |
| [ADR-0014](ADR-0014-computational-formula-authoring.md) | Bounded computational formula authoring | Accepted | Formula authoring is bounded, deterministic, and separate from generic `set`/read-only AI paths |
| [ADR-0015](ADR-0015-stable-semantic-identity.md) | Stable semantic identity and mutable human keys | Accepted | Stable typed surrogate identity is durable; UUIDv7 is the preferred provisional generator, not permanent semantic meaning |
| [ADR-0016](ADR-0016-milestone-02-rust-crate-layering.md) | Milestone 02 Rust crate layering | Accepted | Eight-crate Milestone 02 baseline; workflow evolves into the shared workspace engine while narrower seams may be refined only through explicit Accepted decisions |
| [ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md) | Versioned storage DTOs, explicit migration, and canonical representation | Accepted | Storage owns immutable versioned DTOs and explicit migrations; unsupported/newer semantics fail closed; canonical bytes are version-defined without inventing #24 numeric semantics |
| [ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md) | Bound formulas and deterministic finite binary64 semantics | Accepted | Stable-ID projection can fail without source, rename preserves the 4,096-byte authoring limit atomically, and numeric conversion follows representation resource admission |
| [ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md) | Staged semantic validation and diagnostics contract | Accepted | Separates hard admission, diagnosable semantic candidates, deterministic full validation, semantic-ID diagnostics, and operation gating without adding a validation framework or transport contract |
| [ADR-0020](ADR-0020-first-class-headless-semantic-api.md) | First-class Headless Semantic API boundary | Accepted | All first-party semantic clients share transport-neutral query/command/propose/execute, gating, atomicity, capability-addressability, and compatibility laws; current Rust/serde/transport shapes remain non-authoritative |
| [ADR-0021](ADR-0021-progressive-semantic-strengthening.md) | Progressive semantic strengthening | Accepted | Semantic-first does not imply schema-first; weaker semantic content may strengthen explicitly without weakening the current typed core or fabricating universal identity |
| [ADR-0022](ADR-0022-resident-semantic-runtime-and-host-boundary.md) | Resident semantic runtime and host boundary | Accepted | Interactive authoritative semantic state belongs to the shared Rust runtime; resident topology is preferred while transport/session/revision/persistence mechanics remain replaceable |
| [ADR-0023](ADR-0023-roproj-v1-canonical-tree-and-sharding.md) | `.roproj/v1` canonical tree and entity sharding | Accepted | Fixes the 18-file editable-source tree, version-scoped entity placement, canonical JSON/JSONL materialization, and path-nonidentity boundary; #123 implements the production pure codec plus native exact-tree materialize, canonical-only validate, and explicit bounded canonicalize workflow; optional Git/CI (#44), hostile filesystem races, full durability/recovery, and broader host work remain Deferred |
| [ADR-0024](ADR-0024-revision-pinned-semantic-patch.md) | Revision-pinned SemanticPatch proposal envelope | Accepted | One immutable proposal occurrence binds a Semantic API contract, exact semantic base, and one Command or ordered AtomicBatch without adding a mutation vocabulary, wire DTO, digest, approval, or runtime implementation |
| [ADR-0025](ADR-0025-portable-package-v1.md) | Portable package v1 and payload integrity root | Accepted | Fixes the deterministic 19-entry ZIP32 envelope over exact `.roproj/v1` bytes, integrity root, lossless laws, and tracked-source conflict boundary; #123 supplies the implemented production `.roproj/v1` payload codec/native host seam, while packaged `.ro` ZIP and CLI pack/unpack remain #3 work; Git/CI, hostile filesystem races, full durability/recovery, and broader host work remain separately Deferred |

## How to use ADRs

- Prefer an explicit Accepted ADR over older exploratory architecture, roadmap, research, or Issue prose.
- A newer implementation does not silently supersede an Accepted ADR.
- A Superseded ADR is historical context, not current implementation authority.
- If an ADR defines an Accepted direction but the implementation has not caught up, classify that as implementation lag rather than silently rewriting the decision.
- New expensive-to-reverse public contracts should be promoted through explicit decision work, not hidden inside implementation Issues.

The canonical reconciliation register is the source for cross-document status when a narrower architecture or specification file has mixed decision states.
