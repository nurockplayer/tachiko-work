# Tachiko Canonical JSON Profile

Decision state: Mixed — Accepted deterministic/semantic-preservation rules under ADR-0017; exact Milestone 02 textual mechanics are version-specific and Provisional outside a version that adopts them.

Implementation state: Not yet implemented as an independent canonical writer; v0.1 currently uses `serde_json::to_string_pretty` plus one trailing LF.

Owner: #38

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
- #24 still owns numeric meaning and must not be pre-decided by the serializer.

JCS remains a useful primitive reference for deterministic string escaping, valid Unicode handling, duplicate-free input discipline, and no implicit Unicode normalization.

## Reader versus writer contract

Canonicality is a writer contract, not a requirement that every valid supported input already use canonical whitespace/member order/escape spelling.

A supported-version reader may accept semantically equivalent non-canonical JSON spelling where the version specification permits it, then an explicit canonical encode produces the unique canonical bytes for that version.

This does not weaken strict rules for duplicate members, unknown fields, version dispatch, or semantic validity.

## Encoding

For a version adopting the Milestone 02 textual profile:

- encoding is UTF-8;
- no BOM is emitted;
- structural line endings use LF (`U+000A`, byte `0x0A`);
- CRLF is never emitted by the canonical writer;
- indentation is two ASCII spaces per level;
- no trailing spaces or tabs are emitted;
- output ends with exactly one LF.

These whitespace choices are version/profile mechanics, not semantic invariants across all future Tachiko formats.

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

Canonical storage MUST NOT silently round, coerce, normalize, or invent numeric equivalence.

Numeric representation must follow the Accepted semantic numeric contract owned by #24.

### Deferred exact spelling

The exact canonical spelling/representation for semantic numeric values is Deferred until #24 resolves the relevant meaning.

Current `f64` implementation behavior is evidence, not the durable answer for future representation versions.

If #24 ultimately accepts IEEE-754 binary64 as the durable semantic number model for a representation, RFC 8785 number serialization and its published vectors should be the default candidate rather than inventing a new float-spelling algorithm.

If #24 instead accepts exact integers, decimals, tagged numeric kinds, or another model, the representation must encode that accepted meaning without silent cross-language precision loss.

Legacy direct `.ro` v1 numeric bytes remain part of that historical compatibility profile and are not retroactively rewritten by this rule.

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

## Dependency-library rule

Golden bytes and normative specifications are authority. `serde_json` or another library is an implementation dependency.

A dependency update that changes canonical output bytes must fail conformance tests until the project determines whether the change is:

- an implementation bug;
- a serializer replacement detail that must preserve bytes;
- or an intentional new representation version.

Library behavior must never silently redefine the public format.

## Required #40 fixtures

At minimum:

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
- encode → decode → encode byte stability.

Numeric edge vectors remain Deferred to #24.

## Related

- ADR-0017
- ADR-0015
- #38, #40
- RFC 8259
- RFC 7493
- RFC 8785
