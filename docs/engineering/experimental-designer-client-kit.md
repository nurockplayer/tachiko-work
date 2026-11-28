# Experimental Designer client kit: first contact

Status: bounded experimental kit, with producer and canonical I/O qualification
under Issue #359. This is not a stable/public SDK, npm package, wire protocol,
plugin ABI, or compatibility promise. The kit may change or disappear after
the first external-frontend pilot.

The browser frontend receives **projections**: derived semantic facts intended
for rendering. Every edit carries an **expected revision**, so a stale UI cannot
silently overwrite a newer authoritative state. The resident Rust **runtime is
the authority**: frontend code never parses semantic meaning, recalculates
formulas, validates candidates, or owns canonical state.

## Export and vendor the kit

From a fully clean Tachiko Work checkout with Rust, `wasm32-unknown-unknown`, a
C compiler, and pnpm 12.11.2 installed:

```sh
bash scripts/export-experimental-designer-client.sh \
  /path/to/external-ui/vendor/tachiko
```

The destination must be absent. The exporter rejects staged, modified, or
untracked source files, resolves the full Git commit ID, materializes and
verifies that committed tree, and builds only from that private snapshot. It
then publishes a completed kit with a platform no-replace rename; if another
writer creates the destination first, the export fails without replacing it.

Each source commit uses one exclusive, deterministic temporary build directory
per user. This keeps Cargo identity stable for local dependencies outside the
runtime workspace. An occupied directory makes the exporter refuse the build
and is left intact; it may belong to another export or an interrupted build.
The exporter cleans only the directory it created, and outputs cannot be placed
inside that directory. Source-path remapping remains enabled. Repeat equality
applies to the same runner/toolchain, not different machines or toolchains.

The command compiles and copies one self-contained browser kit:

```text
vendor/tachiko/
├── experimental-client.js       # intended ESM entry
├── experimental-client.d.ts     # TypeScript surface
├── experimental-client.worker.js
├── designer_runtime.wasm
├── host/                         # generated private support modules
├── runtime/                      # generated private support modules
├── package.json                  # ESM/artifact metadata; private, unpublished
├── README.md                     # instability and authority boundary
├── notices/                       # copied existing project and third-party notices
└── artifact-manifest.json         # source identity and SHA-256 asset inventory
```

Keep the directory together and import only `experimental-client.js`. Do not
call the Worker support modules or WASM exports directly. Verify the manifest
before consuming the kit: it lists every regular file other than the manifest
itself, rejects undeclared assets, records the full source commit without
requiring a remote link, names the currently exposed client methods, and names
the copied notices. It is an artifact integrity record, not a cross-toolchain
reproducibility or compatibility claim. The method inventory records
the experimental artifact surface. Use the explicit canonical/portable methods
described below; private `exportProject` bytes are not a portable `.ro` file.

## Open Product Gap and render the first table

Use the repository-owned sample at
[`dogfood/product-gaps.roproj`](../../dogfood/product-gaps.roproj/). A browser
directory input supplies its opaque path/byte records; `projectTransferFromFiles`
does not parse `.roproj`, and Rust performs canonical admission.

```html
<input id="project" type="file" webkitdirectory />
```

```ts
import {
  DesignerRuntimeError,
  createExperimentalDesignerClient,
  projectTransferFromFiles,
} from "./vendor/tachiko/experimental-client.js";

const input = document.querySelector<HTMLInputElement>("#project");
if (input === null || input.files === null) {
  throw new Error("Select a canonical .roproj directory first.");
}
const client = createExperimentalDesignerClient();
const transfer = await projectTransferFromFiles(input.files);
const opened = await client.openProject(transfer);
const table = await client.queryTable(opened.bootstrap.default_collection);

for (const row of table.rows) {
  console.log(row.key, row.fields); // typed stored/calculated/diagnostic projections
}
```

`openProject` transfers its input buffer to the Worker. Copy the buffer first if
your host needs to retain it.

## Bounded table windows

`queryTable` stays the complete-table read and refuses a collection above 1,000
rows with the `use_window_query` failure; `failure.row_count` carries the actual
count so the host never parses the message. Nothing is truncated.

Large tables use a stateless window instead:

```ts
const view = {
  sort: [{ field: amountColumn.id, direction: "desc" as const }],
  filters: [
    { kind: "number_compare" as const, field: amountColumn.id, op: ">=" as const, value: 5 },
    { kind: "text_contains" as const, field: nameColumn.id, value: "beta" },
  ],
  keep_entities: [pinnedRow.id], // at most 256, visible even when filtered out
};
const window = await client.queryTableWindow(revision, collection, view, 0, 256);
const located = await client.locateInView(revision, collection, view, pinnedRow.id);
// located = { revision, result: "found", index: 12 } or { result: "filtered_out" }
```

To total exactly the rows a view shows, pass the view itself to
`queryViewTotals`. It sums each Number column over the view's rows — filtered
rows left out, `keep_entities` rows kept in — in collection order, so the
view's sort cannot change a float sum, and returns
`{ revision, rows, column_totals }` with the same null and blank-cell rules as
the window footer. The request names the view instead of every shown entity, so
a filter that still shows thousands of rows stays well under the request size
limit. A stale revision or an unknown view column refuses with no change. The
window's `column_totals` always cover the whole collection, so this is the way
to total a filtered footer.

