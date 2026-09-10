# Named local versions: preparation and acceptance

Status: **Steward proposal and unqualified executable acceptance seed**.
Owner: nurockplayer/tachiko-work#364. Date: 2026-09-10 JST.
No production, Ready, independent-review or release claim.

## User outcome and boundary

On the new first-party client, keep a named version of the release-plan work,
continue editing Table and Brief, inspect an earlier version against current
work, restore it safely, and recover the confirmed saved result after a complete
application restart. No Git repository, account, cloud or LLM is required.

This is a small product extension after #360, not a new history architecture or
an expansion of #360 itself. It does not change #357's M1/M2/M3 acceptance or
replace #301's session Undo/Redo. Existing Git adapters remain supported; they
are not being removed or redeveloped here.

### Authority and evidence pins

Upstream: `nurockplayer/tachiko-work`.
Inspected main: `8bba9b09cea3c011df383216ba3846ccd003dece`.
Client seed: `f264ead7a8286b0de337792b118055f13aa2c83d`.
Fixture: `planning/independent-work-client/tests/fixtures/release-plan.roproj`
at that client seed; retain all 18 files, including empty shards. Do not recreate
or silently modify the fixture to make the tests pass.

Consume Product Constitution, Design Principles, knowledge-authority,
project-governance, decomposition/throughput policies, ADR-0022/0024/0026,
ADR-0029/0030/0032/0033/0034, and the applicable storage contracts. In particular:

- Current semantic state is authoritative; complete snapshots stand alone.
- Revision occurrence, checkpoint, content and Git identity are different.
- Semantic Delta is evidence, not a replay or restore program.
- Restore goes forward through authorized current-base intent; it never
  resurrects a historical runtime revision or rewinds history.
- Snapshot, history, receipt persistence and semantic publication are separate.
- `.roproj/v1` and portable `.ro` v1 remain closed formats. #330 owns the
  independently Accepted format-2 work; its definitions are unsupported by
  delta-v1, so no partial comparison or generic restore may pretend otherwise.

Live sources to re-read at preflight: #357-#363, #330/#331, #345/#351,
#299-#302, #256/#261, and then-live overlapping PRs. #357 supersedes #354's
old repository-location recommendation. The client repository returned Not Found
through this connection during intake; #358, not this lane, owns bootstrap.
The current experimental DesignerClient exposes scalar/formula operations and
export, but no named-checkpoint, compare-history or atomic-restore entry.
This is interface/source inspection, not an exhaustive proof of missing core code.

Google's official UX reference (checked 2026-09-10):
https://support.google.com/docs/answer/190843?hl=en
It documents view, name, copy and restore. It does not prove Tachiko uniqueness,
market demand, full audit completeness, or implementation readiness. This seed
copies none of Google's implementation and makes no superiority claim.

## Selected first-slice defaults, pending bounded review

These are concrete recommendations, not silently Accepted durable storage law.
Preflight must confirm fit with actual C1/C2 boundaries and record the smallest
profile/spec decision required before production Ready. Do not reopen the
snapshot-first model, event sourcing, CRDT or a universal history service.

### H1 — named local snapshots over one managed work item

Use the C2 release-plan sample: first entity
`1d37df46-01f6-4b05-8fd9-064718dc91ea`, Impact 5, Friction 5, Priority 10.
A is that state. B has Impact 3, Priority 8, and notes
`先完成試玩回饋，再決定下一版範圍。`.

First UI scope covers existing supported scalar edits and authored Text notes;
formulas are retained exactly and evaluated by core. Do not add a formula editor,
structural editing, arbitrary schema creation or grouped sums just for this demo.
Whole restore is admitted only when **all** differences are representable by the
qualified existing command/batch profile. Otherwise reject the whole restore and
allow read/export where supported. Never quietly restore a subset.

A historical snapshot is complete current data at that checkpoint, not a screen
capture, a JS canonical mirror or an operation log. Its explicit logical profile
is snapshot-only: named checkpoints and net snapshot comparisons, **not** complete
edit history, replay, verified-tail, attributed per-edit audit or team causality.
Presentation caches/layout are outside H1 semantic coverage and must be labeled
as such. Notes, stored values, formulas, identities and references are not caches.

### Location, copying and retention

A separately versioned **app-private host store** holds immutable canonical
snapshot bytes and checkpoint metadata outside `.ro` / `.roproj` payloads. An
explicitly managed local work item owns a history instance and confirmed-current
snapshot locator. Creating such an item preserves imported source bytes.

Do not attach history by DocumentId alone, filename, path or content hash. Two
separately imported/copied artifacts may share semantic identity yet must not
silently share local history. Opening a managed item uses its explicit host
identity. Portable export is snapshot-only, contains no hidden history/prompts,
and starts with no implicit local history when opened independently.

Keep-version and restore use immutable new local records. A host's confirmed
current locator advances only after a real verified durable commit. This is a
new H1 capability to qualify; C2's Save-a-copy is **not** evidence that it exists.
It does not authorize source-file overwrite, autosave or migration of another
client's storage/origin. Exact store schema, commit/recovery mechanism and
version upgrade behavior need the bounded pre-Ready profile review.

