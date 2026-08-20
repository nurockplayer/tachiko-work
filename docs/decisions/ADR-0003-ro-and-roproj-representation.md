# ADR-0003: Dual representation for .ro and .roproj

Status: Proposed

## Context

A single binary document file is convenient for users but poor for Git workflows.

A directory-based representation is excellent for version control but less convenient for ordinary file handling.

## Decision

Tachiko Work will maintain one semantic model with multiple representations.

Portable package:

```
project.ro
```

Git working representation:

```
project.roproj/
```

Both must be deterministic serializations of the same model.

## Requirements

- canonical ordering
- UTF-8
- human-readable where practical
- Git diff friendly
- deterministic pack/unpack

## Principle

Git is storage infrastructure, not the user interface.