```ts
const shown = await client.queryViewTotals(revision, collection, view);
```

`queryColumnTotals` naming entity ids stays available for every other set: it
sums the named entities in collection order, and naming every entity of the
collection gives the window footer's totals exactly. The request accepts any
set of unique ids of that collection, up to all of them; an unknown, duplicate
or foreign id, or a stale revision, refuses with no change.

```ts
const allIds = tableRows.map((row) => row.id);
const whole = await client.queryColumnTotals(revision, collection, allIds);
```

`count` is 0..=256 and `0` returns the columns and totals only. `start` clamps
past the end instead of failing, so a shrinking view never turns a scroll into an
error. The reply repeats `revision`, `collection`, `columns`, a
`columns_revision` digest that changes only when a column definition changes,
`total_rows` (after filters and kept entities), `unfiltered_rows`, the clamped
`start`, and the window's `rows` with the same row/field projections as
`queryTable`. Each row carries `position`, its one-based stored position in the
collection; under a filter or a view sort the numbers can have gaps or run out
of order. A host's row number is `position` mapped through the table's header
state (ADR-0048 §5): position 1 is row 2 under a header row and row 1 in a
headerless table, and a host uses that number for A1 geometry instead of the
displayed index. `locateInView` and `findInView` replies carry the same
`position`.

The view is request state owned by the host: it never enters the document,
publication history or revision token. Sort keys apply in order with blanks last
and a stable entity-ID tie-break; filters are AND-ed. A sort key orders Text
with `text_order`: `bytes` (the default, UTF-8 byte order) or `natural`, which
is case-insensitive with runs of ASCII digits compared as numbers, and sorts
accented letters and CJK by code point. `text_order` has no effect on any other
column kind. Text filters compare
case-insensitively and are text-only, number and date filters use
`= <> < <= > >=`, `is_blank`/`not_blank` find missing or present values, and
`in_set` takes literals of the column's declared kind. Formula columns filter and
sort by their calculated Number.

`display_contains` and `display_in` compare the text the grid shows instead, on
every column kind including Reference. `display_contains` trims and
case-folds; `display_in` is exact text with no folding, so `""` means blank and
`"$1,234.50"` finds every value that rounds to it. A blank, failed, unavailable
or Reference cell has the text `""`, and a failed cell counts as blank until
#668 gives it a defined state. Present empty Text in an optional column shows as
two quotation marks, as in Sheet, so it stays apart from a blank cell.

`queryDistinctValues` returns one column's distinct display texts over the rows
a window would return, in Sheet's list order: `""` first when any shown row is
blank, then natural order (case-insensitive, digit runs compared as numbers).
`limit` is 1..=1,000, the reply stays inside the window byte cap, and
`truncated` reports whether more values exist.

`findInView` finds matching cells the way Sheet's Find does: a cell matches when
its display text, lowercased, contains the query lowercased, with neither side
trimmed, and a blank or all-whitespace query matches nothing. Matches run row by
row, then through `fields` — the shown columns in shown order, 1..=32 column IDs
of the collection with no repeats. `from` is a `{row, field}` cell position (a
zero-based view row index and a column ID) that need not itself match, so a
stale position is admissible; `next` and `previous` take the following or
preceding match and wrap at either end. The reply carries the revision, `total`
(every matching cell in the view) and `found`, with its entity, column, view row
and zero-based `ordinal` — Sheet's "n of m". Blank, failed, unavailable and
Reference cells match no text.

A window above the reply cap (1 MiB, or 16 MiB for a text-capacity
document) refuses with
`query_too_large` rather than serialising an oversized response; ask for
fewer rows or fewer columns.

A window can ask for the text the grid shows in its Number cells:

```ts
const window = await client.queryTableWindow(revision, collection, view, 0, 256, {
  display: true,
});
window.rows[0]?.fields[0]?.display; // "1,234.50", or undefined for a non-Number cell
```

`field.display` is present on stored and calculated Number cells only, formatted
through the column's `number_format` exactly as Sheet renders it (en-US
grouping, percent and currency symbols; at most 15 significant digits without a
format). Text, Date, Yes/No and Reference cells carry none. The display text is
counted against the reply cap, and the option defaults to off: a window without
it and every `queryTable` reply encode exactly as before.

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


`querySelectionRangeStatistics(expectedRevision, range)` sends the alternative
compact request below. `first` and `last` are inclusive row EntityIds in the
collection's **stored order**, in either anchor order: ranked rows by rank, then
unranked rows by EntityId, matching the unsorted table window. `fields` is the
ordered list of FieldIds. This is a rectangle, independent of view sorting or
filtering; whole-column selection still requires both anchors.

```ts
await client.querySelectionRangeStatistics(revision, {
  collection, first: firstRowId, last: lastRowId, fields: [amountFieldId, taxFieldId],
});
// Wire: { type: "query_selection_statistics", expected_revision: revision,
//         range: { collection, first, last, fields } }
```

Exactly one of `targets` or `range` is required. Both or neither, unknown or
foreign anchors, undeclared fields and duplicate fields refuse atomically with
`invalid_view`; stale revisions retain `stale_query`. Unknown collections retain
the existing collection refusal. An empty field list selects no cells, but the
anchors must still belong to the collection. Expansion uses the existing
per-cell projection and aggregation semantics, including formula results and
canonical summation order; the response shape is unchanged.

