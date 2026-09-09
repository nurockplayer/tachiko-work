# Executable acceptance and qualification

## Evidence status

This is a Steward-authored seed, not a product test PASS certificate.
`tests/harness.test.mjs`: 30 local tests (six valid observations and 24 deliberately
bad observations rejected). These validate **the oracles only**. They do not
exercise React, a Rust runtime, WASM, a real save, or an agent authorization domain.

The new fixture has 18 JSON/JSONL files and adds ordinary Text notes to the known
upstream dogfood shape. Its JSON/identity/source-shape checks are local; **real
Rust canonical admission remains unverified**. Syntax checks do not promote it.

`tests/kit.mjs` verifies one actual artifact; `tests/storage.mjs` has four canonical
I/O scenarios. `tests/browser.mjs` contains one real-kit canary, six M1 browser
scenarios, and three M2 review-UI scenarios. `tests/agent.mjs` has eight trusted
M2 lifecycle scenarios. These 23 integration checks are **authored, not passed**.
Without their real composition dependencies, commands stop as BLOCKED (exit 78).
No skip, xfail,
mock-only green, missing-import RED, or missing-tool RED counts as product proof.

## Reproduction

Dependency-free checks:

    node --test tests/harness.test.mjs
    node --check tests/browser.mjs
    node --check tests/agent.mjs

The included materialized fixture is ready to inspect. To regenerate, use a fresh
seed tree without `tests/fixtures/release-plan.roproj`, then:

    node scripts/materialize-fixture.mjs

The generator refuses replacement; do not delete user data to make it run.

Actual artifact inventory/integrity and canonical I/O (after C0/C1 wiring):

    WORK_CLIENT_KIT=/absolute/path/to/kit WORK_CORE_COMMIT=<40-hex-source> node tests/kit.mjs
    WORK_STORAGE_DRIVER=/absolute/path/to/reviewed-driver.mjs node tests/storage.mjs

Real-kit canary (after C0 produces and verifies an intact upstream kit):

    WORK_CLIENT_KIT=/absolute/path/to/verified-kit node scripts/serve-canary.mjs
    WORK_CLIENT_URL=http://127.0.0.1:4174 node tests/browser.mjs --canary

Real M1 client (served by its own production build/composition with test hooks):

    WORK_CLIENT_URL=http://127.0.0.1:4173 node tests/browser.mjs

Real M2 trusted bridge:

    WORK_AGENT_DRIVER=/absolute/path/to/reviewed-driver.mjs node tests/agent.mjs

Real M2 review UI (same real bridge, visibly labeled deterministic test provider):

    WORK_CLIENT_URL=http://127.0.0.1:4173 node tests/browser.mjs --agent

Bootstrap must install exact pinned Playwright/tool versions using pnpm and commit
the resulting lockfile. `WORK_PLAYWRIGHT_MODULE` can identify an existing reviewed
Playwright module in a constrained validation environment; `WORK_CHROMIUM` can
identify its compatible browser. Record both versions. Neither variable changes
assertions or permits a browser substitute. The local seed package has no external
runtime dependencies and does not pretend that a browser dependency lock exists.

## Criterion-to-test mapping

