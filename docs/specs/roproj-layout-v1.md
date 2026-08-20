# Tachiko Work .roproj Layout v1 (Implementation Draft)

## Purpose

ADR-0003 is Accepted and establishes `.roproj` as the canonical editable/source representation. This document is an implementation draft for that accepted direction.

`.roproj` is not part of the implemented v0.1 CLI contract yet; current v0.1 workflows persist deterministic `.ro` files.

## Illustrative layout

```text
project.roproj/
├── manifest.json
├── schema.json
├── data/
│   ├── entities.jsonl
│   └── tables.jsonl
├── formulas/
├── views/
├── tests/
└── assets/
```

The exact split and file naming remain subject to implementation validation.

## Required properties

- UTF-8 for textual materialization
- deterministic ordering
- human-readable changes where practical
- stable identifiers
- Git-friendly diffs and merge
- lossless semantic conversion to/from the portable `.ro` artifact

## Design principle

Git is a storage and collaboration protocol, not the user interface.

Users edit through Tachiko Work semantic operations. Git stores semantic history; `.roproj` is the accepted target materialization for that history.
