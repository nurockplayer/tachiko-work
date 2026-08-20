# Unified Semantic Document Model

## Overview

Tachiko Work is based on the idea that documents, spreadsheets, Markdown files, structured data, and computational notebooks should not be separate products.

They are different views over a shared semantic model.

The core object is not a Word document or an Excel workbook.

The core object is a typed, structured, executable document graph.

## Design Principles

### Meaning over formatting

Traditional Office formats store historical representations of documents.

Tachiko Work stores semantic meaning first:

- headings
- paragraphs
- tables
- formulas
- references
- datasets
- assets
- computations
- relationships

Rendering is a projection of this model.

## Core Entities

### Document

A document is a graph of semantic blocks.

Example:

```
Document
├── Heading
├── Paragraph
├── Table
├── Spreadsheet
├── Chart
├── CodeBlock
├── Diagram
└── Metadata
```

### Block

Blocks are typed content units.

Possible blocks:

- Text
- Heading
- List
- Table
- Spreadsheet
- Formula
- Image
- Diagram
- Code
- Query
- Embedded dataset

## Spreadsheet as a Semantic Object

A spreadsheet is not only a grid of cells.

It contains:

- schema
- records
- formulas
- references
- constraints
- views
- calculations

Example:

```
Enemy
├── id: EnemyId
├── hp: Health
├── attack: Damage
├── speed: Speed
└── drops: ItemReference[]
```

The visual table is only one representation.

## Formula Model

Formulas should be represented as an expression tree rather than raw text.

Example:

```
=Attack * Speed
```

becomes:

```
Multiply
├── Reference(Attack)
└── Reference(Speed)
```

Benefits:

- static analysis
- AI understanding
- dependency tracking
- safer refactoring

## References

References are typed relationships, not strings.

Bad:

```
weapon_id = "sword_001"
```

Better:

```
WeaponReference("sword_001")
```

The system can then detect:

- broken references
- unused data
- dependency impact

## AI-Native Operations

AI should operate on semantic objects.

Not:

```
click cell B17
copy text
move cursor
```

But:

```
Update enemy balance model
Explain affected systems
Generate migration plan
Create review summary
```

## Future Direction

This model allows:

- Word-like documents
- Excel-like computation
- Markdown editing
- Git workflows
- AI agents
- collaborative workspaces

without creating separate incompatible products.
