# Tachiko experimental Designer client kit

This vendorable browser kit is the bounded `designer-client-kit/v0` experiment.
It is **not** a stable or public SDK, an npm package, a stable wire protocol, or
a compatibility promise. Every exported TypeScript shape, revision spelling,
Worker detail, filename, and browser requirement may change or disappear.

Keep this directory intact. `experimental-client.js` is the only intended
entry; its neighboring Worker modules and `designer_runtime.wasm` are private
runtime assets. Do not import those assets directly or call the raw WASM ABI.

`artifact-manifest.json` records the exact Tachiko Work commit, every packaged
file's SHA-256 digest, the one intended entry, the current exposed client
method inventory, and copied license notices. Keep the manifest and `notices/`
directory with the kit. The manifest identifies the captured source used for
this build; it does not claim that different machines or toolchains produce
byte-identical output.

```ts
import {
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "./vendor/tachiko/experimental-client.js";

const client = createExperimentalDesignerClient();
const directoryInput = document.querySelector<HTMLInputElement>("#project");
if (directoryInput === null || directoryInput.files === null) {
  throw new Error("Select a canonical .roproj directory first.");
}
const bytes = await projectTransferFromFiles(directoryInput.files);
const opened = await client.openProject(bytes);
const table = await client.queryTable(opened.bootstrap.default_collection);

// Use stable targets from a projection and pin every edit to its revision.
const field = table.rows[0]?.fields.find(
  (candidate) => candidate.editable_scalar === "number",
);
if (field) {
  const publication = await client.editNumber(table.revision, field.target, "3");
  const refreshed = await client.queryFields(publication.resulting_revision, [
    field.target,
    ...publication.affected_calculations,
  ]);
  console.log(refreshed); // calculation and diagnostic projections are authoritative
  const exported = await client.exportCanonicalTree(publication.resulting_revision);
  console.log(exported.files); // full canonical v1 path/byte entries
}

await client.closeProject();
await client.close();
```

Number columns may carry a durable display format. Formats affect presentation
only; stored values and calculated results remain numeric. Replace a format or
pass `null` to clear it, using the current table revision:

```ts
await client.setColumnNumberFormat(table.revision, "tasks", "amount", {
  decimals: 2,
  kind: { type: "currency", code: "USD" },
  thousands_separator: true,
});
```

`kind` is `{ type: "plain" }`, `{ type: "currency", code: "USD" }` (three
uppercase ISO-4217 letters), or `{ type: "percent" }`. `decimals` is an integer
from 0 through 10. A column projection includes `number_format` only when a
format is stored.

Date columns may similarly use `await client.setColumnDateFormat(revision, "tasks", "due", "ymd_cjk")`; pass `null` to clear. The formats are `iso`, `mdy`, `dmy`, `ymd_slash`, `ymd_cjk`, `month_day_year`, and `day_month_year`. Column projections include `date_format` when stored. Dates remain dates for comparisons and sorting.

Column alignment, wrapping and width are presentation edits on any column:

```ts
await client.setColumnAlignment(revision, collection, field, "center");
await client.setColumnWrap(revision, collection, field, false);
await client.setColumnWidth(revision, collection, field, 2560);
```

Use the resulting revision for each subsequent call. Width is measured in 1/256 character units (1–65280). Pass `null` to clear any value; for wrap, `false` explicitly disables wrapping and differs from `null`. Each call is one Undo step. Windows expose these values on their columns.

A bounded table window can carry the text the grid shows in Number and Date cells, so a
client renders Work's string instead of formatting again:

```ts
const window = await client.queryTableWindow(revision, collection, view, 0, 256, {
  display: true,
});
window.rows[0]?.fields[0]?.display; // "1,234.50", or undefined for other cell types
```

`field.display` is present on Date cells and stored and calculated Number cells, formatted
through the column's `number_format` with en-US grouping, percent and currency
symbols, or at most 15 significant digits when the column has no format. Dates use the column format or ISO by default. Text,
Yes/No and Reference cells carry none, and the text counts against the
window's reply cap. The option defaults to off, so every other window and
`queryTable` reply is unchanged.

Two view filters compare that same display text, on every column kind including
Reference. `display_contains` trims and folds case; `display_in` is exact text
with no folding, and `""` means blank, so picking `"$1,234.50"` finds every row
that shows it, including 1234.501. A blank, failed, unavailable or Reference
cell counts as blank for now.

