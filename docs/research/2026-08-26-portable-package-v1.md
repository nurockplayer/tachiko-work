# Portable package v1 decision evidence

Date: 2026-08-26

Decision state: Evidence / Hypothesis; authority is promoted only by
[ADR-0025](../decisions/ADR-0025-portable-package-v1.md) and the
[normative specification](../specs/portable-package-v1.md)

Decision issue: [#43](https://github.com/nurockplayer/tachiko-work/issues/43)

Tracking issue: [#3](https://github.com/nurockplayer/tachiko-work/issues/3)
for production implementation

Repository baseline: `main@6b53565d6a2e61a629b02ac8173993424543b260`

## Question and boundary

Issue #43 asks whether the ADR-0003 portable `.ro` role can be reduced to one
deterministic package over ADR-0023's exact `.roproj/v1` materialization,
without creating another semantic schema or weakening tracked-source
authority.

This pass studies and pressure-tests only that authority/specification seam.
It does not implement production `.roproj` or packaged-`.ro` codecs, add CLI
pack/unpack, change either direct-JSON profile, close #3, or alter #27,
Semantic API, AI, capability, approval, revision, delta, or merge authority.

Before the decision was drafted, live `main`, Issue #43 and its comments, the
open PR set, repository instructions, and the relevant Accepted authority were
checked. Open PR #114 had already reserved ADR-0024 for unrelated
SemanticPatch work, so this independent decision uses ADR-0025. Open PR #115
was also unrelated. No live Accepted authority contradicted the Issue #43
contract.

## Authority consumed

- Product Constitution and Design Principles: user ownership, open formats,
  reproducibility, deterministic behavior, and reuse before invention.
- ADR-0003: `.roproj` is canonical editable source; `.ro` is a derived
  portable artifact.
- ADR-0015: physical paths are not semantic identity.
- ADR-0017: representation-local versions, version-owned DTOs, deterministic
  canonical bytes, explicit dispatch, and fail-closed unknown/newer behavior.
- ADR-0023 and the `.roproj/v1` specifications: one exact 18-file canonical
  payload tree; package and integrity mechanics remained deferred to #43.
- Knowledge Authority and the Canonical Reconciliation Register: Issue prose
  and this evidence cannot silently outrank Accepted authority.

The repository implementation currently reads and writes direct JSON `.ro`.
That is implementation evidence, not permission to package the semantic Rust
aggregate or the direct-JSON DTO. Production conversion remains #3 work.

## Standards mapping

The narrow profile reuses mature primitives while eliminating byte-affecting
container discretion:

| Need | Reused primitive | Profile decision |
| --- | --- | --- |
| Single-file container | PKWARE ZIP records | Ordinary single-disk ZIP32 only |
| Payload storage | ZIP method 0 | Stored bytes; no compression |
| Entry corruption check | ZIP CRC-32 | Required over every exact entry body |
| Filename encoding | ZIP UTF-8 flag | Flags exactly `0x0800`; fixed ASCII names |
| Stable host metadata | ZIP fixed fields | DOS epoch, FAT host, zero attributes/extras/comments |
| Exact payload equality | NIST SHA-256 | Domain-separated path-and-byte root over all 18 files |
| Package dispatch | RFC 8259 JSON | Closed, exact 228-byte `package.json` spelling |

The ZIP field choices follow
[PKWARE APPNOTE 6.3.10](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT).
[NIST FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)
defines SHA-256, and [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)
defines the JSON syntax narrowed by the package manifest profile.

## Candidate profile exercised

The candidate packages exactly these records, in order:

```text
package.json
payload/manifest.json
payload/schemas.json
payload/entities/0.jsonl
...
payload/entities/f.jsonl
```

There are no directory entries or extension areas. Every ZIP record is stored,
unencrypted, descriptor-free, extra-free, comment-free, and timestamped at
the DOS epoch. Sizes, offsets, and the total package length must remain below
ordinary ZIP32 sentinel values; there is no automatic ZIP64 fallback.

For each canonical `.roproj/v1` path `p` with exact body `b`, the candidate
uses:

```text
leaf(p, b) = SHA-256(UTF8(p) || 0x00 || b)

payload_root = SHA-256(
  ASCII("tachiko.portable-package/v1") || 0x00 ||
  ASCII("tachiko.roproj/v1") || 0x00 ||
  leaf_1 || ... || leaf_18
)
```

The package prefix is excluded. All 16 required zero-byte shards contribute
different leaves because each leaf includes its canonical relative path.

## Executable evidence

The disposable Node probe
[`issue-43-portable-package-v1.mjs`](probes/issue-43-portable-package-v1.mjs)
uses only Node standard-library primitives. It manually writes and parses the
selected ZIP32 records, calculates CRC-32 and SHA-256, stages disposable
publication, injects late destination races, and validates the checked-in
fixture. It is deliberately not a production codec or reusable product API.

The injected pre-publication seam proves that a destination discovered after
preparation is rejected unchanged and that staged output is cleaned. It does
not claim Node's directory `rename` is a cross-platform atomic no-replace
primitive or prescribe the production host mechanism. A production
implementation must select a host primitive that satisfies the normative
no-overwrite/publication contract; that mechanism remains Provisional.

The canonical source fixture is
[`empty.roproj/`](fixtures/issue-43-portable-package-v1/empty.roproj/manifest.json).
The decoded byte authority is the static lowercase hexadecimal vector
[`empty-package-v1.hex`](fixtures/issue-43-portable-package-v1/empty-package-v1.hex),
not the output of a ZIP library.

Fresh probe output for the checked-in vector:

| Property | Observed value |
| --- | --- |
| Payload root | `71e2b1170ae3b2c2259cc0c90c217389a1e59c490b5ccde4c6fe2dadae1fed9c` |
| Complete package length | 2,692 bytes |
| Complete package SHA-256 | `1368ebe38c86de28d2379ae6c0ca7a5ca8502543002fe084e33254ad1db4d7bc` |
| `package.json` body | 228 bytes |
| Central-directory offset | 1,359 |
| Central-directory length | 1,311 bytes |
| End-record offset | 2,670 |
| Entry count | 19 |

The platform `unzip -t` implementation independently accepted all 19 stored
entries, and `zipinfo -1` reported the exact required order. This is useful
interoperability evidence; the normative field table and golden bytes remain
authority.

## Pressure results

| Required pressure | Result |
| --- | --- |
| Pack the same canonical source twice | Byte-identical |
| Change source basename, mode, or mtime | Package bytes unchanged |
| Valid unpack | Exact 18 paths and bytes published |
| Corrupt one payload byte | `portable_package.crc_mismatch`; no destination |
| Recompute CRC but retain stale root | `portable_package.integrity_mismatch`; no destination |
| Missing, malformed, or duplicate package metadata | Explicit invalid-manifest failure |
| Unsupported package version | Unsupported-version failure before payload decoding |
| Unsupported version plus a later malformed entry name | Unsupported version still wins before entry decoding |
| Unknown package metadata | Explicit invalid-manifest failure |
| Missing, unknown, duplicate, or aliased entry | Entry-set mismatch |
| Package versus disagreeing tracked source | Source mismatch; neither side mutated |
| Noncanonical ZIP metadata | Noncanonical-container failure |
| Noncanonical record order | Noncanonical-container failure |
| Pack → unpack → pack | Byte-identical |
| All required empty shards | All present, zero bytes, and included as distinct leaves |
| Existing pack or unpack destination | Rejected and left unchanged |
| Destination appears at the pre-publication seam | Rejected and raced destination left unchanged |
| Noncanonical pack source | Rejected without artifact publication |
| Symlinked canonical source directory | Rejected without artifact publication |
| Prepended/trailing framing variation | Invalid-container failure |
| Split EOCD or per-entry disk selection | Invalid-container failure |
| Malformed package after package framing | No direct-JSON fallback |

The probe also checks local/central agreement, exact entry order, zero extras
and comments, lexical version spelling, canonical manifest spelling, inner
payload-profile agreement, source non-mutation, and absent partial
destinations.

Reproduce the complete evidence run with:

```sh
node docs/research/probes/issue-43-portable-package-v1.mjs
```

To retain a decoded package for independent tools, provide a new disposable
directory:

```sh
node docs/research/probes/issue-43-portable-package-v1.mjs \
  --keep-dir /tmp/tachiko-portable-package-v1-inspect
unzip -t /tmp/tachiko-portable-package-v1-inspect/empty-package-v1.ro
zipinfo -1 /tmp/tachiko-portable-package-v1-inspect/empty-package-v1.ro
```

## Findings

1. A deterministic thin package over exact `.roproj/v1` bytes is sufficient;
   no second semantic DTO is needed.
2. Generic ZIP defaults are not deterministic enough. A complete field and
   record-order profile plus a golden vector is required for byte identity.
3. CRC-32 catches ordinary entry corruption; the path-separated SHA-256 root
   catches payload changes even when CRC fields are coherently rewritten.
4. The root proves exact payload equality only. It is not a signature,
   revision identifier, semantic identity, or trust assertion.
5. Exact byte preservation supports both lossless laws without hidden
   canonicalization during pack or unpack.
6. Comparing the same root on a verified package and tracked `.roproj` gives a
   deterministic consistency result while leaving the tracked source in
   authority and both sides unchanged on conflict.
7. Content framing can coexist fail-closed with legacy direct JSON without
   changing current direct readers or writers.

## Rejected alternatives

- **Package the current direct JSON or Rust semantic aggregate:** duplicates
  representation authority and bypasses the Accepted `.roproj/v1` boundary.
- **Permit generic ZIP encoders:** compression, timestamps, attributes,
  extras, comments, descriptors, ZIP64, and order make bytes host- or
  library-dependent.
- **Hash decoded semantic objects only:** loses the exact source-byte and
  zero-shard preservation required by the Issue.
- **Hash concatenated bodies without paths:** permits path/body ambiguity and
  fails to distinguish empty shards.
- **Treat SHA-256 as authentication:** makes a false trust claim against a
  malicious writer who can rewrite both payload and manifest.
- **Auto-canonicalize during pack/unpack:** hides source defects and breaks the
  explicit validation and exact-round-trip boundary.
- **Choose package or tracked source by mtime:** host metadata is unstable and
  cannot override canonical tracked-source authority.
- **Silently upgrade to ZIP64:** changes the byte protocol instead of failing
  the fixed v1 capacity boundary.

## Remaining implementation and security work

Production `.roproj` codecs, packaged-`.ro` codecs, CLI pack/unpack, and their
full semantic validator remain Issue #3 implementation work. #44 owns Git/CI
automation; #45/#46 own delta and merge. #52 owns broader hostile-container
resource policy, and #53 owns signatures/trust. Those tasks must consume this
contract rather than infer new authority from the evidence probe.
