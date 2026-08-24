# ADR-0023: `.roproj/v1` canonical tree and sharding

## Status

Accepted

Decision issue: [#41](https://github.com/nurockplayer/tachiko-work/issues/41)

Validated by: [merged PR #101](https://github.com/nurockplayer/tachiko-work/pull/101)
and [`.roproj/v1` physical layout and sharding evidence](../research/2026-08-24-roproj-v1-layout-and-sharding.md)

Specified by: [`roproj-format.md`](../specs/roproj-format.md) and
[`roproj-layout-v1.md`](../specs/roproj-layout-v1.md)

Related authority: ADR-0003 and ADR-0015 through ADR-0022

## Context

ADR-0003 makes `.roproj` the canonical editable, Git-native materialization of
the semantic document, while `.ro` is a derived portable artifact. ADR-0015
requires durable identity to be opaque and independent of paths, layout,
ordering, names, and mutable content. ADR-0017 requires version-owned DTOs,
explicit version dispatch, canonical emission, and migration rather than
Rust/Serde structures becoming a wire contract.

Issue #41 asks for a concrete `.roproj/v1` tree that keeps ordinary changes
reviewable without making filenames or line positions semantic identity. The
recorded experiment compared a monolithic document, mutable-key paths,
per-object files, range shards, and fixed hash shards. Its 16-way fixed hash
layout retained a constant 18-file tree, localized scalar edits and beginning
insertions to one JSONL line in one file, and avoided the rename/path churn,
per-object file count, and range-shift cascades of the alternatives.

The experiment is sufficient to choose the v1 physical layout. It is not
evidence to freeze a production codec, host persistence behavior, package
integrity, Git integration, semantic delta, or merge protocol.

## Decision

### 1. `.roproj/v1` has one closed canonical tree

Every canonical `.roproj/v1` materialization contains exactly these 18 regular
files at the shown paths. Its root contains only `manifest.json`,
`schemas.json`, and the required `entities/` directory; that directory
contains exactly the 16 files shown and no subdirectories:

```text
project.roproj/
├── manifest.json
├── schemas.json
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

The root basename and host location are discovery/container details, not
semantic identity. Every entity shard exists even when it has no records; an
empty canonical shard is a zero-byte regular file.

`manifest.json` is the only representation-version envelope. A reader selects
the `.roproj/v1` decoder from its manifest before it decodes `schemas.json` or
any entity record. Unsupported, missing, or malformed versions fail closed and
are not semantically decoded, canonicalized, migrated, or rewritten.

### 2. Entities use fixed ID-derived placement

For each decoded stable `EntityId` string `id`, the canonical shard is:

```text
entities/<first lowercase hexadecimal nibble of SHA-256(UTF-8(id))>.jsonl
```

The input to SHA-256 is the exact decoded Unicode scalar sequence of the
stable ID encoded as UTF-8, without Unicode normalization, case folding, or
hashing of its JSON source spelling. The high four bits are rendered as one of
the lowercase ASCII characters `0` through `f`.

SHA-256 is a version-scoped placement function only. It is not an identity,
content address, integrity claim, revision, or security boundary. A future
representation version may choose another fanout or placement function; v1
does not adaptively split, rebalance, index, or widen its 16 shards.

Within each shard, entity records are ordered by their complete stable
`EntityId` in unsigned UTF-8 byte lexicographic order. This comparison is over
the whole decoded ID, not a hash prefix, a human key, a filename, a locale, or
an insertion order. Schemas and fields are likewise ordered by their complete
stable IDs in unsigned UTF-8 byte order whenever their DTO collections are
unordered.

### 3. Paths, shards, and lines are not semantic identity

Only IDs and semantic relationships decoded from version-owned DTOs identify
semantic objects. A path component, root name, shard name, record filename,
line number, record order, placement hash, or JSON spelling MUST NOT create,
delete, rename, retarget, or otherwise identify an object.

Thus moving a record to a wrong input shard or changing its whitespace is a
representation/layout defect, not a semantic mutation. A semantic diff of a
layout-only change is empty; canonical materialization restores the one v1
tree. This preserves ADR-0015's stable-ID law rather than relying on Git rename
heuristics or storage coordinates.

### 4. JSON and JSONL have canonical physical forms

All JSON values use the applicable `.roproj/v1` canonical JSON rules and
version-owned DTO member order. In particular, canonical bytes are UTF-8
without BOM, use LF line endings, preserve decoded Unicode without implicit
normalization or case folding, reject duplicate members after escape decoding,
and reject unknown members in the closed-world v1 DTO.

`manifest.json` and `schemas.json` are pretty canonical JSON with two-space
indentation and exactly one final LF. Every entity record is one compact
canonical JSON object on one JSONL line. A nonempty shard ends with exactly one
final LF; there is no inter-record whitespace and blank JSONL records are
invalid. An empty canonical shard is zero bytes.

Number, formula, validation, and other semantic meaning remain governed by
ADR-0017 through ADR-0019. The exact `.roproj/v1` DTO shapes, member lists,
tags, omission/null/default rules, and field-value encodings belong to
[`roproj-format.md`](../specs/roproj-format.md), not Rust declarations or this
layout ADR.

### 5. Current semantic categories have fixed v1 boundaries

`schemas.json` is one low-cardinality schema file in v1. Each entity record is
one line in its canonical shard and carries the entity's DTO-defined values.
Bound formulas remain inline in the owning entity field value; v1 has no
`formulas/` directory and does not manufacture a `FormulaId` from a path.

Assets, shared views, semantic tests, local caches/indexes, and generated
`.ro` files are outside the canonical v1 tree. The canonical tree therefore
rejects `assets/`, `views/`, `tests/`, `formulas/`, cache directories, and all
other unrecognized children. These exclusions do not define future category
semantics; they prevent this layout decision from inventing them.

### 6. The v1 canonicalizer accepts a bounded non-canonical family

The canonicalizer accepts only this bounded input family:

- `manifest.json`, `schemas.json`, and the ordinary `entities/` directory occur
  at their exact required locations;
- after manifest-first v1 selection, the two JSON files may use non-canonical
  structural whitespace, object-member order, legal string/token spelling,
  and schema/field stable-ID order;
- regular `*.jsonl` files below `entities/` may have non-canonical names,
  nesting, placement, record order, object-member order, legal string/token
  spelling, or non-LF JSON whitespace within a physical record;
- every nonempty JSONL input is a sequence of exactly-one-object records, each
  terminated by one LF; a physical LF cannot occur inside a record, and blank
  records or other inter-record bytes are invalid;
- missing canonical empty shards and extra empty JSONL inputs are admissible;
  and
- no path component supplies semantic identity or relationship meaning.

Ordinary directories below `entities/` are admissible only as traversal
ancestors of at least one accepted regular `*.jsonl` file. Empty directories
and directories without such a descendant are rejected. The canonicalizer
also rejects symlinks, non-regular non-directory entries, unknown top-level
children, and non-JSONL files below `entities/` rather than following or
silently dropping them. It fails closed on duplicate JSON members, blank JSONL
records, unknown DTO members, duplicate schema IDs, duplicate field IDs within
an owning schema, duplicate entity IDs across all accepted entity inputs, and
invalid DTO or semantic content. Lexically equal IDs of different declared
types are not duplicates merely because their string spellings match.

The processing order is manifest-first dispatch, strict version-owned DTO
decode, uniqueness proof at each DTO collection's declared ID scope,
conversion to the semantic aggregate, the operation's Accepted validation
gate, and fresh canonical materialization. Reading or inspecting non-canonical
input does not authorize an implicit durable rewrite.

## Compatibility and preserved authority

This ADR accepts only the `.roproj/v1` physical layout, placement, ordering,
canonical-tree, and bounded-canonicalizer contract. It preserves rather than
amends ADR-0015 through ADR-0022, including stable identity, crate boundaries,
versioned DTOs and migrations, bound-formula and Number semantics, staged
validation, Semantic API behavior, and the resident runtime/host boundary.

In particular, `.roproj/v1` is a representation namespace distinct from
legacy direct `.ro` profiles and any future packaged `.ro` profile. The
semantic `Document` remains the authority; `.ro` remains derived from
`.roproj` under ADR-0003.

## Deferred decisions

This ADR does not decide:

- the production reader/writer codec implementation;
- `.roproj` resource limits, error/resource profiles, or exact error
  precedence;
- whether normal open accepts the bounded non-canonical family or that family
  is limited to an explicit canonicalize/import admission operation;
- host atomic save, replacement, locking, permissions, watcher behavior,
  crash recovery, or symlink-race handling;
- portable `.ro` package/container, integrity, signatures, compression, or
  tree-digest rules ([#43](https://github.com/nurockplayer/tachiko-work/issues/43));
- Git attributes, merge drivers, CI commands, or generated-`.ro` consistency
  checks ([#44](https://github.com/nurockplayer/tachiko-work/issues/44));
- semantic delta and revision-envelope details
  ([#45](https://github.com/nurockplayer/tachiko-work/issues/45));
- semantic three-way merge, conflict projection, or Git merge behavior
  ([#46](https://github.com/nurockplayer/tachiko-work/issues/46));
- canonical locations or semantics for assets, shared views, and semantic
  tests; or
- a larger entity-shard fanout, schema sharding, adaptive shards, or category
  expansion in a later representation version.

## Consequences

- Canonical `.roproj/v1` trees have a predictable, cross-platform-safe path
  vocabulary and fixed file count.
- Local entity edits generally affect one compact JSONL record in one stable
  shard, while object identity remains independent of that physical location.
- Tools can derive the canonical v1 path set without a manifest inventory.
- Implementations must own a strict version-specific DTO conversion boundary;
  they may not serialize semantic Rust types as the format by convenience.
- A later format version is required to change the fanout, placement contract,
  or categories materialized in the canonical tree.

## Evidence and related work

The [Issue #41 research record](../research/2026-08-24-roproj-v1-layout-and-sharding.md)
contains the executable comparison, deterministic-materialization checks, and
Git measurements supporting this decision. At 4,096 representative entities,
the selected tree used 18 files, distributed records across 16 fixed buckets,
and avoided range-shard churn on an insertion before all existing IDs.

## Related

- ADR-0003
- ADR-0015 through ADR-0022
- [Issue #41](https://github.com/nurockplayer/tachiko-work/issues/41)
- [Layout and sharding research](../research/2026-08-24-roproj-v1-layout-and-sharding.md)
