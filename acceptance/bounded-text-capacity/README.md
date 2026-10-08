# Work #489 / Sheet #150 capacity acceptance seed

This Steward-owned seed exercises the synthetic 8,406 × 3 Text profile. It is
executable acceptance, not new architecture authority or evidence of product PASS.
The owning disposition and canonical Work #374 handoff control admission. Formula
growth/reference semantics and the 13-column review shape remain outside this seed.
Date plus saved-definition guards, Sheet's existing pin and #493 ownership remain.

Baseline is Work `33b26e2a8d34593030118c53dbe79b6f8fdc6e0e`. Historical pinned
producer refusal evidence remains Sheet #150 comments 5972691967 and 5972908971.
`oracle.py` creates **new** deterministic synthetic fixtures; these are not the
historical artifact bytes/hashes. The generator uses invented IDs, example.invalid
URLs, exact Text timestamps, leading zeros, UTF-8, quotes, LF and embedded CRLF.
Its manifest contains every expected cell. Python CSV/ZIP/ElementTree readback is
independent of the Rust producer parsers and writers.

| Requirement | Executable case |
| --- | --- |
| CSV/XLSX and old size landmarks | Native `run` and Worker import: 8/64/65/128/129/1024/1025/8406 rows |
| Complete Text integrity and stable identity | `verify` / `checkTable`: every cell, row/field ID, order, count, type and absence of formulas |
| First/middle/final edit and history | Three independent scalar edit→Undo→Redo sequences with complete-table oracle after history |
| Response currentness and interaction time | Every timed publication also queries its exact-revision affected fields |
| Retention pressure | 128 additional distinct scalar edits; Worker checkpoints at 64/128 edits |
| Native full termination and reopen | `run.sh` waits for importer exit, launches fresh `reopen`; native asserts prior PID gone, 10 close/open cycles |
| Browser full termination and reopen | Separate Chromium launches for import and reopen; 10 close/open cycles reuse one real module Worker |
| Save/export closure | Separate opaque exportProject and canonical export_canonical_tree/exportCanonicalTree+projectTransferFromEntries; inspectImportedProject; mandatory metadata-aware open_imported_project/openImportedProject; 10 fresh close/open cycles and resave equality for each source format/carrier; CSV/XLSX export |
| Source and resident preservation | Source SHA retained; malformed/8407 import refusals and stale edits in real Worker; malformed native open and oversized ordinary request |
| Refusal retains history/export state | Full cells, occurrence/revision, project bytes, CSV/XLSX bytes; Undo/Redo proves retained history |
| Full Opened reply limit+1 is atomic | Native exact 16 MiB+1 replacement retains an empty destination or existing resident, complete cells/exports and both history stacks; independent row-limit atomic control |
| Independent output verification | `oracle.py verify` requires exact 32-file main export matrix across both selected save carriers and native/Worker completion receipts; compares every cell with independent CSV/XML parsers |
| Measured producer budgets | Per-operation timings, browser heartbeat, sampled whole-Chromium RSS, actual Worker linear-memory phase samples |
| Actual raw-WASM file defenses | `resource_fixtures.py` and `boundaries.mjs`: source 2 MiB, expanded 8 MiB, XML 100,000 nodes, ZIP 256 members at/+1, preserved parser refusals |
| Enlarged envelopes and closure | Raw ABI ordinary 65,536/65,537 bytes, metadata request 4 MiB/+1, oversized non-metadata controls, Text 4,096/4,097 UTF-8 bytes, aggregate Text 1 MiB/+1 and CSV encoded output 2 MiB/+1 |
| Narrow profile and complete metadata | Raw ABI atomic default-style/header/column-width/13-column disqualification and malformed metadata; native `capacity_profile_boundaries` checks identity/profile-string, metadata 3 MiB, complete projection 16 MiB, selective 64 KiB boundaries and structural disqualification |
| Metadata-independent export closure | Encoded header row 39 bytes/+1, actual exact 2 MiB output, and scalar quote replacement that would add one encoded byte refused atomically |
| Complete metadata order and XLSX escaping | Row/column permutations refuse; exact 39-byte Unicode/XML-sensitive headers and 31-character sheet name roundtrip through actual CSV/XLSX readers and independent CSV/ZIP/XML verification |
| Metadata ABI framing | Type-last Export/InspectProject work; malformed, duplicate, positional-array and deeply nested nonmetadata controls refuse before typed payload decoding; invalid ordinary JSON over64KiB refuses by size; complete resident/history preserved |
| Small promoted capacity closure | Both formats: 64-row Imported payload over64KiB retains all192 cells through save/close/reopen/export/reimport; separate 8×3×4096 Text fixture proves Opened payload over64KiB before and after Worker termination/fresh Worker reopen, retaining all24 cells. Independent XML requires all four artifacts and exact LF/TAB/CR worksheet name |