| Requirement | Executable case | Evidence still needed |
|---|---|---|
| Complete pinned JS/types/Worker/WASM and notices, exact inventory/digests | `kit.mjs` | Actual producer artifact and source provenance |
| Canonical v1 export and genuine `.ro`; fresh source beats old cache; invalid Open preserves work | C1-01/02/03/04 in `storage.mjs` | Real codec/runtime driver and source-preservation audit |
| Fixture reaches real semantic boundary; core returns 5/10 then 3/8 | `browser.mjs --canary` | Complete JS/types/Worker/WASM kit and real Rust admission |
| Notes field admitted; stale edit rejected without byte change | Same canary | Actual run, exact artifact and source SHA |
| Table and Brief show one current target and authored note | M1-01 + `oracles.linked` | Real DOM plus authoritative read observer |
| Save copy survives complete browser restart | M1-01 + `oracles.reopened` | Real persistent profile, new runtime, storage read audit |
| Invalid edit is atomic and preserves the draft | M1-02 + `oracles.rejected` | Real runtime rejection, not a fake client validator |
| IME Enter cannot publish a composition | M1-03 | Real Japanese/Chinese IME and focus checks in addition to synthetic events |
| Save failure retains accepted work and destination | M1-04 + `oracles.saveFailed` | Real host fault injection before durable commit; destination checksum |
| Lost Execute reply does not cause blind retry or false certainty | M1-05 + `oracles.unknown` | Transport drop after real dispatch; bounded fault/reconciliation observation |
| Keyboard cancel and safe close preserve work/focus | M1-06 | Tab/arrow/screen-reader/manual dialog coverage beyond the automated subset |
| Propose/preview and unapproved Execute cannot publish | M2-01/02 | Driver around real trusted lifecycle |
| Exact approved apply, no replay or host effect | M2-03 | Real trusted receipt and host-effect accounting |
| Changed base, revoked authority, altered proposal, closed occurrence | M2-04/05/07/08 | Real lifecycle and fresh exact-head independent review |
| Live disclosure scope protects evidence | M2-06 | Broader unauthorized-scope leakage corpus in the owner crate |
| UI review without publication, explicit apply with a trusted Delegated receipt | M2-UI-01 | Real UI plus C3 bridge; test source is not an LLM |
| Reject and stale review do not publish; request a new proposal | M2-UI-02/03 | Real UI and changed-base/receipt evidence |

This is not exhaustive acceptance for every future child. Before C1/C3 production
Ready, qualify these supplied artifact/canonical-export and bridge-owner tests
against the concrete seam, and reconcile uncovered requirements. Before C4 Ready,
qualify the supplied three browser proposal-review scenarios with the C3 driver;
the M2 core suite alone does not prove a human review UI. C5 still requires a
concrete executable native-host seed and actual OS evidence. These are explicit
remaining preparation tasks, not silent waivers.

## M1 integration wiring contract (test-only)

The UI test language is English; fixture notes/titles are CJK. Stable test markers
identify semantic targets rather than DOM layout or React internals:

- `open-project`: normal directory file input; `project-ready`: real open complete.
- `cell:{entity}:{field}`: rendered Table value (focusable editable grid cell where
  applicable); `brief:{entity}:{field}`: rendered linked Brief fact.
- Each rendered fact carries opaque `data-work-occurrence`, `data-work-revision`,
  `data-work-entity`, `data-work-currentness`. These are observations, not permission.
- Table and Brief tabs; the labeled Edit cell and Decision notes text controls;
  Apply notes, Save a copy, Copy name, Create copy, Close project controls.
- Save status “Saved on this device” only after host commit; operation outcome
  “Outcome unknown” during the injected ambiguous-publication scenario.
- Unsaved work dialog with Keep editing; saved sample action Open saved review-copy.

Use accessible roles and names. Mechanical locator repairs can follow governance;
removing scenarios, changing expected values or replacing a real boundary cannot.
Tabs express the contracted view-switch action, not a required pixel layout.

The reviewed test build provides `window.__tachikoAcceptance`:

`observe()` returns deterministic {occurrence, revision, entity, impact, priority,
notes, canonicalHash} from the actual production runtime and explicit debug
snapshot/codec boundary, not the projection cache or fixture constants.
`savedHash(name)` reads actual durable saved bytes, returning null if absent.
`failNextSave()` fails the next real host save before commit. It must not supply
replacement semantic data. `loseNextExecuteReply()` loses a response only after
real dispatch; `executeRequestCount()` observes the real transport.
`settleFaultWindow()` waits for that bounded failure/recovery experiment, not an
arbitrary sleep or a test-triggered retry. `saveObservation()` and
`unknownObservation()` must read the actual rendered status/currentness, not set
outcome fields. `lastReceipt()` observes the last actual trusted execution receipt;
it must not fabricate delegated provenance or successful publication. No hook may publish semantic state, substitute calculation
results, approve a proposal, fabricate receipts or directly set product success.

