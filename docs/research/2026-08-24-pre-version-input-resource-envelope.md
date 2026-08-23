# Pre-version direct-JSON resource-envelope evidence

Status: **Evidence and decision recommendation for Issue #96 — not Accepted authority**

Repository base: `main@16289f8a5acd48ca7fa36b265b7fdfe7df0e4d12`

Authority reviewed:

- ADR-0017, ADR-0018, and ADR-0019;
- `storage-versioning-and-migration.md`;
- `canonical-json-profile.md`;
- immutable `ro-format-v1.md` compatibility rules;
- Issue/PR evidence from #40, #74, and #92; and
- the current strict JSON, version dispatch, resource-limit, native/WASM, and adversarial implementations and tests.

This record does not amend an ADR or change production behavior. It identifies
the point at which a finite pre-version admission mechanism would change an
Accepted failure order, supplies reproducible hostile-input evidence, and
recommends the smallest decision needed before implementation.

## Executive finding

The current reader has no finite representation-global admission boundary
before it trusts `format_version`.

The only complete-input bound is the Provisional 8 MiB `direct-ro/v2` profile
limit. The reader applies it after UTF-8 validation, two complete strict JSON
passes, recursive duplicate inspection, and successful selection of version 2.
Legacy v1, missing/malformed-version input, and unsupported future-version
input never receive that limit.

Accepted authority does not uniquely determine the fix. ADR-0017 and the
storage specification currently place UTF-8, complete JSON syntax, and
recursive duplicate rejection before version dispatch. A byte admission check
that prevents those scans must make `ResourceLimit` win for an oversized input
that also contains invalid UTF-8, malformed JSON, duplicates, or a version
failure. Keeping the current precedence requires inspecting the complete
unbounded input and therefore does not close the resource-admission gap.

Production implementation must wait for an explicit precedence decision.

## Current implementation and scan surfaces

`tachiko_storage::from_bytes` currently follows this path:

```text
borrowed bytes
  -> full UTF-8 validation
  -> strict JSON syntax pass over the complete input
  -> strict duplicate/version pass over the complete input
  -> version classification
  -> v1: repeat the complete strict envelope inspection, then DTO decode/migrate
  -> v2: apply 8 MiB input and 256-byte number-token limits, then DTO decode
  -> unsupported: return without DTO/semantic-body decode
```

Consequences:

- malformed JSON performs one syntax pass up to the failure; the end-truncated
  probe below makes that pass traverse the complete input;
- admitted missing/malformed/unsupported inputs perform two complete JSON
  passes;
- admitted v1 performs two envelope inspections (four strict JSON passes) plus
  its DTO decode;
- decoded object names are allocated as `String` values and cloned into a
  `HashSet` for duplicate detection, so long names and high member counts
  amplify memory;
- unsupported bodies are never semantically decoded, migrated, canonicalized,
  or rewritten, but they are structurally scanned at every depth;
- v2 checks total input length only after the strict passes; if admitted, its
  number-token scan is another complete byte pass before DTO allocation;
- `serde_json`'s current recursion guard rejects deeply nested JSON as
  `InvalidJson`; the adversarial corpus observed no unwind, but the guard is an
  implementation mechanism rather than an explicit direct-JSON envelope
  contract; and
- total string length, number-token length, and object member count remain
  unbounded before version selection.

## Current exact failure precedence

The current observable order is:

| Rank | Result | Trigger and scope |
| ---: | --- | --- |
| 1 | `InvalidUtf8` | Any invalid byte wins before JSON inspection. |
| 2 | `InvalidJson` | For valid UTF-8, complete syntax validation wins even when a duplicate appeared earlier in the source. |
| 3 | `DuplicateMember` | For syntactically valid JSON, a decoded-name duplicate at any depth wins before version classification. |
| 4a | `VersionMissing` | No root `format_version`; mutually exclusive with malformed version. |
| 4b | `VersionMalformed` | A present root version is not a lexical positive `u32`; mutually exclusive with missing version. |
| 5 | `UnsupportedVersion` | Valid, duplicate-free envelope identifies neither v1 nor v2. No body DTO decode occurs. |
| 6 | `ResourceLimit(input)` | Only selected v2; 8 MiB is admitted and 8 MiB + 1 byte is rejected. |
| 7 | `ResourceLimit(number token)` | Only selected v2 whose total input was admitted; 256 bytes is admitted and 257 is rejected. |

