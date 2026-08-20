# Git-Native Workflow

## Principle

Git is a storage and collaboration protocol, not the user interface.

## Representation

A logical Tachiko Work document can have:

```
project.ro

project.roproj/
├── manifest.json
├── schema.json
├── data/*.jsonl
├── formulas/
├── views/
└── tests/
```

`.ro` is optimized for portable packages.

`.roproj` is optimized for Git workflows.

Both represent the same semantic model.

## Benefits

- Human readable diffs
- Branch based workflows
- Semantic merge
- CI validation
- Reviewable data changes

## Goal

Make non-programmers first-class participants in Git-based workflows.
