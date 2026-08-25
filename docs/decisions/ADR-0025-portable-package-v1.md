# ADR-0025: Portable package v1 and payload integrity root

## Status

Accepted

Decision issue: [#43](https://github.com/nurockplayer/tachiko-work/issues/43)

Validated by: [portable-package v1 evidence](../research/2026-08-26-portable-package-v1.md),
the [executable probe](../research/probes/issue-43-portable-package-v1.mjs),
and its [byte-level golden vector](../research/fixtures/issue-43-portable-package-v1/empty-package-v1.hex)

Specified by: [`portable-package-v1.md`](../specs/portable-package-v1.md)

Related authority: ADR-0003, ADR-0015, ADR-0017, and ADR-0023

## Context

ADR-0003 makes `.roproj` the canonical editable, Git-native materialization
and `.ro` a derived portable artifact. ADR-0015 keeps semantic identity
independent of paths and physical layout. ADR-0017 requires representation-
local version namespaces, version-owned DTOs, fail-closed dispatch, and
deterministic canonical emission. ADR-0023 then fixes one exact canonical
`.roproj/v1` tree of 18 regular files while deliberately deferring package,
integrity, and container decisions to Issue #43.

The repository still implements direct JSON `.ro` profiles rather than a
production `.roproj` or packaged-`.ro` codec. That implementation lag is not
authority to package the current semantic Rust aggregate or direct-JSON DTO.
The portable artifact must wrap canonical `.roproj/v1` bytes without creating
a second semantic schema.

The package also needs a deterministic container profile precise enough for
independent implementations. A generic instruction to “write a ZIP” is
insufficient: ZIP permits compression choices, timestamps, host attributes,
extra fields, comments, entry reordering, data descriptors, ZIP64, and other
byte-affecting variation. The profile therefore has to freeze the small subset
it uses while retaining mature ZIP, CRC-32, JSON, UTF-8, and SHA-256
primitives.

## Decision

### 1. Package role and namespace

The portable package profile is:

```text
tachiko.portable-package/v1
```

Package v1 packages exactly one canonical `tachiko.roproj` version `1`
materialization. It introduces no package-specific semantic `Document`,
schema, entity, field, value, formula, identity, revision, or operation DTO.

The package-profile namespace is independent from:

- `legacy-direct-ro/v1` and `direct-ro/v2`;
- `.roproj/v1`;
- semantic-model versions; and
- Tachiko Work application or release versions.

The artifact currently uses the `.ro` extension, but that extension remains
Provisional and is not protocol identity. Another payload representation or
version, or an incompatible package change, requires another package-profile
version.

### 2. Exact package contents

Package v1 contains exactly 19 regular-file entries in this order:

```text
package.json
payload/manifest.json
payload/schemas.json
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

There are no explicit directory entries, unknown entries, aliases, duplicate
entries, alternate path spellings, or extension areas. `payload/` is a
packaging-only prefix stripped during unpack. It is not semantic identity and
is excluded from the payload integrity-root path input.

`package.json` is a closed-world packaging-only DTO. Its complete shape,
member order, canonical UTF-8 spelling, and one-final-LF rule are fixed by the
normative specification. It identifies the package profile, supported payload
profile, and exact payload root. It contains no semantic metadata, timestamps,
tool versions, source paths, Git data, entry inventory, arbitrary extension,
signature, or trust assertion.

### 3. Canonical ZIP32 profile

Package v1 uses one canonical, store-only ZIP32 profile. Every integer is
little-endian. Every entry has:

- ZIP version-needed `1.0`;
- general-purpose flags exactly `0x0800`, selecting UTF-8 names and no other
  feature;
- compression method `0` (stored);
- MS-DOS time `0x0000` and date `0x0021`, representing
  `1980-01-01 00:00:00`;
- the standard ZIP CRC-32 over exact entry data;
- equal compressed and uncompressed sizes;
- no local or central extra field;
- no file comment; and
- byte-identical local and central names and matching CRC, size, method,
  flags, timestamp, and local-offset claims.

Central records additionally use “version made by” `0x0014` (ZIP 2.0,
MS-DOS/FAT host), disk start zero, and zero internal and external attributes.
The archive is a single disk with exactly 19 entries, one central directory,
one ordinary end-of-central-directory record, and no archive comment.

The archive contains no compression, encryption, data descriptor, ZIP64,
split/spanned record, explicit directory entry, extra field, file/archive
comment, digital signature record, archive-extra record, prepended stub,
padding, or trailing data. Local records and data are contiguous in canonical
entry order; central records follow immediately in the same order; the end
record follows immediately after the central directory.

Every size, count, and offset must fit its ordinary non-sentinel ZIP32 field.
If it does not, package v1 fails with a capacity error. It never silently
switches to ZIP64 or another profile. The production ZIP-library choice is an
implementation detail; normative fields and golden bytes are authority.

### 4. Exact payload integrity root

Package v1 uses SHA-256 over all 18 canonical `.roproj/v1` relative paths and
exact file bytes. For canonical relative path `p` and file bytes `b`:

```text
leaf(p, b) = SHA-256(UTF8(p) || 0x00 || b)
```

Define:

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

Leaves use ADR-0023's canonical path order. Every zero-byte entity shard
contributes its own path-separated leaf. The `payload/` prefix is excluded.

The root does not cover `package.json`, ZIP headers, offsets, CRC fields,
artifact filename, source-root basename, host metadata, or decoded semantic
objects. It proves exact payload equality and detects accidental corruption
after CRCs have been recomputed or bypassed. It does not authenticate an
author, establish trust or authorization, provide confidentiality or
freshness, prevent rollback, resist malicious coordinated tampering, name a
semantic revision, or become `DocumentId` or Git commit identity.

This SHA-256 use is independent from ADR-0023's SHA-256 entity-shard placement
function.

### 5. Pack, unpack, and exact round trips

`pack` accepts only an exact canonical, supported, semantically valid
`.roproj/v1` tree. It copies all 18 files byte-for-byte, including every
zero-byte shard. It rejects noncanonical input rather than canonicalizing or
migrating implicitly, ignores host metadata, never mutates the source, refuses
to overwrite a destination, and publishes atomically so failure exposes no
partial successful artifact. An explicit canonicalize operation may precede
pack but is separate authority and behavior.

`unpack` validates, in a fail-closed order, container framing, the package
manifest and version dispatch, the canonical ZIP profile and entry set,
CRC/sizes, the payload root, the inner `.roproj` manifest agreement, exact
canonical `.roproj/v1` representation, and applicable semantic validity
before atomic publication. It requires an absent destination, strips only
`payload/`, copies no package metadata into `.roproj`, refuses merge/overwrite,
and leaves no partial destination on failure.

For every accepted canonical `.roproj/v1` tree `P`:

```text
unpack(pack(P)) == P
```

Equality means identical relative paths and bytes. For every accepted package
v1 artifact `R`:

```text
pack(unpack(R)) == R
```

Equality means byte-identical package bytes. Host paths, root basenames,
timestamps, permissions, ownership, ACLs, extended attributes, symlinks, Git
history, caches, and unknown files are outside canonical `.roproj/v1` state
and are not preserved.

### 6. Tracked-source conflict authority

When comparing a verified package with a canonical tracked `.roproj`, the
tracked tree's root is calculated by the same algorithm. Equal roots mean
`consistent` and authorize no mutation. Different roots mean an explicit
package/source mismatch; the tracked `.roproj` remains authoritative in that
working context.

Neither side is overwritten, synchronized, merged, or regenerated. A
differing package may be unpacked only to a distinct absent destination.
Timestamps, filenames, sizes, tool versions, `DocumentId`, filesystem order,
and most-recently-opened state never choose a winner. Git automation, semantic
delta, and merge remain owned by #44, #45, and #46.

### 7. Existing direct `.ro` JSON and content framing

This decision does not modify `legacy-direct-ro/v1` or `direct-ro/v2` and does
not make packaged `.ro` the current CLI writer.

A future reader supporting direct JSON and packaged `.ro` classifies by
content framing rather than extension. Exact leading bytes `50 4b 03 04`
select packaged-container handling because package v1 forbids a prepended
stub. Existing direct-JSON framing selects the direct reader separately. Once
classified, malformed input fails in that representation and never falls back
to the other parser.

### 8. Stable rejection meanings, replaceable APIs

Implementations preserve machine-distinguishable failure meanings for invalid
container framing, invalid or unsupported package manifests, noncanonical ZIP
metadata/order, entry-set mismatch, CRC/size mismatch, payload-root mismatch,
inner-manifest mismatch, noncanonical or semantically invalid payload,
noncanonical pack source, ZIP32 capacity, existing destination, publication
failure, and package/tracked-source mismatch.

The normative specification assigns stable symbolic meanings for conformance.
Concrete Rust modules, public APIs, error enums, diagnostic prose, and total
precedence among unrelated defects remain Provisional implementation details.

## Compatibility and preserved authority

This ADR consumes rather than amends ADR-0003, ADR-0015, ADR-0017, or
ADR-0023. In particular:

- the semantic `Document` remains meaning authority;
- `.roproj/v1` remains the exact 18-file source materialization;
- paths and package prefixes never become semantic identity;
- the package has an independent version namespace;
- direct JSON profiles remain unchanged; and
- production `.roproj` and package codecs remain separate implementation
  work.

It does not change Issue #27 or Semantic API, AI, approval, capability,
runtime-mutation, delta, merge, or revision authority.

## Deferred decisions

This ADR does not decide:

- a production `.roproj` reader/writer/canonicalizer;
- a production packaged-`.ro` reader/writer or CLI pack/unpack command (#3);
- compression, ZIP64, alternate containers, or package extension areas;
- signatures, trust, authentication, or malicious-tamper resistance (#53);
- encryption or key management;
- hostile-container resource limits and a complete adversarial corpus (#52);
- cloud distribution;
- Git/CI consistency automation (#44);
- semantic delta or merge (#45 and #46);
- AI, SemanticPatch, approval, or unrelated mutation authority; or
- exact temporary-file, rename, `fsync`, locking, browser-transaction, ABI,
  buffering, or streaming mechanisms.

## Consequences

Positive:

- `.ro` can be implemented as a deterministic thin package without a second
  semantic schema.
- Exact `.roproj/v1` bytes, including empty shards, survive both round trips.
- Independent implementations have one fixed ZIP32 byte profile and golden
  vector rather than library-dependent output.
- CRC-32 plus the payload root distinguishes ordinary corruption from exact
  payload equality while making no false authentication claim.
- Package/tracked-source disagreement fails explicitly without weakening Git
  source authority.

Costs:

- Strict canonical ZIP validation rejects otherwise-readable generic ZIP
  archives.
- Store-only v1 trades package size for byte simplicity and determinism.
- Inputs outside ordinary ZIP32 capacity require a future profile rather than
  an automatic upgrade.
- Implementations must validate the inner canonical representation and
  semantics, not merely extract filenames.

## Evidence and related work

The [Issue #43 evidence record](../research/2026-08-26-portable-package-v1.md)
documents the standards mapping, golden vector, external ZIP-tool check, and
all required pressure tests. The disposable Node probe manually writes and
parses ZIP32 records without a ZIP dependency, then checks determinism,
corruption paths, fail-closed dispatch, exact round trips, atomic publication,
tracked-source mismatch, content framing, and all zero-byte shards.

## Related

- ADR-0003
- ADR-0015
- ADR-0017
- ADR-0023
- [Issue #43](https://github.com/nurockplayer/tachiko-work/issues/43)
- [Issue #3](https://github.com/nurockplayer/tachiko-work/issues/3)
- [Portable package v1 specification](../specs/portable-package-v1.md)