There is no additional statistics-specific cell/work cap. Expansion stays within
the admitted document's row and field bounds (working-size: 10,000 rows per table,
32 fields and 100,000 populated cells), including declared absent cells. A compact
10,000-row × 2-field request is under 1 KiB for ordinary imported identities,
independent of row count, and fits the 65,536-byte ordinary request cap. The
10,000-row document itself requires the existing working-size import profile;
compact requests also work in ordinary and text-capacity imported sessions.

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


## Grouped totals

`queryGroupTotals` sums Number columns grouped by one or two Text, Date or
Yes/No columns over the rows one scope covers. The request names the grouping
dimensions, the Number measures and the scope; the runtime resolves every scope
to collection order before summing.

```ts
const totals = await client.queryGroupTotals(
  revision,
  collection,
  [
    { field: categoryColumn.id },
    { field: dateColumn.id, bucket: "month" }, // "value" | "month" | "year"
  ],
  [amountColumn.id], // one to four Number columns, request order kept
  { kind: "all" }, // or {kind: "entities", entities: [...]}, {kind: "except", entities: [...]}
);
```

`scope` is `all`, `entities` (exactly these rows) or `except` (every row but
these); send the smaller list. The reply echoes `dimensions` and `measures`,
reports `rows` in scope and one `totals` entry per measure over the whole
scope, plus `groups` ordered by their keys ascending:

```ts
type GroupTotalsProjection = {
  revision: string;
  dimensions: GroupDimension[];
  measures: string[];
  rows: number;
  totals: (number | null)[];
  groups: { keys: GroupKey[]; rows: number; totals: (number | null)[] }[];
};
type GroupKey =
  | { kind: "boolean"; value: boolean }
  | { kind: "date"; value: string } // "YYYY-MM-DD"
  | { kind: "month"; value: string } // "YYYY-MM"
  | { kind: "year"; value: string } // "YYYY"
  | { kind: "text"; value: string }
  | { kind: "blank" };
```

Text keys use exact equality: no trim and no case folding, so `"Food"` and
`"food"` are different groups and an empty text is its own group, distinct from
blank. Each dimension has one blank group, which sorts last. Groups are ordered
by key ascending: Boolean `false` before `true`, Date/Month/Year
chronologically, Text by Unicode scalar value, and blank after every present
value. A `bucket` other than `"value"` is meaningful only for a Date column;
Text and Yes/No columns always group by exact value.

Each group's `rows` counts every member, including rows whose measure cell is
blank; blank measure cells are skipped when summing. A total is `null` only
when an unavailable contributor nulls it — there are never partial sums.

At most 200 groups and a 64 KiB reply payload (the projection itself, measured
as JSON) are allowed; requests are limited to 64 KiB. Refusals use
`group_limit_exceeded`, `result_size_exceeded`, `invalid_group_query`,
`stale_revision` and `missing_collection`, with no change.

## Optional native columns

`promoteFirstRowToHeaders(expectedRevision, collection)` sends
`{type:"promote_first_row_to_headers", expected_revision, collection}`. It
promotes the first row in stored order of a headerless collection, independently
of the host's current sort/filter, and returns `published`. The row leaves the
data, `headerless` becomes false and `data_start_row` becomes 2. Re-query the
window and columns after publication. Field IDs, keys, declared types (including
Auto), surviving values and bound column references remain unchanged; this
command does not infer or retype columns.

