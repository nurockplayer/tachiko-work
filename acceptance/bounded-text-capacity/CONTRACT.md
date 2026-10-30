<!-- work505-bounded-profile-amendment:20261011 -->
# Work #505: bounded rectangular Text and Number profile

Decision source: [Work #505](https://github.com/nurockplayer/tachiko-work/issues/505),
with Sol disposition after focused Astra consultation. This amendment becomes
Accepted when its authority PR merges. It authorizes acceptance preparation for
a separate implementation child; it does not claim implementation, qualification,
a new Sheet kit pin, or deployment. The Work #495 material below remains the
original three-Text acceptance record where this amendment does not supersede it.

## Supported extension and preserved contracts

One headered, rectangular worksheet/table may contain 1–16 columns and at most
8,406 data rows, with **rows × columns at most 25,218 logical cells**. Missing
values count toward that product. Fields use existing Text or Number semantics
and the existing missing-value representation. Number values must be finite and
type-valid; Number parsing, canonical representation and inference do not change.
Keep plain/default styles, no column widths, complete ordered metadata, and the
existing refusal of lossy or safe-disabled capacity source findings.

Formulas, references, Date, Boolean, constraints, saved calculation definitions
and capacity Dedup remain outside this extension. Existing ordinary and native
Tracker behavior is preserved. No persisted flag, format version, public API,
discriminator, storage contract or migration is introduced. If implementation
requires one, return that seam for an explicit durable decision before mutation.

Import and type confirmation, scalar edit, history, save, complete process
termination, fresh reopen, and CSV/XLSX export must land as one complete profile.
Every cell, stable identity, type and source order must survive. CSV verification
compares canonical textual values; typed XLSX and saved-project verification also
compare field types. CSV itself is not a typed storage carrier. Formula-leading
Text still refuses; legitimate negative Numbers do not become unsafe Text.

## Capacity eligibility is separate from structural validity

Keep ordinary admission first. An ordinary failure may retry with the enlarged
capacity budget only for the existing eligible three-Text shape, or for a valid
new Text/Number shape with **more than 64 data rows**. Since Work #610 that
shape may also hold Date (`YYYY-MM-DD`, 1900-01-01 through 9999-12-31) and
Boolean columns, written to XLSX as `t="d"` with one fixed `yyyy-mm-dd` cell
format and `t="b"`; the row, column, cell and byte limits are unchanged. V3 remains excluded from
enlarged retry. The previous resident's eligibility cannot authorize a new
candidate or replacement.

Small legacy three-Text capacity continues to work. Small new wide-Text or mixed
documents keep ordinary limits; in particular the existing Number/Text/Text
complete saved-open reply boundary remains 65,536 accepted / 65,537 refused.
Header-only metadata validation checks structure and encoding without requiring
more than 64 rows; it cannot itself authorize the enlarged response budget.

Validate the complete resulting candidate before publication, including history
replay. Imported row operations keep their existing restrictions. If an otherwise
editable native capacity candidate changes from 65 to 64 rows and is not eligible
under the legacy three-Text rule, refuse atomically. This slice does not add an
automatic demotion that assumes ordinary complete-wire closure from row count.

## Finite budgets and encoded closure

All existing byte, identity, metadata, projection, transfer, ZIP, XML-depth and
ordinary request defenses below remain. In particular: Text value 4,096 UTF-8
bytes; aggregate stored Text 1 MiB; source/encoded CSV 2 MiB; expanded XLSX 8 MiB;
ZIP 256 entries; XML 100,000 nodes/depth 64. Ceilings compose; none promises that
every Cartesian combination fits. Refusal precedes resident installation or
publication, and leaves revisions, both histories and saved/exported bytes intact.

For `c` columns reserve **H(c) = max(39, 13 × c)** bytes for the complete encoded
CSV header. Require actual escaped headers, separators and CRLF to fit H(c), and
actual encoded data body plus H(c) to fit 2 MiB. Derive the same reserve after
metadata-free reopen. This preserves the existing three-column 39-byte boundary
and its near-limit projects. Count each row's actual separators/CRLF and the
existing canonical Number output. Do not reuse a three-column `+4` calculation.

The worksheet XML-node upper bound is
`2 + (rows + 1) × (1 + 3 × columns)`, subject to the unchanged 100,000 ceiling.
Independently check the actual escaped worksheet, all auxiliary members, ZIP
framing, expanded size, compressed source size and fresh-reader limits. The old
three-header 16 KiB auxiliary allowance alone is not evidence for the new shape.
Existing unsafe Text, invalid XML and conservative OOXML escape refusals remain.

## Required acceptance before the implementation child is Ready

- Freeze the executable seed and case map on exact main, with actual native and
  raw-WASM baseline refusal for synthetic `320 × 2` Text/Number confirmation.
  Controls must reach the boundary using valid sources and existing ordinary
  imports; a setup or compilation failure is not behavioral RED.
- Qualify that case and a wide mixed case through native and actual browser/module
  Worker lifecycle: independent all-cell manifests, typed edits, Undo/Redo,
  both existing save APIs, process termination/fresh reopen, and both exports.
- Preserve the original `8,406 × 3` Text journey and exact 39-byte header / near-
  2 MiB closure controls. Cover zero, negative, fraction and exponent Numbers,
  missing values, leading-zero Text, UTF-8, quotes, LF and embedded CRLF.
- Cover 16/17 columns, 8,406/8,407 rows and the cell-product boundary, including
  `1,576 × 16` accepted and `1,577 × 16` refused. Cover selected encoded-byte
  limit/+1 and publication refusals with full resident/history/export preservation.
- Retain ordinary Number/Text/Text complete-wire 65,536/65,537 across existing
  saved carriers and actual WASM, mixed 64-row oversize refusal / eligible 65-row
  admission, small legacy capacity success, header-only metadata validation,
  imported row-operation refusal, and native 65→64 candidate/history refusal.
  Capacity residents must not grant ordinary replacements or V3 enlarged retry.
- Keep malformed/scalar-carrier, request-arena, selective-response, stale revision
  and saved-definition controls. Run applicable exact-head regression/release/CI
  gates and one fresh independent final Oracle review before protected merge.

The implementation child owns a single Guarded vertical PR because admitting a
source before edit/save/reopen/export closure creates an unsafe partial product.
Apply decomposition and freeze its production/acceptance file and line estimate
before Ready; do not attach unrelated cleanup or another architecture campaign.
Use available qualified cloud execution with exact source, kit and runtime
receipts; record environment/timings, and treat crashes, hangs, OOM or material
real-use degradation as failures. No obsolete reference-machine procurement or
unavailable historical Mac packet is an admission gate for this new child.

Only a newly qualified immutable producer/kit handoff can reach Opus's separate
Sheet consumer lane. An older kit may fail closed on newly admitted workloads;
rollback must retain a reader capable of reopening those projects. Work producer
PASS does not claim Sheet UI, persistence, pin, deployment or consumer PASS.

---

<!-- work495-resume-acceptance-reconciliation:20261008 -->
## Retained-package reconciliation (acceptance proposal)

This source-only acceptance package is assembled against exact private serial
base811f5de3353d8f3628f9e93266cc2704b1323dc5/tree120666d31ff2d61b4affa0b409bbaddc86b7088f.
It preserves complete v3@589, engine@642, capacity@44 and public248 source units.
The retained nine-file v2 qualification amendment and current native/Worker/raw
supplements are combined, including independently reviewed native inspection
16MiB/+1 additions and explicit saved-open boundary `all` invocation (including
`atomic`8,407). No production Ready or behavioral qualification follows from
source assembly; absent saved-open implementation is an UNVERIFIED setup seam.

Preserve original pure-project raw OpenedProjection65,536/+1, spreadsheet
metadata-aware complete-wire65,536/+1 and capacity complete-wire16MiB/+1 as
separate guards. Preserve original v3 resource/fresh constructor/capability,
origin and refusal of metadata-aware capacity retry. Pure v3 export is not
forbidden merely because a runtime previously used capacity. No fixture or
threshold is changed to conceal the55-byte envelope distinction.

The complete ordinary envelope boundary is an explicit paired executable
obligation, not inferred from the pure-project or enlarged-capacity tests.
`capacity_saved_open_boundaries -- all NEW_FIXTURE_DIRECTORY` independently
constructs valid Number/Text/Text documents with full opened replies exactly
65,536 and65,537 bytes, and captures direct-v2, canonical-v1 and selected-v3
carriers. Both raw projections still fit original pure-project64KiB admission.
Native metadata-aware inspect/open must accept the exact limit and refuse the
one-byte excess with ProjectionTooLarge(65,537,65,536), preserving an empty
resident or every resident cell, occurrence/revision, export and both histories.
`ordinary-saved-open-wire.mjs` uses those exact carriers on the actual WASM
spreadsheet ABI. It checks actual UTF8 reply bytes, complete returned payload,
read-only inspection, replacement scope/revision and typed refusals before any
installation, with export-byte and complete Undo/Redo replay invariants. This
includes selected-v3 metadata-aware admission without widening its pure-project
raw-projection law. The runner requires all six raw carrier/size cases.

The old path/line scope below describes the original capacity unit. It is not a
current integration bound. Retain separately reviewable v3/engine/capacity units;
the manifest and owner case map measure cumulative acceptance growth and record
remaining implementation scope/decomposition before any Ready assignment.
Complete v3 native16/Worker2 capture, historical reader at518aaa55, compatibility
and release gates remain separate executable obligations; seal checks alone do
not run them. Sheet integration and its missing-capability/normal-entry/pin/
manifest/guard/rollback qualification belong to a separate fresh session.

# Work #489 / Sheet #150 bounded Text capacity

Status: acceptance preparation, not a Ready or qualification receipt.
Baseline: `33b26e2a8d34593030118c53dbe79b6f8fdc6e0e`.
Authority: Work #489 disposition `5910023192`, Sheet #150 and comments
`5972691967` / `5972908971`, and Work #374 capacity handoff `5972417514`.
The user's subsequent explicit approval covers necessary Work capacity and
engine implementation. Formula growth/reference semantics, #481, other HOLDs,
Sheet's producer pin and Date + saved-definition guard remain unchanged.

## Supported outcome

One headered worksheet/table, exactly three raw Text fields, up to 8,406 data
rows (25,218 logical cells), no formulas, reference/Date/Boolean/Number fields,
constraints or saved calculation definitions. Empty Text cells may follow the
existing missing-value representation; non-Text stored values do not qualify.
The enlarged profile uses plain/default cell styles and no column widths.
Capacity source admission also rejects every actual lossy or safe-disabled
source finding. XLSX styles must be absent or match the documented private
plain producer tree structurally; rich text and unrepresented presentation do
not qualify. This conservative rule applies at every row count, including
small imports that need the enlarged projection budget. Ordinary generic
admission retains its existing fidelity-ledger behavior.
Its complete row and column metadata follows canonical stable-ID order, matching
the imported source order. Reordered, missing, duplicate or stale mappings are
refused for this enlarged profile; generic mappings keep existing behavior.
Existing smaller generic and native Tracker profiles keep their own behavior.
The approximately thirteen-column review shape is characterization only.

The runtime determines eligibility from actual schema, values and complete
metadata. A filename, host flag or partial projection cannot grant eligibility.
Import, scalar edit, Undo/Redo, project save, fresh process reopen, and CSV/XLSX
export must preserve every logical cell, order, Text type and stable identity.
Unix timestamps remain exact Text. No private source data is permitted.

The synthetic fixture generator in this directory is a new reproducible
acceptance source, not a claim to possess either historical evidence bundle.
The authoritative pinned baseline already establishes eight-row CSV/XLSX
success, a 64-row CSV projection refusal (88,730 > 65,536 bytes), 65/8,406-row
refusals and ten atomic resident-preservation checks. Do not relabel missing
tools or unexecuted browser tests as behavioral failures.

## Finite budgets to qualify

| Boundary | Enlarged Text profile | Preserved boundary |
| --- | ---: | --- |
| Rows / fields / tables | 8,406 / 3 / 1 | Existing generic profiles otherwise |
| Single Text UTF-8 value | 4,096 bytes | Exact Text; no coercion |
| Aggregate stored Text | 1 MiB | Check encoded output closure too |
| Aggregate profile/identity strings | 4 MiB | Each identifier/key at most 256 bytes |
| Complete imported metadata | 3 MiB | Full mappings validated |
| Encoded CSV header row | 39 bytes | Actual CSV escaping, separators and CRLF |
| Complete relevant response/reply | 16 MiB | Ordinary field-query replies 64 KiB |
| Metadata-bearing spreadsheet request | 4 MiB | Export/InspectProject and authorized saved-open OpenProject |
| Ordinary semantic request | 64 KiB | Checked before JSON parsing, including raw ABI |
| Source CSV/XLSX | 2 MiB | Unchanged |
| Expanded XLSX / ZIP entries | 8 MiB / 256 | Unchanged |
| XML nodes / depth | 100,000 / 64 | Unchanged |
| Project transfer | 64 MiB | Storage readers' own limits unchanged |

These ceilings are independent checks, not a promise that every Cartesian
combination fits. Candidate admission and edits must additionally prove actual
save/reopen and encoded CSV/XLSX closure. Unsafe CSV formula-leading Text,
invalid XML characters and any combination whose output exceeds file defenses
are refused before publication in this enlarged both-formats profile. Existing
generic typed-XLSX-only behavior is not removed. Embedded CRLF must survive a
standards-compliant independent XML parser; literal XML line normalization is
not an acceptable fidelity loss.

The capacity profile conservatively excludes every seven-byte
`_x[0-9A-Fa-f]{4}_` sequence anywhere in decoded Text, a header or a sheet name.
Only lowercase `x` matches; hexadecimal letters may use either case. Literal
spellings, escaped-escape forms such as `_x005F_x0041_` and sequences produced
by XML character references are refused. This bounds OOXML ST_Xstring exposure
without introducing an escape codec. Nearby nonmatches and generic behavior
remain unchanged. #495 reconciliation `5977017424` governs this clarification.

The metadata-capable spreadsheet ABI caps its arena at 4 MiB before inspection.
Above 64 KiB, a byte-bounded object discriminator/JSON-skip pass permits only
the existing Export and InspectProject operations and the owner-authorized metadata-bearing OpenProject operation before typed payload decoding.
Non-metadata operations are refused at that stage. The skip pass is bounded by
arena bytes, not a claimed JSON depth limit; full operation decoding retains its
own parser bounds. Unordered object keys remain valid; duplicate discriminators,
malformed controls and positional arrays cannot grant the larger request budget.

CSV admission reserves 39 bytes for the complete header row and requires the
encoded data body plus that reserve to fit 2 MiB. Every metadata-bearing call
also checks its actual header row against 39 bytes. The fixture's header row is
exactly 39 bytes, so its 2 MiB and limit+1 cases are exact output boundaries.
Shorter-header files near the ceiling can be conservatively refused. The same
reserve survives fresh project reopen without retaining spreadsheet metadata or
changing storage/API contracts. XLSX closure independently includes escaped
headers, permitted sheet-name overhead, tags, ZIP members, expanded size and
fresh-reader limits; the CSV reserve alone does not prove XLSX closure.
The private capacity XLSX encoder may use fixed shared-string header indices,
with actual labels and sheet names isolated in bounded uncompressed ZIP entries.
Any metadata allowance must cover their complete escaped XML and ZIP framing;
surrogate compressed header bytes alone cannot prove the bound.

Ordinary semantic commands, formula limits, selective field-query target and
byte limits, occurrence/revision laws, authorization and canonical formats do
not change. A finite full table projection serves initial open/import and
explicit queries; scalar publication plus revision-pinned selective queries
must work without requiring a full-table response for each edit.

## Predeclared reference and performance gates

Reference: Linux 6.18.44 / Debian 13 x86_64, Intel Xeon Platinum 8573C,
cgroup CPU quota 4, memory limit 16 GiB; Node 24.19.0, pnpm 11.25.0,
Rust stable 1.99.0 with exact 1.85.0 compatibility checks. Browser measurements
use system Chromium 151.0.7922.173 with pinned Playwright 1.62.1 explicitly.
The Playwright-bundled Chromium download is blocked by HTTP 403; system-browser
evidence does not silently satisfy an unrun standard bundled-browser gate.

- Release native and actual WASM/module Worker import/open/reopen/save/export:
  each at most 10 seconds.
- Producer scalar edit/Undo/Redo including the selective authoritative response:
  p95 at most 500 ms and maximum at most 1 second.
- During Worker work: main-thread heartbeat p95 at most 100 ms and maximum
  250 ms. This is a responsiveness proxy, not a Sheet painted-input measurement.
- WASM linear memory high-water at most 256 MiB; attributable browser process
  tree RSS increase at most 512 MiB. Report peak sampling and limitations.
- Exercise 128 edits (beyond the 64-operation Undo horizon) and ten complete
  close/reopen cycles. Report memory samples/slope/plateau; WASM high-water is
  not expected to shrink on close. Do not reset revisions to pass a memory test.

Use disk-backed workspace TMP/cache, Cargo jobs=2, bounded browser/test workers
and the external process memory guard. Build/setup and measured behavior remain
separate evidence. A threshold miss is a failed gate, not a reason to increase
the declared budget after observing results.

## Acceptance and ownership boundaries

Executable native and Worker cases must compare every cell against an
independent expected manifest, including UTF-8, leading zeros, quotes, LF,
embedded CRLF and first/middle/final-row sentinels. Preserve target identities,
stale refusal, Undo/Redo and truthful revision. Save artifacts, terminate the
runtime process/browser, start a fresh one and compare all data again. Verify
CSV/XLSX with a parser independent of the producer, including complete counts,
row order and exact Text.

Cover selected limits and limit+1, hostile/malformed sources and the old
64/65, 128/129, 1,024/1,025 landmarks. Over-limit input must leave occurrence,
revision, complete resident data, history and export bytes intact. Bypass the
bridge to prove raw-ABI ordinary 65,537-byte refusal, metadata arena 4 MiB+1
refusal, and oversized non-metadata spreadsheet-operation refusal.

The acceptance seed, actual baseline execution, case map and independent
adequacy must be frozen before production Ready. Production follows on the
same isolated branch, with a sole writer and a fresh independent final reviewer.
Projected review unit: five production paths plus acceptance/unit tests and
documentation, originally at most 20 paths and approximately 4,000 meaningful
changed lines. Review-driven reconciliation in #495 comment 5976675101 retains
18 paths; follow-up reconciliation `5977408805` permits at most 5,150 meaningful
changed lines for complete profile propagation, source-fidelity, small-file
closure, conservative ST_Xstring refusal and independently proved Imported versus
Opened promotion through fresh Worker reopen. It supersedes the 4,900-line
reconciliation `5977017424` and interim 5,050-line `5977362115` without changing
workload or performance gates.
The Steward re-evaluated the 2,000–3,000-line signal before Ready: roughly 2,000
lines are executable acceptance spanning one inseparable capacity journey and
its independent oracle. Import, edit/history, save/reopen and both exports must
land atomically; separating parser allowance, transport allowance or publication
preflight would admit files that cannot complete the promised journey. One
Guarded child and one PR retain a single capacity-profile review model and one
rollback boundary. No unrelated v3/compatibility cleanup is included. Reconcile
growth beyond this estimate before continuing.

#493/#494 retains its separate original v3 branch and historical provenance. The founder transferred convergence ownership to Sol in6044007958; this acceptance package does not mutate either original branch. Historically, #493/#494 owned its separate v3 branch. Its live head was `589fe709` at intake;
older handoffs retain different heads. No mutation or release of that lane is
implied. Shared runtime integration is serialized and requires requalification;
capacity does not incorporate or relax the v3 guard. No automatic main merge,
deployment or Sheet pin change. Capacity files require a capacity-capable reader
on rollback; the old pin is not a compatible reader for the larger workload.

Producer evidence does not close Sheet #150. The exact exported kit must receive
separate Sheet normal-entry import/edit/save, host-persistence receipt, complete
browser termination/restart, reopen/export, rendering/focus, upgrade/rollback
and independent consumer qualification. No new architecture authority is created.