First profile has explicit bounded capacity; at capacity/quota failure, reject
new retention visibly without evicting named/recovery checkpoints. Select and
record the concrete limit from actual host evidence before Ready. No retention
compaction, deletion, cryptographic authenticity or portable-history format is
promised. Browser-local versions are not an independent backup. Preserve an
export escape path for each supported checkpoint and explain storage clearing.

### Capture and restore truth

A named checkpoint captures an exact admitted snapshot. Giving it a name creates
no semantic publication. Equal snapshot bytes may have distinct checkpoint IDs.
Names/timestamps are display metadata, never identity or proof of who edited.
Default attribution is local checkpoint creation, not invented human/agent audit.
No private prompt retention and no inferred rationale presented as a fact.

Checkpoint creation returns the exact captured revision. If live work advances
during persistence, the saved checkpoint may still be valid, but the newer live
work stays unsaved. UI grouping never changes underlying publication atomicity.
Resolve or explicitly retain pending editor drafts; a snapshot contains accepted
work, not uncommitted text in a widget.

Restoring A from B must:

1. derive a complete read-only preview against the exact current occurrence/base;
2. preserve a verified recovery snapshot of the current accepted state, unless
   that exact state is already durably retained; failure here prevents mutation;
3. submit one existing authorized atomic semantic intent/batch from the trusted
   upstream boundary, with fresh gates and exact-base checks;
4. append a new checkpoint for the installed result and commit the confirmed
   current locator, without deleting or changing A or B;
5. report semantic and durable outcomes separately. Successful install followed
   by failed persistence leaves the actual live result available/unsaved and the
   old confirmed saved snapshot intact. Retry persistence, not the mutation.

NoChange creates no semantic revision/transition; separately requested named
snapshot creation can still succeed. Stale preview, invalid source, unsupported
whole diff, wrong document, denied permission or failed safety capture must not
partially publish. A lost reply is unknown until real evidence resolves it, not
an excuse to retry Execute blindly.

Historical reads are separately admitted and read-only, never the active editable
occurrence. A late preview response cannot revive a closed/replaced occurrence.
Missing/corrupt/unsupported history disables that capability truthfully; an
independently valid current snapshot must still open. Never label corrupt history
as a successful empty timeline, or treat history-ahead as authority to advance the
confirmed current snapshot. Permission and required-provenance gates remain in
force even if optional general history is unavailable.

### UI language and walkthrough

Use the existing contextual Changes surface with a Versions section, not a new
Git-shaped screen. Named actions: **Keep version**, **Compare with current**,
**Restore version**, and **Export snapshot**. Keep the project/save status visible.
Show source edits separately from recomputed consequences: Impact 5 -> 3 is one
stored change; Priority 10 -> 8 is a derived result, not a second authored edit.
The notes change is another stored change. Net differences do not establish the
number of intervening edits, their authors, intentions or exact chronology.

Normal path: open sample -> keep A -> edit Impact/notes -> keep B -> restart ->
compare A with B -> explicitly restore A -> confirmed local save -> restart ->
A's original meaning, with A/B/restore checkpoints still available. An unrelated
exported file is unchanged; no Git/account/provider operation takes place.

## Decomposition and implementation sequence

Prepare/read-only work may start now in an isolated worktree. Do not wait for all
spreadsheet backlog items, C3/C4 agent UI, C5 macOS distribution, or a stable SDK.
Runtime/browser qualification needs a real C1 kit and C2 host; missing prerequisites
are a precise blocked result, not permission to fabricate them in this lane.

After preflight, choose only actual missing delivery units:

- upstream producer capability, **only if needed**: historical read/comparison and
  all-or-nothing restore planning/execute via existing meaning; one upstream PR;
- new-client delivery: managed local retention, truthful restore/recovery and UI
  as one coherent product PR consuming the qualified producer capability.

Different repositories, artifact pins and rollback ownership justify separate
producer/consumer PRs. Do not split snapshot storage, list button, diff panel and
restore button into artificial horizontal production PRs. Do not merge a UI that
pretends an unavailable producer exists. Record sizes/ownership before Ready.

Terra High leads preflight and any bounded core/host delivery; Astra High owns
client flow integration when Ready; Luna handles isolated tests/primitives after
contracts settle. Fresh independent Sol/deep review must cover acceptance and
final Guarded heads. No profile file is assumed to exist; no self-merge.

Scheduling recommendation: H1 is an early feature after C2, before optional
client breadth when its dependencies are Ready. It neither pauses #330/#351 nor
adds a blanket #256/#261 launch blocker. C2 remains unchanged. One writer per
client integration branch; do not run competing H1/C4/C5 edits to the same host.

## Executable acceptance, coverage and honest limits