```ts
const view = {
  filters: [{ kind: "display_in", field: "amount", values: ["$1,234.50"] }],
};
```

`queryDistinctValues` lists one column's distinct display texts over the rows a
window would return, in Sheet's list order: `""` first when any shown row is
blank, then natural order (case-insensitive, digit runs compared as numbers).
`limit` is 1..=1,000; `truncated` says whether more values exist than the reply
carries.

```ts
const { values, truncated } = await client.queryDistinctValues(
  revision, "tasks", view, "amount", 200,
);
```

`queryViewTotals` totals each Number column over the rows a view shows — the
same rows a window returns, `keep_entities` included — and sums them in
collection order, so the view's sort never changes a float sum. The request
names the view rather than every shown entity id, so a filtered footer stays
small when thousands of rows are still shown. `queryColumnTotals`, which names
entity ids, stays available for any other set.

```ts
const { rows, column_totals } = await client.queryViewTotals(
  revision, "tasks", view,
);
```

`findInView` walks the view for cells whose display text contains the query,
case-insensitively and untrimmed, row by row and then through the shown columns;
a blank query finds nothing. `from` is a `{row, field}` view position that need
not match, and `next`/`previous` wrap. The reply gives the total match count and
the found cell's entity, column, view row and zero-based ordinal.

```ts
const { total, found } = await client.findInView(
  revision, "tasks", view, "acme", ["name", "notes"], undefined, "next",
);
```

Rename a table with `renameTable(revision, collection, name)`, or send
`{type: "rename_table", expected_revision, collection, name}` through
`trackerCommand` alongside `rename_column`. Names use the same admission rules
as table creation. Rename is one undoable revision and keeps the table ID and
formula references stable.

Columns can be reordered with `moveColumn`. `toIndex` is a zero-based position
after the column is removed from its current place, clamped to the last column.
The order is durable document state: it is saved, undone, diffed and exported,
and a new column is appended last. Projections list columns in this order.

```ts
await client.moveColumn(table.revision, "tasks", "amount", 0);
```

Spreadsheet exports follow the moved order. Native Tracker and Budget export
presentations give one style per column in the table's projected column order
at the export revision, as `queryTable` returns it; build them after a move.

Publication replies include `schemas` and `schema_fields` (stable schema ID or
`{schema, field}` ID pairs) for presentation invalidation, independently of
`entities`, `fields`, and `affected_calculations`. Refresh affected collection
or column labels even when the table has no rows. Names never replace IDs as
targets; label-only changes do not invalidate calculations.

`projectTransferFromFiles` packages browser-selected path/byte records only;
the Rust runtime remains the sole parser and authority for `.roproj` meaning.
The frontend may retain disposable revision-keyed projections and ordinary UI
state, but it must not calculate formulas, validate candidates, invent revision
semantics, or mirror an authoritative document.

`exportCanonicalTree(revision)` returns every canonical `.roproj/v1` file,
including empty shards; `openCanonicalTree(files)` submits those opaque bytes
to Rust admission. `exportPortableRo(revision)` returns genuine portable `.ro`
bytes; `verifyPortableRo(bytes)` verifies them without changing the active work,
and `openPortableRo(bytes)` admits them before replacing it. A rejected open
preserves the active occurrence. The canonical-v1 path rejects Date and saved
definitions. Portable export supports both through canonical `.roproj/v3` and
preflights fresh admission in the ordinary v3 runtime profile. Constrained
packages verify but refuse browser-runtime open; capacity-profile tables beyond
the ordinary 128-row limit refuse portable export. `exportProject` remains the
existing private transfer round-trip operation and
its bytes must not be given a public `.ro` filename.

This is the CORE package capability. Sheet attachments and consumer
qualification remain separate obligations under #488.

`observeOccurrence()` returns the runtime's actual `{scope, revision}`. Scope
is stable within one live occurrence and fresh after a successful open. Treat
both values as opaque. A successful export is a snapshot, not confirmation of
a durable host save.

See the repository's first-contact guide for the Product Gap walkthrough:
<https://github.com/nurockplayer/tachiko-work/blob/main/docs/engineering/experimental-designer-client-kit.md>.

