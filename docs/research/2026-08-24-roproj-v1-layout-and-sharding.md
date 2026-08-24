# `.roproj/v1` physical layout and sharding evidence

Decision state: **Hypothesis with an Accepted-candidate recommendation for
Issue #41 — not Accepted authority**

Repository base: `main@156565a3d2dc7664088a24b7f6e38d02ad4e04fe`

Authority reviewed:

- the Product Constitution, Design Principles, knowledge-authority rules, and
  canonical reconciliation register;
- ADR-0003 and ADR-0015 through ADR-0019;
- the `.ro`/`.roproj`, storage/versioning, canonical JSON, semantic model,
  semantic diff, and Git-native workflow specifications;
- the current storage, semantic-core, formula, diff, and workspace-engine
  implementations and conformance tests; and
- completed work in #25, #37, #38, #40, #70, #74, #89, and #96.

This record does not implement a production reader/writer, amend an ADR, or
change an Accepted semantic contract. Its JavaScript records are deliberately
independent probe DTOs, not durable wire authority.

## Executive finding

Adopt a small closed-world tree with pretty canonical JSON for low-cardinality
metadata and schemas, plus compact canonical JSONL for entities in 16 fixed
stable-ID-derived hash shards:

```text
balance.roproj/
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

Every canonical v1 tree has exactly these 18 regular files and no other
children. Empty entity shards are zero-byte files. The root basename and its
host path are discovery/container details, not identity.

For an entity with stable `EntityId` value `id`, its canonical shard is:

```text
entities/<first-lowercase-hex-nibble(SHA-256(UTF-8(id)))>.jsonl
```

SHA-256 is a version-scoped placement function here. It is not a semantic ID,
content identity, integrity claim, revision, or security boundary. Records in
each shard are ordered by the complete stable ID using unsigned UTF-8 byte
order.

This is the smallest tested option that simultaneously avoids mutable-name
path churn, per-object file explosion, range-split cascades, and one large
entity file. At 4,096 representative entities it used 18 files, capped the
largest file at 83,162 bytes, and localized scalar edits and beginning
insertions to one changed path and one JSONL line.

## Authority and non-goals

The recommendation preserves these existing decisions rather than redesigning
them:

- the semantic `Document` is meaning authority;
- `DocumentId`, `SchemaId`, `FieldId`, and `EntityId` are opaque stable
  identities independent of names, paths, layout, ordering, or content;
- human keys are mutable authoring addresses;
- a `.roproj` directory does not create a new `ProjectId`;
- a formula remains a bound AST anchored to an entity field; neither its source
  text nor a formula filename becomes durable meaning;
- version-owned storage DTOs, not Rust/Serde declarations, own wire spelling;
- `.roproj/v1` is a representation namespace distinct from `direct-ro/v1`,
  `direct-ro/v2`, and the future packaged `.ro` namespace;
- canonical Number, Unicode preservation, collection ordering, and validation
  semantics remain those of ADR-0017 through ADR-0019; and
- `.ro` remains a derived portable artifact, not a second editable authority.

This work does not define Git integration (#44), a semantic delta (#45), a
three-way merge protocol (#46), packaged `.ro` integrity (#43), diagnostics,
formula-failure projection, or host persistence transactions.

The current 8 MiB direct-JSON input and 256-byte number-token limits do not
apply to `.roproj/v1`. Reusing them would violate their explicitly
representation-local scope.

## Recommended v1 contract

### Manifest

`manifest.json` is the only version envelope and contains low-churn document
metadata. The candidate outer shape and member order are:

```json
{
  "format": "tachiko.roproj",
  "format_version": 1,
  "document": {
    "id": "opaque-document-id",
    "title": "Balance"
  }
}
```

The file must not contain entity/schema counts, shard inventories, per-file
digests, timestamps, tool versions, absolute paths, or generated `.ro`
metadata. The fixed v1 tree already implies its shard inventory; volatile
summary fields would turn the manifest into a global diff hotspot.

An implementation must select the `.roproj/v1` decoder from this exact envelope
before decoding `schemas.json` or entity records. Unsupported versions are not
semantically inspected or rewritten.

### Schemas

`schemas.json` is a pretty canonical JSON array of schema records, ordered by
stable `SchemaId`. Each schema owns an array of field definitions ordered by
stable `FieldId`. Reference field types target stable `SchemaId` values.

Schemas are intentionally one file in v1. They are low-cardinality structural
metadata in the current game-balance model; splitting by mutable schema key is
incorrect, while splitting each small record adds filesystem complexity with
no measured benefit. A future representation version may shard schemas if
real scale evidence invalidates this assumption. V1 must not split it
adaptively.

### Entities

Each entity is one compact canonical JSON object on one JSONL line. It contains
its stable `EntityId`, mutable key, stable schema target, and field values keyed
or ordered by stable `FieldId` as declared by the normative v1 DTO. Reference
values and bound formula references use stable IDs.

All 16 shard files always exist. For each entity:

1. take the decoded stable `EntityId` string exactly as persisted;
2. encode that scalar sequence as UTF-8 without Unicode normalization;
3. compute SHA-256;
4. take the high four bits, rendered as one lowercase ASCII hex digit; and
5. place the record in that shard, then sort the shard by the full ID using
   unsigned UTF-8 byte order.

Hashing the decoded ID value, rather than JSON source spelling, makes alternate
valid input escapes converge. Hashing avoids the timestamp-prefix clustering
of UUIDv7 while continuing to treat every ID family as opaque. Fixed lowercase
ASCII filenames avoid host case-folding, reserved-name, separator, and
normalization differences.

No record filename contains a mutable key, an unescaped raw ID, a content hash,
or an array/range position.

### JSON and JSONL bytes

The candidate adopts the existing Tachiko canonical JSON profile for every
JSON value:

- UTF-8, no BOM, LF only, and no trailing whitespace;
- fixed record member order declared by the `.roproj/v1` DTO specification;
- unordered semantic collections ordered by opaque stable ID using unsigned
  UTF-8 bytes;
- semantic ordered sequences preserved exactly;
- no Unicode normalization or case folding;
- ADR-0018 finite-binary64 conversion and canonical number tokens;
- duplicate JSON members rejected after escape decoding; and
- recursive unknown members rejected for the closed-world v1 DTO.

`manifest.json` and `schemas.json` use two-space indentation and end with
exactly one LF. An entity JSONL line is compact canonical JSON with no
inter-record whitespace. A nonempty shard ends with exactly one LF; a canonical
empty shard is zero bytes. Blank JSONL records are invalid.

The physical proposal fixes record boundaries and ordering, but the probe's
plain-JavaScript nested value spelling is not normative. Before acceptance,
`roproj-format.md` must explicitly redeclare every v1 DTO member, tag, required
field, omission rule, and member order. It may adopt the current
identity-aware `direct-ro/v2` logical meanings, but it must not import Rust
struct layout or silently alias the `direct-ro/v2` representation namespace.

### Canonical tree and semantic identity

For the same valid semantic document under `.roproj/v1`:

```text
canonical_paths(document_a) == canonical_paths(document_b)
canonical_bytes(document_a) == canonical_bytes(document_b)
```

when `document_a` and `document_b` differ only by construction order or by
accepted non-canonical representation spelling.

The converse does not make paths semantic. Moving an entity record to the
wrong input shard, renaming a non-canonical shard, or changing input whitespace
may change filesystem bytes but cannot create, delete, or rename the entity.
Only the stable ID inside the decoded record identifies it. A semantic diff of
layout-only changes is empty; canonical materialization then restores the one
v1 tree.

### Supported non-canonical input and normalization

An explicit v1 canonicalizer should support a bounded non-canonical input
family rather than arbitrary directory guessing:

- exact `manifest.json` and `schemas.json` locations are required;
- regular `*.jsonl` files below `entities/` may have non-canonical names,
  nesting, record order, shard placement, whitespace, and legal JSON spellings;
- missing canonical empty buckets and extra empty JSONL inputs are accepted;
- symlinks, non-regular files, unknown top-level children, and non-JSONL files
  under `entities/` are rejected rather than followed or silently dropped;
- duplicate JSON members, duplicate stable record IDs across any files, blank
  JSONL records, unknown DTO fields, invalid typed references/formulas, and an
  invalid semantic document fail closed; and
- no path component contributes semantic identity or relationship meaning.

The pipeline is: select the version from the manifest, strictly decode all
version-owned DTOs, prove cross-file stable-ID uniqueness, convert to the
semantic aggregate, apply the operation's Accepted validation gate, and emit a
fresh canonical tree. Opening or inspecting non-canonical input does not by
itself authorize rewriting durable source. Atomic replacement, recovery, and
locking remain host-persistence concerns.

### Formulas, assets, views, tests, and cache

V1 is closed to categories not represented by the current semantic model:

| Category | `.roproj/v1` boundary |
| --- | --- |
| Bound formulas | Inline in the owning entity field value. No `formulas/`; a path must not invent `FormulaId`. |
| Assets | Outside the v1 tree until asset identity/reference semantics are Accepted. Repository-adjacent assets are not part of v1 canonical equality. |
| Shared views | Outside the v1 tree until shared-view semantics and lifecycle exist. Local UI views are workspace state. |
| Tests | Outside the v1 tree until a semantic test model is Accepted. Repository-adjacent test fixtures remain ordinary project tooling. |
| Local cache/indexes | Outside the v1 tree, rebuildable, and ignored by Git. Never consulted as meaning or included in canonical bytes. |
| Generated `.ro` | Outside the v1 tree and derived from the semantic document. If tracked, #44 must check agreement explicitly. |

Canonical v1 therefore rejects top-level `assets/`, `views/`, `tests/`,
`formulas/`, cache directories, and unrecognized files. This exclusion is not a
claim that those product concepts are unimportant; it avoids inventing durable
semantics in a physical-layout issue.

## Executable experiment

The checked-in [probe](probes/issue-41-roproj-layout.mjs) uses Node standard
library APIs only and the repository's existing Git prerequisite. It creates
disposable Git repositories and compares five materializations:

| Variant | Physical strategy | Purpose |
| --- | --- | --- |
| `monolith_json` | One pretty JSON document | Simplest baseline |
| `mutable_key_jsonl` | Schema/entity files named by mutable schema key | Identity/path-churn negative control |
| `per_object_json` | One pretty JSON file per schema/entity, filename-safe base32hex of stable ID | Maximum localization baseline |
| `range_jsonl` | Sorted entities in sequential groups of 256 | Count/range sharding baseline |
| `hash_jsonl` | 16 fixed SHA-256 stable-ID buckets | Recommended candidate |

The representative game-balance fixture contains characters, weapons, items,
and economy schemas; typed references; inline bound formulas; stable IDs
separate from mutable keys; the ADR-0018 `0.000001` vector; normalized negative
zero; and canonically distinct NFC/NFD-like text values. Runs use 16-entity and
4,096-entity documents.

Every Git metric comes from two real commits and `git diff --numstat` plus
`git diff --name-status`, both with `-M` and with `--no-renames` where rename
interpretation matters. The fixture generator also reverses source schema and
entity construction order and asserts identical materialized-tree digests for
every variant.

### Tree size and file count

| 16 entities | Files | Total bytes | Largest file |
| --- | ---: | ---: | ---: |
| Monolith JSON | 1 | 12,455 | 12,455 |
| Mutable-key JSONL | 9 | 6,872 | 1,739 |
| Per-object JSON | 21 | 10,132 | 803 |
| Range JSONL | 3 | 7,123 | 4,715 |
| Hash JSONL | 18 | 7,123 | 2,265 |

| 4,096 entities | Files | Total bytes | Largest file |
| --- | ---: | ---: | ---: |
| Monolith JSON | 1 | 2,504,369 | 2,504,369 |
| Mutable-key JSONL | 9 | 1,218,686 | 447,704 |
| Per-object JSON | 4,101 | 2,053,246 | 803 |
| Range JSONL | 18 | 1,218,937 | 76,212 |
| Hash JSONL | 18 | 1,218,937 | 83,162 |

The 4,096 records distributed across hash buckets from 236 to 275 records and
69,300 to 83,162 bytes. This is evidence for an initial fixed width, not a
semantic or infinite-scale size guarantee.

### Local edit and rename results

Notation below is `Git paths; added/deleted lines`. `M`, `A`, `D`, and `R` mean
modified, added, deleted, and rename-inferred paths.

| 16-entity change | Monolith | Mutable-key | Per-object | Range JSONL | Hash JSONL |
| --- | --- | --- | --- | --- | --- |
| Scalar edit | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` |
| Bound formula edit | `1M; +12/-3` | `1M; +1/-1` | `1M; +12/-3` | `1M; +1/-1` | `1M; +1/-1` |
| Entity-key rename | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` |
| Schema-key rename | `1M; +1/-1` | `2R; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` |
| Field-key rename | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` | `1M; +1/-1` |
| Add entity | `1M; +15/-0` | `1M; +1/-0` | `1A; +15/-0` | `1M; +1/-0` | `1M; +1/-0` |
| Delete entity | `1M; +0/-15` | `1M; +0/-1` | `1D; +0/-15` | `1M; +0/-1` | `1M; +0/-1` |

