# Formula Engine Specification

## Overview

The Tachiko Work formula engine is a deterministic semantic computation system.

## Design Principles

- deterministic evaluation
- typed expressions
- dependency tracking
- bounded parsing and resource limits
- static analysis and explainability

## Expression Model

Formulas are represented as typed expression trees and rendered with a canonical
copyable syntax.

Implemented v0.1 authoring syntax:

```text
[entity.field] + 1.5 * ( [a.b] / 2 )
```

The syntax includes:

- finite decimal and scientific numeric literals
- `+`, `-`, `*`, `/` with standard precedence
- unary `+` and `-`
- parentheses
- `min(left, right)` and `max(left, right)`
- semantic references in bracketed form `[entity.field]`

Examples:

```text
min(60, [iron_sword.damage] / [iron_sword.attack_interval] + 5)
-[enemy.defense] + 2
max(0, [attacker.power] - [target.armor])
```

Canonical text is emitted by `tachiko explain` and is valid input to
`tachiko formula set --expression`.

Boundaries enforced before recursion:

- source input: max 4096 bytes
- canonical text: max 4096 bytes
- nodes: max 256
- post-desugaring depth: max 64

All failures return stable byte offsets before recursive evaluation.

Reference resolution and semantic validation are performed by the document model.

## Required Capabilities

v0.1 implemented capabilities:

- canonical parse + format
- typed numeric operations
- dependency graph extraction
- deterministic recalculation
- cycle detection and diagnostics
- division-by-zero and non-finite result diagnostics

Not implemented in v0.1:

- aggregation clauses
- conditional expressions
- lookups
- user-defined functions
- schema-level computed defaults
- simulation or optimization planners
- AI-generated formula language extension

## Game Development Use Cases

Current focus examples:

- DPS calculation
- TTK analysis
- economy curves
- progression systems
- balance-impact reasoning

## Validation

Formula evaluation is covered by parser, validation, and calculation tests.

Example:

```text
Dragon DPS must stay below threshold.
Early game economy growth must remain within range.
Invalid syntax and bounded resource failures must not recurse.
```

The formula engine becomes the foundation for computational documents.