For an explicitly selected opaque `.roproj/v3` transfer, call
`client.exportProjectV3(expectedRevision)`. The bytes support Date and saved
grouped-sum definitions together and reopen through `inspectProject` and
`openProject`. They are not canonical-v1 entries or a portable `.ro` artifact.
Ordinary `exportProject` refuses an occurrence successfully opened from v3;
continue saving it with the explicit method. Retain a compatible v3 reader
after emitting these artifacts, including during producer rollback.

### Selection statistics

`querySelectionStatistics(expectedRevision, targets)` reads distinct
`{ entity, field }` targets across collections and returns
`{ revision, count, numerical_count, sum, average, min, max, error_count }`.
It reads retained formula results without changing revision or history or
creating cell projections. Unknown entities, undeclared fields, duplicates and
stale revisions refuse before aggregation. Target order does not affect results.

The request counts against the session's advertised request limit (64 KiB for
ordinary and text-capacity sessions, 64 MiB for working-size sessions), oversized
selections receive `request_too_large`, and clients can compare the encoded
request size against that advertised limit before sending.

| Selected cell | count | Numeric contribution |
| --- | --- | --- |
| Absent optional field | 0 | Skip |
| Text `""` | 0 | Skip |
| Other Text (including `"123"`), Boolean (including false), Date, Reference | 1 | Skip; never coerce |
| Number or formula with retained Number result | 1 | Include; numerical_count +1 |
| Formula Failure or Unavailable | 1 | Skip; error_count +1 |

Any error makes all four aggregates null. Without numbers or errors, sum is 0
and average/min/max are null; an empty selection also has all counts 0.
Currency and percent use raw Numbers without conversion or rounding. An
overflowing sum is null; average uses the formula reducer's overflow-safe
arithmetic, and min/max still cover the numeric members.


## Working-size sessions

A host can ask for the working-size session profile when the occurrence is
created or opened, through `newTableWorkingSize`, `newTrackerWorkingSize`, `newBudgetWorkingSize`, `inspectProjectWorkingSize`,
`openProjectWorkingSize`, `openCanonicalTreeWorkingSize`,
`openLocalDocumentWorkingSize` and `openPortableRoWorkingSize`, and when it
imports a workbook or inspects and opens a saved import, through
`importSpreadsheetWorkingSize`, `inspectImportedProjectWorkingSize` and
`openImportedProjectWorkingSize`. The profile is fixed when the occurrence is
created and cannot be switched afterwards. Not asking changes nothing: every
ordinary method keeps today's request bytes and reply shape.

A working-size import reply (`imported_window`) carries the same metadata and
fidelity ledger as an ordinary import plus one bounded window instead of the
whole table. Imports still have today's geometry and do not move to the text
profile, so a workbook an ordinary import accepts only through that profile may
be refused until a later change.

A working-size reply carries the bootstrap and one bounded window, never the
whole table. That window may hold fewer than 100 rows when the text is large,
so read `window.total_rows` and page with the window query instead of assuming
a full first page. A window over its byte budget says so with
`byte_limited: true` and carries fewer rows: continue at
`start + rows.length`, and use `total_rows` to know where the view ends rather
than treating a short page as the end. A publication that reaches more targets
than a reply lists comes back with `many: true` and empty `entities`, `fields`
and `affected_calculations`: re-query the visible window and totals instead of
reading the lists. Both keys are absent on ordinary and text-capacity replies.

A working-size publication also carries `changed`, one field per changed cell
in the same shape a window's `rows[].fields` uses, with `display` text for
Number cells. Redraw those cells from `changed` with no second call. `fields`
(and `affected_calculations`) still names the changed cells, so a host can
decide which sort and filter keys a redraw affects. A removed row's cells are
never in `changed`: read `entities` for the rows that disappeared. Footer
totals are still a separate totals query, and a reply that says `many` must be
re-queried as a window instead of read for `changed`. On ordinary and
text-capacity replies the `changed` key is absent too.

A working-size session takes a whole column in one request: `edit_cells` and
`copy_formula` accept up to 100,000 targets, the session's populated-cell
limit, and a request may be up to 64 MiB. `paste_cells` is the range form for
pastes and can create rows when `start_entity` is omitted; a paste is still
admitted by the same row and cell limits. A batch over the listed target count
replies `many: true`, as above. Ordinary and text-capacity sessions keep
today's 1,024-target and 64 KiB limits.