The 64-row fixture promotes the complete Imported reply; its Opened payload fits
the ordinary limit. It does not prove promoted Opened/fresh-open behavior. The
separate `promoted-opened` recipe does. Each import browser leg has two Workers:
the promoted-Opened preflight terminates its Worker, then the fresh Worker runs
the remaining small closure and complete 8,406-row/128-edit journey. Its four main
memory checkpoints retain that one Worker. The separate full-browser-restart leg
still uses one Worker for all ten close/reopen cycles; the small preflight earns
no full-browser-termination credit. Receipts retain payload sizes and both recipes.

Actual unchanged-source baseline: new seed native eight-row CSV/XLSX passes;
64-row CSV refuses an 88,107-byte projection against 65,536. Four native boundary
groups fail at the existing 128-row/1,024-entity limits before their deeper byte
checks. All five structural-disqualification controls refuse. Fourteen raw-WASM
parser/ordinary-ABI controls pass, including new eight-row CSV/XLSX all-cell
readback. These are baseline observations, not repaired-product qualification.

Reference gates supplied by the Steward: Linux x86_64 Xeon 8573C, cgroup 4 CPU /
16 GiB, Chromium 151.0.7922.173, release native/WASM. Import/open/save/export max
10 seconds; edit/Undo/Redo including selective response p95 ≤500 ms/max ≤1 second;
main-thread heartbeat p95 ≤100 ms/max ≤250 ms; actual Worker linear memory
≤256 MiB and browser process RSS increase ≤512 MiB. These are test gates on the
named reference environment, not a general device SLA.

The Worker observer uses CDP to pause startup, wrap `instantiateStreaming` with a
same-arguments/receiver call and retain only the actual exported `WebAssembly.Memory`.
It returns the original result and never replaces Rust/bridge operations or kit
source bytes. Phase samples precede Worker termination. This instrumentation and
CDP/RSS sampling have overhead; no zero-overhead claim is made. Linear memory is
monotonic within one Worker, so retained phase samples bound prior allocations;
128-edit and 10-reopen samples expose retention growth for review. Whole-browser RSS
is sampled every 100 ms and can miss shorter host-memory peaks.

Acceptance capture returns binary artifacts as `Uint8Array` and metadata as a JSON
string. The #495 comment 5977814701 control verified identical hashes for all six
captured files while avoiding Playwright's ordinary byte-number array expansion.
This changes only test transport; all cells, artifacts and timing boundaries remain.
Phase timestamps permit correlation with RSS samples. The original pre-page RSS
baseline and budgets remain; no overhead is subtracted or earlier failed gate waived.

Run after the exact-head seed/implementation and exported kit are qualified for
execution, under the shared memory guard and serial heavy slot:

```sh
bash acceptance/bounded-text-capacity/run.sh /absolute/exact-kit /absolute/new-evidence /absolute/same-candidate-v3-capture <exact-v3-run-uuid>
```

Provide disk-backed TMP/cache, repository-pinned pnpm and `CAPACITY_CHROMIUM` for
the reference system Chromium. The harness uses only locked repository Rust/Node
dependencies and Python's standard library through uv. Invoke the shell under an
external finite timeout and process memory guard. The output directory must not
exist; artifacts are retained when an assertion fails. A final completion file
does not replace the independent verifier or other repository gates.

The runner rejects dirty/uncommitted source and binds the complete exported kit
to the current commit using the existing frozen C1 integrity checker. It captures
the source commit and manifest SHA-256, verifies every declared artifact byte and
complete inventory, then checks source/kit identity again after qualification.
All native, raw ABI and Worker stages run sequentially. Python bytecode output is
disabled; the generated fixtures and measurements stay in the new evidence tree.

Selected exact resource-boundary coverage lives in the companion boundary seed
owned by the Steward. All these sources require independent adequacy before
production admission. Strict syntax/type/lint checks alone are not behavioral
qualification. Real Sheet navigation/focus/paint, normal host persistence receipts,
the exact new producer/consumer pairing, hosted gates and independent final review
remain separate required consumer/integration gates; this harness does not clear
them or authorize a pin change, merge or deployment.

## Proposed saved-open qualification amendment — UNAPPLIED / UNRUN

This amendment is proposed against exact public248e7d47, not frozen acceptance
or an implementation-admission verdict. Work495 owner5999679572 selects the new
metadata-aware saved-open APIs; ordinary malformed-open controls remain ordinary.
The private811f5de/tree120666 baseline is unavailable here and unqualified. Missing
API/compile/setup evidence is NOTRUN/UNVERIFIED, never behavioral RED or PASS.

