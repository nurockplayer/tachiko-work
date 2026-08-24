# Tachiko Canonical JSON Profile

Decision state: Mixed — deterministic/semantic-preservation rules under ADR-0017 and the finite-binary64 numeric primitive under ADR-0018 are Accepted. The `.roproj/v1` rules adopted below are Accepted for that representation; direct-JSON textual/resource-limit mechanics remain version-specific and Provisional outside a version that adopts them.

Implementation state: Implemented by the dedicated `direct-ro/v2` canonical
writer. Frozen v1 retains its historical version-scoped writer.

Authority: ADR-0017

Accepted numeric authority: ADR-0018; decision record: #24

Implementation and conformance: #74, #40. `.roproj/v1` authority: [ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md), [roproj-layout-v1.md](roproj-layout-v1.md), and [roproj-format.md](roproj-format.md).

## Purpose

Define deterministic JSON emission suitable for Tachiko Work's Git-readable persisted representations without treating a particular Rust serializer as the public contract and without adopting RFC 8785 JCS wholesale where it conflicts with Tachiko requirements.

This profile is a Tachiko-owned deterministic JSON profile. It selectively reuses mature I-JSON/JCS primitive rules, but it MUST NOT be advertised as RFC 8785/JCS compliant.

## Why not full JCS

Full RFC 8785 JCS is optimized for a single compact canonical machine representation. Its requirements include:

- no structural whitespace between JSON tokens;
- recursive object-property sorting by UTF-16 code units;
- ECMAScript/IEEE-754 binary64 number serialization.

Those choices do not fully match Tachiko Work's accepted requirements:

- `.roproj` must remain Git-readable and human-inspectable where practical;
- canonical semantic collection order is driven by stable semantic identity rather than universal property-name sorting;
- numeric meaning comes from ADR-0018 rather than the serializer.

JCS remains a useful primitive reference for deterministic string escaping, valid Unicode handling, duplicate-free input discipline, and no implicit Unicode normalization.

## Reader versus writer contract

Canonicality is a writer contract, not a requirement that every valid supported input already use canonical whitespace/member order/escape spelling.

A supported-version reader may accept semantically equivalent non-canonical JSON spelling where the version specification permits it, then an explicit canonical encode produces the unique canonical bytes for that version.

This does not weaken strict rules for duplicate members, unknown fields, version dispatch, or semantic validity.

The current normal direct-JSON profile applies its shared 8 MiB complete-input
admission before UTF-8 validation or any JSON scan for both legacy v1 and v2.
An over-limit failure is a structural representation failure and precedes all
latent format/version failures. For admitted v2 input, the separate 256-byte
number-token admission remains subordinate and occurs before ADR-0018
exact-decimal conversion.

These concrete limits are representation/profile mechanics owned by the
adopting representation and #74/#96. They are not semantic Number or product
limits. The normal-profile rejection of legacy direct-`.ro/v1` above 8 MiB is
an intentional admission tightening; it does not change the v1 wire meaning.
A future explicit legacy import/migration operation may define another finite
bounded profile, but no unbounded reader bypass is permitted. This is a
`legacy-direct-ro/v1`/`direct-ro/v2` admission rule only. `.roproj/v1` MUST
NOT inherit its 8 MiB complete-input or v2 256-byte number-token limits; a
`.roproj` limit requires its own version-scoped decision.

## Encoding

For a direct-JSON version adopting the Milestone 02 textual profile:

- encoding is UTF-8;
- no BOM is emitted;
- structural line endings use LF (`U+000A`, byte `0x0A`);
- CRLF is never emitted by the canonical writer;
- indentation is two ASCII spaces per level;
- no trailing spaces or tabs are emitted;
- output ends with exactly one LF.

These whitespace choices are version/profile mechanics, not semantic invariants across all future Tachiko formats. `.roproj/v1` adopts the specific JSON and JSONL rules below instead of treating a whole directory as one direct-JSON byte stream.

## Accepted `.roproj/v1` JSON, JSONL, and tree profile

`.roproj/v1` is an editable directory representation, not a direct `.ro` JSON
file. It has its own version-owned DTOs and canonical materialization. Its
complete specification is constrained by ADR-0017, ADR-0018, ADR-0019,
[ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md),
[roproj-layout-v1.md](roproj-layout-v1.md), and [roproj-format.md](roproj-format.md);
it does not adopt Rust/Serde declaration order, direct-JSON admission limits,
or a future `.ro` package profile.

### Exact canonical tree

Every canonical `.roproj/v1` materialization contains exactly these eighteen
regular UTF-8 text files, relative to the `.roproj` root, and no other path:

```text
manifest.json
schemas.json
entities/0.jsonl
entities/1.jsonl
entities/2.jsonl
entities/3.jsonl
entities/4.jsonl
entities/5.jsonl
entities/6.jsonl
entities/7.jsonl
entities/8.jsonl
entities/9.jsonl
entities/a.jsonl
entities/b.jsonl
entities/c.jsonl
entities/d.jsonl
entities/e.jsonl
entities/f.jsonl
```

The root basename and host path are discovery/container details, not semantic
identity. The tree does not include a tree digest, file digest, inventory,
timestamp, tool version, absolute path, generated `.ro` artifact, or another
derived summary. Such data cannot be introduced as an unversioned extension.

`manifest.json` is the only version envelope. It is a closed-world,
version-owned DTO whose canonical outer member order is `format`,
`format_version`, `document`; its v1 values identify `tachiko.roproj`, version
`1`, and the version-owned document DTO. `schemas.json` is a closed-world
version-owned array of schema records. Schema records are ordered by stable
`SchemaId`; each schema's field definitions are ordered by stable `FieldId`.
The detailed record members, tags, required/optional/null rules, and member
orders remain those explicitly owned by the `.roproj/v1` DTO specification,
not by `direct-ro/v2` or semantic-core types.

### JSON file and JSONL bytes

`manifest.json` and `schemas.json` are pretty canonical JSON: UTF-8 without a
BOM, LF line endings, two ASCII spaces per nesting level, no trailing spaces
or tabs, and exactly one final LF. They use this profile's deterministic
escaping, duplicate-member rejection, Unicode preservation, and Number rules.

An entity record is one compact canonical JSON object on exactly one JSONL
line. A JSONL record has no leading/trailing whitespace and has no whitespace
between its JSON tokens. The only separator between records is one LF byte;
blank lines and every other inter-record whitespace are invalid, including a
second trailing LF. A nonempty shard therefore ends in exactly one LF, while a
canonical empty shard is exactly zero bytes. A line is not an entity-record
boundary if it contains only JSON whitespace.

Each record's member order is the `.roproj/v1` DTO's declared order. It carries
the opaque stable `EntityId`, mutable key, stable schema target, and field
values under the version-owned DTO rules; typed references and bound-formula
references use stable IDs. Unknown members are rejected recursively and JSON
member-name duplicates are rejected after escape decoding.

### Entity placement and ordering

For each entity, canonical materialization takes the decoded persisted
`EntityId` scalar sequence without Unicode normalization, encodes it as UTF-8,
computes SHA-256, and uses the high four bits as the lowercase hexadecimal
shard filename. Records in a shard are ordered by their complete stable ID as
unsigned UTF-8 bytes. The placement hash is a v1 layout function, not a
semantic identity, content identity, integrity claim, or tree digest.

Stable-ID ordering also applies to the unordered schema and field collections
described above and to every other unordered `.roproj/v1` semantic collection.
Semantic sequences retain their declared order. Canonical order never derives
from a mutable key, path, filesystem enumeration, locale, insertion order, or
hash iteration.

### Canonical tree equality and rematerialization

For the same valid `.roproj/v1` semantic document, construction order and an
accepted non-canonical spelling cannot affect its materialization:

```text
canonical_paths(document_a) == canonical_paths(document_b)
canonical_bytes(document_a) == canonical_bytes(document_b)
```

Canonical comparison is exact relative-path equality plus exact file-byte
equality; no digest is substituted for either comparison. Conversely, paths
are not semantic identity: a record placed in a wrong shard or non-canonical
path neither changes nor creates its identity. A layout-only input change has
an empty semantic diff, and canonical rematerialization restores the one v1
tree above.

## JSON object members

### Duplicate prohibition

Duplicate object member names are invalid at every depth.

Duplicate comparison occurs after JSON escape decoding. Raw spellings that decode to the same member name are duplicates.

### Fixed record member order

Each version-specific DTO record defines a normative member order in its own specification.

Canonical emission follows that spec-defined order.

Rust declaration order, `serde` derive order, hash-map iteration, and library implementation behavior are not normative unless the version spec explicitly adopts the same ordering.

### Unknown fields

Unknown fields in a recognized closed-world version are rejected unless the version explicitly defines an opaque extension area with preservation and canonicalization rules.

## Collections

### Unordered semantic collections

An unordered semantic collection is canonically ordered by stable semantic identity or another explicitly Accepted stable semantic key.

For opaque textual IDs in the Milestone 02 JSON profile, canonical comparison uses the canonical persisted ID token encoded as UTF-8 and compares bytes lexicographically as unsigned byte sequences.

Canonical order MUST NOT be derived from:

