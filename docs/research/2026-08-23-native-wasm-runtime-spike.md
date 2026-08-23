# Native/WASM Resident Runtime Spike Evidence

Decision state: Provisional executable evidence for Issue #26

Source baseline: `origin/main` at `342f69f2fc252554c240650d1438cc0d6cd82e2f`

Date: 2026-08-23

## Outcome

ADR-0016's portable application set and the current
`tachiko-workspace-engine` can support one native/WASM semantic implementation.
A Worker-hosted WASM runtime can retain the authoritative Rust `Document`,
accept semantic commands/queries, and return revisioned projections without a
React or JavaScript document mirror.

Resident runtime is therefore architecturally viable, but the current
snapshot-style engine is not yet performance-complete for an interactive UI.
It avoids full-document JS/WASM transfer while retaining full Rust-side
candidate cloning, validation, calculation, and diff work. At 1000 synthetic
entities, boundary bytes became effectively constant for a one-field edit, but
the resident mutation still took about 300 ms median in release WASM.

No production crate or Accepted semantic contract was changed by this spike.
The standalone adapter's JSON/ABI is deliberately not a public SDK.

## Executable artifacts

- `spikes/issue-26-runtime/src/lib.rs`: resident Rust owner, stateless comparison
  seam, command/query projection, deterministic synthetic projects.
- `spikes/issue-26-runtime/src/wasm.rs`: minimal byte-buffer WASM ABI.
- `spikes/issue-26-runtime/worker/`: TypeScript Worker transport and client.
- `spikes/issue-26-runtime/src/bin/native-driver.rs`: native JSONL driver over
  the same Rust request handler.
- `spikes/issue-26-runtime/test/`: real Worker/WASM integration and exact
  native/WASM differential tests.
- `scripts/issue-26-portability-audit.sh`: target compilation, source leakage,
  and dependency-tree checks.
- `spikes/issue-26-runtime/benchmark/runtime-benchmark.ts`: full-snapshot versus
  resident command/query measurements.

## Recommended runtime topology

```text
React main thread
  ├─ ephemeral UI state
  └─ revision-keyed projections/cache
           │ semantic command/query messages
           ▼
Web Worker
  └─ TypeScript transport only
           │ copied request/result bytes
           ▼
WASM adapter (host/client-specific, Provisional)
  └─ resident runtime
       ├─ authoritative Rust Document
       ├─ revision / command serialization
       └─ rebuildable indexes and caches (future)
           │ Rust calls, no serialization
           ▼
tachiko-workspace-engine
  ├─ semantic-core
  ├─ formula-engine
  ├─ diff-engine
  └─ merge-engine
```

Native CLI/Tauri/server composition should use the same engine operations with
a native resident owner when residency is useful. A native host does not need
to emulate the Web Worker transport. WASM remains an execution target rather
than the semantic foundation.

## State ownership

| State | Owner | Examples |
| --- | --- | --- |
| Authoritative semantic state | Rust resident runtime | `Document`, accepted semantic mutations, current revision |
| Rebuildable semantic runtime state | Rust resident runtime/engines | address/dependency/reverse-dependency indexes, validation/calculation caches |
| Frontend projection/cache | React/client adapter | visible entity rows, calculated values, diagnostic projections, revision-tagged query results |
| Ephemeral UI state | React | selection, viewport, panels, focus, hover, pending command IDs, unbound editing buffers |
| Raw authoring state | UI/host until admitted | unfinished formula text, incomplete import/draft bytes |
| Durable representation | native/browser host plus storage boundary | file bytes, IndexedDB records, atomic save/recovery transaction |

Frontend projections must carry the runtime revision that produced them.
Receiving a newer projection invalidates older cache entries; frontend code may
not promote optimistic projection changes into canonical document meaning.
Raw authoring buffers become semantic only after the Rust admission/binding and
operation gate succeeds.

## JS/WASM boundary

The evidenced useful shape is:

- open/load a complete semantic snapshot once;
- send small typed semantic commands with an expected/current revision;
- return a new revision plus affected projection patches and semantic results;
- issue selective queries for views that are not already cached;
- request a complete snapshot only for explicit persistence, export, recovery,
  debugging, or branch exchange;
- serialize commands per resident runtime until a transaction/concurrency model
  is explicitly accepted.

The spike uses direct Serde JSON and a four-function raw memory ABI because
they are cheap executable probes. They are not suitable contract authority:
semantic Rust structs are workspace-internal, workspace result types are mostly
not serializable, error strings do not implement ADR-0019 diagnostics, and Rust
2024 stable export symbols require an unsafe attribute allowance. A production
adapter should isolate reviewed FFI/`wasm-bindgen` mechanics above
workspace-engine and define versioning with #10 rather than freeze this spelling.