`VersionMissing` and `VersionMalformed` share one stage rather than overriding
one another. Within v2 resource admission, the complete-input limit wins over
the number-token limit.

There is no `ResourceLimit` rank for v1 or unsupported input. For example, the
fixed corpus admits a valid 8 MiB + 1-byte v1 input and returns
`UnsupportedVersion` for an equally large future body.

## Attack matrix

The checked-in [Rust probe](probes/issue-96-pre-version-envelope.rs) calls the
production `tachiko-storage` reader. Its [Node harness](probes/issue-96-pre-version-envelope.mjs)
executes the same fixed result-class corpus from a `wasm32-unknown-unknown`
build.

| Hostile class | Constructed evidence | Current observation | Resource/panic observation |
| --- | --- | --- | --- |
| Oversized valid v1 | 32 MiB title string | `Ok` after migration | 68,845,568–69,074,944 peak RSS bytes across three runs; no panic |
| Oversized valid v2 | 32 MiB valid document | `ResourceLimit(input)` | 35,192,832–35,274,752 peak RSS bytes; limit is reached only after strict inspection |
| Oversized malformed JSON | 32 MiB, unterminated final string | `InvalidJson` | Complete syntax scan; 35,094,528 peak RSS bytes; no panic |
| Oversized duplicate | 32 MiB, nested `"a"` plus `"\u0061"` | `DuplicateMember` | Recursive decoded-name rule preserved; 35,127,296 peak RSS bytes across three runs |
| Duplicate plus later malformed JSON | 32 MiB | `InvalidJson` | Confirms syntax-over-duplicate precedence |
| Oversized missing version | 32 MiB valid JSON string body | `VersionMissing` | Complete strict inspection before classification |
| Oversized malformed version | 32 MiB with `"format_version":"2"` | `VersionMalformed` | Complete strict inspection before classification |
| Oversized unsupported version | 32 MiB future string body | `UnsupportedVersion` | 35,045,376–35,209,216 peak RSS bytes; no DTO/semantic decode |
| Invalid UTF-8 | Invalid byte at end of 32 MiB input | `InvalidUtf8` | Full UTF-8 scan; 34,947,072 peak RSS bytes |
| Huge member name | One 32 MiB unsupported-body member name | `UnsupportedVersion` | 102,187,008–102,268,928 peak RSS bytes (about 3.05× source size) |
| Huge member count | 500,000 unique unsupported-body members, 11,000,031 bytes | `UnsupportedVersion` | 83,197,952–89,669,632 peak RSS bytes; 201–267 ms reader time |
| Near-v2-size member count | 381,000 members, 8,382,031 bytes | `UnsupportedVersion` | 56,868,864–56,934,400 peak RSS bytes; finite only because the probe selected a finite source size |
| Deep nesting | 10,000 array levels | `InvalidJson` | Current recursion guard rejects without panic |
| Huge v2 number token | 1 MiB token | `ResourceLimit(number token)` | Strict JSON passes precede the token limit |
| Huge unsupported number token | 1 MiB token | `UnsupportedVersion` | v2 token profile correctly does not interpret a future body, but no pre-version token bound applies |

RSS includes the source `Vec` built by the probe and is evidence for this
environment, not a product SLA. The important observations are the absence of
a pre-version bound, the stable result classes, and allocation amplification
from member-name/member-count inspection.

The existing #92 adversarial test independently executes 20,000 structured
mutations plus nesting cases under `catch_unwind`. The Issue #96 probe adds
large pre-version inputs, exact byte boundaries, unsupported bodies, and
native/WASM error-class comparison. Neither corpus observed a panic.

## Native/WASM observable parity

The fixed corpus emits `index|class|input_bytes`. Native and WASM output matched
exactly:

```text
0|0|70
1|0|70
2|2|8388609
3|3|8388609
4|4|8388609
5|5|8388609
6|6|8388609
7|1|8388609
8|0|8388609
9|0|8388608
10|7|8388609
11|8|287
12|6|287
13|2|8388609
14|2|2079
```

The class mapping is `0 Ok`, `1 InvalidUtf8`, `2 InvalidJson`,
`3 DuplicateMember`, `4 VersionMissing`, `5 VersionMalformed`,
`6 UnsupportedVersion`, `7 ResourceLimit(input)`, and
`8 ResourceLimit(number token)`.

This parity is current behavior evidence. It does not choose the future
pre-envelope precedence.

## Alternatives considered