The full lifecycle now repeats independently for source CSV/XLSX × selected
opaque/canonical save carrier. Native run/reopen uses distinct native-opaque and
native-canonical capture directories; actual Worker uses eight distinct browser
legs, retaining the original sampler, pre-page RSS baseline, heartbeat, history
counts, 10 reopen/resave counts, and fixed thresholds in every leg. Small64 and
promoted-Opened8 preflights also use both carriers. Identical carrier bytes do not
replace proof that the two actual save APIs and framing paths were invoked.

The independent oracle requires exactly32 main exports and8 small XLSX artifacts;
it verifies complete cells and independently rechecks timing, counts, heartbeat,
RSS and linear-memory phase receipts. Raw Worker receipts are explicitly
performance_qualification:false because they are persisted before assertions.
Only completed capacity checks may record capacity_performance_qualification:true.
producer_qualification remains false: these checks do not qualify complete v3,
the historical reader, hosted/release gates, independent review or Sheet.

Retain complete v3 acceptance at589fe709 and its65-path compatibility delta,
including16 native cases, both real Worker cases, capture seals, original
65,536/65,537-byte admission and typed failures, v3-origin histories/reset,
selected export, fixture/API/explicit-v3 regressions, and the pinned518aaa55
historical-reader refusal. Keep retained compact66/Dedup, exact16MiB/+1 saved-open,
raw-ABI4MiB/+1 and absent-capability supplements. Their separate case map and
source identities must be admitted by the acceptance owner and independently
reviewed with this amendment before Ready. No old seed, summary or prior receipt
is a substitute for the complete package. All proposed behavior is UNRUN.

## RSS measurement completeness amendment v2 — UNAPPLIED / UNFROZEN / UNRUN

Independent review6049456855 found inherited missing-live-PID zero substitution.
The acceptance-only capacity-rss.ts helper now records each enumerated PID/type
as measured positive integer RSS, proven exited via kill(pid,0) ESRCH with check
timestamp, or unverified read/parse/liveness failure. ENOENT or absent VmRSS alone
cannot establish exit. Incomplete samples carry null aggregate RSS and explicit
BLOCKED_MEASUREMENT/performance UNVERIFIED evidence; no invented measured zero.

Pre-page baseline and100ms sampling cadence,512MiB increase,256MiB Worker memory,
timing/heartbeat thresholds and all v1 matrices remain unchanged. Every captured
sample must cover the exact enumerated PID/type set. The independent rss_oracle.py
validates identities, uniqueness, integer bytes/timestamps, exit evidence, complete
coverage and aggregate equality before evaluating the unchanged512MiB budget.
Aggregate-only v1 receipts cannot qualify. A missing or malformed live renderer,
unknown/permission-denied liveness, omitted renderer or unproved exit blocks
measurement qualification. Fully measured over-budget RSS remains a separate
performance assertion, only after measurement completeness is established.

Small deterministic TypeScript sensor controls serialize typed sample receipts;
Python oracle controls consume those exact receipts. They include complete64MiB
baseline→432MiB positive, missing/unparseable live renderer negatives, missing
VmRSS, unknown exit, explicit ESRCH exit positive, omitted900MiB renderer despite
positive32MiB aggregate, missing exit proof and old aggregate-only evidence. The
complete932MiB control checks the unchanged512MiB budget separately. These are
synthetic helper checks, not measured product/browser/reference evidence.

Native lifecycle, ordinary/raw ABI controls, v3 and both saved carriers are
unchanged from v1. Each of all eight Worker browser legs needs complete RSS
evidence. Both proposal versions and independent NOT ADEQUATE finding are retained;
fresh independent adequacy and acceptance-owner reconciliation remain required.
# Ordinary saved-open complete reply boundary

The source-only reconciliation adds an independently sized ordinary full-wire
65,536/65,537 pair, separate from original pure-v3 raw65,536/65,537 and enlarged
capacity16MiB/+1. A Number column prevents capacity promotion. The native
`capacity_saved_open_boundaries -- all NEW_FIXTURE_DIRECTORY` produces the
direct-v2, canonical-v1 and selected-v3 bytes plus expected complete projections.
`ordinary-saved-open-wire.mjs MATCHING_WASM FIXTURE_DIRECTORY NEW_RESULT` checks
all six through the actual spreadsheet ABI, measures actual reply bytes, and
proves install/refusal atomicity and both seeded histories. `run.sh` invokes both
stages serially; these additions are uncompiled/unrun until admitted execution.
