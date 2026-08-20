# Rust Crate Architecture

## Goal

Define the live implementation boundary for Tachiko Work.

The system should not be organized around Office applications. It should be organized around a shared semantic core.

## Implemented Workspace

```
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

Schema types and validation live in `semantic-core` because they enforce one
document invariant together. Version compatibility and future migrations live
in `storage`. Separate crates should appear only when an implemented need
requires an independent lifecycle or dependency boundary.

## Core Principles

### semantic-core

Owns the document graph:

- schemas and typed fields
- entities
- typed references and numeric expressions
- deterministic semantic diagnostics

It has no UI, filesystem, or wire-format assumptions. Blocks, revisions, and
operation logs remain future semantic capabilities rather than implemented
contracts.

### storage

Handles serialization:

- canonical version-1 `.ro` JSON
- format compatibility checks
- exclusive-create persistence

The semantic model remains authoritative. `.roproj` is still proposed by
ADR-0003 and does not belong to the implemented storage contract yet.

### formula-engine, diff-engine, and merge-engine

`formula-engine` owns deterministic calculation and dependency tracking.
`diff-engine` compares semantic documents and reports both direct changes and
derived formula impact as typed values. It also provides a deterministic,
domain-level text summary; the CLI remains responsible for terminal behavior.
`merge-engine` owns three-way reconciliation of typed semantic documents. It
uses the semantic core and formula engine to reject invalid merged candidates
before persistence; the CLI performs exclusive output creation and terminal
rendering. None of these engines owns persistence, raw-text merge behavior,
Git-driver configuration, or interactive resolution.

### ai-api and workflow

`ai-api` is the read, analysis, explanation, and approval-required suggestion
boundary for AI callers.

`workflow` owns reusable, opinionated product operations such as starters,
semantic overviews, field explanations, and validated edit previews. It does
not read files or render terminal output, so CLI and future graphical clients
can share the same behavior.

### cli

`cli` is a thin adapter over storage and workflow APIs. It owns arguments,
filesystem paths, safe exclusive output creation, and human/machine rendering.

### adapters

External formats belong at the boundary:

- DOCX
- XLSX
- Markdown
- CSV
- JSON

## Rust Responsibilities

Rust is responsible for:

- deterministic processing
- memory safety
- computation
- parsing
- validation
- concurrency

The UI should consume semantic APIs rather than internal structures.
