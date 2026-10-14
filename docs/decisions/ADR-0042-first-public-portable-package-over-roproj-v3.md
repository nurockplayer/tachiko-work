# ADR-0042: First-public portable package over canonical .roproj/v3

## Status

Accepted

Decision issue: [#488](https://github.com/nurockplayer/tachiko-work/issues/488)

Authority: [Founder decision, comment 6126009165](https://github.com/nurockplayer/tachiko-work/issues/488#issuecomment-6126009165)

Supersedes: [ADR-0025](ADR-0025-portable-package-v1.md)'s developmental
package/v1-over-roproj/v1 payload binding, including its requirement to choose
another package version for this pre-release replacement. ADR-0037 §4 and
ADR-0041's package forward-references are superseded only on that binding.
Unchanged container, integrity construction, and no-overwrite laws are adopted
below and restated in the current specification.

Specified by: [Portable package v1](../specs/portable-package-v1.md)

Related authority: [ADR-0003](ADR-0003-ro-and-roproj-representation.md),
[ADR-0017](ADR-0017-versioned-storage-and-canonical-representation.md),
[ADR-0023](ADR-0023-roproj-v1-canonical-tree-and-sharding.md),
[ADR-0037](ADR-0037-roproj-v2-keyed-grouped-sum-persistence.md), and
[ADR-0041](ADR-0041-bounded-durable-field-constraints.md)

## Context

### Pre-release payload amendment — ADR-0043

[ADR-0043](ADR-0043-native-display-names-separate-from-keys.md) explicitly
supersedes the unreleased v3 Schema/FieldDefinition shape bound by this package
profile: optional `display_name`, omitted when absent and never null. The
current package/v1 strict reader/writer uses that revised v3 definition; no v4,
new package profile, dual reader/writer, or down-conversion is selected. Older
binaries refuse name-bearing packages. The nineteen payload paths, twenty ZIP
entries, manifest, hash domains/algorithm, container rules, bounds, and CORE
attachment disclosure remain unchanged. Named payload bytes change their leaf,
root, CRCs, and ZIP bytes. Existing valid nameless current-profile vectors must
stay byte-identical; #529's separate Guarded implementation must add an
independent named fixture. ADR-0042's retirement of developmental v1-over-v1
vectors is unchanged. The sections below are read subject to this amendment.

Tachiko has not released a product and has no users. ADR-0025's package/v1
binding to `.roproj/v1` was never a released product contract. Maintaining two
package versions solely for pre-release history would preserve obsolete
representation rather than user data. The founder's binding comment replaces
comments [6125233113](https://github.com/nurockplayer/tachiko-work/issues/488#issuecomment-6125233113)
and [6122958814](https://github.com/nurockplayer/tachiko-work/issues/488#issuecomment-6122958814)
where they conflict. This ADR promotes that compatibility direction into
Accepted package authority; it does not claim implementation completion.

Canonical `.roproj/v3` already represents Date, saved keyed grouped-sum
definitions, and bounded durable field constraints. One current canonical
representation avoids a lowest-version selector, implicit down-conversion,
and duplicate package machinery without inventing another semantic schema.

## Decision

### 1. First-public profile and exact payload

Keep `tachiko.portable-package/v1` as the first PUBLIC v1, redefined to carry
exactly one storage-validated canonical `tachiko.roproj/v3` tree. The
[v3 layout](../specs/roproj-layout-v3.md) and `ROPROJ_V3_PATHS` in
`crates/storage/src/roproj/v3.rs` both specify 19 payload files:
`manifest.json`, `schemas.json`, `definitions.json`, and sixteen fixed entity
shards. With `package.json`, the ZIP has exactly **20 regular-file entries**.
Payload bytes and meaning remain owned by [v3 DTO authority](../specs/roproj-format-v3.md).
There are no new semantic DTOs, extension entries, or registry.

### 2. Reuse the container and integrity primitives

Adopt ADR-0025 §3's exact store-only ZIP32 subset, CRC rules, manifest-first
layout, local/central ordering and agreement, and ADR-0025 §4's path-separated
SHA-256 leaf/root construction. Use the v3 path list with `definitions.json`
after `schemas.json`; retain shard order 0–f. The package manifest retains its
five members/order, with `payload_format_version: 3`; the ZIP entry count is
20 and the hash domain's payload label is `tachiko.roproj/v3`, with nineteen
leaves. No compression, signing, encryption, trust semantics, or alternate
container is introduced. The current spec restates all byte rules, so no
historical v1 payload claim must be inferred as current.

### 3. One writer and one strict reader

The writer packages only exact storage-validated canonical v3 state. There is
no lowest-representable-version selection, implicit down-conversion/migration,
or dual writer. An older pre-release project must first use existing explicit
Accepted migration paths to v3; this ADR adds none.

The single package reader uses strict manifest-first dispatch, fails closed on
other/unknown profiles or payload versions (including developmental roproj/v1
packages), unknown/missing/duplicate entries, CRC/integrity mismatch, ZIP
features outside the subset, noncanonical or invalid payloads, and bounded-size
violations. Reuse existing bounds. Pack→unpack→fresh reopen preserves the exact
canonical v3 tree and admitted meaning; pack→unpack→pack is byte-identical.
Preparation and atomic no-replace publication prevent partial success or
silent overwrite, including a destination created in a publication race.
Tracked-source disagreement remains explicit and never selects a winner by
filename, timestamp, or tool version.

### 4. Retire developmental packages without legacy machinery

There is no legacy package reader/writer, old-golden-vector preservation, or
data-migration obligation for pre-release v1-over-roproj/v1 artifacts. The
implementation PR may remove or replace their code/tests/fixtures and MUST add
a new byte-level golden vector for the redefined profile. Historical research,
probe, and fixture records remain in Git as Superseded evidence, not active
conformance authority. This docs-only PR marks that evidence and deletes none.
Existing editable-directory migration authority is unchanged.

### 5. CORE package, honest attachments, separate consumers

This is a CORE package. Sheet client-held import source bytes, import
metadata/fidelity ledger, and digest-bound report settings are not carried.
Any product download must disclose actually omitted attachments before consent
as explicit `lossy_on_export` findings, following
[#513](https://github.com/nurockplayer/tachiko-work/issues/513); absent
attachments must not generate invented loss. A core-package round trip alone
does not establish #488's complete user-owned workbook exit. Work and Sheet
consumer qualification remain separate.

`direct-ro/v2` stays Provisional and is not a public file format. This decision
does not promote it, alter private host envelopes, or authorize new conversion
paths, public DTOs, runtime semantics, CI, or production implementation.

## Consequences and rejected alternatives

One package profile reuses mature ZIP32/CRC/SHA-256 primitives and canonical
storage authority. It deliberately rejects developmental packages and generic
ZIP variations. Users of older pre-release project representations must select
an existing explicit migration before packaging.

Public package/v2 solely to retain unreleased v1, dual readers/writers,
lowest-version selection, automatic down-conversion, and elaborate migration
machinery are rejected. Compression, signing, encryption, extension registries,
and trust remain outside this decision. Future incompatible public changes
require explicit versioned authority.

## Implementation and validation boundary

Authority only: no code, test, script, or CI change. Implementation follows in
a separately qualified Guarded PR under #488. Acceptance here is documentary
consistency, one-hop links, exact entry-count/order agreement, and diff checks;
executable codec acceptance and the new golden vector belong to implementation.
Independent review and protected integration gates remain required. #516's CI
repair and Sheet consumer work retain their separate ownership.