- mutable human key/name/label;
- storage path;
- UI coordinates;
- insertion order;
- hash iteration;
- filesystem enumeration;
- locale;
- wall clock;
- thread completion order.

### Ordered semantic sequences

Arrays/sequences whose order is semantic preserve that semantic order exactly. The canonical writer does not sort them merely to create stable bytes.

## Stable IDs

In the Milestone 02 JSON profile, stable semantic identities are represented as opaque JSON strings.

The storage layer does not require those strings to be UUIDv7 and must not infer timestamp/business meaning from their spelling.

Normal creation and legacy migration may produce different ID-generation families while the semantic/storage boundary continues to treat the token as opaque identity.

## Strings

### Decoded value preservation

The decoded Unicode scalar sequence is the semantic string value from storage's perspective.

The canonical writer MUST NOT apply NFC, NFD, NFKC, NFKD, case folding, confusable folding, or another normalization transform.

For example, these remain distinct decoded sequences unless another semantic contract says otherwise:

```text
U+00E9
U+0065 U+0301
```

### Deterministic escaping

For a version adopting the Milestone 02 profile, string escaping follows RFC 8785 §3.2.2.2 / ECMAScript-compatible deterministic spelling where applicable:

- quote and reverse solidus are escaped;
- required control characters use deterministic JSON escape spelling;
- otherwise valid Unicode characters are preserved rather than gratuitously escaped;
- invalid Unicode/lone-surrogate input is rejected rather than emitted.

Alternative legal input escapes may be accepted by the reader; canonical re-encoding emits the profile's unique spelling.

## Required, optional, default, and null fields

Each version-specific DTO defines whether a field is:

- required and always emitted;
- optional and omitted when absent;
- nullable and explicitly emitted as `null`;
- defaulted with an explicit canonical presence/omission rule.

There is no blanket `skip_serializing_if`, Serde default, or implementation-library omission policy in the durable contract.

## Numbers

### Accepted boundary

The writer MUST NOT round or otherwise change an already-semantic Number. The
reader's exact-decimal-to-Number conversion is specified below rather than left
to a parser/library accident.

Storage may implement only the semantic numeric contract Accepted by ADR-0018.

### Accepted Milestone 02 binary64 primitive

For a representation version that adopts ADR-0018, semantic
`Number` is finite IEEE 754 binary64 with both IEEE zero encodings normalized to
semantic positive zero. NaN and positive/negative infinity are invalid and MUST
fail before canonical emission.

The canonical JSON token is the RFC 8785 §3.2.2.3 / ECMAScript
`Number::toString` radix-10, Note-2-enhanced shortest-roundtrip spelling. This
selects only the number primitive; Tachiko keeps the whitespace, member-order,
collection-order, and Unicode rules in this profile and MUST NOT claim full JCS
conformance.

The emitted token must parse through correctly rounded binary64 conversion to
the same normalized value. Semantic zero emits exactly `0`. Initial required
vectors include:

| Binary64 bits | Canonical token |
| --- | --- |
| `0000000000000000` | `0` |
| `8000000000000000` | `0` |
| `0000000000000001` | `5e-324` |
| `8000000000000001` | `-5e-324` |
| `7fefffffffffffff` | `1.7976931348623157e+308` |
| `ffefffffffffffff` | `-1.7976931348623157e+308` |
| `444b1ae4d6e2ef50` | `1e+21` |
| `3eb0c6f7a0b5ed8d` | `0.000001` |

### Accepted reader resource admission and conversion

Before representation-version selection, the normal direct-JSON profile first
applies its shared complete-input limit. After an admitted input selects v2,
the reader applies the v2-specific number-token limit. A token exceeding that
declared limit is rejected as a structural representation-limit failure before
exact-decimal conversion. It is not classified as overflow, underflow,
non-finite conversion, or another Number semantic failure. The concrete values
remain replaceable profile mechanisms rather than semantic invariants.

For every syntactically valid RFC 8259 JSON number token admitted by those
limits, the reader interprets the token as an exact mathematical decimal and
converts it to the nearest binary64 value using `roundTiesToEven`.

- conversion to positive or negative infinity is invalid representation data;
- finite normal and subnormal results are accepted;
- correctly rounded underflow to either zero sign is accepted and normalized to
  semantic positive zero;
- `-0` and alternate valid zero spellings normalize to semantic positive zero;
  and
- lexical distinctions are not preserved after conversion, so every accepted
  token re-encodes through the one canonical writer spelling.

Representative decode/re-encode results are:

