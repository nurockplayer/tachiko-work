# #809: shared storage for the in-memory Document and retained state

Status: **Proposed**. Implementation waits for work-lead's approval of this
design ([#809 comment 6875202499][approval]). Acceptance row: none directly;
#809 edit budget. This document proposes Rust representation and ownership
changes only; it does not amend an Accepted contract or authorize implementation.

Source baseline: `2da87c09` (this branch's base). Measurements are from
`cf71ae685c280bf2edde63a1f0ce0ae1394e132e`, in the separate, never-merge
[#816 diagnosis][diagnosis] and [phase tables][phases]. Source line references
below use this branch's base, not the instrumented source's shifted lines.

## Goal and budget

At 10,000 rows × 10 columns, including three formula columns, one scalar edit
plus its following zero-row window must take ≤50 ms natively and in the browser
worker. The diagnosis measured edit p50 187.000 ms and window p50 8.061 ms;
195.061 ms is a sum of medians, not a measured combined percentile. Set A
(preflight reuse, V1 evidence without snapshots, scoped formula work) is separate
and in progress. Its estimated remainder is 90–115 ms.

Set B targets these disjoint edit leaves from [the phase tables][phases]:

| Leaf | Measured p50 ms | Proposed replacement |
| --- | ---: | --- |
| Runtime snapshot clone | 8.689 | One planner-owned candidate, with shared untouched chunks |
| Planner entity copy | 8.525 | Consume that candidate; no second command replay/copy |
| Retained-state clone | 13.221 | Shared calculation/graph partitions and address index |
| Old retained-state release | 13.054 | Release handles and replaced partitions only |
| Old Document release | 5.733 | Release handles and replaced entity chunks only |
| Separate evaluated Document release | 4.272 | Move the sole candidate into publication |

The opportunity is 53.494 ms before replacement costs. The 10.132 ms graph
copy is **inside** 13.221 ms; do not add it again. Estimate: B saves a further
30–50 ms, yielding roughly 40–85 ms after A. This range does not establish
≤50 ms or a p95 guarantee. Reconcile overlap against the actual merged A head.

Last-owner release stays O(owned size). Closing/replacing a session, discarding
a full rebuild, or deleting most rows can still destroy the whole table/graph
synchronously. B removes repeated ownership of unchanged data during an edit;
it does not promise constant-time destruction or move destruction outside timers.
Full comparisons, ordered totals (~8 ms), required evidence, authorization,
receipts and actual dirty calculation remain. B does not change reduction order
or substitute delta arithmetic for ordered binary64 folds.

## Existing ownership and compatibility boundaries

- [model.rs:380–418][document] exposes `Document` fields, including
  `BTreeMap<EntityId, Entity>`, derives serde with `deny_unknown_fields` and
  `PartialEq`, and manually counts `Document::clone`. `Entity` owns its field
  map ([model.rs:656][entity]). IDs already share immutable `Arc<str>` while
  preserving `Send`/`Sync` ([model.rs:14][ids]).
- The runtime clones and manually applies commands before preparing evaluation
  ([runtime lib.rs:7270][runtime]); the authoritative planner separately clones
  the base ([patch_lifecycle.rs:3940][planner]).
- `prepare_evaluation(candidate: Document)` clones retained state before update
  ([resident_session.rs:448][prepare]). `PreparedEvaluation` borrows base
  document/calculation but owns `PreparedInstallation`; `into_installable`
  ends that borrow ([workspace lib.rs:736,820][prepared]).
- `publish_if_current` accepts a borrowed candidate; the prepared installation
  contains another owned candidate. Publication checks occurrence, revision and
  content, then assigns retained state and Document
  ([resident_session.rs:1112–1175][publish]). Execute keeps the evaluated
  Document for verification ([patch_lifecycle.rs:2608–2674][execute]).
- Storage owns format DTOs, not the in-memory entity container: direct `.ro/v2`
  uses its own `BTreeMap` ([direct_ro/v2.rs:20][ro]); `.roproj/v3` converts,
  sorts and shards entities ([roproj/v3.rs:647][roproj]). Its decoder currently
  returns a concrete semantic `BTreeMap` ([roproj/v3.rs:911][decode]).

Preserve [ADR-0018][adr18] bound ASTs, exact normalized numeric equality,
ordered evaluation and atomic failure; [ADR-0024][adr24] exact proposal and
base-revision binding; [ADR-0026][adr26] trusted footprints and current-time
authorization; [ADR-0030][adr30] complete, deterministic direct-state evidence;
and [ADR-0033][adr33] complete authoritative snapshots independent of retained
history. Shared allocation identity is none of those identities. JSON, WASM
ABI, `.ro`/`.roproj` bytes, refusals, diagnostics and semantic contracts stay
unchanged. Chunks are neither persisted shards nor replay/history records.

## Representation options

Let E=10,000 entities, C=128 maximum entries per chunk, P=number of chunks,
K=edited entities, and T=distinct chunks touched. Costs below exclude unavoidable
changed values and small Document metadata/schema copies. They are algorithmic
estimates, not measured milliseconds. All alternatives retain `EntityId` order.

| Option | Clone | Edit | Release |
| --- | --- | --- | --- |
| A: existing `BTreeMap<EntityId, Arc<Entity>>` | O(E) tree copy, key clones and entity-handle increments | O(log E) lookup plus clone one Entity on first shared mutation | O(E) map/key/handle release even while entities survive; last entity owner releases its fields |
| B: ordered range chunks, `Arc<BTreeMap<EntityId, Entity>>` | O(P) directory/handle copy | O(log P + log C) lookup; first write copies ≤C entities; K edits copy O(T·C), plus local splits/merges | O(P) handle release plus O(T·C) replaced owned entries; final owner O(E) |
| C: persistent ordered tree crate | O(1) root-handle clone | O(log E) path copying per edit, plus changed value ownership | Shared root O(1); replaced unique paths O(log E) per edit; final owner O(E) |

**A:** for this fixture, one clone increments 10,000 entity handles and
10,000 ID handles (IDs already use `Arc<str>`), and allocates/copies the tree.
Entity handles alone occupy ~80 KB on a 64-bit native target or ~40 KB on
wasm32, excluding keys, BTree nodes and allocator overhead. It avoids copying
100,000 stored field entries, including 30,000 formula ASTs, but remains an
O(E) clone/drop path. These counts follow the exact fixture
([scratch capacity_bench.rs:1737–1815][fixture]). Direct use changes inserts,
removes, owned iteration and mutable access to wrap/unwrap/COW entities;
read-only auto-deref helps field reads but does not preserve concrete API types.

**B (recommended):** a directory `Vec<Arc<BTreeMap<K,V>>>` with cached lengths
and ordered, non-overlapping key ranges; `EntityMap` specializes it for entities.
Keep each non-edge chunk between 64 and 128 entries by local splitting,
merging/redistribution. Bulk loading 10,000 sorted entries gives 79 chunks;
editing history can leave roughly twice as many. One candidate clone therefore
bumps about 79–157 chunk handles, not 10,000 entity handles. A scalar edit copies
at most 128 entities/1,280 stored fields (1.28% of this fixture), not one entity.
Directory insertion/removal shifts O(P) handles; no global repartition on edit.
This is bounded partition copying, not an O(K) claim for arbitrary table sizes.

Lookup binary-searches ranges; iteration concatenates chunks in key order,
including reverse iteration where consumers need it. Insert/remove maintain
cached length and range bounds. `get_mut` makes only the selected chunk unique;
safe mutable iteration visits chunks individually. Bulk `values_mut` can
intentionally copy every visited chunk; it must not implement scalar edits.
The chunk size is an initial estimate to measure, not a new configuration knob.

**C:** compare `rpds::RedBlackTreeMapSync` with `im::OrdMap`, not hash maps:

| Candidate | Licence, maintenance and Rust 1.85 compatibility |
| --- | --- |
| `rpds` 1.1.2, ordered Sync tree | [Manifest][rpds-manifest]: MIT, declared MSRV 1.85.0; [API][rpds-api] documents O(1) clone and O(log E) updates. Upstream remains active, with [v1.2.1][rpds-release] a later release; 1.1.2 is the metadata-verified comparison version, not an installation proposal. Exact locked transitive dependencies and wasm32 still need a Rust 1.85 build before adoption. |
| `im` 15.1.0, `OrdMap` | [Manifest][im-manifest]: MPL-2.0+, declared MSRV 1.46.0 (below 1.85). [RustSec][im-maintenance] records the project as archived/unmaintained; reject for new adoption. No build or licence-closure approval is inferred from the MSRV field. |

Both tree choices need an adapter for current BTree APIs and mutation/owned
iteration. A persistent tree reduces directory copying but adds dependency,
notice/licence, allocation, indirect-call and WASM qualification work. B needs
no new dependency and reuses std's tree ordering. A crate would be an explicit
tooling decision under AGENTS.md, not incidental implementation cleanup.

For **all three** options, serialization must emit one ordinary ordered map,
with identical entries, field order, numeric spelling and existing DTOs. No
Arc/chunk/tree tags enter JSON. B supplies map serde explicitly and preserves
the existing map decoder's duplicate-key behavior; retain Document/entity
`deny_unknown_fields`. C's optional serde support is not byte-identity evidence.
Equality stays content equality, worst-case O(E + stored fields) for unrelated
allocations; A may skip identical entity handles, B identical chunks, C identical
subtrees where supported. Different chunk layouts with equal contents compare
equal. No hash-only, revision-only or pointer-only equality replaces `PartialEq`.

The brief's ~1,500 uses in ~160 files is a migration risk, not a promise to edit
all sites. A scoped `.entities` inventory here finds 126 files: semantic-core 13,
formula-engine 17, diff-engine 8, merge-engine 4, workspace-engine 39, storage 23,
ai-api 3, CLI 3, browser runtime 16, including tests. Estimate: A directly changes
hundreds of construction/mutation/owned-value sites; B/C through an adapter
change tens to low hundreds of construction/concrete-type sites, while most
borrowed reads stay unchanged. Exact counts require classifying expressions;
the file counts are not changed-call-site counts.

## Public Rust API and migration

Recommend keeping `pub entities: EntityMap` with map-like operations. Keep the
other public fields as they are. Making every field private and requiring
Document accessors would migrate nearly every reader and every struct literal;
it adds no sharing benefit needed by #809. The new field type still changes the
public Rust source API and requires the lead's approval. No stable wire change
follows from it.

Stage through `pub type EntityMap = BTreeMap<EntityId, Entity>` first. Move
semantic construction sites to `EntityMap::new`, `collect`, or explicit `.into()`
at BTree conversion boundaries while the alias still denotes the old type.
Migrate signatures that mean the semantic entity store, not storage-owned DTO
maps. Then replace the alias with the chunk-backed wrapper in one green PR;
consumers already use only its named methods and iterator contracts. The shared
ordered partition primitive lives in semantic-core, reused by formula-engine;
there is no new crate or dependency edge.

Support only observed operations: lookup/indexing, length/emptiness, ordered
keys/values/iteration, insertion/removal, mutable lookup/iteration, collection
and explicit BTree conversions. Preserve borrowed lookup and owned iterator
item types. Do not expose `DerefMut<BTreeMap<...>>` or materialize a BTree just
to satisfy a read. Bulk mutable examples already exist
([keyed_grouped_sum_operations.rs:638][bulk]); storage decode boundaries above
show why a bare alias swap cannot compile without preparatory conversions.

## Retained state and publication

`Calculation.values`, `RetainedCalculationState.nodes/failures`, and all seven
dependency graph maps are owned trees today
([formula lib.rs:1109,1134][calculation], [1980][graph]). Partition their outer
ordered maps/sets by their actual typed keys. Large adjacency sets
(`cell_readers`, `reference_uses`, column readers/slots) also need partitioned
ordered sets; sharing an entire high-fanout set and copying it on insertion
would reproduce a table-sized copy. No row-locality assumption for cross-row
formulas, totals, conditional aggregates or lookup dependencies is allowed.

Clone the retained container's directories into a candidate; untouched chunks
remain shared with the base. Existing dirty discovery traverses old and new
reverse closures and replaces formula edges
([formula lib.rs:1322–1367][impact]); values/failures are removed/replaced only
for dirty nodes ([1370–1417][recompute]). Preserve this algorithm and ordering;
change its storage accesses to partition COW. The large graph then survives a
scalar edit through shared chunks instead of a ~10 ms copy and later full drop.
Membership changes and a global success/failure transition still report the
complete affected projections required today; full rebuilds remain full work.

`ResidentDerivedState` additionally owns a validation report, diagnostic index
and `Result<AddressIndex, AddressIndexError>`
([resident_session.rs:550–611][derived]). Keep the immutable address-index
result in one Arc: scalar edits do not change it. Rebuild and replace it when
`address_index_changed`, retaining existing invalid-index/full-rebuild behavior.
The index maps human keys to stable IDs ([address.rs:9][address]); it is not a
second canonical Document. Share unchanged reports; partition diagnostic entries
when updating them, preserving complete diagnostic order at report boundaries.
No full report/index/map clone is hidden behind a new Arc's first mutation.

Publication moves the candidate Document and retained-state directories into
the resident after all current guards pass. It releases old handles immediately;
unchanged partitions are still owned by the installed revision. A refused
candidate drops only its handles and newly detached partitions. Snapshot holders
can keep old partitions alive until their normal final release. There is no
new thread, queue, deferred drop, unsafe code or alternate publication authority.

`Calculation::values()` and `complete_values()` currently return concrete
`&BTreeMap` references ([formula lib.rs:1460,1790][values]). Add ordered borrowed
iteration/lookup APIs, migrate their callers, then change internal storage;
do not rebuild a complete map on each query to preserve those signatures.
This is another Rust API migration explicitly covered by the proposed approval.

## One candidate, borrowed base, owned publication

Replace the runtime's command replay with an engine planning entry that borrows
`&ResidentWorkspaceSession`/its witness and the admitted ordered patch body,
creates one candidate, derives trusted associated writes, then calls preparation.
Initially that candidate still clones the Document once; B's chunks make it
cheap. Capacity/profile checks borrow the planned/prepared candidate before
proposing; their rejection order and limits remain unchanged.

Change the resident proposal entry from `Option<&PreparedEvaluation>`
([patch_lifecycle.rs:1916][propose]) to consuming the engine-created evaluation
and its exact planned-body binding. Its memo owns one installation plus
evidence/report/footprint, instead of a second `EvaluatedPatch.document`.
Private construction binds occurrence, revision, commands in order and exact
candidate; an arbitrary caller-supplied Document cannot acquire this proof.
Base revision alone cannot distinguish two candidates from the same base.

Change the resident execution/publication seam to consume that installation
once. `PreparedEvaluation::into_installable` already shows how to end the base
borrow before taking `&mut` publication authority. Keep the existing general
borrowed-candidate authority path for non-resident callers, including its full
comparison checks ([patch_lifecycle.rs:1272][authority]). The resident owned
entry must preserve exactly those scope, fresh-time authorization, staleness,
NoChange and revision-exhaustion rules. NoChange still uses semantic equality.

Before moving the candidate, keep the evaluated report and exact binding needed
for verification. Verify the installed snapshot returned inside the guarded
publication against that consumed engine-owned candidate's provenance and
evidence, with existing result/receipt semantics. Do not retain a deep candidate
copy just for verification or accept an arbitrary returned revision as proof.
Any trait/API extension and its implementations land together. These are Rust
APIs only; WASM exports and transport DTOs do not change.

## Arc rationale and risks

Arc expresses that base, candidate and retained snapshots own the same immutable
semantic partitions concurrently. It preserves the current `Send`/`Sync` model
already documented for IDs; it is not a response to a borrow-checker error.
The base borrow ends before publication. Nothing crosses an `.await`; this
design adds no async or `'static` requirement, locks or interior mutability.
`Arc::make_mut` detaches only bounded chunks; no `Arc<Mutex<_>>`, `RefCell`,
whole-map COW or mutable object reachable from multiple revisions is allowed.

- Whole-map COW, full-map conversion for compatibility, eager mutable iteration,
  or flattening nested adjacency sets can restore O(table) work on each edit.
  Measure detached entries and the existing clone/drop leaves, not root clones.
- Shared allocation lifetime can increase peak memory when proposals/snapshots
  remain live. Keep existing memo/record lifetimes; no global Arc caches, cycles
  or new retention policy. Check rejection, stale execution and session close.
- Keep content equality reflexive and independent of chunk boundaries, including
  absent versus present stored cells. Pointer equality is only a safe fast path
  for immutable identical allocations; it never establishes revision identity.
- Keep the Document instrumentation counter as **calls to Document::clone**
  ([model.rs:408][document]), not bytes copied or deep copies. A nonzero shallow
  clone can be cheap; zero Document clones can still hide graph copies. Report
  chunk detach work alongside it in #816's measurement, without changing its
  historical meaning or inventing a new performance gate.
- WASM may trade allocator/refcount overhead and code size for sharing gains.
  Compare actual worker latency, peak memory and artifact size; native timings
  alone do not select the representation. Std-only also requires wasm32/Rust
  1.85 qualification; no unsupported target-specific shortcut is proposed.

## Proposed PR sequence and deciding measurements

Each row is one independently green PR after design approval, with its own
bounded Issue/readiness and existing relevant tests. These are preparation
steps for a source-API change, not permission for a repository-wide refactor.
Serialize hotspot changes with set A; rebase onto its accepted result first.

| PR | Change and crates touched | Measurement/condition to proceed |
| --- | --- | --- |
| 1 | One authoritative planned/prepared candidate consumed into resident proposal/publication; workspace-engine + browser runtime and their tests | #816 bench: remove second planner copy and separate evaluated-Document drop; same refusal, evidence, receipt and verification results |
| 2 | Introduce EntityMap alias/partition API and migrate semantic-core construction; add ordered calculation read APIs in formula-engine | #816 bench unchanged within run variation; existing serde/equality and calculation callers remain green |
| 3 | Migrate formula-engine semantic entity constructors and calculation read callers | Same bench/output; no map materialization on reads |
| 4 | Migrate diff-engine to the entity-store API | Same bench; exact semantic diff/order preserved |
| 5 | Migrate merge-engine to the entity-store API | Same bench; exact merged state/conflict order preserved |
| 6 | Migrate workspace-engine constructors/concrete entity-store signatures and calculation reads | Same bench; trusted writes, invalidation and publication outcomes preserved |
| 7 | Migrate storage semantic conversion boundaries, retaining all format DTOs | Same bench; exact `.ro` and `.roproj` canonical byte fixtures and admission errors |
| 8 | Migrate ai-api semantic entity-store sites | Same bench; existing API projections/errors unchanged |
| 9 | Migrate CLI semantic entity-store sites | Same bench; existing command output/no-overwrite behavior unchanged |
| 10 | Migrate browser runtime constructors and calculation reads | Same bench and real worker edit/window output; unchanged ABI/DTOs |
| 11 | Activate chunk-backed EntityMap in semantic-core; only necessary adapter/export updates | #816 runtime snapshot-copy and old-Document-drop leaves shrink; ≤128 entities detached for a scalar edit; byte/equality contracts pass |
| 12 | Partition retained values, nodes, failures and graph in formula-engine; share index/report and partition diagnostics in workspace-engine | #816 retained-copy/old-retained-drop leaves shrink; unchanged partitions survive; full-calculation oracle agrees on success/failure, membership and dependency changes |

Use `capacity_809_formula_table` from #816 on an isolated measurement checkout
of each candidate; do not merge the scratch branch or its entire instrumentation.
Its recorded native invocation is `cargo test --release --manifest-path
packages/browser-client/runtime/Cargo.toml --locked --features capacity-bench
capacity_809_formula_table -- --ignored --test-threads=1 --nocapture`.
Run only on an approved remote Rust runner for this 16 GB host; no cargo on the
host or in orb-test for this task. No benchmark, tests or workflow dispatch is
part of this documentation PR.

For performance-changing rows, compare before/after on the same runner/build,
fixture and repetitions (30 edits; retain all samples); include synchronous
replacement/release and the following zero-row window in each paired sample.
Keep #816's leaf medians and clone count, report combined p50/p95/max separately,
and repeat the real WASM worker path. Check open/save/reopen and both clear
widths for regressions. An API-preparation PR needs no latency improvement;
storage PRs must demonstrate the predicted leaf reduction before continuing.
Only the actual final native **and** browser edit-plus-window measurements can
establish ≤50 ms. A 40–85 ms estimate is not acceptance evidence.

## Assumptions and lead questions

Recommendation: **B, std-only ordered range chunks**, with one consumed
candidate and partitioned retained state. No new dependency is proposed.
The 128-entry cap and migration names/order are implementation proposals;
the lead approves the public Rust representation/API direction before code.
The document exceeds 300 lines to cover the required alternatives, ownership
seam, migration boundaries and measurement decisions in one reviewable place.

Open for work-lead: approve this direction and the EntityMap/calculation-read/
resident ownership API changes, or select a persistent ordered tree instead.
Reconcile the implementation PR scopes/order with set A. If measured A+B still
exceeds 50 ms, return remaining leaf evidence to #809; do not change the budget,
wire/storage formats or semantics to claim success.

[approval]: https://github.com/nurockplayer/tachiko-work/issues/809#issuecomment-6875202499
[diagnosis]: https://github.com/nurockplayer/tachiko-work/blob/32899683b42ec6c3892d80d0dde93e16c7b80b8f/scratch/809-diagnosis.md
[phases]: https://github.com/nurockplayer/tachiko-work/blob/32899683b42ec6c3892d80d0dde93e16c7b80b8f/scratch/809-diagnosis-phases.md
[fixture]: https://github.com/nurockplayer/tachiko-work/blob/cf71ae685c280bf2edde63a1f0ce0ae1394e132e/packages/browser-client/runtime/src/capacity_bench.rs#L1737
[document]: ../../crates/semantic-core/src/model.rs#L380
[entity]: ../../crates/semantic-core/src/model.rs#L656
[ids]: ../../crates/semantic-core/src/model.rs#L14
[runtime]: ../../packages/browser-client/runtime/src/lib.rs#L7270
[planner]: ../../crates/workspace-engine/src/patch_lifecycle.rs#L3940
[prepare]: ../../crates/workspace-engine/src/resident_session.rs#L448
[prepared]: ../../crates/workspace-engine/src/lib.rs#L736
[publish]: ../../crates/workspace-engine/src/resident_session.rs#L1112
[execute]: ../../crates/workspace-engine/src/patch_lifecycle.rs#L2608
[ro]: ../../crates/storage/src/direct_ro/v2.rs#L20
[roproj]: ../../crates/storage/src/roproj/v3.rs#L647
[decode]: ../../crates/storage/src/roproj/v3.rs#L911
[calculation]: ../../crates/formula-engine/src/lib.rs#L1109
[graph]: ../../crates/formula-engine/src/lib.rs#L1980
[impact]: ../../crates/formula-engine/src/lib.rs#L1322
[recompute]: ../../crates/formula-engine/src/lib.rs#L1370
[derived]: ../../crates/workspace-engine/src/resident_session.rs#L550
[address]: ../../crates/semantic-core/src/address.rs#L9
[values]: ../../crates/formula-engine/src/lib.rs#L1460
[bulk]: ../../crates/workspace-engine/src/keyed_grouped_sum_operations.rs#L638
[propose]: ../../crates/workspace-engine/src/patch_lifecycle.rs#L1916
[authority]: ../../crates/workspace-engine/src/patch_lifecycle.rs#L1272
[adr18]: ../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md
[adr24]: ../decisions/ADR-0024-revision-pinned-semantic-patch.md
[adr26]: ../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md
[adr30]: ../decisions/ADR-0030-canonical-semantic-delta.md
[adr33]: ../decisions/ADR-0033-snapshot-first-semantic-history-and-checkpoints.md
[rpds-manifest]: https://docs.rs/crate/rpds/1.1.2/source/Cargo.toml.orig
[rpds-api]: https://docs.rs/rpds/1.1.2/rpds/map/red_black_tree_map/struct.RedBlackTreeMap.html
[rpds-release]: https://github.com/orium/rpds/releases/tag/v1.2.1
[im-manifest]: https://docs.rs/crate/im/15.1.0/source/Cargo.toml.orig
[im-maintenance]: https://rustsec.org/advisories/RUSTSEC-2026-0248.html
