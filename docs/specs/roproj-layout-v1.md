# Tachiko Work .roproj Layout v1

## Purpose

`.roproj` is the Git-native working representation of a Tachiko Work project.

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

## Requirements

- UTF-8
- deterministic ordering
- human readable changes
- stable identifiers
- Git friendly diffs

## Design Principle

Git is a storage and collaboration protocol, not the user interface.

Users edit through Tachiko Work. Git stores semantic history.
