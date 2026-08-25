# Tachiko Work portable package v1

Decision state: Accepted

Implementation state: Not implemented by a production `.roproj` codec,
packaged-`.ro` codec, or CLI command

Package profile: `tachiko.portable-package/v1`

Payload profile: `tachiko.roproj/v1`

Authority: [ADR-0025](../decisions/ADR-0025-portable-package-v1.md),
constrained by ADR-0003, ADR-0015, ADR-0017, and ADR-0023

Payload authority: [`.roproj/v1` wire DTOs](roproj-format.md) and
[physical layout](roproj-layout-v1.md)

Evidence: [portable-package v1 research record](../research/2026-08-26-portable-package-v1.md),
[executable probe](../research/probes/issue-43-portable-package-v1.mjs), and
[byte-level golden vector](../research/fixtures/issue-43-portable-package-v1/empty-package-v1.hex)

Tracking issue: [#3](https://github.com/nurockplayer/tachiko-work/issues/3)
for production codecs and CLI implementation

## Purpose and normative language

This specification defines the complete portable-package v1 byte profile,
payload integrity root, pack/unpack behavior, exact-byte round-trip laws,
package/tracked-source comparison, and stable failure meanings.

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

This document does not define a production codec or command. The exact
container and hash logic in the disposable evidence probe demonstrates the
contract independently of a ZIP library but is not a product API.

## Representation role and version ownership

The semantic `Document` remains meaning authority. `.roproj/v1` is the
canonical editable and Git-native materialization. Portable package v1 is a
derived single-file envelope over the exact canonical `.roproj/v1` paths and
bytes.

Portable package v1 MUST NOT introduce or persist a second semantic schema.
In particular, it defines no package-owned semantic document, schema, field,
entity, value, formula, reference, identity, revision, operation, or
authorization DTO.

The version namespace `tachiko.portable-package/v1` is independent from:

- `legacy-direct-ro/v1`;
- `direct-ro/v2`;
- `tachiko.roproj/v1`;
- a semantic-model version;
- a Tachiko Work release version; and
- any future signing, trust, extension, transport, or distribution profile.

Package v1 supports exactly `payload_format: "tachiko.roproj"` with
`payload_format_version: 1`. Supporting another payload representation or
version requires another package-profile version. An incompatible change to
any byte rule below also requires another package-profile version.

The `.ro` extension is Provisional. It is a current artifact naming convention,
not protocol identity and not sufficient input classification.

## Exact entry vocabulary and order

A package v1 archive contains exactly these 19 regular-file entries in the
shown local-header and central-directory order:

| Index | Exact entry name | Entry bytes |
| ---: | --- | --- |
| 1 | `package.json` | Canonical package manifest defined below |
| 2 | `payload/manifest.json` | Exact `.roproj/v1` `manifest.json` bytes |
| 3 | `payload/schemas.json` | Exact `.roproj/v1` `schemas.json` bytes |
| 4 | `payload/entities/0.jsonl` | Exact shard bytes, including zero bytes |
| 5 | `payload/entities/1.jsonl` | Exact shard bytes, including zero bytes |
| 6 | `payload/entities/2.jsonl` | Exact shard bytes, including zero bytes |
| 7 | `payload/entities/3.jsonl` | Exact shard bytes, including zero bytes |
| 8 | `payload/entities/4.jsonl` | Exact shard bytes, including zero bytes |
| 9 | `payload/entities/5.jsonl` | Exact shard bytes, including zero bytes |
| 10 | `payload/entities/6.jsonl` | Exact shard bytes, including zero bytes |
| 11 | `payload/entities/7.jsonl` | Exact shard bytes, including zero bytes |
| 12 | `payload/entities/8.jsonl` | Exact shard bytes, including zero bytes |
| 13 | `payload/entities/9.jsonl` | Exact shard bytes, including zero bytes |
| 14 | `payload/entities/a.jsonl` | Exact shard bytes, including zero bytes |
| 15 | `payload/entities/b.jsonl` | Exact shard bytes, including zero bytes |
| 16 | `payload/entities/c.jsonl` | Exact shard bytes, including zero bytes |
| 17 | `payload/entities/d.jsonl` | Exact shard bytes, including zero bytes |
| 18 | `payload/entities/e.jsonl` | Exact shard bytes, including zero bytes |
| 19 | `payload/entities/f.jsonl` | Exact shard bytes, including zero bytes |

Entry names are exactly the listed ASCII bytes, which are also their UTF-8
encoding. Names are case-sensitive and use `/` as the only separator. There
are no explicit `payload/` or `payload/entities/` directory entries.

The following are invalid rather than equivalent:

- a missing, unknown, extra, or duplicate entry;
- `./`, `../`, an absolute path, a drive prefix, or an empty path component;
- `\` as a separator;
- percent encoding or another escaped alias;
- alternate Unicode or case spelling;
- an explicit directory entry; or
- the right names in another order.

The `payload/` prefix exists only inside the package. Unpack strips exactly
that prefix from entries 2 through 19. It neither persists the prefix nor uses
it as semantic identity.

## `package.json`

### Complete DTO

`package.json` is a closed-world packaging-only object. Its complete member
set and order are:

1. `format`
2. `format_version`
3. `payload_format`
4. `payload_format_version`
5. `payload_root_sha256`

Its logical shape is:

```json
{
  "format": "tachiko.portable-package",
  "format_version": 1,
  "payload_format": "tachiko.roproj",
  "payload_format_version": 1,
  "payload_root_sha256": "<64 lowercase hexadecimal digits>"
}
```

Every member is required, non-null, and always emitted. Unknown or duplicate
members are invalid. There is no extension object.

- `format` MUST be the exact JSON string `"tachiko.portable-package"`.
- `format_version` MUST be lexical JSON integer `1`. String, fraction,
  exponent, zero, negative, and missing forms are malformed.
- `payload_format` MUST be exact string `"tachiko.roproj"`.
- `payload_format_version` MUST be lexical JSON integer `1`.
- `payload_root_sha256` MUST be exactly 64 lowercase ASCII hexadecimal digits
  containing the integrity root defined below.

### Canonical bytes

The file is UTF-8 without BOM, uses the exact two-space indentation and member
spelling above, has no trailing spaces or blank lines, and ends in exactly one
LF byte. Because all fixed strings and the hexadecimal root are ASCII,
canonical `package.json` is always exactly 228 bytes.

For root
`71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c`,
the exact file is:

```json
{
  "format": "tachiko.portable-package",
  "format_version": 1,
  "payload_format": "tachiko.roproj",
  "payload_format_version": 1,
  "payload_root_sha256": "71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c"
}
```

The LF immediately before the closing fence is part of the file. No other
whitespace or JSON spelling is canonical.

`package.json` MUST NOT contain semantic document metadata, a copy of the
inner document ID/title, timestamps, tool or application versions, source
paths, source-root basename, Git metadata, entry inventories, individual file
hashes, arbitrary extensions, signatures, keys, provenance, trust, or
authorization data.

## Payload integrity root

### Leaf construction

Let `p` be one exact canonical `.roproj/v1` relative path without the package
prefix, and `b` its exact file bytes. In the fixed path order below:

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

compute:

```text
leaf(p, b) = SHA-256(UTF8(p) || 0x00 || b)
```

`UTF8(p)` is exactly the ASCII/UTF-8 byte sequence shown. No length prefix,
final separator, normalization, case folding, `./`, root basename, or
`payload/` prefix is included. Each SHA-256 leaf is its raw 32 digest bytes,
not hexadecimal text.

Every path contributes a leaf. In particular, each of the 16 required
zero-byte shards has a distinct leaf because its path precedes the NUL.

### Root construction

Define these exact bytes:

```text
domain =
    ASCII("tachiko.portable-package/v1")
    || 0x00
    || ASCII("tachiko.roproj/v1")
    || 0x00
```

Then:

```text
payload_root = SHA-256(domain || leaf_1 || ... || leaf_18)
```

The root stored in `package.json` is the lowercase hexadecimal encoding of
those 32 bytes.

This algorithm is unrelated to ADR-0023's SHA-256 entity-placement function.
The placement function hashes one decoded `EntityId`; the package root hashes
canonical paths and exact file bytes under a distinct domain.

### Claim boundary

The payload root covers exactly the 18 canonical payload paths and bodies. It
does not cover:

- `package.json`;
- `payload/`;
- ZIP headers, names as repeated in ZIP records, offsets, CRCs, or sizes;
- the artifact filename;
- the source-root basename or host path;
- mtimes, permissions, ownership, ACLs, extended attributes, or symlinks;
- Git history or commit identity; or
- decoded semantic objects.

The root detects corruption and establishes exact payload equality. It is not
an authentication code, signature, author identity, trust decision,
authorization grant, confidentiality mechanism, freshness or rollback proof,
malicious coordinated-tamper defense, semantic revision identity,
`DocumentId`, content-addressed object ID, or Git commit ID. An actor able to
change the payload can also recompute an unauthenticated root and CRCs.

## Canonical ZIP32 container

Package v1 adopts the ordinary local header, stored data, central directory
header, and end-of-central-directory structures from PKWARE APPNOTE 6.3.10.
Every multibyte integer below is unsigned little-endian.

### Overall record sequence

The complete archive is:

```text
local header 1 || stored data 1
...
local header 19 || stored data 19
central header 1
...
central header 19
end of central directory
```

There are no bytes before local header 1, between a local header/name and its
stored data, between the last stored data and central header 1, between
central records, between the last central record and the end record, or after
the end record except bytes required by the records themselves.

There is exactly one ordinary end record. Package v1 has no encryption header,
data descriptor, archive decryption header, archive extra-data record, digital
signature record, ZIP64 end record/locator, split marker, padding, prepended
stub, or trailing data.

### Local file header fields

Each local header uses these exact values:

| Field | Width | Canonical value |
| --- | ---: | --- |
| Local signature | 4 | `0x04034b50` |
| Version needed | 2 | `0x000a` (ZIP 1.0) |
| General-purpose flags | 2 | `0x0800` (UTF-8/EFS only) |
| Compression method | 2 | `0x0000` (stored) |
| Last-modification time | 2 | `0x0000` |
| Last-modification date | 2 | `0x0021` (1980-01-01) |
| CRC-32 | 4 | Exact CRC-32 of stored entry bytes |
| Compressed size | 4 | Exact entry byte length |
| Uncompressed size | 4 | Same exact entry byte length |
| File-name length | 2 | Exact listed ASCII/UTF-8 name length |
| Extra-field length | 2 | `0x0000` |
| File name | variable | Exact listed name bytes |
| Extra field | variable | Absent |

Because bit 3 is clear, CRC and sizes are present in the local header and no
data descriptor follows the entry. Because method 0 is used, stored data is
the exact entry body without transformation. A zero-byte shard has size zero,
the standard CRC-32 of the empty byte string (`0x00000000`), and no data bytes.

### Central directory header fields

Each central header uses these exact values:

| Field | Width | Canonical value |
| --- | ---: | --- |
| Central signature | 4 | `0x02014b50` |
| Version made by | 2 | `0x0014` (ZIP 2.0, host 0 MS-DOS/FAT) |
| Version needed | 2 | `0x000a` |
| General-purpose flags | 2 | `0x0800` |
| Compression method | 2 | `0x0000` |
| Last-modification time | 2 | `0x0000` |
| Last-modification date | 2 | `0x0021` |
| CRC-32 | 4 | Same exact value as local header |
| Compressed size | 4 | Same exact value as local header |
| Uncompressed size | 4 | Same exact value as local header |
| File-name length | 2 | Exact name length |
| Extra-field length | 2 | `0x0000` |
| File-comment length | 2 | `0x0000` |
| Disk number start | 2 | `0x0000` |
| Internal attributes | 2 | `0x0000` |
| External attributes | 4 | `0x00000000` |
| Relative local-header offset | 4 | Exact byte offset from archive start |
| File name | variable | Same exact bytes as local header |
| Extra field | variable | Absent |
| File comment | variable | Absent |

For each index, local and central records MUST agree on the exact name, flags,
method, timestamp, CRC, sizes, and local-header location. The central directory
uses the same canonical entry order as local records.

### End-of-central-directory fields

The one end record is exactly 22 bytes:

| Field | Width | Canonical value |
| --- | ---: | --- |
| End signature | 4 | `0x06054b50` |
| Number of this disk | 2 | `0x0000` |
| Disk with central start | 2 | `0x0000` |
| Entries on this disk | 2 | `0x0013` (19) |
| Total entries | 2 | `0x0013` (19) |
| Central-directory size | 4 | Exact byte length |
| Central-directory offset | 4 | Exact byte offset |
| Archive-comment length | 2 | `0x0000` |
| Archive comment | variable | Absent |

The central-directory offset points to central header 1. Its size consumes
exactly central headers 1 through 19. The end record follows immediately and
is the final archive byte sequence.

### CRC-32

CRC-32 is the standard ZIP CRC using the reflected polynomial
`0xedb88320`, an initial register of `0xffffffff`, and a final one's
complement. It is calculated over exact uncompressed entry bytes. Because
entries are stored, compressed and uncompressed input bytes are identical.

CRC-32 is an entry-level accidental-corruption check, not authentication or
the payload equality root. A reader verifies both local/central CRC agreement
and recomputation from entry data.

### ZIP32 capacity and forbidden variation

No count, size, or offset field may use `0xffff` or `0xffffffff` as a ZIP64
sentinel. Each entry body length, each local-header offset, the central offset,
the central size, and the complete archive length MUST be at most
`0xfffffffe`. Package v1 has a fixed entry count of 19, already below the
ordinary 16-bit sentinel.

If canonical source bytes cannot satisfy every ordinary ZIP32 field and the
complete-length rule, pack fails with `portable_package.capacity_exceeded`.
It MUST NOT switch to ZIP64, compression, splitting, another container, or a
different entry profile.

The following otherwise-valid ZIP variations are noncanonical package v1 and
MUST be rejected:

- any compression method or compression-level output;
- general-purpose flags other than exactly `0x0800`;
- encryption;
- a data descriptor;
- another version-needed or version-made-by value;
- another timestamp;
- host/file attributes;
- local or central extra fields;
- file, archive, or digital-signature comments;
- directory entries;
- ZIP64 or split/spanned records;
- reordered central records;
- a local/central disagreement;
- an archive extra-data or digital-signature record;
- a prepended executable/self-extracting stub; or
- padding or trailing bytes.

Library defaults are never authority. A dependency update that changes any
canonical byte is an implementation failure unless a later package profile is
accepted.

## Pack contract

Pack accepts a source root only when all of these are true before publication:

1. The root is an ordinary directory supplied explicitly by the caller.
2. It has exactly the canonical `.roproj/v1` tree and ordinary regular-file
   types defined by ADR-0023.
3. `manifest.json` selects supported exact `.roproj/v1` before other payload
   semantic decoding.
4. Every JSON/JSONL body, ordering rule, shard placement, and zero-byte shard
   is already canonical under the two `.roproj/v1` specifications.
5. Version-owned DTO conversion and the applicable Accepted semantic
   validation gate succeed.
6. The resulting exact bytes fit package v1 ZIP32 capacity.
7. The destination is absent.

Pack MUST NOT invoke the bounded `.roproj/v1` canonicalizer, repair placement,
sort records, rewrite JSON, migrate another representation, follow symlinks,
drop unknown files, or infer semantic identity from paths. A caller may run an
explicit canonicalize operation first; it is a separate operation and output.

Pack processes payload files in the fixed path order, calculates every leaf
and the root over exact source bytes, creates canonical `package.json`, and
writes the fixed ZIP profile. It copies all 18 payload files byte-for-byte,
including all zero-byte shards.

The source is never mutated. Root basename, absolute host location, filesystem
enumeration order, locale, mtimes, permissions, ownership, ACLs, extended
attributes, and other host metadata do not affect package bytes.

For byte-identical canonical input under package v1, output is byte-identical
on every conforming implementation.

Pack prepares and validates the complete candidate before atomic publication.
It refuses to overwrite an existing file or other path. Failure leaves the
destination absent and never exposes a partial file as a successful artifact.
Exact temporary-file, rename, `fsync`, browser-transaction, buffering, and
streaming mechanisms remain host-owned implementation details.

## Unpack contract

### Required validation order

Unpack fails closed through these dependency-ordered stages:

```text
content framing and complete container structure
→ package.json structural probe and package-version dispatch
→ canonical ZIP profile, local/central agreement, and exact entry set/order
→ stored sizes and CRC-32
→ payload SHA-256 integrity root
→ package payload claims versus payload/manifest.json
→ exact canonical .roproj/v1 representation
→ applicable semantic conversion and validation
→ atomic publication to an absent destination
```

This order fixes dependencies and security boundaries, not a total precedence
among independent failures discovered within one stage.

Before package-version selection, a reader may parse only enough ordinary ZIP
structure and the unique stored `package.json` entry to dispatch. Missing,
malformed, duplicate, or invalid package metadata fails explicitly. A
syntactically valid `tachiko.portable-package` version other than `1` produces
`portable_package.unsupported_version` before payload CRC, integrity,
`.roproj` DTO, or semantic decoding. Unsupported payload bytes MUST NOT be
interpreted, canonicalized, migrated, or published.

For selected v1, unpack then requires the exact canonical ZIP and entry
profile. It rejects unknown, missing, duplicate, aliased, reordered, or
metadata-bearing entries. It checks local/central agreement and recomputes
every CRC before calculating the payload root.

After the root matches, unpack verifies that `payload/manifest.json` selects
the exact payload representation claimed by `package.json`. It then validates
the payload as an already-canonical `.roproj/v1` tree, not as the bounded
noncanonical canonicalizer input family. Wrong placement, spelling, ordering,
unknown fields, extra content, or another supported-but-noncanonical form is
rejected rather than normalized during unpack.

The complete payload is converted through version-owned DTOs and the
applicable semantic validation gate. No partial semantic document or
human-key-based repair is permitted.

### Publication

The destination MUST be absent. Unpack MUST NOT merge with or overwrite an
existing file or directory. After every preceding stage succeeds, it strips
only the exact leading `payload/` prefix and publishes exactly the 18
canonical relative paths and bytes.

No `package.json` field, root, ZIP metadata, filename, or package path is
copied into `.roproj`. Failure at any stage leaves the destination absent and
no partial tree visible as success. A publication race that discovers an
existing destination fails without replacing it.

## Exact lossless laws

For every canonical, supported, semantically valid `.roproj/v1` tree `P`
accepted by pack:

```text
unpack(pack(P)) == P
```

Equality means the same 18 relative paths and byte-identical body for every
path. It is stronger than semantic equivalence.

For every canonical package-v1 artifact `R` accepted by unpack:

```text
pack(unpack(R)) == R
```

Equality means byte-identical package bytes. It follows from exact payload
preservation, one deterministic manifest/root construction, and one canonical
ZIP profile.

The laws intentionally exclude root basenames, host paths, timestamps,
permissions, ownership, ACLs, extended attributes, symlinks, Git metadata,
caches, and unknown files because none is canonical `.roproj/v1` state.

## Package versus tracked `.roproj`

To compare a verified package with a canonical tracked `.roproj` source:

1. Complete package validation without publishing or mutating it.
2. Validate the tracked source as exact canonical `.roproj/v1`.
3. Calculate the tracked root with the identical path/byte algorithm.
4. Compare the two 32-byte roots.

Equal roots yield machine state `consistent`; no write, timestamp update,
regeneration, or synchronization follows. Different roots yield
`portable_package.source_mismatch` and the tracked `.roproj` remains
authoritative in that working context.

The package and tracked source are never automatically merged or overwritten.
A differing package may be unpacked only to another distinct absent
destination. Filename, modification time, size, tool version, `DocumentId`,
filesystem order, and most-recently-opened state do not choose authority.

Git attributes, CI consistency checks, semantic delta, and merge are outside
this contract and remain owned by #44, #45, and #46.

## Direct JSON coexistence and framing

Package v1 neither changes nor supersedes `legacy-direct-ro/v1` or
`direct-ro/v2`. It is not the current CLI writer.

A reader that supports both families MUST classify content before selecting a
version-specific parser:

- exact initial bytes `50 4b 03 04` select portable-package container handling;
- existing valid direct-JSON framing selects the direct-JSON reader; and
- the `.ro` extension alone never selects either representation.

Package v1 forbids a prepended stub, so its local-header signature occurs at
offset zero. Once package framing is selected, every package error remains a
package error. Malformed or unsupported package input MUST NOT fall back to a
direct-JSON parser. The inverse is also true for direct JSON.

This rule does not add a new direct-JSON version or change either direct-JSON
admission profile.

## Stable failure meanings

The following symbolic meanings are stable package-v1 conformance outcomes.
Concrete language enums, exception types, transport envelopes, and diagnostic
text may map them differently but MUST preserve the distinctions.

| Meaning | Required use |
| --- | --- |
| `portable_package.invalid_container` | Truncated, impossible, stubbed, trailed, split, or otherwise structurally invalid container framing |
| `portable_package.invalid_manifest` | Missing, duplicate, malformed, unknown-member, or wrong-profile `package.json` metadata for a selected/claimed profile |
| `portable_package.unsupported_version` | Syntactically valid positive package profile version not supported by the implementation |
| `portable_package.noncanonical_container` | Structurally parseable archive with noncanonical ZIP fields, manifest bytes, or entry order |
| `portable_package.entry_set_mismatch` | Missing, unknown, duplicate, extra, or aliased entry name/set |
| `portable_package.crc_mismatch` | Stored size or CRC disagreement after canonical profile selection |
| `portable_package.integrity_mismatch` | Recomputed 18-leaf SHA-256 payload root differs from `package.json` |
| `portable_package.payload_manifest_mismatch` | Inner `.roproj` manifest disagrees with package payload claims |
| `portable_package.noncanonical_payload` | Payload paths or bytes are not exact canonical `.roproj/v1` representation |
| `portable_package.invalid_semantic_payload` | Exact decoded payload fails applicable Accepted semantic conversion/validation |
| `portable_package.source_not_canonical` | Pack source is not an exact supported canonical `.roproj/v1` tree |
| `portable_package.capacity_exceeded` | Canonical package cannot fit ordinary package-v1 ZIP32 fields/length |
| `portable_package.resource_limit` | A declared finite host/implementation safety limit below package capacity rejects the input |
| `portable_package.destination_exists` | Pack or unpack destination is not absent, including a publication race |
| `portable_package.publication_failed` | Host publication fails after validation/preparation without exposing partial success |
| `portable_package.source_mismatch` | Verified package root differs from canonical tracked `.roproj` root |

`portable_package.capacity_exceeded` is a profile fact. A smaller finite
`portable_package.resource_limit` is a Provisional host/implementation
mechanism and MUST report its declared resource and bound; it must not be
misreported as corrupt payload or semantic invalidity.

Exact prose, Rust types, error payload fields, and total ordering among
unrelated defects are not fixed. These meanings may be wrapped in a broader
diagnostic envelope, but they may not be collapsed in a way that loses the
distinction between unsupported version, corruption, noncanonical input,
semantic invalidity, conflict, capacity, resource admission, destination
existence, and publication failure.

Every rejection before successful publication is atomic: an absent
destination remains absent. If the destination existed, it and its contents
remain unchanged.

## Normative byte-level golden vector

The golden source is the exact empty `.roproj/v1` fixture at
[`docs/research/fixtures/issue-43-portable-package-v1/empty.roproj/`](../research/fixtures/issue-43-portable-package-v1/empty.roproj/manifest.json):

- `manifest.json` is the 121-byte empty-document vector from
  `roproj-format.md`;
- `schemas.json` is exactly three bytes: `5b 5d 0a`;
- all 16 required entity shards exist and are zero bytes; and
- no other path exists.

Its payload root is:

```text
71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c
```

The normative package bytes are encoded as one lowercase hexadecimal line in
[`empty-package-v1.hex`](../research/fixtures/issue-43-portable-package-v1/empty-package-v1.hex).
The terminal LF in the `.hex` text file is not part of the decoded package.

After hexadecimal decoding, the package has:

| Property | Exact value |
| --- | ---: |
| Package length | 2,692 bytes |
| SHA-256 of complete package | `1368ebe38c86de28d2379ae6c0ca7a5ca8502543002fe084e33254ad1db4d7bc` |
| `package.json` body length | 228 bytes |
| Central-directory offset | 1,359 |
| Central-directory length | 1,311 bytes |
| End-record offset | 2,670 |
| Entry count | 19 |

The static hex vector, not a ZIP library's output, is the byte authority. The
disposable probe manually constructs every record and asserts equality with
the vector. Standard `unzip -t` also accepts all 19 stored entries, which is
interoperability evidence rather than normative authority.

## Required conformance cases

A production implementation must cover at least:

1. packing the same canonical `.roproj` twice produces identical bytes;
2. unpacking a valid package publishes the exact 18 paths and bytes;
3. one corrupted payload byte fails at CRC/size validation and publishes
   nothing;
4. corruption with recomputed CRC but stale SHA-256 root fails integrity and
   publishes nothing;
5. missing, duplicate, unknown, or malformed package metadata fails closed;
6. an unsupported package version wins before payload semantic decoding;
7. unknown, missing, duplicate, extra, or aliased entries fail closed;
8. a disagreeing tracked `.roproj` produces source mismatch without mutation;
9. noncanonical ZIP metadata and noncanonical entry order fail closed;
10. pack → unpack → pack is byte-identical;
11. all 16 zero-byte shards exist in the package and contribute distinct
    integrity leaves;
12. local and central record disagreement fails closed;
13. prepended stubs, archive/file comments, explicit directories, data
    descriptors, ZIP64, and trailing bytes fail closed;
14. an existing destination remains untouched;
15. a noncanonical pack source creates no artifact; and
16. content classification never falls back after selecting a malformed
    package or direct-JSON representation.

The checked-in evidence probe covers the Issue #43 pressure set plus host
basename/mode/mtime independence, missing/aliased/duplicate entries, duplicate
metadata, trailing data, existing pack and unpack destinations, noncanonical
pack source, and content framing. Its empty-payload
semantic checker is intentionally fixture-specific; production conformance
must use the normative `.roproj/v1` DTO and semantic validation implementation
when those codecs are built.

The probe's injected pre-publication hook verifies the stable outcome when a
destination appears after preparation. It does not select or prove a
production cross-platform atomic no-replace directory primitive; exact host
publication mechanics remain Provisional while every implementation must
satisfy the normative no-overwrite and no-partial-success result.

## Security and resource boundary

Package inputs are untrusted. Implementations must use bounded parsing,
overflow-checked arithmetic, exact entry counts, and fail-closed publication.
Package v1 fixes the ZIP32 capacity boundary but does not fix smaller host
memory, archive-byte, per-entry-byte, nesting, or time limits. Such finite
limits are Provisional implementation details until #52 establishes a broader
hostile-container profile; they must remain explicit and machine-
distinguishable.

CRC-32 and the unauthenticated payload root are not security against a
malicious writer. Signatures, trust roots, authentication, freshness,
rollback protection, and malicious coordinated-tamper resistance remain #53.
Encryption and key management are Deferred.

Path validation occurs on archive names before host path construction.
Implementations must never extract an unrecognized or aliased path, follow an
archive-provided symlink, or allow a path to escape the absent destination.
Because the exact package vocabulary contains only fixed ASCII paths and
ordinary file entries, a conforming unpacker does not need generic archive
path sanitization as a substitute for exact-set validation.

## Explicitly out of scope

This specification does not define or authorize:

- a production `.roproj` reader, writer, or canonicalizer;
- a production packaged-`.ro` codec;
- CLI pack/unpack commands or flags;
- a change to the current direct-JSON CLI writer;
- compression or ZIP64;
- package extension areas;
- signatures, trust, encryption, or key management;
- cloud distribution;
- Git/CI automation, generated-artifact policy, or synchronization;
- semantic delta, revision, operation-log, or merge formats;
- AI, SemanticPatch, approval, capability, or mutation authority;
- concrete Rust crates, modules, public APIs, or error enums;
- exact diagnostic prose or unrelated-failure precedence;
- streaming versus buffering; or
- temporary-file, rename, `fsync`, locking, browser-transaction, or recovery
  mechanics.

## External primitive references

- [PKWARE APPNOTE 6.3.10](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
  defines the ordinary ZIP records, little-endian fields, UTF-8 flag, store
  method, timestamps, CRC-32, ZIP64 sentinels, and path separator rules used by
  this narrower canonical profile.
- [NIST FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)
  defines SHA-256.
- [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) defines JSON syntax; the
  package manifest deliberately fixes a much narrower closed canonical DTO.

## Related

- [ADR-0025](../decisions/ADR-0025-portable-package-v1.md)
- [ADR-0023](../decisions/ADR-0023-roproj-v1-canonical-tree-and-sharding.md)
- [`.roproj/v1` wire DTOs](roproj-format.md)
- [`.roproj/v1` layout](roproj-layout-v1.md)
- [Storage versioning and migration](storage-versioning-and-migration.md)
- [Issue #43](https://github.com/nurockplayer/tachiko-work/issues/43)
- [Issue #3](https://github.com/nurockplayer/tachiko-work/issues/3)