The mutable-key schema rename was inferred as two renames (`R100` for the
entity file and `R095` for the schema file). With rename detection disabled it
was two deletes plus two adds. This demonstrates why a contract must not rely
on Git's heuristic rename inference to recover identity.

At 4,096 entities, a scalar edit remained `1M; +1/-1` for every variant. An
entity inserted before every existing stable ID produced:

| Variant | Git result |
| --- | --- |
| Monolith JSON | `1M; +15/-0` |
| Mutable-key JSONL | `1M; +1/-0` |
| Per-object JSON | `1A; +15/-0` |
| Range JSONL | `16M + 1A; +17/-16` |
| Hash JSONL | `1M; +1/-0` |

The range result is structural churn: one boundary record moved through each
of 16 existing shards and created a seventeenth. Fixed hash placement did not
move any existing record.

One-line entity JSONL deliberately trades intra-record pretty formatting for
stable record boundaries and low file count. A deeply nested formula edit is
therefore one removed and one added entity line. #44 should pair that useful
raw record-level diff with the existing stable-ID semantic diff for field- and
formula-level meaning.

### Canonicalization and determinism

The probe's tree digest length-frames each sorted UTF-8 relative path and exact
file body before SHA-256. It excludes modes because every probe artifact is a
regular non-executable file. This digest is an evidence oracle only and is not
stored in the proposed manifest.