| Input token | Semantic result | Canonical token |
| --- | --- | --- |
| `1`, `1.0`, `1e0` | bits `3ff0000000000000` | `1` |
| `9007199254740993` | bits `4340000000000000` | `9007199254740992` |
| `5e-324` | bits `0000000000000001` | `5e-324` |
| `1e-4000` | semantic positive zero | `0` |
| `-0`, `-0.0`, `-1e-4000` | semantic positive zero | `0` |
| `1e400`, `-1e400` | invalid: conversion would be infinite | none |

`NaN`, `Infinity`, and `-Infinity` are not RFC 8259 number tokens and are
rejected as invalid JSON rather than converted.

Current `f64`, Rust display, Serde, or another formatter's output is evidence,
not authority. In particular, the current `serde_json` version emits `1e-6`,
which does not match the Accepted ECMAScript token `0.000001`.

Exact integer, decimal, fixed-point, money, or tagged numeric kinds remain
Deferred. A future representation must encode any such Accepted meaning without
silent conversion through binary64.

Legacy direct `.ro` v1 numeric bytes remain part of that historical
compatibility profile and are not retroactively rewritten by this rule. Merely
opening or decoding v1 does not opt it into ADR-0018. Current semantic operations
require an explicit version-labelled migration candidate; creating that
candidate in memory does not authorize rewriting durable source. A retained
legacy evaluator remains version-scoped and non-conformant to ADR-0018.

## Canonical round-trip properties

For a supported version and canonical state:

```text
encode(decode(canonical_bytes)) == canonical_bytes
```

For semantically equivalent supported inputs differing only in non-semantic construction/insertion order or accepted non-canonical JSON spelling:

```text
canonical_encode(input_a) == canonical_encode(input_b)
```

This property is scoped to the same representation version. A future version may define different canonical bytes for the same semantic meaning.

For `.roproj/v1`, `canonical_bytes` means the exact file bodies associated with
the exact canonical relative paths above. It is not a hash or a promise that
another representation version has the same tree.

## Dependency-library rule

Golden bytes and normative specifications are authority. `serde_json` or another library is an implementation dependency.

A dependency update that changes canonical output bytes must fail conformance tests until the project determines whether the change is:

- an implementation bug;
- a serializer replacement detail that must preserve bytes;
- or an intentional new representation version.

Library behavior must never silently redefine the public format.

## Required conformance fixtures

The direct-JSON conformance work in #40 includes at minimum:

- top-level duplicate member;
- nested duplicate member;
- escaped-equivalent duplicate member;
- recursive unknown field in supported version;
- reverse/insertion-order equivalence;
- stable-ID collection ordering;
- ordered sequence preservation;
- NFC/NFD-equivalent-but-distinct string pair preservation;
- alternative valid JSON escape spellings re-encode identically;
- CRLF/non-canonical whitespace input re-encodes to canonical LF output where accepted;
- no BOM;
- exactly one final LF;
- RFC 8785 Appendix B finite binary64 vectors, including exponent-threshold
  neighbors and round-trip-sensitive mantissas;
- both IEEE zero encodings normalize and emit as `0`;
- NaN and positive/negative infinity fail before emission;
- `1`, `1.0`, and `1e0` decode identically and re-encode as `1`;
- `9007199254740993` rounds ties-to-even and re-encodes as
  `9007199254740992`;
- finite subnormal, underflow-to-zero, and overflow-to-infinity decode vectors;
- exact declared complete-input and number-token resource boundaries, plus a
  one-byte-over case for each; admitted tokens proceed to syntax/semantic
  conversion, while over-limit inputs fail structurally before Number
  conversion;
- oversized direct-JSON inputs with latent UTF-8, JSON, duplicate-member, or
  version errors proving the shared Stage-0 resource precedence, plus
  under-limit equivalents proving the existing strict order;
- native and Wasm storage readers/writers agree on normalized binary64 values
  and exact canonical bytes for the adopted edge corpus where both targets are
  supported;
- encode → decode → encode byte stability.

A future production `.roproj/v1` codec must add representation-owned fixtures
for:

- `.roproj/v1` exact eighteen-path canonical tree, including every empty
  zero-byte shard and every nonempty shard's one final LF;
- `.roproj/v1` compact one-object JSONL records, rejection of blank lines and
  inter-record whitespace, stable-ID shard placement/order, and duplicate
  entity IDs across shards;
- `.roproj/v1` exact path-and-byte equality and rematerialization after a
  layout-only non-canonical input change, without a stored or comparison tree
  digest.

## Related

- ADR-0017
- ADR-0018 (Accepted)
- ADR-0019 (Accepted; decision issue #23)
- [ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md)
- ADR-0015
- [roproj-layout-v1.md](roproj-layout-v1.md)
- [roproj-format.md](roproj-format.md)
- #24, #38, #40, #41, #74
- RFC 8259
- RFC 7493
- RFC 8785
