# .roproj Format Specification (Deferred, ADR-0003)

## Purpose

`.roproj` is the proposed Git-native working representation defined by ADR-0003.
It is a deferred direction and is **not implemented** in the current v0.1 release.

## Goals (deferred)

This goal remains proposed:

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

Current v0.1 users work through `.ro` files for branch/merge/review/validate flows.
`.roproj` workflow details are deferred until ADR-0003 is implemented.

## Canonical Ordering

Serialization must define:

- stable ordering
- normalized formatting
- deterministic output

so equivalent documents produce equivalent Git history.

## Design Goal

Git remains a storage and collaboration protocol, not the user interface.