- reversing source collection construction order produced identical tree
  digests for both fixture sizes and all five variants;
- the canonical 4,096-entity hash tree digest was
  `7b765bb9737e710eb2f2156b5a96bf5c87cc99de6e64b5d2c4b64fd86df407a1`;
- 17 non-canonical entity record files with wrong names/nesting, reverse record
  order, alternate whitespace, reverse schema order, and no canonical bucket
  files normalized to that exact digest;
- a duplicate stable entity ID across files was rejected; and
- a blank JSONL record was rejected.

The disposable normalizer does not claim production strict-JSON, unknown-file
or symlink rejection, resource, semantic-validation, or atomic-write coverage.
Those must use storage-owned v1 DTOs and the Accepted reader/validation
boundaries.

### Relative implementation complexity

| Variant | Materializer complexity | Long-term cost |
| --- | --- | --- |
| Monolith | Lowest: one encode | One growing file and conflict hotspot |
| Mutable-key split | Moderate: group by schema key | Rename/case/path safety and identity confusion are permanent costs |
| Per-object stable-ID files | Moderate-high: path codec plus O(records) filesystem operations | Excellent isolation, but file enumeration/watch/index overhead scales with every object |
| Range JSONL | Moderate: sort and chunk | Simple initial output, but insertion/deletion can cascade through all following shards |
| Fixed hash JSONL | Moderate: one standard hash, group, and stable sort per entity | Fixed 18-file tree, no index, no rebalance, bounded path vocabulary |

