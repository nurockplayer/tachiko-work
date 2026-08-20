# .roproj Format Specification

## Purpose

`.roproj` is the Git-native working representation of a Tachiko Work project.

It exists because binary document formats are poor Git citizens.

## Goals

A `.roproj` directory should be:

- deterministic
- human readable
- UTF-8
- diff friendly
- merge friendly
- CI friendly

## Example

```
project.roproj/
├── manifest.json
├── schema.json
├── data/
│   ├── enemies.jsonl
│   └── weapons.jsonl
├── formulas/
├── views/
└── tests/
```

## Git Workflow

Users should be able to:

- branch
- commit
- review
- merge
- validate

without manually editing raw files.

## Canonical Ordering

Serialization must define:

- stable ordering
- normalized formatting
- deterministic output

so equivalent documents produce equivalent Git history.

## Design Goal

Git becomes a storage and collaboration protocol, not the user interface.
