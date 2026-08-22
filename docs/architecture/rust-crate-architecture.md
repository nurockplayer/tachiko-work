# Rust Crate Architecture

Decision state: v0.1 baseline is Provisional; target is Proposed in ADR-0016

Implementation state: v0.1 baseline implemented; target migration not started

Decision owner: #20

## Purpose

This document records the live Rust workspace evidence that motivated #20 and
the relationship between the current implementation and the proposed decision.

[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
is the single source for the proposed Milestone 02 tree, dependency DAG,
ownership/API boundaries, composition roots, portability rules, forbidden
edges, Provisional seams, rejected alternatives, and migration sequence. It
remains Proposed until reviewed and promoted.

## Current v0.1 workspace

```text
tachiko-work/
├── crates/
│   ├── semantic-core/
│   ├── formula-engine/
│   ├── storage/
│   ├── diff-engine/
│   ├── merge-engine/
│   ├── ai-api/
│   ├── workflow/
│   └── cli/
```

Arrows point from a dependent crate toward the crate it uses:

```text
storage ────────────────────────────────────────────────────→ semantic-core
formula-engine ─────────────────────────────────────────────→ semantic-core
diff-engine ───────────────→ formula-engine, semantic-core
merge-engine ──────────────→ formula-engine, semantic-core
ai-api ────────────────────→ diff-engine, formula-engine, semantic-core
workflow ──────────────────→ diff-engine, formula-engine, semantic-core
cli ───────────────────────→ storage, workflow, diff-engine, merge-engine,
                               formula-engine, semantic-core
```

The graph is acyclic. It is implementation evidence, not a declaration that
every current boundary or public Rust type is stable.

## Current responsibility evidence

### semantic-core

The crate currently owns the string-backed ID newtypes, document/schema/entity
model, typed values and relationships, formula expression representation,
semantic diagnostics, and whole-document validation. It has only `serde` as a
non-dev dependency and no UI, filesystem, network, or other host dependency.

Its domain types currently derive `serde`. That convenience, and storage's
reuse of parts of those types in v0.1 DTOs, is migration debt under #25 rather
than a durable wire-format promise.

### formula-engine

The crate owns the current bounded expression parser/formatter, deterministic
calculation, cycle/error behavior, and derived dependency indexes. It depends
only on semantic-core among workspace crates. Exact source/bound AST, binding,
numeric, error, and dependency contracts remain Provisional under #24.

### diff-engine and merge-engine

Diff depends on core and formula calculation to report direct semantic changes
and derived formula impact. Its detailed behavior is an implemented Provisional
contract, not an Accepted ADR.

Merge depends on core and formula calculation to reconcile typed three-way
candidates and reject invalid or uncalculable results. ADR-0011 governs the
implemented merge contract; broader protocol and conflict semantics remain
separate work.

### storage

Storage currently combines canonical version-1 `.ro` JSON string codecs,
format checks, semantic validation, and native filesystem load/save APIs. This
mix identifies a host boundary; #25/#26 own whether portable codecs and
native/browser persistence later justify separate crates.

ADR-0003 remains authoritative: `.roproj` is the target canonical editable
materialization and `.ro` is a derived portable artifact. The v0.1 direct `.ro`
implementation does not supersede that direction.

### workflow

Workflow currently provides UI-independent document starters, overview and
explanation queries, scalar/formula edits, and entity lifecycle operations.
Its edit path already coordinates clone/change, semantic validation, formula
calculation, semantic diff, and immutable preview without storage or UI
dependencies.

This is the code evidence for ADR-0016's shared application-engine decision.
There is no separate workspace aggregate in v0.1; all current operations are
document-local.

### ai-api

AI API currently exposes deterministic descriptions, formula/impact
explanations, and inert approval-required suggestions. It directly coordinates
core validation, formula complexity/calculation, and diff behavior, duplicating
part of workflow's candidate-operation pipeline.

### cli

CLI owns arguments, OS paths, create-new output behavior, and human/machine
rendering, but it also directly coordinates six workspace crates. Validation,
calculation, diff, merge, export projection, and persistence sequencing are
therefore chosen at individual command entry points rather than behind one
complete application boundary.

## Current cross-boundary pressure

The code shows four concrete reasons for the proposed target:

1. Workflow already has the correct host-independent shape for shared
   application behavior; adding a parallel orchestration crate would duplicate
   ownership.
2. CLI dependency fan-out would otherwise need to be recreated by each future
   native, WASM, AI, or server client.
3. AI suggestions independently clone, validate, and calculate candidates,
   creating a second application path that can drift from workflow.
4. Storage paths/file I/O and domain calculation have different native/WASM
   capability requirements and should remain sibling boundaries.

Invariant enforcement is currently entry-point-dependent: storage load validates
but does not calculate; CLI validate adds calculation; merge validates and
calculates; workflow edits validate and calculate; diff calculates; AI queries
select different subsets. The proposed application boundary centralizes that
policy without moving it into semantic-core.

## Portability evidence

At the #20 baseline, these provider-free crates contain no filesystem, process,
network, or target-specific code and compile together for
`wasm32-unknown-unknown`:

- semantic-core;
- formula-engine;
- diff-engine;
- merge-engine;
- workflow;
- ai-api.

Storage's string codecs may be reusable on WASM, but its public path/file APIs
make the current crate host-facing. CLI is native-only. No WASM ABI/binding
crate exists yet; #26 owns that boundary.

One implementation risk crosses #20 and #24: wire-authored expression trees can
currently reach recursive semantic validation and calculation without the
parser/AI complexity gate. A shared structural limit must be applied before
untrusted native or WASM evaluation.

## Proposed target summary

ADR-0016 proposes retaining eight target crates, evolving current `workflow` in
place into the host-independent `workspace-engine` role, making AI and CLI thin
adapters over that shared behavior, and keeping storage as a sibling host
boundary. It does not add schema, validation, diagnostics, common-types,
plugin, collaboration, or host-abstraction crates now.

The ADR also makes clear that the target name does not create a `Workspace` or
`Project` semantic aggregate. ADR-0015 keeps v1 semantic references
document-local unless separate evidence justifies broader authority.

See ADR-0016 rather than copying its target matrix or migration steps here.

## Status and follow-up

- #20 remains open while ADR-0016 is Proposed.
- #23/#24 may refine schema, validation, diagnostic, and formula sub-boundaries
  without reversing the macro dependency direction.
- #25 owns storage DTO/codec/migration/package/host subdivision.
- #26 owns stateful runtime, native/WASM capability, and bridge details.
- #10 owns external Semantic API stability and versioning.
- A separate implementation issue should own the staged crate/dependency
  migration after ADR promotion.

## Related authority

- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [Semantic core rationale](semantic-core-rationale.md)
- [Knowledge authority](../governance/knowledge-authority.md)
- GitHub issues #10, #20, #23, #24, #25, #26