The hash option adds one deterministic placement function but removes adaptive
split state, range boundary metadata, path escaping, rename handling, and a
manifest index. It is the minimum sufficient complexity among the candidates
that satisfy all Issue #41 acceptance criteria.

## Rejected alternatives

### One monolithic pretty JSON file

Reject for v1 source materialization. It is simple and scalar diffs can be
small, but the complete entity set remains one ever-growing read/write and
merge-conflict surface. The 4,096-entity fixture already produced a 2.5 MiB
single file.

### Mutable-key or display-name paths

Reject categorically. A schema rename changed two paths and depended on Git's
rename heuristic; without rename detection it appeared as four path events.
Mutable names also introduce case-folding, reserved-name, separator, and
Unicode-normalization problems. Most importantly, they conflict with
ADR-0015.

### One stable-ID-named JSON file per object

Reject as the default. It localizes edits and can use a safe reversible path
codec, but created 4,101 files for 4,096 entities and turns ordinary scans,
watchers, checkout, and index work into O(entity count) filesystem operations.
Raw opaque IDs cannot safely be filenames, so the layout also needs a permanent
path codec. The benefit over one-line JSONL did not justify the cost.

### Sorted range/count shards

Reject. A beginning insertion touched every existing 256-record shard. Fixed
or content-sized ranges merely move the cascade threshold; adaptive split
metadata also creates additional canonical state and threshold churn.

