# .ro and .roproj Format Direction

## Purpose

Tachiko Work requires a format that works for both humans and version control
systems.

In v0.1, this direction is split by implementation stage: `.ro` is the shipped
single-file persistence format; `.roproj` remains a deferred design direction.

## Canonical Principle

The semantic model is the source of truth.

Neither .ro nor .roproj owns meaning.

They are serialization forms.

## .ro

Portable package representation (implemented).

Optimized for:

- desktop users
- sharing
- backup
- transport
- archive

`Project.ro` is the v0.1 production artifact consumed by CLI commands and
runtime export tooling.

Example:

```
Project.ro
```

Users should not need to understand the internal structure.

## .roproj

Git working representation (proposed, not yet implemented).

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

Planned future requirements for `.roproj`:

- deterministic
- canonical
- UTF-8
- human readable
- diff friendly

Status note: these properties are currently guaranteed for `.ro`, while `.roproj`
parity and conversion are deferred under ADR-0003.

## Why Not Binary Only?

A single binary file recreates the same problems found in traditional spreadsheet files:

- poor diffs
- merge conflicts
- opaque history
- difficult review

## Why Not Only Text Files?

Pure text files are excellent for Git but less convenient for normal users.

The long-term plan remains both representations, but only one (`.ro`) is active
today.

## Migration

Legacy formats such as DOCX and XLSX should be handled by adapters.

The core format should not inherit historical compatibility problems.
