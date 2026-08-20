# MVP Implementation Plan

## First Implementation Principle

Do not build Office compatibility first.

Prove the new model first.

## MVP Components

```
Rust Core
 |
 +-- semantic model
 +-- schema engine
 +-- formula engine
 +-- storage
 +-- diff engine
 +-- CLI
 +-- AI semantic read/query
 +-- game balance example
```

## MVP Success Criteria

A developer or technical designer can create and modify game balance data.

A developer can review semantic changes through Git-friendly diff.

The system can validate documents, calculate formulas, and explain the model and
impact of changes through the CLI and AI read/query interface.

This proves the semantic foundation before expanding into general productivity.

## Deferred After the MVP

The graphical workspace, including React and spreadsheet-style interaction, is a
later layer. Semantic merge, realtime collaboration, cloud SaaS, Office
compatibility, engine plugins, and enterprise permissions are also deferred.
