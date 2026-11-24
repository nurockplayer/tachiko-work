# Experimental browser client producer

This package builds and qualifies Tachiko Work's experimental browser client
and standalone Rust/WASM runtime. It is a technical producer boundary, not a
first-party application, stable SDK, published package, or compatibility
promise.

From the repository root, install the pinned toolchain and run its checks:

```sh
pnpm --dir packages/browser-client install --frozen-lockfile
pnpm --dir packages/browser-client exec playwright install chromium
bash scripts/browser-client-check.sh
```

The browser check runs the external consumer workflow from
`examples/experimental-designer-client` against an exported kit. The producer
also retains direct runtime, source identity, manifest, notices, no-clobber,
and strict consumer canaries. Its emitted `designer_runtime.wasm` filename and
Rust crate ABI identity remain compatibility bytes.

## Headerless geometry and positional paste

Use `headerRow: 0` (`header_row: 0` on the wire) for a headerless table whose
first data row is A1. `translateA1Formula` binds grid text to semantic references;
`renderA1Formula` renders a stored formula against the same revision-pinned
geometry, returning A1 text in the existing `a1_translation.source` response.
Headered geometry keeps its existing positive header offset. Auto formula
admission and partial/future-row ranges retain their existing restrictions.

`pasteCellsAt(expectedRevision, collection, row, column, rows)` sends
`{type: "paste_cells_at", expected_revision, collection, row, column, rows}`.
Row and column are 1-based stored positions. A nonempty rectangular block uses
the existing string or tagged `PasteCellInput` values and creates missing Auto
columns and sparse rows like `writeCellAt`, including on a blank workbook.
Structure, classified values and implied displays publish in one revision and
Undo/Redo together. Invalid input and capacity/lifecycle refusals leave the
document and history unchanged. Existing `paste_cells` calls are unchanged.

## Range-based cell styles

`setCellStyleRange(expectedRevision, collection, row, rowCount, fields, changes)`
sends `{type: "set_cell_style_range", expected_revision, collection, row, row_count, fields, changes}`.
Row 1 is the first stored data row, independent of headers or view sorting.
The nonempty range must fit within stored rows. Supply field IDs in selection
order for outline borders. The command applies the same style changes as
`setCellStyle`, including bold or italic toggles that clear only when every
selected cell already has that style. Blank cells in stored rows receive
presentation without creating values. One Undo restores the prior
presentation for the entire range, and one Redo reapplies it.

## Explicit v3 project transfers

The experimental client offers `exportProjectV3(expectedRevision)` to explicitly
select an opaque canonical `.roproj/v3` project transfer. It supports Date and
saved grouped-sum definitions together, with fresh admission and projection
budgets checked before export. Use the existing `inspectProject` and
`openProject` methods for these bytes. A successful v3 open requires explicit
v3 export for subsequent saves; ordinary export refuses that occurrence.

The origin marker belongs to the live occurrence, survives edits and history
operations, and resets on successful New, Import, legacy open or Close. Keep a
v3-capable reader available when rolling back a producer that has emitted v3
artifacts. Sheet host save/restart qualification remains a separate consumer gate.

## Conditional formulas

`updateFormula` accepts IF, Number comparisons (`= <> < <= > >=`), AND/OR/NOT,
ROUND with literal precision `0..=10`, percent literals, and ISBLANK. Conditions
are intermediate types, not stored values; invalid placements return the existing
`formula_type_mismatch` refusal. IF evaluates only its selected branch while
tracking both branches' dependencies. Absent optional Number inputs read as zero;
ISBLANK observes presence without converting Text or other types to Numbers.

Canonical `.roproj/v3` and direct `.ro/v2` preserve the new bound operations;
frozen project v1/v2 writers refuse them. XLSX emits corresponding Excel formula
text and current computed results. Formula copy and collection duplication remap
references inside conditions and both branches. See the
[formula authoring reference](../../docs/engineering/formula-authoring-reference.md).
The existing formula, node, byte, depth, and history bounds are unchanged.

## Bounded Text/Number capacity profile

The producer supports the private [bounded Text capacity profile](../../acceptance/bounded-text-capacity/CONTRACT.md):
one headered table with 1–16 Text or finite Number fields, at most 8,406 rows
and 25,218 logical cells, plain styles, and no column widths, formulas,
constraints or saved calculation definitions. Admission tries ordinary limits
first; enlarged retry preserves legacy three-Text eligibility and requires more
than 64 rows for new shapes. The CSV header reserve is `max(39, 13 × columns)`.
Inspection retains raw Text and proves whole-column Number feasibility when
needed; explicit type confirmation and finite complete projections, exact Text
preservation and both CSV/XLSX export closure precede candidate publication.
The existing generic and native Tracker profiles retain their own limits.
Capacity Text edit requests retain proposal evidence only for that request; the
existing 64-operation session Undo horizon is unchanged.

The ordinary Designer request ABI checks 64 KiB before parsing. The spreadsheet
ABI caps its arena at 4 MiB, then uses a bounded operation-kind discriminator;
above 64 KiB it admits only Export, InspectProject and metadata-aware OpenProject
operations before decoding their complete payload. Other controls and selective
field replies remain at 64 KiB. Capacity artifacts require a capacity-capable
reader; downgrading the producer cannot make the old pin support them. The
capacity profile remains separate from v3 and does not alter v3 origin/export
behavior or the Sheet's Date + saved-definition guard. Producer qualification
and the separate Sheet import/edit/durable save/restart/reopen qualification
remain distinct.

Saved spreadsheet carriers use metadata-aware admission: native Rust callers
can use `open_imported_project(runtime, input, metadata, occurrence_id)`, and
the Worker client exposes the optional `openImportedProject` capability through
the existing spreadsheet operation. Inspection does not authorize a later
open; actual open revalidates the carrier, metadata mappings, XLSX/CSV closure
and complete reply before replacing the resident occurrence. It has no ordinary
open fallback. V3 keeps its strict ordinary profile and origin behavior;
capacity promotion applies only to eligible non-v3 Text/Number candidates. Enlarged
Text-capacity Deduplicate removals are refused before preview or publication.
Qualification binds acceptance to the exact exported kit. Sheet independently
qualifies its consumer pin and durable user journey.
