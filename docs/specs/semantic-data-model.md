# Semantic Data Model Specification

Decision state: Accepted semantic-first direction and stable identity under
ADR-0015; broader document/block graph remains Provisional

Implementation state: Milestone 02 schema/entity/field model, stable identity,
typed references, bound formulas, validation, diff, and merge are implemented

## Purpose

The semantic data model is the foundation of Tachiko Work.

Tachiko Work does not treat Word files, spreadsheets, Markdown files, or game data files as separate primary objects.

The primary object is a typed semantic graph.

## Core Principle

Meaning comes before representation.

A document should describe:

- content
- structure
- relationships
- computation
- constraints
- history

Rendering is a view over the model.

## Core Objects

### Current Milestone 02 aggregate

The implemented aggregate is intentionally narrower than the long-term graph:

```text
Document(DocumentId, title)
├── Schema(SchemaId, SchemaKey)
│   └── FieldDefinition(FieldId, FieldKey, FieldType, required)
└── Entity(EntityId, EntityKey, SchemaId)
    └── FieldId -> typed Value
```

`DocumentId`, `SchemaId`, `FieldId`, and `EntityId` are opaque typed stable
identities. Schema, field, and entity keys are mutable human addresses. A key
rename leaves the stable ID unchanged. Deterministic runtime indexes resolve
human keys before an operation stores or executes a bound relationship; those
indexes are derived state and are not persisted.

The closed scalar profile currently includes finite `Number`, `Text`,
`Boolean`, and date-only `Date` values. The M07 `Date` addition is the explicit
semantic transition accepted by [Issue #266](https://github.com/nurockplayer/tachiko-work/issues/266)
and implemented by [Issue #267](https://github.com/nurockplayer/tachiko-work/issues/267)
through the existing [ADR-0020 Semantic API](../decisions/ADR-0020-first-class-headless-semantic-api.md).
It extends this closed profile only; it does not promote the broader document
graph or introduce a new type architecture. `Date` is a proleptic Gregorian
civil date in the bounded `0001..=9999` range, canonically represented as
`YYYY-MM-DD` without time or timezone semantics.

### Document

A document is a graph containing semantic blocks.

Possible blocks:

- text
- heading
- table
- spreadsheet
- formula
- code
- diagram
- dataset
- chart
- query result
- embedded application

### Entity

Entities represent typed domain objects.

Examples:

```text
Enemy
Weapon
Quest
Customer
Experiment
FinancialMetric
```

Entities have:

- identity
- schema
- fields
- references
- validation rules

## Typed References

References are not plain strings.

Bad durable relationship:

```text
weapon_key = "iron_sword"
```

Implemented bound relationship:

```text
EntityReference(EntityId("opaque-stable-id"))
FormulaReference(EntityId("opaque-stable-id"), FieldId("opaque-field-id"))
```

Human authoring still uses `[iron_sword.damage]`; binding resolves that address
in one snapshot, and later projection proves the current address round-trips to
the same stable IDs. This enables:

- dependency analysis
- safe rename
- impact analysis
- AI reasoning

## Computation

Computed values are first-class objects.

Examples:

- formulas
- derived statistics
- simulations
- reports

The same computation engine can power:

- spreadsheets
- game balancing
- business models
- scientific documents

## Long Term Goal

A single semantic model should support:

- Office documents
- Markdown workflows
- Git repositories
- AI agents
- collaborative editing
- computational documents