`tests/acceptance.mjs` drives a real host/core through delivery-owned test wiring.
`tests/browser.mjs` drives the real new-client UI through Playwright. The fixture
and tool prerequisites are explicit; missing prerequisites exit 78 (BLOCKED).
`tests/oracles.test.mjs` checks only independently specified assertion logic.
Those controls are not product acceptance and cannot grant Ready.

| Case | Required proof |
|---|---|
| H01 | Immutable named snapshots, no semantic edit caused by naming, real restart |
| H02 | Complete exact net diff, no mutation and no authored derived-value claim |
| H03 | Forward restore, protected B, new occurrence revision, durable restart |
| H04 | Stale preview rejects all changes |
| H05 | Failed safety capture prevents restore and preserves confirmed data |
| H06 | Post-install persistence failure is truthful; no blind replay |
| H07 | Corrupt historical payload does not prevent valid current open |
| H08 | Equal content is not checkpoint identity; NoChange is not publication |
| H09 | Snapshot export and independent copy have no implicit history association |
| H10 | Unknown history version cannot authorize restore |
| HU01 | Real human controls, Table/Brief, complete browser-process restart |
| HU02 | Pending notes draft and cancelled restore survive; no hidden discard |

Still to materialize/qualify against concrete boundaries before final acceptance:
incomplete semantic-diff refusal; write-failure/crash matrix including lost response/unknown outcome, competing-tab
writes and capture-vs-edit race; wrong-document/permission rejection; exact source
preservation; quota/capacity and unsupported command-family fixtures; actual
snapshot/export admission and release-build absence of fault hooks. Existing C2
IME/focus/accessibility checks remain applicable; manual Japanese/Chinese IME,
keyboard/screen-reader and storage-clearing walkthrough are not automated here.
This is a bounded first seed, not a waiver of those requirements.

### Test-only driver contract

`createDriver({fixture})` admits real canonical files and returns an isolated
real host/runtime. Required methods appear in `tests/acceptance.mjs`. `observe`
returns {documentId, occurrence, revision, impact, priority, notes,
canonicalFiles}; canonicalFiles is the complete real codec export as a
path -> UTF-8-content map, not reconstructed from expected values. `persisted`
reads actual confirmed host bytes; `restart` destroys runtime/host objects and
reopens their durable data. Full process death is additionally tested in HU01.

`versions` returns immutable checkpoint observations, excluding dynamic current-row
flags; `persisted` separately observes the confirmed current locator and bytes.

`keepVersion`, `compare`, `restore`, `edit`, `exportSnapshot`, `openIndependent`,
and `retryPersistence` call actual production boundaries. `compare` normalizes
real complete semantic delta entries to {entity,field,before,after}; values retain
their {kind,value} DTO. It cannot synthesize successful results from fixture data.
`restore` reports separate semantic/persistence categories as asserted. `fault`
may inject only the named host/transport/isolated-history corruption fault. It
must not fake successful persistence, mutate semantic state, approve operations,
or create provenance. Every adapter and fault location requires independent
review. None of these method names is a public SDK or prescribed implementation.

### Run and qualify

    node --test planning/version-history/tests/oracles.test.mjs
    node --check planning/version-history/tests/acceptance.mjs
    node --check planning/version-history/tests/browser.mjs

    TACHIKO_HISTORY_FIXTURE=/absolute/path/to/pinned/release-plan.roproj \
    TACHIKO_HISTORY_DRIVER=/absolute/path/to/reviewed-real-driver.mjs \
      node planning/version-history/tests/acceptance.mjs

    TACHIKO_HISTORY_FIXTURE=/absolute/path/to/pinned/release-plan.roproj \
    WORK_CLIENT_URL=http://127.0.0.1:4173 \
    WORK_PLAYWRIGHT_MODULE=/absolute/path/to/pinned/playwright/index.mjs \
      node planning/version-history/tests/browser.mjs

Use existing C0/C2 pinned pnpm tooling; no new package manager or mocked runtime.
Qualify fixture admission and C2's existing 5/10 -> 3/8 save/restart first. Only
then can absence of a real history capability be behavioral RED. A missing app,
driver, module, hook, browser, network or invalid fixture is BLOCKED, never RED.
Record exact main/client/kit/fixture/seed identities, commands, logs and real
results. Independent acceptance review plus finite storage/profile disposition
returns to Steward for the explicit Ready/decomposition decision. Do not open a
production PR or merge this seed before that gate.

## Browser observation wiring

The test build exposes `window.__tachikoHistoryAcceptance` with read-only
`observe`, `versions`, and `persisted` as above; observe also returns the real
managed `localWorkId` after creation. Keep-version and restore controls call the
production host/runtime, never a hook that manufactures success. Stable markers
`version:{checkpointId}` and `open-history:{localWorkId}` locate visible version
rows and recent managed work. They expose targets to tests, not IDs that a user
must type. The Versions action, Keep version / Version name dialog, Version
comparison region, Restore version confirmation and pending-draft Keep editing
action are test wiring conventions, not layout or public API commitments.
Mechanical selector repairs preserve the scenario and oracle. Audit that these
observers/fault facilities are excluded from distributable builds.