### Raw stable-ID prefix shards

Reject. Stable IDs are opaque and may come from multiple generation families.
UUIDv7 places a Unix timestamp in its leading 48 bits, so leading-token buckets
would cluster contemporaneous records and accidentally give ID spelling
placement semantics.

### Adaptive, content-defined, or manifest-indexed shards

Reject for v1. Counts, byte thresholds, split points, per-shard digests, or file
lists create global updates or cascading rematerialization. They also require
more state and recovery rules than a fixed 16-way function. If 16 buckets prove
insufficient, a later representation version should change the placement
contract explicitly.

### Separate formula files

Reject under the current semantic model. A formula is the value of a stable
entity field, not an independently identified aggregate. A formula directory
would either duplicate its owner address or accidentally manufacture path
identity.

### 256 always-present hash buckets

Defer. It would preserve the good placement properties but impose 258 files on
even an empty or tiny project. The measured 16-bucket maximum was only 83,162
bytes at 4,096 entities, so v1 has no evidence for that permanent baseline.

## Decision-state recommendation

### Promote as the Issue #41 Accepted candidate

- the exact 18-file closed-world v1 tree;
- one pretty `manifest.json`, one pretty `schemas.json`, and 16 compact entity
  JSONL files;
- the exact first-nibble SHA-256 placement rule over decoded stable-ID UTF-8;
- all 16 lowercase ASCII shard files always present, including zero-byte empty
  shards;
- stable-ID unsigned-UTF-8 ordering within schema/field collections and within
  every shard;
- fixed DTO member order plus the existing canonical primitive rules;
- path/layout strictly excluded from semantic identity;
- supported non-canonical shard/order/whitespace input canonicalizable only
  after strict decode, uniqueness proof, semantic conversion, and validation;
- formulas inline with the owning field; and
- assets, views, tests, caches, and generated `.ro` outside the v1 canonical
  tree.

These are candidates until a maintainer accepts an ADR/spec amendment. Merging
this research record alone must not silently elevate them to authority.

### Keep Provisional or Deferred

- `.roproj` total/tree/file/line/string/member/count resource limits and exact
  error precedence;
- host atomic-save, locking, recovery, permissions, symlink-race, and watcher
  behavior;
- whether ordinary open accepts the supported non-canonical family or only an
  explicit `canonicalize`/import operation does;
