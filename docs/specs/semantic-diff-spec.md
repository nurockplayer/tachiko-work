# Semantic Diff Specification

## Problem

Traditional spreadsheet diff compares files.

Humans need to understand meaning.

## Goal

Tachiko Work provides semantic changes instead of raw text changes.

## Example

Traditional diff:

```diff
- goblin,180,18,1.4
+ goblin,210,21,1.4
```

Semantic diff:

```text
Goblin

HP
180 -> 210 (+16.7%)

Attack
18 -> 21 (+16.7%)
```

## Required Features

- field-level changes
- type awareness
- calculated impact
- dependency tracing
- human-readable summaries

## Git Integration

Semantic diff should power:

- pull requests
- reviews
- merge decisions
- release notes
- AI summaries

## Principle

Git stores changes.

Tachiko Work explains meaning.