| Alternative | Resource property | Precedence/compatibility consequence | Verdict |
| --- | --- | --- | --- |
| Version-specific limits only | v2 DTO/numeric conversion is bounded; pre-version, v1, and future input remain unbounded | Preserves the current table | Reject: this is the demonstrated gap |
| Pre-parse global byte admission | O(1) length rejection before any scan or allocation amplification | `ResourceLimit` must beat every other error for oversized input; v1 acceptance changes | Safe and minimal only after an Accepted precedence amendment |
| Bounded/streaming envelope inspection | Can avoid buffering in a future host adapter, but the current `&[u8]` API already receives complete bytes | A trusted early version is impossible while member order is flexible and a later duplicate/syntax error must win; recursive duplicate proof still needs bounded key state or new structural budgets | Not sufficient by itself; host streaming belongs to #26 |
| Version-independent envelope within the direct-JSON representation | Bounds v1, v2, missing/malformed, and unsupported inputs in one namespace | Same precedence decision as a global limit; must explicitly constrain legacy v1 reader admission | Recommended scope |
| Hybrid two-stage admission | Stage 0 bounds the complete direct-JSON envelope; admitted input then uses current strict pipeline and version-specific token/DTO limits | Introduces one explicit over-limit precedence while preserving all current under-limit results | Recommended contract |

A streaming probe cannot safely stop after the first `format_version`: a later
root member may duplicate it, an unsupported body may contain a decoded-name
duplicate, or the final byte may make the complete JSON invalid. Preserving the
current syntax/duplicate precedence therefore requires reaching the end. New
depth, member-count, key, or token budgets would themselves require a decision
about when `ResourceLimit` wins.

## Recommended contract requiring acceptance

Adopt a version-independent, two-stage envelope within the direct-JSON
representation:

1. Before UTF-8 validation or any complete-input scan, compare `source.len()`
   with a finite direct-JSON envelope byte limit.
2. If over the envelope, return representation-local `ResourceLimit` without
   inspecting syntax, duplicates, version, or body.
3. If admitted, preserve the existing order:
   `InvalidUtf8` → `InvalidJson` → `DuplicateMember` →
   `VersionMissing`/`VersionMalformed` → `UnsupportedVersion`.
4. After supported-version selection, retain version/profile-specific limits;
   v2's number-token limit remains after its complete-input admission.
5. Never DTO-decode, semantically decode, migrate, canonicalize, rewrite, or
   mutate an unsupported body.
6. Keep storage failures in `FormatError`; do not project this admission result
   into ADR-0019 semantic diagnostics.

Recommended resulting precedence:

| Rank | Admitted state | Result |
| ---: | --- | --- |
| 1 | `source.len()` is over the direct-JSON envelope | `ResourceLimit(direct JSON input)`; wins over all other latent defects |
| 2 | Envelope admitted | `InvalidUtf8` |
| 3 | Envelope admitted and UTF-8 valid | `InvalidJson` |
| 4 | Envelope admitted and JSON valid | `DuplicateMember` |
| 5a | Strict envelope valid | `VersionMissing` |
| 5b | Strict envelope valid | `VersionMalformed` |
| 6 | Strict version token valid but unsupported | `UnsupportedVersion` |
| 7 | Supported profile selected | Version/profile `ResourceLimit`, including v2 number-token admission |

The smallest Milestone 02 mechanism is to promote the existing 8 MiB value
from a v2-only input limit to a Provisional direct-JSON envelope constant while
leaving the 256-byte number-token limit v2-specific. The 8 MiB value must remain
clearly classified as a replaceable representation/profile mechanism, not a
Number, identity, migration, or ecosystem semantic invariant.

That concrete reuse is a recommendation, not current authority. It changes
historical v1 reader admission, so the owner may instead select another finite
direct-JSON value. Implementation should not choose between those compatibility
policies implicitly.

## Minimal amendment text

The decision can be made without reopening identity, migration, canonical JSON,
Number, diagnostics, or host durability. The minimum ADR-0017 amendment is:

> A direct-JSON reader MUST apply a finite representation-envelope byte
> admission check before UTF-8 validation or any complete-input structural
> scan. An over-limit envelope fails with a representation-local resource-limit
> result and takes precedence over latent UTF-8, JSON, duplicate-member, or
> version failures. Admitted inputs retain the existing strict reader order.
> Concrete byte and subordinate structural/token limits are versioned
> representation/profile mechanisms, not semantic-model invariants.

The storage specification should then place `direct-JSON envelope admission`
before UTF-8 in the reader diagram, add the resulting precedence table above,
and state that the envelope covers legacy v1, v2, missing/malformed versions,
and unsupported future versions received by the same direct-JSON entry point.

