# Git-Native Workflow

## Principle

Git is a storage and collaboration protocol, not the user interface.

## Representation

Current v0.1 workflow storage is the single-file `.ro` envelope in this repository.

```
project.ro
```

`.ro` is optimized for portable packages, deterministic review snapshots, and direct
CLI read/write operations.

`.roproj` remains a deferred design direction in ADR-0003 and is not implemented in
this release.

The model itself is the compatibility boundary for future representations.

## Benefits

- Git reviewable, byte-stable diffs at the semantic-document level
- Branch based workflows
- Semantic merge
- CI validation
- Reviewable data changes

## Goal

Make non-programmers first-class participants in Git-based workflows.
