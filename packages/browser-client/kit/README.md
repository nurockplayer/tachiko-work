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
