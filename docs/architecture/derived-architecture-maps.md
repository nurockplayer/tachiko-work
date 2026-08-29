# Derived architecture maps

Status: explanatory architecture/tooling policy. This document does not create product, semantic, runtime, storage, or release authority.

Tracking decision/evidence: [Issue #128](https://github.com/nurockplayer/tachiko-work/issues/128)

## Purpose

Tachiko Work may use Archify or an equivalent repository-aware mapping tool to generate revision-pinned architecture maps and Before/Delta/After comparisons for developer documentation and architecture review.

These maps are **derived projections of repository state**. They help reviewers understand topology, ownership boundaries, representation flows, and implementation-state changes. They are not a second architecture authority.

## Authority

When a generated map and repository material disagree, use the normal repository authority order:

1. Product Constitution and foundational principles;
2. Accepted governance and ADRs;
3. normative specifications;
4. architecture and product documentation;
5. implementation and tests;
6. Issues, research, and discussion evidence.

A map may summarize or visualize those sources, but it cannot amend them. Architecture reachability or diagram topology must not be treated as proof of runtime impact, blast radius, risk, semantic equivalence, security, mergeability, or authorization.

## Revision pinning and staleness

Every repository-backed map intended for review or durable reference must identify the exact 40-character Git commit SHA it describes.

The pinned SHA defines the map's observation boundary. If `main` advances later, the map remains valid only as historical evidence for its pinned revision. It must not silently be presented as current architecture.

Do not rewrite an old checkpoint to make it appear current. When a newer view is useful, either:

- generate a new map pinned to the newer revision; or
- generate an explicit Before/Delta/After comparison between two pinned revisions.

A later merge during map generation does not invalidate a correctly frozen run. Evidence from an unmerged PR must not be represented as implementation-real state in a map pinned to `main`.

## Accepted architecture versus implementation state

Maps should distinguish durable authority from implementation evidence.

An Accepted boundary may exist before production code implements it. Conversely, implementation may change inside an Accepted boundary without changing the architecture decision. Maps and deltas should label that distinction rather than converting implementation progress into a fictitious authority change.

For example, ADR-0022 accepts resident Rust runtime ownership while concrete resident session/revision mechanics remain separately implemented under #93. A map must not present those mechanics as production-real until its pinned revision proves them.

## Persistence policy

The v0 durable strategy is **source in Git, generated artifacts on demand**.

When an architecture checkpoint is worth preserving, prefer committing the small revision-pinned typed map source. Do not routinely commit self-contained generated HTML, screenshots, visual-check captures, or other renderer intermediates.

Generated HTML or comparison artifacts may be produced locally, attached to review workflows, or published by a future documentation host. Their storage location does not change their derived status.

Do not persist every generated map. A durable checkpoint should have a concrete use, such as:

- a material Accepted topology change;
- a major subsystem boundary becoming implementation-real;
- a milestone architecture checkpoint;
- stable onboarding documentation; or
- a high-value architecture review reference.

Routine implementation churn should not create generated-map churn.

## Regeneration and validation

Generation should remain reproducible and optional:

- freeze one exact repository revision before authoring;
- ground repository-backed components and relationships in evidence from that revision;
- keep implementation-state and authority claims separate;
- run the strongest applicable map validation/delivery gate;
- correct material composition or visual defects before treating the output as reviewable evidence; and
- keep generated artifacts outside the repository unless a deliberate checkpoint policy says otherwise.

The exact generator, renderer version, local paths, screenshots, and validation receipt format are tooling details, not Tachiko semantic or architecture contracts.

## Architecture review use

Architecture maps complement rather than replace existing review surfaces:

```text
Code-level change        -> Git diff / ordinary PR review
Semantic meaning change  -> Tachiko semantic diff / validation
Architecture topology    -> revision-pinned architecture map / delta
```

Use architecture deltas selectively when a change plausibly affects crate/subsystem topology, Semantic API/runtime/storage/host ownership, durable representation or adapter topology, major first-party composition, or the implementation status of an important Accepted boundary.

Do not generate architecture deltas for ordinary local refactors, tests, copy changes, or small bug fixes when topology is unchanged.

## CI policy

Architecture-map generation is not required CI.

Manual use has demonstrated review value, but required-tooling maintenance cost is not yet justified. Reconsider CI only after repeated use across materially different changes shows consistently high signal and low churn.

## Removal boundary

Archify and its map IR must remain removable without affecting Tachiko semantic correctness, storage correctness, runtime behavior, Git interoperability, product behavior, or Accepted authority.

No canonical Tachiko contract should be changed merely to fit a diagramming tool.
