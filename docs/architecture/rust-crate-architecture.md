# Rust Crate Architecture Blueprint

## Goal

Define a possible implementation boundary for Tachiko Work.

The system should not be organized around Office applications. It should be organized around a shared semantic core.

## Proposed Workspace

```
tachiko-work/
├── crates/
│   ├── semantic-core/
│   ├── schema/
│   ├── formula-engine/
│   ├── storage/
│   ├── migration/
│   ├── diff-engine/
│   ├── validation/
│   ├── collaboration/
│   ├── ai-api/
│   └── cli/
```

## Core Principles

### semantic-core

Owns the document graph:

- blocks
- entities
- relationships
- revisions
- operations

No UI or file format assumptions.

### storage

Handles serialization:

- .ro
- .roproj
- future formats

The semantic model remains authoritative.

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