The driver is delivery-owned mechanical wiring subject to independent review.
Test hooks must be absent from the distributable build. Observation snapshots are
allowed at this explicit debug boundary; ordinary product editing still uses
bounded queries. Do not convert test driver names into a stable public API.

## M2 driver contract (test-only)

`createDriver({fixture})` creates a fresh real runtime and trusted authorization
domain per case. `observe()` is a Human-authorized deterministic query/debug
snapshot, including canonical hash, revision and the tested current values.
`propose`, `preview`, `execute` act through a Delegated occurrence. `approveAsHuman`
is a separate trusted Human action, not renderer-supplied `approved:true`.
`editAsHuman`, `revokeDelegatedAuthority`, `revokeQueryAuthority`, `reopenFixture`
exercise the genuine owner boundaries; `executeAltered` tests rejected tampering.
`externalEffectCount` observes attempted host effects. `close` cleans up every case.

The driver normalizes result categories to published/denied/etc only for tests;
it cannot normalize a real successful mutation into denial. `preview` after Query
revocation returns disclosure-safe denial with empty disclosedSubjects/Values.
Broader owner-crate tests must also audit indirect leaks through messages/metadata.

## C1 artifact/storage wiring (test-only)

`artifact-manifest.json` declares sourceRepository, exact sourceCommit, experimental
stability, the public experimental entry, required asset paths with SHA-256 digests,
and licenseNotices paths. All kit files except the manifest are declared exactly
once; no symbolic links, traversal or undeclared asset is accepted. This packaging
manifest is provisional and does not stabilize a client SDK or wire protocol.

The storage `createDriver({fixture})` creates a real consumer/core occurrence.
`editImpact`, `editNotes`, `exportCanonicalTree`, `exportRo`, and
`verifyRoUsingCore` must call the real semantic/storage boundaries.
`reopenCanonicalWithOldPresentationCache` starts a fresh occurrence with the supplied
canonical tree while retaining the old disposable view cache. `tryOpenCanonical`
exercises the production admission/replacement boundary; rejection preserves the
previous work. `observe` uses the actual core and `close` releases the host.
The test edits exported fixture bytes only to independently challenge persistence;
that is not permission for the production frontend to parse or mutate the format.

## Required C0 output and production-readiness decision

Record core commit, client seed/branch commit, actual artifact digest/capability
manifest, dependency lock, browser/OS versions, canary output, complete failing
scenario output, and whether failure is product behavior or setup/wiring.
A valid pre-Ready result proves fixture admission and existing behavior first,
then identifies the missing intended behavior. Do not label a missing app, module,
network tool, invalid fixture or test hook as qualified product RED.

The Steward then records Ready or a bounded correction on each owning Issue.
Unverified browser/agent tests stay unverified. The delivery agent writes its own
unit tests; final review covers both those and the adequacy of this acceptance.
No acceptance exception waives an applicable unit-test class or release gate.

## Manual/external evidence owners

Astra/client owner: recorded ordinary-user walkthrough, readable narrow-width/zoom
layouts, CJK IME and screen-reader focus checks, no inactive fake controls, coherent
selection and draft preservation. Terra/host owner: actual canonical save/import,
corruption/unsupported input/source preservation, complete asset packaging,
upgrade/rollback and platform file permissions. macOS owner: packaged `.app`, cold
and warm OS open, cancel/failure/dirty-open and full-process save/reopen. Steward:
exact-head review, requirement-to-proof audit and #256/#261 launch decisions.
No such evidence was manufactured in this seed.
