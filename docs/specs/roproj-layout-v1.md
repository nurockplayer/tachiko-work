# Tachiko Work .roproj Layout v1 (Deferred)

## Purpose

This is a deferred ADR-0003 draft. `.roproj` is not part of the implemented
v0.1 CLI contract.

## Example

```
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

## Requirements (deferred)

- UTF-8
- deterministic ordering
- human readable changes
- stable identifiers
- Git friendly diffs

## Design Principle

Git is a storage and collaboration protocol, not the user interface.

Users edit through Tachiko Work. Git stores semantic history.