## Host and persistence boundary

The host composition root remains responsible for:

1. reading browser/native durable bytes;
2. selecting the versioned storage decoder and explicit migration;
3. opening the resulting semantic snapshot in the resident runtime;
4. requesting a snapshot at an explicit save/export/recovery boundary;
5. canonical encoding; and
6. atomic filesystem or browser transaction commit.

Filesystem paths, dialogs, credentials, Git, process integration, IndexedDB,
and Tauri commands stay outside workspace-engine. The current storage crate
contains portable codecs and native `Path`/file APIs in one host-facing crate;
compile success for WASM does not make its filesystem functions a browser
persistence implementation.

## Portability evidence

`scripts/issue-26-portability-audit.sh` independently compiles these packages
for `wasm32-unknown-unknown`:

- `tachiko-semantic-core`
- `tachiko-formula-engine`
- `tachiko-diff-engine`
- `tachiko-merge-engine`
- `tachiko-workspace-engine`
- provider-free `tachiko-ai-api`

The audit found no production-source use of filesystem/path, clock, ambient
randomness, threads, environment, sockets, or network clients in that set. Its
WASM normal-dependency tree contains no audited native/ambient runtime package.

Leakage is correctly outside the portable set:

- storage uses native `Path`/file I/O and UUIDv5 for representation migration;
- CLI uses `PathBuf`, file/process I/O, and host-supplied UUIDv7 creation;
- the spike adapter uses `thread_local!` only to retain one single-threaded WASM
  instance and does not add threading to semantic behavior.

Browser storage, clocks, randomness, and threads therefore do not need injected
interfaces in semantic-core/workspace-engine today. ID creation remains the one
existing explicit host seam. Add another capability only when an accepted
semantic operation actually requires it.

## Native/WASM parity evidence

The differential test drives the same Rust command handler through:

1. the native release JSONL driver; and
2. TypeScript `RuntimeClient` → Node Worker → real WASM module.

It compares exact JSON records for deterministic synthetic generation, open,
overview, full calculation, scalar mutation, semantic diff/formula impact,
snapshot, stateless snapshot mutation, independent branch construction,
three-way merge, and post-merge calculation. Every record matched byte for
byte.

This supplements the existing release corpus, which already executes 24
production semantic/storage/workspace/AI records natively and in WASM. No
implementation divergence bug was found, so no production regression fix was
needed.

## Performance evidence

Environment: Node `v24.15.0`, macOS arm64, release WASM (998,314 bytes), two
independent Worker/WASM instances per scale, 20 sequential alternating scalar
mutations. Each entity has two stored Number fields and one independent bound
formula. Times are within-run medians and are evidence for topology, not a
browser/device performance promise.

| Entities / formulas | Snapshot bytes | Main-thread JSON stringify | Whole-snapshot mutation median | Resident mutation median | Request byte reduction | Response byte reduction | Full calculation result |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 10 / 10 | 5,903 | 0.008 ms | 0.719 ms | 0.480 ms | 53.4× | 14.3× | 3,901 B |
| 100 / 100 | 54,055 | 0.069 ms | 7.558 ms | 5.080 ms | 479.6× | 122.1× | 38,333 B |
| 1000 / 1000 | 536,457 | 1.225 ms | 346.238 ms | 299.645 ms | 4,748.6× | 1,202.7× | 384,435 B |

For 1000 entities, 20 whole-snapshot mutations transferred 10,731,839 request
bytes and 10,738,538 response bytes. The resident path transferred 2,260
request bytes and 8,929 response bytes. The affected mutation result stayed
roughly constant because it contained only the directly changed field and its
formula impact.

The main finding is a separation of costs:

- resident ownership decisively prevents O(document) boundary traffic;
- plain JavaScript stringify alone was not the dominant cost in this Node run;
- current mutation execution remains O(document)-like because workspace-engine
  clones the candidate and repeatedly performs full validation/calculation/diff;
- a full `calculate` query itself produced an O(document) payload (384 KB at
  1000 entities), so clients need selective projection queries even after state
  becomes resident.

Resident runtime is necessary for the intended UI boundary, but it is not a
substitute for incremental calculation/validation, batch commands, or selective
queries.

## Concrete workspace-engine pressure

1. Mutation functions accept `&Document`, clone a complete candidate, and
   return `EditPreview { document, diff }`. A resident wrapper can adopt the
   returned document but cannot avoid the clone.
