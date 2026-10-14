# Tachiko Work portable package v1

Decision state: Accepted under [ADR-0042](../decisions/ADR-0042-first-public-portable-package-over-roproj-v3.md).

Implementation state: #519 implements this Accepted first-public CORE package
profile with a new independent golden vector. The superseded developmental
package/v1-over-roproj/v1 profile from #3 is refused. Sheet attachments and
Work/Sheet consumer qualification remain separate obligations; #488 is not
complete. Browser-runtime verification accepts constrained packages, while
runtime open refuses their unsupported constraints. Portable export preflights
fresh admission in the ordinary v3 runtime profile, refusing capacity-profile
documents beyond its limits.

Package profile: `tachiko.portable-package/v1`

Payload profile: `tachiko.roproj/v3`

Authority: [ADR-0042](../decisions/ADR-0042-first-public-portable-package-over-roproj-v3.md); unchanged container rules originate in
[ADR-0025 §3](../decisions/ADR-0025-portable-package-v1.md#3-canonical-zip32-profile)
and are restated below. Payload authority: [v3 wire DTOs](roproj-format-v3.md)
and [v3 physical layout](roproj-layout-v3.md).

Decision issue: [#488](https://github.com/nurockplayer/tachiko-work/issues/488)

## Scope and version ownership

### Native display-name payload amendment — ADR-0043

[ADR-0043](../decisions/ADR-0043-native-display-names-separate-from-keys.md)
supersedes this unreleased profile's v3 Schema/FieldDefinition payload shape
to include optional `display_name`. The sole current writer and strict reader
use [the revised v3 DTOs](roproj-format-v3.md#schema-and-field-definition):
canonical omission when absent, never null. Names survive exact-tree
pack/unpack/fresh reopen. No package version change, v4, dual profile, or
down-conversion is selected. Older binaries refuse name-bearing packages;
Sheet rollback past the name-capable Work re-pin cannot reopen them.
Name support remains pending #529 implementation; #519 evidence predates it.

All twenty entries, nineteen payload paths, manifest members/order, hash
domains/algorithm, container primitives, and bounds remain unchanged. Named
`schemas.json` changes its leaf, payload root, affected CRCs, and ZIP bytes.
Existing valid nameless current-profile fixtures MUST remain byte-identical,
including hashes/CRCs/repack bytes. #529's separate implementation MUST add a
new independent static named fixture, with exact schema/package bytes and
fresh-reopen/repack evidence. This does not revive historical v1-over-v1
vectors. CORE attachment-loss disclosure below remains required; native name
support does not promote private CSV/XLSX labels to canonical names.

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.
This is the first PUBLIC v1: exactly one canonical `.roproj/v3` tree in a
thin CORE package, with no package-owned semantic DTO or extension area.
It replaces the unreleased v1-over-roproj/v1 contract. There is no legacy
package reader/writer, retained-old-vector requirement, or pre-release package
migration obligation. Future incompatible public changes require another
package-profile version. Package, payload, semantic-model, and release
versions are independent namespaces. The `.ro` extension remains Provisional;
content and manifest dispatch identify the protocol. `direct-ro/v2` remains
Provisional and is not a public file format.

## Exact entry vocabulary and order

Exactly 20 regular-file entries occur in this local-header and central-directory
order: one package manifest plus all 19 canonical v3 files. This count and
order agree with `ROPROJ_V3_PATHS` in `crates/storage/src/roproj/v3.rs` and
[the v3 layout](roproj-layout-v3.md#canonical-tree).

```text
package.json
payload/manifest.json
payload/schemas.json
payload/definitions.json
payload/entities/0.jsonl
payload/entities/1.jsonl
payload/entities/2.jsonl
payload/entities/3.jsonl
payload/entities/4.jsonl
payload/entities/5.jsonl
payload/entities/6.jsonl
payload/entities/7.jsonl
payload/entities/8.jsonl
payload/entities/9.jsonl
payload/entities/a.jsonl
payload/entities/b.jsonl
payload/entities/c.jsonl
payload/entities/d.jsonl
payload/entities/e.jsonl
payload/entities/f.jsonl
```

The payload bodies MUST be exact canonical v3 bytes, including required
`definitions.json` (even when `[]\n`) and all sixteen shards (zero bytes when
empty). Names are the exact shown case-sensitive ASCII/UTF-8 bytes, with `/`
as the only separator. No explicit directory entry, unknown/missing/extra/
duplicate path, alias, `./`, `../`, absolute/drive path, empty component,
backslash, escaped spelling, Unicode variant, or reordered entry is accepted.
Unpack strips exactly `payload/` from the nineteen payload names. This prefix
is not persisted or used as semantic identity.

## `package.json`

The closed five-member object has this exact member order and fixed values:

```json
{
  "format": "tachiko.portable-package",
  "format_version": 1,
  "payload_format": "tachiko.roproj",
  "payload_format_version": 3,
  "payload_root_sha256": "<64 lowercase hexadecimal digits>"
}
```

Every member is required and non-null. Unknown or duplicate members (including
escaped-equivalent names) are invalid. Both versions MUST be lexical positive
JSON integers; strings, fractions, exponents, zero, negative, or missing forms
are malformed. The only supported values are package `1` and payload `3`.
The root is exactly 64 lowercase ASCII hexadecimal digits.

Canonical bytes are UTF-8 without BOM, exactly the shown two-space indentation,
member spelling/order, comma and colon spacing, LF line endings, no trailing
spaces or blank lines, and exactly one final LF. Substituting the actual root
makes the manifest exactly 228 bytes. There is no document ID/title, timestamp,
tool version, host path, Git metadata, inventory, per-file hashes, extension,
signature, provenance, trust, or authorization data.

## Payload integrity root

For each relative payload path `p` in the entry order above with `payload/`
removed, and exact file bytes `b`, calculate:

```text
leaf(p, b) = SHA-256(UTF8(p) || 0x00 || b)
domain = ASCII("tachiko.portable-package/v1") || 0x00
         || ASCII("tachiko.roproj/v3") || 0x00
payload_root = SHA-256(domain || leaf_1 || ... || leaf_19)
```

Each leaf is its raw 32 digest bytes, not hexadecimal text. No path length,
final separator, normalization, case folding, root basename, or `payload/`
prefix is included. All nineteen files contribute, including each empty shard
and `definitions.json`. Store the root's lowercase hexadecimal encoding in
`package.json`. The algorithm is unchanged from ADR-0025 §4; its payload
domain and leaf count now select v3.

The root covers exact payload paths and bytes only, not the package manifest,
ZIP fields, host metadata, Git state, or decoded semantic objects. CRC and the
unauthenticated root detect corruption/equality; they do not authenticate an
author, confer trust, protect against coordinated malicious tampering, provide
confidentiality/freshness/rollback protection, or define semantic identity.

## Canonical ZIP32 container

Package v1 adopts the ordinary local header, stored data, central directory
header, and end-of-central-directory structures from PKWARE APPNOTE 6.3.10.
Every multibyte integer below is unsigned little-endian.

### Overall record sequence

The complete archive is:

```text
local header 1 || stored data 1
...
local header 20 || stored data 20
central header 1
...
central header 20
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
| Entries on this disk | 2 | `0x0014` (20) |
| Total entries | 2 | `0x0014` (20) |
| Central-directory size | 4 | Exact byte length |
| Central-directory offset | 4 | Exact byte offset |
| Archive-comment length | 2 | `0x0000` |
| Archive comment | variable | Absent |

The central-directory offset points to central header 1. Its size consumes
exactly central headers 1 through 20. The end record follows immediately and
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
`0xfffffffe`. Package v1 has a fixed entry count of 20, already below the
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

## Writer and publication

Pack MUST accept only storage-validated, exact canonical `.roproj/v3` state
under the v3 DTO/layout and applicable semantic validation rules. It probes
the source manifest before body interpretation. Other recognized versions
fail as unsupported payload representation; invalid or noncanonical v3 fails
without repair. The writer copies all nineteen paths and bytes, computes the
root and manifest, and prepares the complete canonical archive before atomic
publication to an absent destination. Source state is never mutated.

No lowest-representable-version selection, implicit migration/down-conversion,
dual writer, omitted file, silent canonicalization, symlink following, or
host-metadata-dependent output is allowed. An older pre-release project is not
packaged until explicitly migrated to v3 through existing Accepted paths in
[storage-versioning-and-migration.md](storage-versioning-and-migration.md).
This decision adds no migration path or private-project conversion.

## Single reader and manifest-first dispatch

Finite complete-input admission and container framing precede dispatch.
Exact leading bytes `50 4b 03 04` select package handling. The extension alone
never selects a parser; after selection, failures MUST NOT fall back to direct
JSON or another package/payload decoder.

The reader first locates the unique first stored entry `package.json` and
performs bounded structural JSON inspection, rejecting duplicate members and
malformed profile/version fields. It probes exact
`format: "tachiko.portable-package"` and lexical positive `format_version`
before requiring v1-owned fields. A different syntactically valid positive
package version fails as `portable_package.unsupported_version` before
v1-specific or payload decoding, even if it has a future-owned manifest shape.
An unknown/other format fails as `portable_package.invalid_manifest`.

For selected package v1, require the closed manifest, probe `payload_format`
and `payload_format_version`, and reject any other payload profile/version
as `portable_package.unsupported_payload_representation` before interpreting
payload bodies. This includes developmental v1 packages carrying roproj/v1.
Malformed/missing payload fields fail as `portable_package.invalid_manifest`.
Then validate in dependency order:

```text
canonical manifest bytes and ZIP fields, exact entry set/order, local/central agreement
→ stored sizes and recomputed CRC-32
→ recomputed nineteen-leaf SHA-256 payload root
→ inner manifest agreement: tachiko.roproj, lexical format_version 3
→ exact canonical v3 DTO/tree and applicable storage semantic validation
→ atomic publication to an absent destination
```

An inner manifest that disagrees with the outer claims fails as
`portable_package.payload_manifest_mismatch` before body interpretation.
These stages fix dependencies, not total precedence among unrelated defects.
Unsupported bodies are never decoded, canonicalized, migrated, or published.
Unpack publishes only the exact nineteen v3 files, without package metadata.
Every failed publication leaves an absent destination absent; an existing
path, including one created in a publication race, remains unchanged.
Atomic no-replace publication and complete preparation are required for pack
and unpack; concrete host transaction/durability mechanisms remain host-owned.

## Exact round trips and tracked-source authority

For every admitted canonical v3 tree `P` and accepted package `R`:

```text
unpack(pack(P)) == P              # identical relative paths and bytes
pack(unpack(R)) == R              # byte-identical archive
fresh_reopen(unpack(pack(P))) == fresh_reopen(P)
```

Fresh reopen uses v3 storage admission and preserves Date, saved definitions,
constraints, and all other admitted meaning. It adds no formula-calculation
admission guarantee beyond the existing storage gate. Host paths, permissions,
symlinks, caches, attachments, and Git history are outside this equality.

A canonical tracked v3 source uses the same root algorithm. Equal roots mean
`consistent` and authorize no mutation. Different roots mean
`portable_package.source_mismatch`; tracked source remains authoritative in
that working context. Neither side is overwritten, merged, synchronized, or
regenerated. A differing package may be unpacked only to a distinct absent
destination. Filename, mtime, size, tool version, DocumentId, filesystem order,
and recent-open state never choose authority.

## Stable failure meanings

The following symbolic meanings are stable package-v1 conformance outcomes.
Concrete language enums, exception types, transport envelopes, and diagnostic
text may map them differently but MUST preserve the distinctions.

| Meaning | Required use |
| --- | --- |
| `portable_package.invalid_container` | Truncated, impossible, stubbed, trailed, split, or otherwise structurally invalid container framing |
| `portable_package.invalid_manifest` | Missing, duplicate, malformed, unknown-member, or wrong-profile `package.json` metadata for a selected/claimed profile |
| `portable_package.unsupported_version` | Syntactically valid positive package profile version other than 1 |
| `portable_package.noncanonical_container` | Structurally parseable archive with noncanonical ZIP fields, manifest bytes, or entry order |
| `portable_package.entry_set_mismatch` | Missing, unknown, duplicate, extra, or aliased entry name/set |
| `portable_package.crc_mismatch` | Stored size or CRC disagreement after canonical profile selection |
| `portable_package.integrity_mismatch` | Recomputed 19-leaf SHA-256 payload root differs from `package.json` |
| `portable_package.payload_manifest_mismatch` | Inner `.roproj` manifest disagrees with package payload claims |
| `portable_package.noncanonical_payload` | Payload paths or bytes are not exact canonical `.roproj/v3` representation |
| `portable_package.invalid_semantic_payload` | Exact decoded payload fails applicable Accepted semantic conversion/validation |
| `portable_package.source_not_canonical` | Pack source is not an exact supported canonical `.roproj/v3` tree |
| `portable_package.unsupported_payload_representation` | Manifest claims select any other payload profile/version, or pack source selects a recognized unsupported `.roproj` version, including v1 and v2 |
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

## Bounds and untrusted input

Reuse the existing finite host bounds: at most 64 MiB (`67,108,864` bytes) of
complete package input, with the same finite byte budget while reading a pack
source; the version-independent package manifest probe admits at most 256
nested JSON containers. Exactly-at-limit input is admitted; one over fails
as `portable_package.resource_limit`. V3's own DTO, formula, and constraint
bounds remain applicable. These host bounds remain Provisional mechanisms,
not semantic limits or ZIP32 capacity. Any additional finite host bound MUST
identify its resource and value and preserve the distinct resource-limit
outcome. Parsing and arithmetic MUST be bounded and overflow-checked.

Path validation precedes host path construction. No archive-provided symlink
or unrecognized/aliased path may be extracted or escape the absent destination.
Compression, ZIP64, signing, encryption, extension registries, new semantic
DTOs, and trust semantics are outside this profile.

## CORE package and host attachment disclosure

Sheet client-held import source bytes, import metadata/fidelity ledger, and
digest-bound report settings are not carried. A product download MUST disclose
actually omitted attachments before user consent as explicit `lossy_on_export`
findings following [#513](https://github.com/nurockplayer/tachiko-work/issues/513).
Absent attachments MUST NOT generate invented loss findings. A lossless CORE
package round trip alone does not satisfy #488's complete user-owned workbook
exit. Work and Sheet consumer qualification remain separate obligations.

## Required implementation evidence

The implementation PR MUST add a new static byte-level golden vector for this
redefined profile and verify independent exact-byte agreement. The historical
[Issue #43 evidence](../research/2026-08-26-portable-package-v1.md), probe, and
fixture bytes remain Superseded evidence in Git; they are not current vectors.
Developmental code/tests/fixtures may be removed or replaced in implementation;
no legacy package migration or compatibility obligation is imposed.

Conformance MUST cover deterministic pack; exact nineteen-file unpack;
pack→unpack→pack and pack→unpack→fresh reopen with Date, saved definitions,
and both non-`none` constraints; ADR-0043 native names in their separate
implementation evidence; all empty shards and empty definitions;
strict manifest-first rejection of unknown profiles, package/payload versions
(including payload v1), missing/duplicate/unknown members and entries;
CRC corruption and recomputed-CRC/stale-root corruption; inner-manifest
mismatch; noncanonical ZIP fields/order and all forbidden features;
noncanonical/invalid semantic source and unsupported older source refusal;
exact/one-over bounds; tracked-source mismatch without mutation; existing
and racing destinations without overwrite or partial success; and honest
attachment disclosure before consent plus no fabricated absent-attachment loss.
