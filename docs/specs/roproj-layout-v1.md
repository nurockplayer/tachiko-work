# Tachiko Work .roproj Layout v1 (Implementation Draft)

Decision state: Provisional

Implementation state: Not implemented in v0.1

Decision owner: #41, constrained by ADR-0003 and #21/#25/#38

## Purpose

ADR-0003 is Accepted and establishes `.roproj` as the target canonical editable/source representation. This document is an implementation draft for that accepted direction, not an Accepted physical-layout contract.

The current v0.1 CLI does not yet implement `.roproj`; current workflows persist deterministic `.ro` files.

## Illustrative layout

```text
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

The exact split, file naming, sharding, and directory layout remain subject to implementation evidence and #41.

None of these paths may become semantic object identity merely because they are convenient for Git materialization.

## Required directional properties

The eventual `.roproj` materialization must preserve the accepted goals of:

- UTF-8 where textual
- deterministic/canonical output
- human-readable changes where practical
- stable semantic identity independent of storage paths
- Git-friendly diffs and merge
- lossless semantic relationship with the portable `.ro` artifact

The exact canonical rules are being hardened in #21, #25, #37, #38, and #41.

## Design principle

Git is a storage and collaboration protocol, not the user interface.

Users edit through Tachiko Work semantic operations. Git stores reviewable materialization/history; `.roproj` is the accepted target representation for that workflow.