2. Finalization and diff perform repeated full calculation. Producing a compact
   calculated projection patch through current public queries adds another full
   calculation because there is no direct application-level patch result.
3. `calculate_fields` returns every calculated field. It is suitable as a
   correctness query but not a large-view incremental transport.
4. `SemanticDiff`, most workspace result structures, and `WorkspaceError` are
   not intentional serialization DTOs. Adapters must currently inspect/map
   workspace-internal types or degrade to rendered text.
5. There is no engine-owned resident session, revision precondition, batch,
   transaction, cancellation, or selective query API. The spike had to add a
   revision and serial command queue above the engine.
6. Derived address/formula indexes and calculation results are rebuilt per
   call. Nothing is retained for the next command.
7. Three-way merge reasonably consumes three snapshots, but a resident client
   still has to provide `base` and `theirs` in full. Branch/history residency is
   not modeled and should remain open.
8. Current semantic Serde derives are adequate for the probe but explicitly
   forbidden as accidental durable/public wire authority by ADR-0016/0017.
9. `IdGenerator` is a good native host seam, but creation over serialized FFI
   still needs an explicit adapter command/result design.

These are application/runtime API pressures, not evidence for moving host
concerns into semantic-core.

## Issue #26 questions with enough evidence

- Keep one authoritative semantic aggregate in Rust.
- Use a resident runtime for interactive Web/WASM rather than snapshot
  roundtrips per edit.
- Place the Web WASM runtime in a Worker by default to isolate expensive
  semantic work from React rendering/input.
- Treat React state as revisioned projection/cache plus ephemeral UI state.
- Keep filesystem/browser storage/Tauri/Git outside workspace-engine and
  compose persistence explicitly at the host.
- Use command/query/result/patch messages for normal interaction and complete
  snapshots only at explicit lifecycle boundaries.
- Keep semantic execution single-threaded initially; no current semantic
  dependency requires ambient threads or target-specific behavior.
- Require executed native/WASM differential fixtures, not compile-only target
  checks.

## Questions that should remain open

- The stable external SDK, exact command names, JSON/binary encoding, API
  version negotiation, batch shape, and bypass policy (#10).
- Exact diagnostics wire structures, delivery protocol, and presentation. This
  spike consumes current errors only and does not amend ADR-0019/#23.
- Interactive invalid-candidate retention, crash recovery, autosave, and
  browser draft persistence (#13/#41 plus #26 host work).
- Revision conflict semantics, optimistic commands, cancellation, and
  multi-command transaction rollback.
- Multi-document residency, memory limits, eviction, branch/history retention,
  and collaboration attachment.
- Browser-specific engine/memory conformance beyond Node's WASM runtime.
- When shared-memory WASM threads are justified; current evidence does not
  require them.
- Whether production host adapters justify new crates. Adding a ninth workspace
  crate or changing direct edges requires an explicit ADR-0016 amendment.

## Recommended decision amendments and implementation tickets

Issue #26 should produce a focused ADR that accepts the ownership/topology and
lifecycle rules above while leaving exact public wire syntax Provisional to
#10. After acceptance:

1. Amend `frontend-backend-boundary.md`, `wasm-strategy.md`, and
   `performance-model.md` with the resident Worker topology, state classes,
   explicit snapshot lifecycle, and selective-query requirement.
2. Add an internal resident runtime/session implementation ticket: engine-owned
   `Document`, monotonic revision, serial command application, expected-revision
   guard, explicit snapshot import/export, and no persistence capability.
3. Add a projection-patch/selective-query ticket so mutations return affected
   stable subjects/current values and views can query bounded subsets without
   serializing all calculated fields.
4. Add a performance ticket to remove redundant full calculations and implement
   ADR-0018-equivalent retained dependency/calculation state before targeting
   keystroke-rate editing. Preserve the full oracle as the correctness fallback.
5. Add a dedicated Web adapter ticket with reviewed FFI memory ownership,
   Worker lifecycle, request serialization, browser test matrix, and size/memory
   budgets. If this becomes a new production crate, amend ADR-0016 first.
6. Add native resident/Tauri composition tests proving the same command corpus
   without copying Web transport decisions into native semantic code.
7. Keep browser/native persistence adapter work separate from the runtime
   session and coordinate any portable-codec split with #25/#41 rather than
   adding filesystem or IndexedDB capabilities to workspace-engine.

No ADR-0015, ADR-0017, ADR-0018, or ADR-0019 semantic amendment is required by
the evidence. ADR-0016 needs amendment only if the accepted production crate
set/DAG changes.
