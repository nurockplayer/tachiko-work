# Unified Semantic Document Model

Decision state: Accepted direction; detailed object model is Provisional

Implementation state: Partially implemented

Hardening owner: #21, with progressive-typing implications in #13

## Authority note

Tachiko Work has accepted the semantic-first direction: documents, structured data, formulas, references, and future views should share meaning through the semantic model rather than letting historical file/UI representations own the truth.

The exact long-term graph shape, identity policy, set of block/object types, containment rules, and reference representation are not all frozen by this document. #21 owns the expensive-to-reverse semantic identity/document-graph decision.

Examples below illustrate the direction. A named example type is not automatically a required v1 primitive.

## Overview

Tachiko Work is based on the idea that documents, spreadsheets, Markdown files, structured data, and computational notebooks should not require separate incompatible foundations.

They can be different views over shared semantic structures where that unification creates practical value.

The core object is not a Word document or an Excel workbook.

The architectural direction is a typed, structured, executable semantic graph.

## Design Principles

### Meaning over formatting

Traditional Office formats encode application-specific and historical representation choices.

Tachiko Work stores semantic meaning first where that meaning is useful to computation, validation, references, versioning, migration, multiple views, or AI reasoning.

Possible semantic concepts include:

- headings
- paragraphs
- tables
- formulas
- references
- datasets
- assets
- computations
- relationships

Rendering is a projection of this model rather than the owner of semantic truth.

## Candidate long-term objects

### Document

A richer future document may contain or reference semantic content such as:

```text
Document
├── Heading
├── Paragraph
├── Table
├── Spreadsheet/Table View
├── Chart
├── CodeBlock
├── Diagram
└── Metadata
```

This is illustrative. The current Developer MVP is narrower and centers on typed schemas, entities, fields, references, and formulas.

### Block

Blocks are a candidate abstraction for richer document content.

Possible future blocks include:

- Text
- Heading
- List
- Table
- Formula
- Image
- Diagram
- Code
- Query
- Embedded dataset

Block identity, ordering, containment, and persistence remain part of later semantic-model design rather than an implemented v0.1 contract.

## Spreadsheet as a Semantic View

A spreadsheet-like surface should not reduce all meaning to a grid of coordinates.

Structured work can contain:

- schema
- records/entities
- formulas
- references
- constraints
- views
- calculations

Example semantic shape:

```text
Enemy
├── id: EnemyId
├── hp: Health
├── attack: Damage
├── speed: Speed
└── drops: ItemReference[]
```

The visual table can be one representation over this meaning.

## Formula Model

Formulas are semantic expression trees rather than opaque calculator strings in the core model.

For illustration:

```text
Attack * Speed
```

can correspond to a typed expression such as:

```text
Multiply
├── Reference(Attack)
└── Reference(Speed)
```

This supports:

- static analysis
- AI understanding
- dependency tracking
- safer refactoring

The durable binding/reference/numeric semantics are owned by #21 and #24 rather than fully specified here.

## References

References are typed semantic relationships rather than unvalidated application strings.

The project wants safe rename, dependency analysis, diagnostics, and migration. Exactly what a durable reference stores and how stable identity is generated are Open Questions in #21.

Do not infer from current human-readable identifiers that display names, storage paths, or `entity.field` strings are permanent identity.

## AI-Native Operations

AI should operate on semantic capabilities.

Not primarily:

```text
click cell B17
copy text
move cursor
```

But operations such as:

```text
Update balance data
Explain affected systems
Generate migration proposal
Create review summary
```

The authority and approval model remains governed by ADR-0007 and its follow-up issues.

## Future Direction

This semantic foundation can support:

- document-like editing
- spreadsheet/table views
- Markdown workflows
- Git-native review
- AI agents
- collaborative workspaces

without requiring separate incompatible sources of truth.
