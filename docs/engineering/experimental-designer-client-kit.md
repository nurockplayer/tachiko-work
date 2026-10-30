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

To total exactly the rows a view shows, pass their entity ids to
`queryColumnTotals`. It sums each Number column over the named entities, in
collection order, and returns `{ revision, rows, column_totals }` with the same
null and blank-cell rules as the window footer. Naming every entity of the
collection gives the footer's totals exactly. The request accepts any set of
unique ids of that collection, up to all of them; an unknown, duplicate or
foreign id, or a stale revision, refuses with no change. Use it for the footer of a filtered view:
the window's `column_totals` always cover the whole collection.

```ts
const shownIds = visibleRows.map((row) => row.id);
const shown = await client.queryColumnTotals(revision, collection, shownIds);
```

`count` is 0..=256 and `0` returns the columns and totals only. `start` clamps
past the end instead of failing, so a shrinking view never turns a scroll into an
error. The reply repeats `revision`, `collection`, `columns`, a
`columns_revision` digest that changes only when a column definition changes,
`total_rows` (after filters and kept entities), `unfiltered_rows`, the clamped
`start`, and the window's `rows` with the same row/field projections as
`queryTable`.

The view is request state owned by the host: it never enters the document,
publication history or revision token. Sort keys apply in order with blanks last
and a stable entity-ID tie-break; filters are AND-ed. Text filters compare
case-insensitively and are text-only, number and date filters use
`= <> < <= > >=`, `is_blank`/`not_blank` find missing or present values, and
`in_set` takes literals of the column's declared kind. Formula columns filter and
sort by their calculated Number. A window above the reply cap (1 MiB, or 16 MiB for a text-capacity
document) refuses with
`query_too_large` rather than serialising an oversized response; ask for
fewer rows or fewer columns.

## Optional native columns

Under [ADR-0045](../decisions/ADR-0045-optional-native-text-and-number-fields.md),
`newTable(name, columns)` accepts `required?: boolean` per column. Omission
defaults to `true`; any scalar column (Text, Number, Date or Yes/No) may be
declared optional ([#598](https://github.com/nurockplayer/tachiko-work/issues/598)). The same
property is accepted by the `add_column` command through `trackerCommand`.
An optional added column needs no `initializers`; a supplied subset fills only
those entities. `insert_row` may omit optional initializers, leaving those slots
blank. Required initializers remain mandatory; neither path invents defaults.

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

`ScalarEditInput` adds `{kind: "clear"}` alongside typed sets. `edit_cells`
can mix both atomically. `paste_cells.rows` accepts `PasteCellInput[][]`:
each cell is a raw string or a typed scalar input, including explicit clear.
Raw `""` in Text means present empty Text; raw `""` in Number refuses.
For example, `[["", {kind: "clear"}]]` sets empty Text in the starting
Text column and clears the next optional column. Untargeted cells stay unchanged;
omitted optional fields of newly pasted rows remain blank.

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
