# Driver Common Profile acceptance record

Version: `driver-common-profile-acceptance/v1` (preparation, not promotion).

**Public Product Gate: NOT PASSED. Commercial Gate: NOT ATTEMPTED.**
The merged implementation provides useful bounded workflows, but it does not
complete all 15 capability bundles or six golden journeys in
[Issue #256](https://github.com/nurockplayer/tachiko-work/issues/256).
This record applies the
[Steward calibration](https://github.com/nurockplayer/tachiko-work/issues/256#issuecomment-5548686122)
and [#261 preparation authorization](https://github.com/nurockplayer/tachiko-work/issues/261#issuecomment-5547884895).
Documenting a limitation does not waive a failed declared golden journey.
A preparation PR references #261 and must not close it.

## Candidate identity and evidence meaning

The implementation baseline is merged `main` at
`55ee8ff18df6559dc98e599a586038fb07ea4bd8`. The RC run must record its
manifest-generated `commit`, archive/file hashes, version, actual local
entry path, validation logs and browser artifacts in the single
[RC execution record](https://github.com/nurockplayer/tachiko-work/issues/256#issuecomment-5550508660).
Only results explicitly recorded for that artifact count as executed. Do not
infer that run from the baseline,
a PR merge, or the presence of executable tests. A local maintainer preview is
not an ordinary public distribution.

| Delivery | Reviewed implementation HEAD | Merged main commit | Recorded execution evidence |
|---|---|---|---|
| [Budget PR #285](https://github.com/nurockplayer/tachiko-work/pull/285) | `adabaa338706f280a32863c296b8262c640ba27a` | `dc480fa3f2d94548004604448b000a24690c0bd7` | [Exact-head release gate](https://github.com/nurockplayer/tachiko-work/pull/285#issuecomment-5548834145) |
| [Interop PR #286](https://github.com/nurockplayer/tachiko-work/pull/286) | `a4e7aa50b74b6393a16f59b9da1b0be7ebfd729f` | `69d0fa0e1f51028b9abc61625f7feec54f25fa39` | [Exact-head release gate](https://github.com/nurockplayer/tachiko-work/pull/286#issuecomment-5550040069), [bounded reference evidence](https://github.com/nurockplayer/tachiko-work/pull/286#issuecomment-5550044074) |
| [Reports PR #290](https://github.com/nurockplayer/tachiko-work/pull/290) | `ce6614c6256c535c6656975908d72d248b620307` | `55ee8ff18df6559dc98e599a586038fb07ea4bd8` | [Exact-head release gate](https://github.com/nurockplayer/tachiko-work/pull/290#issuecomment-5550393642) |

The source links below identify reproducible implementation checks, not new run
results. The recorded delivery runs passed their bounded checks. None measures
representative target-user success or establishes general Excel compatibility.
Read the [Designer scope](../../apps/designer/README.md),
[interop profile](../../apps/designer/interop-profile.json) and
[report profile](../../apps/designer/report-profile.json) with this matrix.

## All 15 capability bundles

“Partial” compares the implemented UI against the entire #256 bundle. An
implemented bounded subset is not a waiver for the omitted workflow.
`#256 → #261` means the product-boundary owner must reconcile the concrete gap
and its smallest delivery owner before promotion; the focused escape and lookup gaps are recorded in
[#292](https://github.com/nurockplayer/tachiko-work/issues/292) and
[#293](https://github.com/nurockplayer/tachiko-work/issues/293), respectively.
Both await Steward readiness reconciliation and authorize no new production lane.

| # / bundle | Current first-party UI capability | Coverage and concrete remaining gap | Executable source evidence | Owner / next disposition |
|---|---|---|---|---|
| 1. Lifecycle, undo, recovery | New Tracker/Budget; atomic browser Save/Save As; close/reopen; dirty guards; failed admission/write preservation; bounded Tracker session undo. | **Partial.** Formula apply/copy, accepted cleanup and chart changes clear prior Tracker history; formula-to-scalar restoration is absent. Reopen starts fresh history. No autosave, durable recovery history or portable browser-sidecar backup. | [Tracker browser](../../apps/designer/e2e/tracker.spec.ts), [mixed history](../../apps/designer/e2e/tracker-history.spec.ts), [Budget recovery](../../apps/designer/e2e/budget-recovery.spec.ts), [interop recovery](../../apps/designer/e2e/interop-recovery.spec.ts), [report history](../../apps/designer/e2e/reports.spec.ts) | #256 → #261: recovery/edit coverage and escape requirements; preserve calibrated scoped delivery. |
| 2. Grid navigation, selection, retrieval | Tracker mouse/keyboard selection; arrows, Tab, Home/End, Page Up/Down; literal find/filter; imported table filter. | **Partial.** Fixed bounded grid; no general address-based go-to or arbitrary-sheet navigation. Find/filter does not prove workbook-wide find/replace. | [Tracker browser](../../apps/designer/e2e/tracker.spec.ts), [Tracker grid](../../apps/designer/src/tracker-grid.ts) | #256 → #261: representative navigation/retrieval task coverage. |
| 3. Direct editing and structure | Atomic Tracker multi-cell edits, append/remove, row view moves; typed imported scalar edits and import-time extra output columns. | **Partial.** Tracker has three fixed fields and at most 128 rows. No general insert/delete/move columns or cell ranges; row moves are presentation. Formula-to-scalar editing is absent. | [Tracker browser](../../apps/designer/e2e/tracker.spec.ts), [interop cleanup](../../apps/designer/e2e/interop-cleanup.spec.ts), [runtime import/edit](../../apps/designer/runtime/tests/interop_document.rs) | #256 → #261: ordinary structural editing steps; no incidental schema/Core expansion. |
| 4. Clipboard interchange | Tracker copies displayed values and accepts bounded rectangular TSV from other apps, with atomic typed rejection. | **Partial.** No general cut/paste formula/style interchange or imported-workbook grid clipboard workflow. Values-only TSV is not full workbook interchange. | [External clipboard journey](../../apps/designer/e2e/tracker.spec.ts), [TSV parser](../../apps/designer/src/tracker-model.ts) | #256 → #261: required external-app escape and clipboard semantics. |
| 5. Fill/copy semantics | Formula picker copies to selected numeric destinations using relative rows/columns and fixed/cross-collection references. | **Partial.** No general scalar/date series or universal fill handle. Copy clears session undo and rejects out-of-range/type/cycle cases atomically. | [Budget copy journey](../../apps/designer/e2e/budget.spec.ts), [formula-copy runtime](../../apps/designer/runtime/tests/budget_copy.rs) | #256 → #261: series/fill steps beyond the delivered Budget task. |
| 6. Multiple sheets | Budget and imported collections; cross-collection references; named views can be added, renamed, reordered, duplicated and deleted. | **Partial.** Views alias existing source collections. Duplicate does not copy worksheet data; delete does not delete a collection. General worksheet-data authoring remains missing. | [Budget views journey](../../apps/designer/e2e/budget.spec.ts), [view model](../../apps/designer/src/budget-views.ts) | #256 → #261: independent sheet-data operations; #258's equivalent-view acceptance remains intact. |
| 7. Types and display | Text, finite Number, Boolean, ISO date-only Date; Number/JPY/USD/percentage display; explicit CSV typing. | **Partial.** No Time value. Time/elapsed XLSX numeric formats block. Decimal-dot entry; en-US Number/percentage/USD and ja-JP JPY display. Admitted numeric currency/locale patterns outside the JPY/USD mapping, or mixed numeric display markers, use plain Number without conversion; unknown syntax and mixed Number/Date/time semantics block admission. | [Budget types](../../apps/designer/e2e/budget.spec.ts), [adapter](../../apps/designer/runtime/tests/interop_adapter.rs), [number-format tests](../../apps/designer/tests/interop-number-format.test.ts) | #256 → #261: date/time and promised locale scope must be accepted explicitly. |
| 8. Formatting/layout | Tracker emphasis, fill, border, wrap, alignment, header styling, bounded dimensions; imported basic style retention; sticky headers. | **Partial.** No general freeze/hide control or page/layout system. Hidden imported structures are explicitly blocked, not silently made visible. | [Tracker formatting](../../apps/designer/e2e/tracker.spec.ts), [adapter inventory](../../apps/designer/runtime/tests/interop_adapter.rs) | #256 → #261: actual layout needs and accessibility/keyboard checks. |
| 9. Formulas/recalculation | Rust-bound arithmetic and bounded min/max; stable same/cross-collection references; fixed/relative copy; explicit invalid/cyclic/calculation-failing rejection. | **Partial.** No general comparison/range-function/lookup/grouping authoring. Imported A1 formulas are a bounded adapter profile, not an Excel function engine. Formula undo and scalar restoration limits remain. | [Budget browser](../../apps/designer/e2e/budget.spec.ts), [interop formulas](../../apps/designer/runtime/tests/interop_document.rs), [reference provenance](../../apps/designer/tests/fixtures/interop/REFERENCE.md) | #293: lookup/grouped-summary golden task is not satisfied by arithmetic summaries. |
| 10. Sort/filter | Stable typed Tracker sorting; imported values/formulas/errors/blanks; filters are view-only; charts retain explicit source order. | **Partial.** Bounded cases are implemented; no general workbook filter system or target-user acceptance. Tracker sort/filter is session-only; imported table view persists privately. | [Tracker](../../apps/designer/e2e/tracker.spec.ts), [imported view tests](../../apps/designer/tests/interop-table-view.test.ts), [chart invariance](../../apps/designer/e2e/reports.spec.ts) | #261: representative case/interaction validation within #256's final profile. |
| 11. Structured tables/ranges | Stable semantic columns/rows; Tracker append; imported headers/optional missing values; Budget explicit totals. | **Partial.** No ordinary UI for arbitrary structured-table schema, totals/range auto-expansion or progressive schema strengthening. New chart rows are explicitly selected, never silently added. | [Tracker runtime](../../apps/designer/runtime/tests/tracker.rs), [import model](../../apps/designer/runtime/tests/interop_document.rs), [report model](../../apps/designer/tests/report-model.test.ts) | #256 → #261: structured range and expansion workflow. |
| 12. Validation/dropdowns | Tracker Boolean true/false dropdown and Rust typed entry; explicit invalid input feedback. | **Partial.** No custom list, enum/range/conditional validation-rule authoring. Boolean projection is not arbitrary dropdown rules. Imported unsupported validation is inventoried. | [Tracker browser](../../apps/designer/e2e/tracker.spec.ts), [hostile inventory](../../apps/designer/tests/fixtures/interop/hostile/README.md) | #256 → #261: required dropdown-rule task scope. |
| 13. CSV/XLSX and fidelity | Imported workbooks export current values/formulas/styles within the bounded profile, after loss acknowledgement; retained source and blocking unsupported inventory. | **Partial; escape gap.** CSV/XLSX export UI is limited to imported workbooks. Native New Tracker/Budget have no corresponding CSV/XLSX export path. CSV is selected-sheet values only; editable charts are not preserved. Browser storage is not portable backup. | [Interop browser](../../apps/designer/e2e/interop.spec.ts), [cleanup/export](../../apps/designer/e2e/interop-cleanup.spec.ts), [ABI admission](../../apps/designer/e2e/interop-abi.spec.ts), [export review](../../apps/designer/tests/interop-export-review.test.ts) | #292 / #256 Gate D: native-authored escape path must be resolved before general product promotion. |
| 14. Cleanup | Imported trim, literal replace, split into two declared Text columns, explicit conversion, missing fill and stable deduplication; revision-pinned preview/commit. | **Partial.** Works only in the declared imported-table profile and existing output slots; not general workbook-wide find/replace or arbitrary structural cleanup. Commit clears session undo. | [Complete bounded cleanup journey](../../apps/designer/e2e/interop-cleanup.spec.ts), [atomic runtime tests](../../apps/designer/runtime/tests/interop_document.rs) | #261: representative messy-workbook task; #256 owns broader missing steps. |
| 15. Charts | Column/line; up to eight charts, 16 explicit rows, three Number series; title/axes/legend; current static PNG; atomic private persistence. | **Bounded implementation complete; promotion unproven.** No editable XLSX chart fidelity, live-linked image or portable configuration. Failed/missing/stale sources refuse output. Chart changes clear prior Tracker undo. | [Actual PNG and reopen journeys](../../apps/designer/e2e/reports.spec.ts), [export race](../../apps/designer/tests/report-export.test.ts), [persistence](../../apps/designer/tests/report-persistence.test.ts) | #261: representative report readability/accessibility and actual shipped-channel validation. |

## All six golden journeys

A “bounded automation PASS” below refers to the linked historical delivery run
and named executable fixture only. Full golden-task acceptance and target-user
results remain separate. All six target-user runs on the new RC are **NOT RUN**.

| # / golden journey | Bounded automated evidence | Full journey disposition and remaining failed/missing step | Owner |
|---|---|---|---|
| 1. Formatted tracker with dropdown rules, sort/filter, edits | [Tracker journey](../../apps/designer/e2e/tracker.spec.ts): 40-row fixture, Boolean dropdown, clipboard, formatting, edits, save/reopen; covered by delivered release gates. | **Partial.** Fixed three-field schema and Boolean constraint do not establish user-defined dropdown rules or arbitrary structural editing. | #256 profile decision; #261 representative participant task. |
| 2. Monthly budget with copied formulas and multiple sheets | [Budget journey](../../apps/designer/e2e/budget.spec.ts): selected fixed/relative/cross-collection references, updates, aliases and reopen; bounded automation PASS in PR #285. | **Partial.** Named-view duplication is not independent worksheet-data duplication; formula-to-scalar and general formula undo remain absent. | #256 calibrated product gap; #261 golden-task verification. |
| 3. Messy import → correct/clean/find-replace/sort/filter → export | [Cleanup journey](../../apps/designer/e2e/interop-cleanup.spec.ts), [ordinary XLSX](../../apps/designer/e2e/interop.spec.ts): bounded fixtures and acknowledged output; automation PASS in PR #286. | **Partial.** Literal column cleanup is narrower than general workbook find/replace; representative file-estate coverage and external-user escape verification are unmeasured. | #261 fixtures/pilot; #256 missing workflow scope. |
| 4. Cross-sheet lookup and grouped summary | [Budget tests](../../apps/designer/e2e/budget.spec.ts) demonstrate bound cross-collection arithmetic and predeclared totals. | **Missing lookup/grouping; partial summary.** No delivered lookup or grouped-aggregation UI/function vocabulary. This golden journey cannot be marked PASS by substituting direct references. | #293: concrete journey owner; Steward readiness/semantic reconciliation remains required. |
| 5. Readable formatted report with chart and sharing | [Report journey](../../apps/designer/e2e/reports.spec.ts): real Rust scalar/formula updates, saved settings, actual PNG signature/decoded pixels, source revision and before/after artifacts; automation PASS in PR #290. | **Partial promotion evidence.** Static PNG route is implemented; target-user readability, keyboard/accessibility and same-shipped-artifact/channel validation are NOT RUN. PNG does not satisfy native project escape or XLSX chart preservation. | #261 report/pilot/distribution acceptance. |
| 6. Close → reopen → change → review → save → reopen without drift | [Tracker](../../apps/designer/e2e/tracker.spec.ts), [Budget recovery](../../apps/designer/e2e/budget-recovery.spec.ts), [interop recovery](../../apps/designer/e2e/interop-recovery.spec.ts), [report persistence](../../apps/designer/tests/report-persistence.test.ts) cover bounded browser-host behavior and refusal cases. | **Partial.** No target-user RC journey; origin-local storage has no portable sidecar backup. Formula/cleanup/chart history resets require explicit task acceptance, not a claim of universal undo. | #261 complete journey; #256 durability/escape scope. |

## Trust, escape and promotion evidence

The release gates exercise stale/invalid atomic refusal, canonical round trips,
formula/cache/date boundaries, source inventory, output admission and private
sidecar preflight. The
[reference evidence](https://github.com/nurockplayer/tachiko-work/pull/286#issuecomment-5550044074)
separates actual LibreOffice checking from synthetic hostile inputs and later
byte-identical replay. It is not Microsoft Excel certification or a measured
representative-estate success rate. Re-run affected checks on the RC identity;
a newly discovered supported-path trust failure blocks promotion regardless of
other passing tests.

| Gate | Current disposition | Evidence still required / owner |
|---|---|---|
| A: capability set | NOT PASSED | Reconcile the concrete partial/missing bundle steps above; #256. |
| B: all six golden tasks | NOT PASSED | Lookup/grouped task is missing (#293); complete representative workflows and genuine user evidence; #256/#261. |
| C: trust | Bounded delivery checks passed; exact RC results in the linked execution record | Exact RC fixture/run results and no unresolved supported-path trust finding; #261. Do not infer universal absence of loss from finite tests. |
| D: escape | NOT PASSED for the whole product | Native-authored Tracker/Budget CSV/XLSX escape (#292) and portable persistence limitations remain; #256/#261. |
| E: ordinary distribution | NOT PASSED | Approved, supported first-party channel usable without repository/toolchain setup, then exercise exactly its artifact; #261 and release owner. Local HTTP preview is preparation only. |
| F: promotion metrics | NOT RUN | Representative target users, common-profile file estate, performance, accessibility/keyboard and locale evidence; #261. |
| Public Product Gate | NOT PASSED | Gates A–F and the actual channel must support the bounded claim. |
| Commercial Gate | NOT ATTEMPTED | Only applicable channel/legal obligations, including #15/#202 when implicated; release/business owner. No paid release is authorized here. |

There are **0 target-user participants/results** in this preparation. Targets
remain ≥85% completing every core task without expert help; ≥90% overall task
completion with no task below 80%; median time ≤approximately 1.5× the familiar
baseline; ≥95% expected-content open/display; ≥90% edit/save without blocking
fidelity issues; ≥99% in-profile formula cells matching reference tolerances;
100% unsupported-construct inventory; zero known silent loss in promotion
fixtures. These are targets, not results. Synthetic fixtures, automated clicks,
operator rehearsal and aggregate CI counts do not populate these denominators.

## Existing R1 preparation and remaining external work

Reuse the [single preparation record](https://github.com/nurockplayer/tachiko-work/issues/256#issuecomment-5547882355)
and [Steward-accepted R1 operator rehearsal](https://github.com/nurockplayer/tachiko-work/issues/256#issuecomment-5548237554).
That rehearsal remains pinned to
`7c831dec642882aa4e0d4447b61d75fb38423a50`; it is not this RC's evidence.
P01 remains an unfilled participant code. Its original conditions remain:
Excel A → standard Chrome Designer B, the validated fresh baseline copy,
dedicated test profile and pre-session save/close/open check, participation
consent and separate recording consent. Do not mix newer builds into R1 or
infer user success, speed advantage or release permission from its acceptance.

After automated preparation, #261 remains open. The next external work needs
an authorized ordinary distribution/channel decision and deployment permission,
then appropriately consented representative user sessions on the identified
candidate. This batch does not authorize public deployment/promotion,
recruitment, new paid services or commercial release. Product gaps above still
need reconciliation even if those external permissions are granted. The two focused follow-ups #292/#293 record actual failed gates after live owner
reconciliation; neither starts a new production lane in this batch.
