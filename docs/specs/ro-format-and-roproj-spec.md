# .ro and .roproj Format Direction

## Purpose

Tachiko Work requires a format that works for both humans and version control systems.

The design separates logical model from physical representation.

## Canonical Principle

The semantic model is the source of truth.

Neither .ro nor .roproj should own meaning.

They are serialization forms.

## .ro

Portable package representation.

Optimized for:

- desktop users
- sharing
- backup
- transport
- archive

Example:

```
Project.ro
```

Users should not need to understand the internal structure.

## .roproj

Git working representation.

Optimized for:

- branch workflows
- pull requests
- reviews
- merge operations
- CI validation

Example:

```
Project.roproj/
├── manifest.json
├── schema.json
├── data/
│   ├── enemies.jsonl
│   └── weapons.jsonl
├── formulas/
├── views/
└── tests/
```

## Requirements

The Git representation should be:

- deterministic
- canonical
- UTF-8
- human readable
- diff friendly

## Why Not Binary Only?

A single binary file recreates the same problems found in traditional spreadsheet files:

- poor diffs
- merge conflicts
- opaque history
- difficult review

## Why Not Only Text Files?

Pure text files are excellent for Git but less convenient for normal users.

Therefore both representations are required.

## Migration

Legacy formats such as DOCX and XLSX should be handled by adapters.

The core format should not inherit historical compatibility problems.