Headings use the cell's displayed scalar text and ADR-0043's frozen whitespace
trim and spelling rules. Empty Text and missing values are blank headings.
The first occurrence of each exact trimmed name keeps its spelling. Blank or
later duplicate headings receive their positional `Column A`, `Column B`, …
label, with ` (2)`, ` (3)`, … appended when needed. All real heading names are
reserved before allocation. The reply's optional
`header_fallbacks: {field, name, reason:"blank"|"duplicate"}[]` reports every
generated heading in column order. Other invalid name spellings use the
existing naming refusal. A headed table refuses with `table_already_has_headers`;
an empty headerless table refuses with `no_header_row`. Refusals publish nothing.
One Undo restores the complete original headerless schema and first row;
Redo reapplies promotion. Both v3 and portable `.ro` preserve the promoted state
using the existing persisted representation (#924, #668 PR4).

Under [ADR-0045](../decisions/ADR-0045-optional-native-text-and-number-fields.md),
`newTable(name, columns)` accepts `required?: boolean` per column. Omission
defaults to `true`; any scalar column (Text, Number, Date or Yes/No) may be
declared optional ([#598](https://github.com/nurockplayer/tachiko-work/issues/598)). The same
property is accepted by the `add_column` command through `trackerCommand`.
An optional added column needs no `initializers`; a supplied subset fills only
those entities. `insert_row` may omit optional initializers, leaving those slots
blank. Required initializers remain mandatory; neither path invents defaults.

`insert_row` also accepts optional `position: {before: entityId}` or
`position: {after: entityId}` through `trackerCommand`. The anchor must belong
to the named collection. Both keys, an unknown anchor, or a foreign anchor
refuse without publication or history changes. Omission appends as before.
The optional positive integer `count` defaults to one. A count greater than one
inserts a batch of blank rows and requires a `before` anchor and empty `initializers`.
Revision, capacity, required-field and formula-fill admission are unchanged.
Ranks and the new rows publish in one revision, with one Undo restoring exact prior
ranks (including absence) and one Redo replaying the forward facts. Window,
locate, Find, save/reopen and XLSX/CSV export follow that stored order.
Tracker `append_row` keeps its existing profile and refusal contract.

```ts
await client.trackerCommand({type: "insert_row", expected_revision: revision,
  collection: collectionKey, position: {before: rowId}, initializers: [], count: 3});
```

Use stable IDs and the current revision from projections:

```ts
await client.clearField(revision, {entity: rowId, field: noteFieldId});
// A different operation: stores present empty Text, never clears.
await client.editText(nextRevision, {entity: rowId, field: noteFieldId}, "");
await client.setColumnRequiredness(latestRevision, collectionKey, noteFieldId, false);
```

`clearField` encodes `edit_scalar` with exactly `input: {kind: "clear"}`.
`setColumnRequiredness` encodes `set_column_requiredness` with `collection`,
`field` and `required`. Both are revision-pinned ordinary publications. Clearing
a required field refuses. Strengthening (`required: true`) refuses while any
entity is blank; it never fills or coerces. Ordinary final validation can also
refuse a clear that breaks a surviving formula. An already-blank clear is
`no_change`. Rejected/no-change attempts add no history; `undo`/`redo` through
`trackerCommand` restore exact prior presence and values as forward operations.
When strengthening is refused for blank values, the failure code is
`required_values_missing`; its single diagnostic identifies the column and
reports the authoritative blank count. The message says how many rows have no
value in the column.

## Auto scalar columns

Create an Auto field with `{name: "Value", field_type: "auto", required: false}`
through `newTable`, `newTableWorkingSize`, or `add_column`. Auto is always
optional and unconstrained; strengthening it to required is refused. Windows
project `field_type: "auto"` and `editable_scalar: "auto"`, including blanks.
Each present cell uses the existing tagged `stored` union.

```ts
await client.editAuto(revision, {entity: rowId, field: valueFieldId}, "00123");
await client.editAuto(nextRevision, {entity: rowId, field: valueFieldId},
  "22/11/2026", "day_first");
```

The wire input is `{kind: "auto", input: string, date_order?: "month_first" |
"day_first"}`. Work runs its existing classifier: `42` is Number, `00123` is
exact Text, ISO dates are Date, `TRUE` is Boolean, and other text stays exact.
Full-width, grouped, currency and percent input follow that same classifier;
implied formats are per-cell presentation and survive portable `.ro` save/reopen
and the same Undo/Redo action as the value. Raw paste into Auto uses the
classifier without a date order; typed paste cells may supply `date_order`.
Raw empty input clears; explicit `{kind: "text", value: ""}` stores empty Text.

Auto input starting with `=` binds through Work's existing formula source
grammar. A1 clients can use `translate_a1_formula` for existing references
before submitting the translated source. The published cell carries its
formula and a tagged Number, Date, Text or Boolean result. Read the tag in
`calculated.result`; the numeric `calculated.value` slot uses zero for Text and
Boolean. Auto evaluation failures publish in `calculated` with status
`failure`, the error code and its display text, such as `#VALUE!`. A declared
formula that fails still refuses the edit atomically. The bound formula survives
v3 and portable `.ro` reopen and is recalculated on admission. Formula authoring
is one Undo step. Auto retyping and aggregates remain separate slices. `exportPortableRo` and v3 exports preserve
Auto; frozen legacy project/direct-ro and v1 delta/conflict writers refuse it.
The v2 evidence contracts preserve Auto fields and their tagged values.

## Changing a column's type

`changeColumnType(expectedRevision, collection, field, fieldType)` encodes
`change_column_type` with the column's stable ID and one of `text`, `number`,
`boolean` or `date`. The whole column changes type as one revision-pinned
publication and one `undo` step: every stored value is converted, then the
definition keeps its ID, key and display name, keeps its `required` flag, keeps
`number_format` only when the new type is Number, and resets its constraint.
A column that already has the target type refuses.

The command converts present values and never invents data. An empty Text cell
becomes blank in an optional column; in a required column it refuses.
Any conversion the column cannot express refuses the whole command without
changing anything, and the message names the row, column and value, for example
`row 3 of column 'amount' holds 'twelve': conversion requires an unambiguous
finite decimal Number`. A column holding formula cells, or one a surviving
formula reads, refuses: a calculated column keeps its type. There is no preview
in this slice; use the returned `PublicationProjection` for the new revision.

`ScalarEditInput` adds `{kind: "clear"}` alongside typed sets. `edit_cells`
can mix both atomically. `paste_cells.rows` accepts `PasteCellInput[][]`:
each cell is a raw string or a typed scalar input, including explicit clear.
Raw `""` in Text means present empty Text; raw `""` in Number refuses.
For example, `[["", {kind: "clear"}]]` sets empty Text in the starting
Text column and clears the next optional column. Untargeted cells stay unchanged;
omitted optional fields of newly pasted rows remain blank.

### Moving cells (`move_cells`)

Send `{type: "move_cells", expected_revision, moves: [{from, to}]}` through
`trackerCommand`, where both targets are `{entity, field}` stable IDs. The reply
is the same `PublicationProjection` as `edit_cells`. Cut remains pending in the
client, including Esc cancellation: send nothing until Paste.

The runtime resolves all targets and captures their original contents, assigns
each destination its source's original contents, then clears only sources that
are not destinations. The whole batch is one revision and one Undo/Redo entry;
Undo restores overwritten destinations as well as cleared sources.

| Case | Behaviour |
| --- | --- |
| Overlap, shift in either direction, swap or cycle | Allowed; all reads use originals, and destination assignment wins over source clearing. |
| Duplicate source or destination | Refuse the entire request. |
| Destination field type differs from the source value type | Refuse atomically, without Text/Number/Date coercion. |
| Required source would become blank | Refuse; allowed when that source also receives another move. |
| Blank source | Move absence; the destination must admit an `edit_cells` clear. |
| Across collections in one document | Allowed when both sides pass normal admission. |
| Formatting/style | Stay at their existing targets; destination formatting is preserved. |
| All moves are self-moves, or all final contents already match | Existing `NoChange` refusal; no revision or history entry. |
| Empty moves or union of affected targets exceeds the session's batch limit | Refuse, as for `edit_cells`. |
| Stale revision, unknown entity, undeclared field or profile refusal | Same `DesignerError` variants as `edit_cells`, with no partial effect. |

Formulas move as formulas with their dependencies unchanged. Explicit and
column references stay unchanged; a same-row reference in a formula moved to a
different row becomes an explicit reference to its original source row. Other
formulas keep their existing targets, with ordinary validation and calculation
refusals applied to the final candidate.

Table columns (`ColumnProjection`, also exported by the public TS entry) expose
`display_name_explicit?: true`: Rust emits `true` only when a stored display
name exists and otherwise omits the member; the client rejects any other value,
including `false`. This uses the imported export rule,
including a rename whose spelling equals the generated key. With retained import
metadata, use `column.display_name_explicit === true ? column.display_name :
metadataColumn.name`; join by stable field ID. A current field absent from
metadata needs an explicit name or metadata export refuses. Undo removes the
flag when it restores name absence; v3/package reopen preserves stored presence.
Native `newTable`/`add_column` create explicit names, including names equal to
their keys; built-in Tracker/Budget templates currently have no explicit names.
The kit manifest's `capabilities` includes `columnDisplayNameExplicit` as a
projection feature marker (not a callable method). Consumers must detect that
marker before relying on omission to mean fallback in older kits. The client
preserves `true` or omission and rejects every other defined value; Rust emits
only `true` or omission. Storage formats and rename semantics are unchanged.

`duplicateCollection(expectedRevision, collection, name, columnNames?, toIndex?)` can name
the copy's columns in the same command. `columnNames` maps each source column's
stable field ID to the copy's display name; a name passes the same spelling rule
as renaming a column. A blank name, two copy columns with the same effective name, or a
source ID that is not a column of the table refuses without publication. The
names are part of the copy's single history step, so one Undo removes the copy
with its names and Redo restores both. Omitting `columnNames` or passing `{}`
keeps the source names. An imported CSV or XLSX table has no stored names; pass
the header text to give the copy its headers.

Pass `toIndex` to place the copy at a zero-based worksheet index, clamped to
the end. Omit it to retain the existing placement behavior. The destination
is part of the copy's single history step and is retained by v3 and portable
`.ro` saves and XLSX export and reimport. The copy, Undo and Redo publish the
current import metadata for imported workbooks.

`rangeToTable(expectedRevision, range, name, hasHeader)` copies a stored
rectangle into a new table. The range uses the same collection key, inclusive
row IDs, and field IDs as `querySelectionRangeStatistics`. Supply a contiguous
slice of fields in source column order. With `hasHeader` enabled, the first
selected row supplies displayed header text and is omitted from the copied
data. Blank and repeated headings receive generated names, as in
`promoteFirstRowToHeaders`. With it disabled, the table uses generated names
(`Column A`, `Column B`, and so on) and copies every selected row.

The command returns the new collection and its publication. The source stays
intact. A copied formula follows its copied cell dependencies and keeps
dependencies outside the copied data bound to the source. Whole-column
dependencies follow the copy only when their full row and column domain was
copied. One Undo removes the table, Redo restores its identities, and a save
and reopen retains them.

`moveCollection(expectedRevision, collection, toIndex)` moves an existing
worksheet identified by its collection key. Read the new order from
`bootstrap().collections`. This edit is one history step and is carried by v3
and portable `.ro` saves and native-table or imported-workbook XLSX export. The
kit manifest advertises the `moveCollection` capability.

Table columns always project `required: boolean`. Each row's `fields` array
contains **only present values**. `blank_fields?: string[]` names absent declared
FieldIds in column order; omission means `[]`. Join by stable ID rather than
zipping columns and fields by index. `queryFields` can inspect an absent declared
slot: its `FieldProjection.blank` is `true` and `stored`, `formula`, and
`calculated` are null. Omitted `blank` means `false`; empty Text remains a
present `{kind: "text", value: ""}`. These omission defaults are preserved in
client replies, not materialized as extra payload members. Worker reply
validation rejects malformed presence metadata. Optional Date/Boolean authoring,
blank coercion and skip-blank calculation are outside this surface.

## Field constraints (v3 list and number range)

`setColumnConstraint(expectedRevision, collection, field, constraint)` replaces
one durable constraint on a native Text or Number column:

```ts
await client.setColumnConstraint(revision, collectionKey, statusFieldId, {
  kind: "text_literal_set",
  values: ["Doing", "Done", "Todo"],
});
await client.setColumnConstraint(nextRevision, collectionKey, amountFieldId, {
  kind: "number_inclusive_range",
  min: 0,
  max: 100,
});
// Clear the declaration; the column becomes unconstrained again.
await client.setColumnConstraint(clearedRevision, collectionKey, statusFieldId, {kind: "none"});
```

The literal list is a complete closed set, as the semantic declaration requires:
values are non-empty, unique, byte-sorted ascending (`["Doing", "Done", "Todo"]`,
not `["Todo", ...]`), and bounded; range bounds must be finite and ordered.
A constraint kind that does not match the column type is refused. The sets are
closed: only an explicit `{kind: "none"}` clears a constraint, and calling the
command with the constraint already in force is `no_change`.

`ColumnProjection` gains `constraint?: FieldConstraint`:
`{kind: "text_literal_set", values}` or
`{kind: "number_inclusive_range", min, max}`. Omission means unconstrained.
A literal set also fills the existing `dropdown_options` property, so it can
drive a select control without a second lookup; range columns project no
dropdown. The client validates the shape and rejects malformed constraints,
but never normalizes, sorts or widens the declaration.

An edit that violates a constraint publishes nothing and rejects with
`DesignerRuntimeError`. Its `failure.code` is `field_constraint_violation` and
`failure.constraint_failure` carries the typed shape:
`{reason: "field_value", schema, field, constraint, offending_rows}`. Declaring
a constraint that existing rows violate is refused the same way with
`code: "existing_values_violate_constraint"`, `reason: "existing_values"` and
the offending row count; the declaration is not applied. The UI can explain
allowed values or bounds from `constraint_failure.constraint` without parsing
the message.

Constraints are stored only in `.roproj/v3`. A constrained project continues to
save through `exportProjectV3`/`exportCanonicalTree` and reopens with its
declarations and values intact, but legacy `exportProject` (v1/v2) refuses with
`code: "legacy_export_unsupported"` instead of silently dropping constraints.
The kit manifest's `capabilities` includes `columnConstraintProjection` as a
projection feature marker (not a callable method); consumers must detect it
before relying on `ColumnProjection.constraint` in older kits.

## Keyed grouped-sum totals

`queryKeyedGroupedSum` and `createKeyedGroupedSum` return
`KeyedGroupedSumProjection`, which now carries an authoritative overall total:

```ts
const summary = await client.queryKeyedGroupedSum(definitionId);
for (const {category, value} of summary.groups) renderCategory(category, value);
renderTotal(summary.total); // number | null
```

`total` is the sum of `groups` at the same `revision`; Work derives both from one
evaluation, so a client never adds groups itself or reads a cached total.
`total` is `null` whenever the result is unavailable (missing or ambiguous keys,
invalid input, an invalid definition) and `groups` is then empty with
diagnostics explaining why. `null` is never a zero or a previous revision's
value, so render it as "unavailable", not as 0. An available result with no
contributing rows reports `total: 0` and `groups: []`. If the aggregate
cannot be represented as a finite number (overflow), `total` is `null` while the
already-evaluated `groups` remain; `null` still means "unavailable", never a
corrected or clamped number.

## Publish one revision-safe edit and observe authority

Targets come from projections and contain stable semantic IDs. Do not construct
targets from table indexes, paths, or JSON pointers.

```ts
const row = table.rows.find((candidate) => candidate.key === "designer_profile_bound")!;
const impactColumn = table.columns.find((column) => column.key === "impact")!;
const priorityColumn = table.columns.find((column) => column.key === "priority")!;
const impact = row.fields.find((field) => field.target.field === impactColumn.id)!;
const priority = row.fields.find((field) => field.target.field === priorityColumn.id)!;

const publication = await client.editNumber(table.revision, impact.target, "3");
const observed = await client.queryFields(publication.resulting_revision, [
  impact.target,
  priority.target,
]);

console.log(observed.fields); // priority calculates to 8; diagnostics come from Rust
const exported = await client.exportCanonicalTree(publication.resulting_revision);
```

The same client exposes revision-pinned `editText` and `editBoolean`. A stale
call throws `DesignerRuntimeError` with `failure.code === "stale_revision"`;
query or export the reported current revision instead of rebasing the edit in
frontend code. `exportCanonicalTree(revision)` requires the exact current
revision and returns `{revision, files: [{path, bytes}]}` containing all 18
canonical v1 files, including empty shards. Rust owns their contents.
`openCanonicalTree(files)` passes the opaque entries back to Rust admission.

For a genuine portable `.ro` file, use the existing storage codec through:

```ts
const portable = await client.exportPortableRo(publication.resulting_revision);
await client.verifyPortableRo(portable.bytes); // verifies without changing active work
const reopened = await client.openPortableRo(portable.bytes.slice(0));
const occurrence = await client.observeOccurrence();
console.log(reopened.bootstrap.revision, occurrence.scope);
```

Open installs a candidate only after successful admission. Rejected candidates
preserve the previous occurrence and data. Occurrence scope comes from the
resident runtime, stays stable for that occurrence, and changes on successful
open; it is not a document ID. Revision and scope remain opaque tokens.

The canonical-v1 path rejects Date and saved definitions. Portable-v1 export
supports both through canonical `.roproj/v3`, with fresh-admission preflight in
the ordinary v3 runtime profile. Constrained packages verify but refuse open in
the browser runtime. Capacity-profile tables exceeding the ordinary 128-row
limit refuse portable export. Opening a supported portable package preserves
v3 origin, so ordinary `exportProject` refuses and explicit v3 or portable export
is required. The existing private transfer path retains its Date round trip;
those private bytes must not be relabeled as `.ro`. Export produces a snapshot.
Only the separate host persistence operation can confirm a durable save.

Imported-workbook exports (`exportProject`, `exportProjectV3` and
`exportPortableRo`) return an optional `import_metadata` companion beside
`revision` and `bytes`. Persist the companion with the export, including after
add/rename/delete and Undo/Redo. It is returned for every imported occurrence so
repeated saves after reopen retain it; native exports omit it. Neither the
private project bytes nor the `.ro` package contains it. ADR-0042 §5 excludes
this attachment, and ADR-0046 does not authorize an additional package entry;
this is #869's companion fallback.

```ts
const saved = await client.exportPortableRo(revision);
// Persist saved.bytes and JSON.stringify(saved.import_metadata) separately.
if (saved.import_metadata !== undefined) {
  await client.openImportedProject(saved.bytes.slice(0), saved.import_metadata);
}
```

`openImportedProject` / `inspectImportedProject` and their working-size variants
accept portable `.ro` bytes as well as private project transfers together with
the companion. Existing mapping validation runs before installation. The client
supplies the authoritative mapping; there is no packaged mapping to override.
A `.ro` opened alone still has CORE-only behavior. An untouched import and every
existing CORE package retain exact export bytes.

This implements the CORE package capability in #519. Sheet attachments and
consumer qualification remain separate; #488 is not complete.

Always tear down the occurrence and Worker when the UI is finished:

```ts
await client.closeProject();
await client.close();
```

For executable evidence, run:

```sh
bash scripts/experimental-designer-client-smoke.sh
```

This check requires ripgrep (`rg`), which is also a prerequisite of the full
browser-client qualification; install it with the operating system's package
manager.

That throwaway consumer compares two complete exports under the same toolchain,
imports no `apps/designer` source, opens Product Gap, queries its typed table,
publishes a Number edit, observes the priority calculation and diagnostic list,
proves a stale edit leaves exported canonical bytes unchanged, and reopens an
exact round trip.

The same smoke also runs the preserved Sheet kit, C1 storage and runtime-canary
acceptance against its just-built artifact. `tests/consumer-kit/seed/provenance.json`
records the unchanged seed bytes; the public-kit driver calls the real Worker
and existing codecs. These checks qualify the producer boundary, not Sheet UI
journeys or host durability. To check a retained artifact without rebuilding:

```sh
bash scripts/experimental-designer-client-acceptance.sh /absolute/kit EXACT_SOURCE_SHA
```

Deeper authority: [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
the [Semantic API specification](../specs/semantic-api.md), and the
[frontend/runtime boundary](../architecture/frontend-backend-boundary.md).

## Recovering imported worksheet mappings

After losing an imported worksheet add/rename/delete/move or Undo/Redo reply, call
`inspectCurrentProject()`. It sends the resident `inspect_project` command
through the ordinary native/WASM request bridge and returns a
`ProjectInspectionProjection`: the current `revision` and optional
`import_metadata`. Use that mapping with the returned revision for
`exportSpreadsheet`, and persist it alongside the project for later reopen.
The read does not mutate or replace the occurrence and needs no saved bytes
or caller-held mapping. It works in ordinary and working-size sessions.

The mapping is present once this occurrence has published an imported worksheet
lifecycle mapping, including after Undo restores the original mapping. It is
omitted for untouched imports and non-imported projects. Existing replies are
unchanged. `inspectProject(bytes)` and `inspectImportedProject(bytes, metadata)`
still inspect saved candidates in a temporary occurrence; use
`inspectCurrentProject()` to recover the current resident mapping instead.

## Checking an XLSX before overwriting it

After a successful XLSX import, keep the `scope` from `observeOccurrence()`
with the source file. Call `exportSpreadsheetOverwrite({scope, revision})`
with that scope and the revision being saved. The Worker returns XLSX bytes
only when it can reproduce every original package part from the untouched
import and export the requested revision without known loss. The comparison
ignores ZIP compression and entry order. Changes inside an original package
part require a refusal unless preservation can be established.

On refusal, show the `DesignerRuntimeError` message and its
`failure.diagnostics` and offer Save a Copy via `exportSpreadsheet`. The
overwrite proof belongs to the live XLSX occurrence. To regain it after
closing that occurrence or opening a saved Work project, open the original
XLSX. The client owns the file write.

Before editing a live XLSX import, call
`reopenImportedXlsxWorkingSize(await client.observeOccurrence())` to open its
exact snapshot as a fresh working-size occurrence. Keep the metadata from the
import and obtain the new scope with `observeOccurrence()` for overwrite
exports. Formatting and typed overwrite refusals carry over.

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

### Builds behind an existing HTTP proxy

The exporter isolates package and compiler configuration. On a host whose
approved network transport requires an existing HTTP proxy, set
`TACHIKO_EXPORT_INHERIT_PROXY=1` for the export command. This opt-in preserves
only `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` and their lowercase equivalents,
with the caller's existing values. The default (`0` or unset) passes none.
It does not select a new proxy, import package-manager configuration, change
certificate trust, or disable locked-install and supply-chain verification.
Do not print proxy values in build logs. This option does not make a build
on a different host qualify as reference-host performance evidence.

If that existing proxy also requires the CA file already configured in the
caller's `NODE_EXTRA_CA_CERTS`, separately set `TACHIKO_EXPORT_INHERIT_CA=1`.
The file must be an existing readable absolute file path. The exporter passes
that exact path to Node as `NODE_EXTRA_CA_CERTS` and Cargo as `CARGO_HTTP_CAINFO`.
It does not download a CA, modify the system trust store, or inherit TLS-disable
switches. Missing or invalid paths refuse before package execution. Unset or
`0` keeps CA overrides excluded; any other opt-in value refuses. Only use this
option for a CA file whose existing trust and provenance have been verified.

## Cell styles (#815, #889, #965)

Use `cell_styling.status` from the current table or table-window projection
to enable formatting for that collection. Both style commands use the same
eligibility rule. The projection returns `{status: "supported"}` for eligible
native tables, direct Tracker rows and imported sheets, or
`{status: "unsupported", reason: "read_only_table"}` otherwise. Selection,
revision and capacity checks still apply when a command is submitted.

`setCellStyle(expectedRevision, collection, entities, fields, changes)` applies
`entities × fields` atomically, including blank cells. Use stable record and
field IDs in selection order. The equivalent `trackerCommand` request is:

```ts
{
  type: "set_cell_style",
  expected_revision: revision,
  collection,
  entities: selectedRecordIds,
  fields: selectedFieldIds,
  changes: [
    {kind: "toggle_bold"},
    {kind: "italic", value: true},
    {kind: "fill", color: "#FFFF00"},
    {kind: "border", target: "outline", value: true},
  ],
}
```

`CellStyleChange` also accepts `{kind: "bold", value: boolean}`,
`{kind: "toggle_italic"}` and `{kind: "fill", color: null}`. False removes bold
or italic; null clears fill, including a column-inherited fill, while preserving
other records' current appearance. Changes run in array order. A toggle clears
its attribute when every selected cell has it, otherwise it sets the whole
selection. Colours use ADR-0049's canonical `#RRGGBB` spelling with uppercase hex.

`setCellStyleRange(expectedRevision, collection, row, rowCount, fields, changes)`
uses 1-based stored rows and stable field IDs. Both style commands accept
`{kind: "number_format", format: NumberFormat}` to set a display-only override
of the column's number format on each selected cell. It uses the same plain,
currency and percent formats as `setColumnNumberFormat`. Invalid declarations
return `invalid_number_format`. The override follows the cell through Undo,
Redo, v3 and portable `.ro` saves, and supported numeric XLSX export/import.

Borders use `{kind: "border", target, value}`. Targets are `"top"`, `"right"`,
`"bottom"`, `"left"`, `"outline"` and `"all"`; true draws thin borders and false
clears them. A side target applies that side to every selected cell; all applies
all four sides. Outline changes only the selection's outer edges, using the
supplied entity and field array order (first entity = top, first field = left).
A single-cell outline draws all four sides. Other sides and attributes remain
as they were, including column-inherited borders. For a fresh outline that also
clears existing interior borders, send all/false followed by outline/true in the
same changes array.

One call makes one presentation-only revision and one Undo/Redo step. It changes
no values, formulas or calculations. Publication sets `many: true`; re-query the
visible window. Table/window rows expose optional `styles` in column order,
using the same style shape as imported rows (`bold`, optional `italic`, `fill`,
`wrap`, `border`, optional `border_sides`, `alignment`, `number_format`).
`border_sides` lists drawn `"top"`, `"right"`, `"bottom"`, `"left"` sides in that
canonical order; `border` remains true when any side is drawn. For older source
metadata, true without sides means a full box. Fill in this projection keeps the
imported shape's hex spelling without `#`. Omitted `styles` means default styles;
omitted `italic` means false. Styles belong to record/field identities, follow a
view sort and persist in v3 project trees and portable `.ro` files.

Failures leave revision, presentation and history unchanged. Codes include
`invalid_style_color`, `read_only_table` for a collection outside the kit's row
authoring/imported-table profiles, `missing_collection`, `unknown_entity`,
`unknown_view_field`, `stale_revision`, `no_change`, `capacity_exceeded` and
`style_capacity_exceeded` (palette limit). The runtime has no separate table-lock
setting; this command uses its existing authoring profiles and publication
authority.

XLSX exports of native tables and imported workbooks resolve edited bold,
italic, fill and exact border sides from the same presentation store, including
blank styled cells. Import retains italic and individual border sides in the
same projection. Existing export profile and capacity limits still apply.
Sheet owns toolbar controls, shortcuts and selection; the kit owns style state
and toggle semantics.
