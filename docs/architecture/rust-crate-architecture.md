# Rust Crate Architecture

Decision state: Provisional

Implementation state: Implemented v0.1 baseline

Hardening owner: #20

## Goal

Describe the live implementation boundary for Tachiko Work while preserving room to harden crate ownership and dependency direction before public APIs become expensive to change.

The system should not be organized around Office applications. It should be organized around a shared semantic core.

The crate graph below is strong implementation evidence, not a declaration that every current boundary is permanently frozen. #20 owns the focused architecture decision for the durable dependency DAG.

## Implemented Workspace

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

The workspace keeps the live direct-crate dependency direction explicit; arrows
point from a dependent crate toward the crate it uses:

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

Schema types and validation currently live in `semantic-core` because they
enforce document invariants together. Version compatibility and future
migrations currently belong to `storage`. Separate crates should appear only
when an implemented or researched need justifies an independent lifecycle or
dependency boundary.

These current placements remain reviewable in #20; implementation location alone
does not make a crate boundary constitutional.

## Core Principles

### semantic-core

Currently owns the implemented document graph:

- schemas and typed fields
- entities
- typed references and numeric expressions
- deterministic semantic diagnostics

It has no UI, filesystem, or wire-format assumptions. Blocks, revisions, and
operation logs remain future semantic capabilities rather than implemented
contracts.

### storage

Currently handles serialization:

- canonical version-1 `.ro` JSON
- format compatibility checks
- exclusive-create persistence

The semantic model remains authoritative.

ADR-0003 is Accepted and defines `.roproj` as the target canonical editable/source
materialization with `.ro` as the portable artifact. `.roproj` materialization
is not implemented in v0.1 and therefore does not belong to the live storage
contract yet.

### formula-engine, diff-engine, and merge-engine

`formula-engine` owns deterministic calculation and dependency tracking in the
current implementation. `diff-engine` compares semantic documents and reports
both direct changes and derived formula impact as typed values. It also provides
a deterministic, domain-level text summary; the CLI remains responsible for
terminal behavior.

`merge-engine` owns current three-way reconciliation of typed semantic documents.
It uses the semantic core and formula engine to reject invalid merged candidates
before persistence; the CLI performs exclusive output creation and terminal
rendering. None of these engines owns persistence, raw-text merge behavior,
Git-driver configuration, or interactive resolution.

Formula binding/numeric invariants and broader merge protocol semantics are
still being hardened separately; the current crate arrangement must not pre-decide
them by accident.

### ai-api and workflow

`ai-api` is the implemented read, analysis, explanation, and approval-required
suggestion boundary for AI callers.

`workflow` owns reusable, opinionated product operations such as starters,
semantic overviews, field explanations, and validated edit previews. It does
not read files or render terminal output, so CLI and future graphical clients
can share the same behavior.

### cli

`cli` is a thin adapter over storage and workflow APIs. It owns arguments,
filesystem paths, safe exclusive output creation, and human/machine rendering.

### adapters

External formats belong at explicit boundaries:

- DOCX
- XLSX
- Markdown
- CSV
- JSON

These are future adapter categories; only core JSON `.ro` input/output is active
in v0.1.

## Rust Responsibilities

Rust is responsible for the deterministic semantic/runtime behavior implemented
by the core crates, including:

- deterministic processing
- memory-safe core implementation
- computation
- parsing
- validation

Concurrency, host capabilities, native/WASM orchestration, and future plugin
runtime boundaries should be introduced only through explicit architecture work
rather than inferred from this v0.1 crate list.

The UI should consume semantic APIs rather than internal storage structures.

## Hardening rule

When #20 finalizes the dependency DAG, it should use evidence from the expensive-to-reverse semantic/storage/formula decisions rather than creating crates for every future subsystem named in roadmap documents.

Prefer a small stable kernel and explicit replaceable seams over a pre-emptive monolith or inner platform.
