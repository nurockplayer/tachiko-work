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
| Save/export closure | Native/Worker exportProject, inspectImportedProject, independent fresh reopen, canonical byte equality, CSV/XLSX export |
| Source and resident preservation | Source SHA retained; malformed/8407 import refusals and stale edits in real Worker; malformed native open and oversized ordinary request |
| Refusal retains history/export state | Full cells, occurrence/revision, project bytes, CSV/XLSX bytes; Undo/Redo proves retained history |
| Full Opened reply limit+1 is atomic | Native exact 16 MiB+1 replacement retains an empty destination or existing resident, complete cells/exports and both history stacks; independent row-limit atomic control |
| Independent output verification | `oracle.py verify` requires exact 16-file export matrix and native/Worker completion receipts; compares every cell with independent CSV/XML parsers |
| Measured producer budgets | Per-operation timings, browser heartbeat, sampled whole-Chromium RSS, actual Worker linear-memory phase samples |
| Actual raw-WASM file defenses | `resource_fixtures.py` and `boundaries.mjs`: source 2 MiB, expanded 8 MiB, XML 100,000 nodes, ZIP 256 members at/+1, preserved parser refusals |
| Enlarged envelopes and closure | Raw ABI ordinary 65,536/65,537 bytes, metadata request 4 MiB/+1, oversized non-metadata controls, Text 4,096/4,097 UTF-8 bytes, aggregate Text 1 MiB/+1 and CSV encoded output 2 MiB/+1 |
| Narrow profile and complete metadata | Raw ABI atomic default-style/header/column-width/13-column disqualification and malformed metadata; native `capacity_profile_boundaries` checks identity/profile-string, metadata 3 MiB, complete projection 16 MiB, selective 64 KiB boundaries and structural disqualification |
| Metadata-independent export closure | Encoded header row 39 bytes/+1, actual exact 2 MiB output, and scalar quote replacement that would add one encoded byte refused atomically |
| Complete metadata order and XLSX escaping | Row/column permutations refuse; exact 39-byte Unicode/XML-sensitive headers and 31-character sheet name roundtrip through actual CSV/XLSX readers and independent CSV/ZIP/XML verification |
| Metadata ABI framing | Type-last Export/InspectProject work; malformed, duplicate, positional-array and deeply nested nonmetadata controls refuse before typed payload decoding; invalid ordinary JSON over64KiB refuses by size; complete resident/history preserved |
| Small promoted capacity closure | Actual Worker64-row projection over64KiB saves/closes/reopens/exports/reimports with every cell checked; independent XML verifies exported cells and LF/TAB/CR worksheet name |

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

Run after the exact-head seed/implementation and exported kit are qualified for
execution, under the shared memory guard and serial heavy slot:

```sh
bash acceptance/bounded-text-capacity/run.sh /absolute/exact-kit /absolute/new-evidence
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