A working-size session admits 10,000 rows and entities,
100,000 populated cells, 100,000 formulas, 1,000,000 formula AST nodes and
1,000,000 dependency edges, with one column read counting as one edge. These
are the counts a session admits, not a performance claim, so a host should not
advertise them until later stages land.

Every state a working-size session accepts can be saved through both
`exportProjectV3` and `exportPortableRo`. The runtime accounts for the bytes
those writers produce and refuses an edit that would push the saved size past
its bound with `capacity_exceeded` and `limit: "saved_bytes"`, before the edit
publishes: the last accepted revision stays saveable, and the same bound
refuses an open or import that could never be saved. The 8 MiB local-document
export and the spreadsheet export's 128 rows per table are bounded exports,
not session saves, and ordinary sessions are unchanged.

## Authoring formulas

`updateFormula(revision, target, source)` submits source text against the
expected revision. The runtime parses it, binds `[entity.field]` references
(canonical lowercase **keys**, not display names or row labels) to stable IDs,
type-checks them, rejects cycles, and evaluates the result. A refused update
publishes nothing and leaves the previous formula source, calculated value, and
revision unchanged.

```ts
const publication = await client.updateFormula(
  table.revision,
  rentReserve,
  "[rent.expense] * [settings.rate]",
);
const copied = await client.copyFormula(publication.resulting_revision, {
  source: rentReserve,
  destinations: [adsReserve, toolsReserve],
  fixed_references: [settingsRate],
  relative_rows: true,
  relative_columns: false,
});
console.log(copied.affected_calculations); // refresh these projections
```

Every accepted destination receives its own canonical, fully parenthesised
`formula.source` (`([ads.expense] * [settings.rate])`). Treat that string as a
copyable projection of the current document, not as numeric-token authority or
a stable spelling contract. `duplicateCollection` rewrites a reference only
when both its entity and its field belong to the copied collection, so a
duplicated plan reads its own rows but keeps reading the original shared
settings row.

`addTable(revision, name, columns)` appends an empty table and returns its collection and publication. `deleteTable(revision, collection)` removes a table in one undoable edit. Deleting the last table is refused with `last_table`; references from other tables or saved definitions are refused with `table_referenced` and diagnostics identify their owners.

A same-row shorthand names a field on the formula's own row, so one source can
be copied without per-row rewriting:

```ts
await client.updateFormula(revision, rentReserve, "[.expense] * [settings.rate]");
// rentReserve.formula.source === "([.expense] * [settings.rate])", value 8000
await client.copyFormula(nextRevision, {
  source: rentReserve,
  destinations: [adsReserve, toolsReserve],
  fixed_references: [],
  relative_rows: true,
  relative_columns: false,
});
// ads/tools keep the same source and calculate 3000/1000.
// duplicateCollection keeps it too: a copied plan reads the copy's rows.
```

The shorthand stores only the field identity; the owner row is resolved by the
formula itself, in memory and after a v3 save/reopen. A missing field refuses as
`formula_unknown_reference`, a non-Number field as `formula_type_mismatch`, and
referring to the formula's own field (`[.reserve]` inside `reserve`) as
`formula_cycle`.

A column total names one Number column and sums it over every row of that
table. The table projection already carries the same numbers as a footer:

```ts
const table = await client.queryTable("plan");
table.column_totals; // { [columnId]: 12000 }, same revision as table.rows
await client.updateFormula(
  table.revision,
  summaryTotal,
  "[plan.reserve:total] + [plan_copy.reserve:total]",
);
// A summary formula reads both tables' footer totals.
```

`column_totals` has one entry per Number column, keyed by column ID, and is
`null` when any contributing value is unavailable; blank cells are skipped, as
in the formula. The reference is identity-based, so renaming the table or the
field keeps the formula source valid. Referencing a column total from that same
column (`[plan.reserve:total]` inside a `plan.reserve` cell) refuses as
`formula_cycle`, and `duplicateCollection` moves a total of the copied table's
own column onto the copy.

See the
[formula authoring reference](https://github.com/nurockplayer/tachiko-work/blob/main/docs/engineering/formula-authoring-reference.md).
ADR-0014 and the formula engine specification remain the language authority.
