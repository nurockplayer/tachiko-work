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
| Metadata-bearing spreadsheet request | 4 MiB | Only existing Export/InspectProject |
| Ordinary request | 64 KiB | Checked before parsing, including raw ABI |
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

CSV admission reserves 39 bytes for the complete header row and requires the
encoded data body plus that reserve to fit 2 MiB. Every metadata-bearing call
also checks its actual header row against 39 bytes. The fixture's header row is
exactly 39 bytes, so its 2 MiB and limit+1 cases are exact output boundaries.
Shorter-header files near the ceiling can be conservatively refused. The same
reserve survives fresh project reopen without retaining spreadsheet metadata or
changing storage/API contracts. XLSX closure independently includes escaped
headers, permitted sheet-name overhead, tags, ZIP members, expanded size and
fresh-reader limits; the CSV reserve alone does not prove XLSX closure.

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
documentation, at most 20 paths and approximately 4,000 meaningful changed lines.
The Steward re-evaluated the 2,000–3,000-line signal before Ready: roughly 2,000
lines are executable acceptance spanning one inseparable capacity journey and
its independent oracle. Import, edit/history, save/reopen and both exports must
land atomically; separating parser allowance, transport allowance or publication
preflight would admit files that cannot complete the promised journey. One
Guarded child and one PR retain a single capacity-profile review model and one
rollback boundary. No unrelated v3/compatibility cleanup is included. Reconcile
growth beyond this estimate before continuing.

#493/#494 owns its separate v3 branch. Its live head was `589fe709` at intake;
older handoffs retain different heads. No mutation or release of that lane is
implied. Shared runtime integration is serialized and requires requalification;
capacity does not incorporate or relax the v3 guard. No automatic main merge,
deployment or Sheet pin change. Capacity files require a capacity-capable reader
on rollback; the old pin is not a compatible reader for the larger workload.

Producer evidence does not close Sheet #150. The exact exported kit must receive
separate Sheet normal-entry import/edit/save, host-persistence receipt, complete
browser termination/restart, reopen/export, rendering/focus, upgrade/rollback
and independent consumer qualification. No new architecture authority is created.