- Git attributes, text-conversion drivers, CI commands, and generated `.ro`
  consistency mechanics (#44);
- machine delta and revision envelope details (#45);
- semantic merge/conflict projection and Git merge-driver behavior (#46);
- schemas beyond the evidenced low-cardinality scale;
- future canonical semantics and locations for assets, shared views, and
  semantic tests;
- package integrity, signatures, compression, and stored tree digests (#43);
  and
- a wider shard fanout in a later representation version.

## Downstream consequences

### #44 Git/CI integration

- CI can derive the complete expected path set without reading a manifest
  index and reject missing/extra paths or non-canonical bytes.
- Raw Git review sees one entity record per line and mutable renames at stable
  paths. The semantic diff remains authoritative for field/formula meaning.
- CI must reconstruct meaning from record IDs and content, never infer it from
  shard paths.
- A layout-only Git diff may have an empty semantic diff; canonicalization
  should then be the repair.
- If a generated `.ro` is tracked, compare its decoded semantic document with
  the `.roproj`, not its path, mtime, or manifest digest.
- Do not add `.gitattributes`, diff drivers, or Git commands in #41.

### #45 semantic delta

- Delta targets remain stable semantic IDs and field/element identities.
- Shard names, line numbers, filenames, record order, and placement hashes are
  forbidden delta targets.
- Canonical rematerialization or a future v1-to-v2 layout migration produces no
  semantic delta when meaning is unchanged.

### #46 three-way merge

- Authoritative merge consumes semantic `base`, `left`, and `right` states,
  then canonicalizes the result.
- Raw Git line/path conflicts are transport/review signals, not semantic
  conflict identity.
- Two independent entity edits in one hash shard may create a textual conflict
  in some Git operations while still being semantically mergeable; #46 must not
  inherit the shard as a conflict unit.
- No merge experiment or merge rule is added by this research.

## Required authority/spec changes after maintainer approval

1. Add ADR-0020 (or the next available ADR) to accept the version-scoped
   physical layout, placement function, path/nonidentity rule, category
   boundary, and migration consequence without reopening ADR-0015 through
   ADR-0019.
2. Replace the illustrative `roproj-layout-v1.md` tree with the exact tree,
   filename grammar, JSONL rules, placement algorithm, closed-world input
   rules, and canonicalizer contract.
3. Expand `roproj-format.md` into a complete `.roproj/v1` wire specification:
   exact manifest/schema/entity DTO records, tags, member order,
   required/optional/null/default behavior, strict failures, and normative
   golden bytes. Do not point at Rust structs as the schema.
4. Reconcile `ro-format-and-roproj-spec.md` and
   `architecture/ro-and-roproj-format.md` with the exact representation split
   and external category boundaries.
5. Extend `canonical-json-profile.md` with version-scoped JSONL rules, including
   compact records, zero-byte empty files, final LF, blank-line rejection, and
   tree/path canonicality. Keep the probe digest non-normative unless #43
   independently accepts one.
6. Extend `storage-versioning-and-migration.md` with the `.roproj` representation
   namespace, manifest-first dispatch, cross-file uniqueness/validation order,
   supported non-canonical input family, and explicit layout-version migration.
7. Update `git-native-workflow.md` only with the path/nonidentity and
   canonical-materialization consequences; leave operational Git integration
   to #44.
8. After those normative edits are accepted, update the specs/decisions
   indexes, knowledge-authority references, and canonical reconciliation
   register in the same change.

ADR-0015 through ADR-0019 need references to the new decision only if useful;
their identity, crate-boundary, storage, formula/Number, and diagnostics
decisions do not need semantic amendments.

## Non-blocking open questions

- Which finite `.roproj/v1` resource profile is safe for native and browser
  hosts? It must be measured independently and must not reuse direct-JSON
  limits by convenience.
- Should non-canonical layout admission be a normal read capability or an
  explicit repair/import command? Both can preserve meaning if neither rewrites
  implicitly.
- At what observed schema scale would a future version shard `schemas.json`?
- What atomic directory replacement and crash-recovery protocol should native
  and browser hosts use?
- Which asset, view, and test concepts eventually become semantic, shared
  adjunct, or purely local state?
- Does a future package/integrity profile need a normative canonical-tree
  digest, and if so, which path framing and metadata rules belong to #43?
- When real projects materially exceed this 4,096-entity fixture, is 16-way
  fanout still sufficient, or should `.roproj/v2` widen it?

None changes the v1 physical choice or Issue #41 identity boundary. Infinite
scale, host durability, package integrity, and future category semantics are
explicitly separate decisions.

## Reproduction

Recorded environment:

```text
macOS 15.7.4 (24G517), arm64
Node.js v24.15.0
Git 2.55.0
```

Run the disposable probe:

```bash
node --check docs/research/probes/issue-41-roproj-layout.mjs
node docs/research/probes/issue-41-roproj-layout.mjs \
  > /tmp/issue-41-report.json
```

To retain all materialized trees and temporary committed-repository inputs for
inspection, provide a new path that does not already exist:

```bash
node docs/research/probes/issue-41-roproj-layout.mjs \
  --keep-dir /tmp/issue-41-roproj-evidence > /tmp/issue-41-report.json
```

The probe refuses to overwrite an existing retained directory. Without
`--keep-dir`, it removes its temporary evidence tree after emitting one
machine-readable JSON report.

## External primary references

- Git diffcore explains that rename detection is a path-pairing transform and
  similarity heuristic, not semantic identity:
  <https://git-scm.com/docs/gitdiffcore>
- Git's `--find-renames` option documents the configurable similarity
  threshold:
  <https://git-scm.com/docs/git-diff#Documentation/git-diff.txt---find-renamesn>
- JSON Lines defines UTF-8, one valid JSON value per line, no blank lines, and
  the conventional final LF:
  <https://jsonlines.org/>
- RFC 9562 defines UUIDv7's leading 48-bit Unix timestamp, motivating a hash
  rather than raw-prefix placement function:
  <https://www.rfc-editor.org/rfc/rfc9562.html#name-uuid-version-7>
- FIPS 180-4 defines SHA-256 used only as the v1 placement function:
  <https://csrc.nist.gov/pubs/fips/180-4/upd1/final>
- Windows filename and case rules motivate the fixed lowercase ASCII internal
  vocabulary:
  <https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file>
