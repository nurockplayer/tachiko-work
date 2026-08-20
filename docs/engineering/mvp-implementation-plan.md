# MVP Implementation Plan

## First Implementation Principle

Do not build Office compatibility first.

Prove the new model first.

## Suggested Components

```
Rust Core
 |
 +-- semantic model
 +-- schema engine
 +-- formula engine
 +-- storage
 +-- diff engine

Frontend
 |
 +-- React UI
 +-- spreadsheet interaction
```

## MVP Success Criteria

A designer can modify game data.

A developer can review changes through Git.

The system can validate and explain the impact of changes.

This proves the foundation before expanding into general productivity.
