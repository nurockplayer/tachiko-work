# .ro and .roproj Format Concept

## Goal

Support users and Git workflows from one semantic model.

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

Current implementation status: `.ro` is the implemented persistence format in v0.1.

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

Planned representation (not implemented in v0.1, proposed in ADR-0003):

- Git diff
- branch workflows
- code review
- CI

This representation is intentionally deferred until ADR-0003 is accepted.

## Rule

`.ro` is the active v0.1 persisted representation.

`.ro` and `.roproj` are the target dual-representation model under ADR-0003.

The semantic model is the source of truth.

The system does not yet provide deterministic `.ro` ↔ `.roproj` conversion.
