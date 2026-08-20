# Formula Engine Specification

## Overview

The Tachiko Work formula engine is not a text calculator.

It is a semantic computation system.

## Design Principles

- deterministic evaluation
- typed expressions
- dependency tracking
- static analysis
- AI-readable operations

## Expression Model

Formulas are represented as expression trees.

Example:

```text
Attack * Speed
```

becomes:

```text
Multiply
├── Reference(Attack)
└── Reference(Speed)
```

## Required Capabilities

Initial engine:

- arithmetic
- aggregation
- conditional expressions
- lookup
- references
- constraints

Future:

- simulation
- optimization
- statistical analysis
- AI generated formulas

## Game Development Use Cases

Examples:

- DPS calculation
- TTK analysis
- economy curves
- progression systems
- drop probability

## Validation

Formula evaluation should support tests.

Example:

```text
Dragon DPS must stay below threshold.
Early game economy growth must remain within range.
```

The formula engine becomes the foundation for computational documents.
