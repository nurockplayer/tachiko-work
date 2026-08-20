# Semantic Data Model Specification

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

Bad:

```text
weapon_id = "sword_001"
```

Preferred:

```text
WeaponReference("sword_001")
```

This enables:

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