## Regression plan after acceptance

The first production change should be an exact failing regression proving the
selected over-limit precedence. Then add:

- exact direct-JSON envelope boundary and one byte over;
- oversized otherwise-valid v1 and v2;
- oversized invalid UTF-8, malformed JSON, recursive/escaped-equivalent
  duplicates, missing/malformed version, and unsupported future version;
- under-limit equivalents proving the existing strict order is unchanged;
- hostile depth, huge strings, huge number tokens, huge member names, and high
  member counts;
- proof that unsupported bodies never reach a supported DTO/semantic decoder;
- no-panic mutation/depth cases; and
- the same result-class records under native and `wasm32-unknown-unknown`.

No semantic decode, migration, identity, canonicalization, Number, ADR-0019
diagnostic, filesystem, mmap, IndexedDB, IPC, or persistence-transaction change
belongs in that implementation.

## Reproduction environment and commands

Recorded environment:

```text
macOS 15.7.4 (24G517), arm64, Mac14,10, 16 GiB
rustc 1.97.1 (8bab26f4f 2026-07-14)
Node.js v24.15.0
```

Build and run the native probe against the production storage crate:

```bash
cargo build --release --package tachiko-storage
storage_rlib="$(ls -t target/release/deps/libtachiko_storage-*.rlib | head -n 1)"
rustc --edition=2024 -C opt-level=3 \
  -L dependency=target/release/deps \
  --extern "tachiko_storage=${storage_rlib}" \
  docs/research/probes/issue-96-pre-version-envelope.rs \
  -o target/issue-96-pre-version-envelope-native

target/issue-96-pre-version-envelope-native
/usr/bin/time -l target/issue-96-pre-version-envelope-native \
  huge-member-name 33554432
/usr/bin/time -l target/issue-96-pre-version-envelope-native \
  many-members 500000
```

The recorded matrix used the following complete measurement set. The first
block captured every hostile class once; the second captured the reported
three-run ranges for the amplification-sensitive cases.

```bash
measure_issue96() {
  /usr/bin/time -l target/issue-96-pre-version-envelope-native \
    "$1" "$2" 2>&1 | \
    awk '/^case=| real |maximum resident set size/'
}

measure_issue96 valid-v2 70
measure_issue96 valid-v1 33554432
measure_issue96 valid-v2 33554432
measure_issue96 malformed-json 33554432
measure_issue96 duplicate 33554432
measure_issue96 duplicate-then-malformed 33554432
measure_issue96 missing-version 33554432
measure_issue96 malformed-version 33554432
measure_issue96 unsupported-version 33554432
measure_issue96 invalid-utf8 33554432
measure_issue96 huge-member-name 33554432
measure_issue96 deep 10000
measure_issue96 v2-number 1048576
measure_issue96 unsupported-number 1048576
measure_issue96 many-members 500000

for measurement_round in 1 2 3; do
  measure_issue96 valid-v1 33554432
  measure_issue96 valid-v2 33554432
  measure_issue96 unsupported-version 33554432
  measure_issue96 huge-member-name 33554432
  measure_issue96 many-members 500000
  measure_issue96 many-members 381000
done
```

Build and execute the WASM probe, then compare it with native output:

```bash
cargo build --target wasm32-unknown-unknown --package tachiko-storage
wasm_storage_rlib="$(ls -t \
  target/wasm32-unknown-unknown/debug/deps/libtachiko_storage-*.rlib | head -n 1)"
rustc --edition=2024 --target wasm32-unknown-unknown --crate-type cdylib \
  -C opt-level=2 \
  -L dependency=target/wasm32-unknown-unknown/debug/deps \
  -L dependency=target/debug/deps \
  --extern "tachiko_storage=${wasm_storage_rlib}" \
  docs/research/probes/issue-96-pre-version-envelope.rs \
  -o target/issue-96-pre-version-envelope.wasm

diff -u \
  <(target/issue-96-pre-version-envelope-native) \
  <(node docs/research/probes/issue-96-pre-version-envelope.mjs \
    target/issue-96-pre-version-envelope.wasm)
```

The clean evidence branch also passed `bash scripts/release-check.sh`: format,
workspace dependency policy, warning-denied Clippy, 229 tests, production
native/WASM conformance, warning-denied rustdoc, Rust 1.85, Cargo packages, four
product smoke journeys, and release-archive safety/concurrency checks.
