# .ro and .roproj Format Concept

## Goal

Support both normal users and Git repositories without creating separate sources of truth.

## Representations

### Portable package

```
project.ro
```

Designed for:

- sharing
- backup
- desktop usage
- transport

### Git working representation

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

Designed for:

- Git diff
- branch workflows
- code review
- CI

## Rule

`.ro` and `.roproj` are two representations of the same semantic model.

The semantic model is the source of truth.

The system must provide deterministic round-trip conversion.
