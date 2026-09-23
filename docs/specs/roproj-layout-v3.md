# Tachiko Work `.roproj/v3` layout and canonicalizer

Decision state: Accepted under [ADR-0041](../decisions/ADR-0041-bounded-durable-field-constraints.md).

Implementation state: The strict exact-tree constructor, native manifest-first
reader, atomic no-clobber publisher, and explicit canonical v1/v2 conversion
are implemented by #449. Designer selection/save and portable-package support
remain outside this implementation slice.

## Canonical tree

Every canonical `.roproj/v3` tree contains exactly these 19 regular files:

```text
project.roproj/
├── manifest.json
├── schemas.json
├── definitions.json
└── entities/
    ├── 0.jsonl
    ├── 1.jsonl
    ├── 2.jsonl
    ├── 3.jsonl
    ├── 4.jsonl
    ├── 5.jsonl
    ├── 6.jsonl
    ├── 7.jsonl
    ├── 8.jsonl
    ├── 9.jsonl
    ├── a.jsonl
    ├── b.jsonl
    ├── c.jsonl
    ├── d.jsonl
    ├── e.jsonl
    └── f.jsonl
```

The root contains only the three JSON files and `entities/`; the directory has
only the sixteen shown shards and no subdirectories. Every shard exists.
`definitions.json` exists even when its canonical body is `[]\n`. An empty
entity shard is exactly zero bytes. No assets, views, tests, formulas, caches,
indexes, inventory, or other child is part of this closed v3 tree.

## Placement and bytes

For a decoded stable Entity ID, v3 uses the incorporated v2 placement function:

```text
bucket = first lowercase hexadecimal nibble of SHA-256(UTF-8(id))
path   = entities/<bucket>.jsonl
```

The exact decoded Unicode scalar sequence is hashed; no normalization, folding,
path-derived identity, or raw JSON spelling is used. Each shard sorts records by
full Entity ID in unsigned UTF-8 byte lexicographic order. Schema and field
collections use the same stable-ID order; semantic sequences retain semantic
order.

`manifest.json`, `schemas.json`, and `definitions.json` use the v3 DTO's
canonical two-space JSON and one final LF. Entity records are compact canonical
JSONL with one LF per record; an empty shard is zero bytes. UTF-8/BOM,
duplicate-member, unknown-member, number, and whitespace rules are the fixed v3
format authority. Paths, filenames, shard names, line numbers, and record
positions never identify semantic objects.

## Admission and migration boundary

Manifest-first dispatch selects exactly v3 before any other content is decoded.
Missing, malformed, unsupported, or mismatched versions fail closed. A reader
never treats an unknown file as an extension or silently rewrites a different
version. Canonicalization is explicit and bounded by the v3 format contract.

V2 migration is explicit, deterministic, no-clobber, and complete: decode and
admit the source, add `none` to every field, validate, canonicalize all nineteen
files, and publish only a complete candidate to an absent destination. V1 first
uses its existing explicit v1→v2 migration. Ordinary reads and saves do not
upgrade. Portable-package/v1 remains its exact v1 payload and package-v2 is
outside this layout; unsupported requests report that boundary truthfully.
