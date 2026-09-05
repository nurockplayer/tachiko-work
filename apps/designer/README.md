# Tachiko Designer vertical slice

This is the first production graphical projection and persistence composition
over the resident Rust workspace runtime. It intentionally supports a bounded
product path: open a canonical `.roproj/v1`, deterministically select and browse
a typed collection, edit directly stored Text, Number, and Boolean values,
publish against the expected revision, selectively refresh affected fields and
dependent formulas, Save As to a new browser-local destination, then destroy
and reopen the occurrence.

The built-in Moonfall balance demo remains the default launch occurrence. The
repository-owned [`Product Gap` dogfood project](../../dogfood/product-gaps.roproj/)
proves that the same app-local profile admits and operates an ordinary second
domain without requiring Moonfall collection names or control subjects. The
dogfood project is product evidence, not a public template or a source of
repository-delivery truth.

The Worker retains the Rust-authoritative workspace occurrence. Browser state
contains only bounded, revision-keyed projections and edit buffers. The
app-local JSON DTOs and raw WASM ABI are private delivery mechanics, not a
public SDK, stable wire protocol, canonical document model, or storage format.

Open sends only selected path/byte records through a separately bounded private
project arena. Rust `tachiko-storage` performs exact canonical admission and the
workspace prepares the complete initial projection set before a fresh
occurrence replaces the current one. The browser host supplies a fresh cryptographic occurrence token
for every demo or Open, so document-scope authority is not reused across Worker
or page lifetimes. Rejected admission or initial projection fails before
replacement and leaves the current occurrence unchanged.

Save As captures one exact `ResidentWorkspaceSession::export_snapshot()`
revision and encodes existing non-Date projects through the canonical
`.roproj/v1` codec. Date-bearing Budget projects use the existing
`direct-ro/v2` codec inside an explicit private Browser host record; this is
not `.roproj/v2`, a portable package, or a new public project format.
IndexedDB commits the opaque complete tree as one create-only record, so an
existing project name is never overwritten and the UI marks only the confirmed
revision durable. Dirty occurrences guard in-app replacement/close and browser
unload; the unload guard is removed after Save As or occurrence teardown.
Browser projects survive Worker teardown and page reload, but
remain browser-origin data and can be removed by clearing site data. The Tracker workflow below adds explicit same-project Save and bounded session
undo; autosave, cloud persistence, and distribution remain separate work.

Save and Save As require all pending cell and formula drafts to be applied or
cancelled. Rejected numeric edits retain the draft for correction while the
published value and calculations stay unchanged. Cancel pending edits restores
the published values; it does not undo accepted edits.

## Development

Use pnpm exclusively:

```sh
pnpm --dir apps/designer install --frozen-lockfile
pnpm --dir apps/designer dev
pnpm --dir apps/designer lint
pnpm --dir apps/designer typecheck
pnpm --dir apps/designer test
pnpm --dir apps/designer build
pnpm --dir apps/designer exec playwright install chromium
pnpm --dir apps/designer test:browser
```

`build` compiles the private Rust adapter for `wasm32-unknown-unknown` before
Vite assembles the application. `dev` performs the same runtime build before
starting Vite, so a clean checkout never serves without its Worker artifact.
General schema authoring, autosave/recovery history, and public
transport/storage/SDK stabilization remain outside this slice.

## Experimental external client kit

Issue #232 adds one deliberately unstable, vendorable frontend kit over this
same private Worker/WASM seam. It is not a published package or stable SDK.
Export it to a caller-owned directory with:

```sh
bash scripts/export-experimental-designer-client.sh /path/to/external-ui/vendor/tachiko
```

Frontend engineers should start with
[Build an experimental Tachiko frontend](../../docs/engineering/frontend-integration-guide.md),
which explains the assignment, responsibility split, first useful milestone,
current limits, and pilot feedback format without requiring engine knowledge.

Use the
[client-kit technical guide](../../docs/engineering/experimental-designer-client-kit.md)
while wiring the generated assets. The external-style smoke consumer lives under
[`examples/experimental-designer-client`](../../examples/experimental-designer-client/)
and imports only the generated kit.


## Driver Tracker (#257)

Choose **New Tracker**, then paste into the empty grid or append a row. The
stock tracker has three required fields: **task** (Text), **estimate** (finite
Number), and **done** (Boolean). The Boolean dropdown projects the Rust type
constraint; this slice does not introduce custom enum or schema-rule authoring.
The checked-in [operations tracker](../../dogfood/operations-tracker.roproj/)
contains 40 practical operational tasks and is generated through this same
runtime from the [clipboard fixture](e2e/fixtures/operations-tracker.tsv).

- Click/Shift-click selects cells/ranges. Arrow keys, Tab, Home/End and
  Page Up/Down navigate; Enter focuses the cell editor. Apply to selection is
  one atomic typed operation. Apply or cancel drafts before changing selection.
- Clipboard interchange supports rectangular plain TSV, including quoted
  tabs/newlines and escaped quotes. Number input uses decimal `.` with no
  locale-specific thousands separators; Boolean paste accepts exactly `true`
  or `false`. Unsupported or malformed input rejects the entire operation.
  Copy exports the selected displayed values as TSV, never internal IDs.
- Paste extends rows atomically, up to 128 rows, 3 columns, 48,000 clipboard
  characters and the runtime's existing 64 KiB projection admission budget.
  Oversized cell contents can reach that budget before 128 rows. Rejection
  leaves accepted work intact. Multi-row paste/range editing requires original
  order and no filter; **Original order** clears manual row moves.
- Append/remove operate on stable entity IDs. Move rows changes only the view.
  This fixed stock schema needs no column-schema editing. Sort is stable and
  uses numeric/Boolean ordering or case-sensitive UTF-16 code-unit string comparison;
  ties retain manual order, then canonical order. Missing values sort after
  values and diagnostics after missing in both directions. Find/filter is a
  case-insensitive literal search across displayed fields. Sort/filter is
  session-only and never changes canonical row identity.
- Bold, fill, wrapping, borders, alignment, header emphasis and bounded column
  widths/row heights are stable-ID presentation data. Sticky column headers
  keep context across the representative 40-row workload. The private browser
  sidecar saves formatting and manual order with canonical bytes in one
  IndexedDB transaction. It is not a new canonical `.roproj` format: opening a
  folder from outside the browser has default formatting.
- **Save** updates the current browser project only if both its original bytes
  and presentation still match; a stale destination or failed transaction keeps
  accepted local edits dirty and offers Save As/reopen. **Save As** remains
  create-only. Save, close and reopen retains document and row identities.
  Browser storage is origin-local and clearing site data removes it.
- Undo/redo covers supported semantic edits and formatting in the current open
  session (last 64 combined actions). Semantic undo/redo uses Rust-authoritative
  inverse atomic batches. A successful edit in the generic workbench clears both
  Tracker undo/redo histories with an explanation, preserving all accepted data
  and formatting. Read-only collection switches and rejected/no-change edits do
  not clear history. Reopening starts a fresh undo stack. A failed refresh
  leaves the accepted revision saveable and provides **Retry refresh**.

This closes only the bounded tracker journey, not Excel parity or the public
launch gate. No CSV/XLSX compatibility, multi-sheet, formula expansion, custom
validation engine, cloud sync, or durable history is added.

## Driver Budget (#258)

Choose **New Budget** to open a small monthly plan with **Budget Items** and
**Budget Summary** collections. The formula panel lets you select a numeric
target and insert references by collection, row and field labels. The generated
`[entity.field]` addresses are parsed,
bound to stable semantic IDs, type-checked, calculated, and published by the
Rust lifecycle. Up to 32 named views can be added, duplicated, renamed,
reordered and deleted over these collections. Views share their source data;
duplicating a view does not copy its data and deleting a view does not remove
the underlying collection. View names and order cannot retarget bound formulas.

- Formula authoring accepts only the existing bounded arithmetic and
  `min`/`max` vocabulary. Invalid, unbound, non-numeric, stale, or
  calculation-failing input is rejected before publication.
- Copy a formula to selected numeric destinations in the same collection in
  one atomic operation. Relative rows/columns use the displayed canonical
  table order; selected fixed dependencies and cross-collection references
  retain their stable targets. Out-of-range, wrong-type, duplicate or cyclic
  candidates reject the entire copy. Formula authoring/copy clears session
  undo history; conversion from a formula back to a scalar is not supported.
- Date uses canonical Gregorian `YYYY-MM-DD`. Number formatting can cycle
  between ordinary Number, JPY, Percentage, and USD presentation. Formatting
  is private Browser presentation metadata and never changes Number arithmetic,
  introduces Money, or promises cross-currency safety.
- Number entry uses a decimal dot without grouping or currency symbols; a
  percentage display does not reinterpret input (0.2 displays as 20%, while
  20 displays as 2,000%). Number/percentage/USD display uses en-US and JPY uses
  ja-JP. Date input is date-only ISO, with the browser's native date control.
  Pending Budget input must be applied or cancelled before saving.
- A Date-bearing Budget Save uses the private `direct-ro/v2` host record and
  reopens through the Rust storage authority. Existing `.roproj/v1` Browser
  projects retain their original codec and behavior.
- Named views, their active selection, and number formatting are persisted
  atomically with the matching semantic snapshot in the browser-only sidecar.
  Same-project Save uses the existing compare-and-replace host boundary;
  failed writes preserve accepted local work and leave it unsaved.

## Imported spreadsheets and cleanup

The private `designer-spreadsheet/v1` [profile](interop-profile.json) is the
bounded CSV/XLSX path. Choose **Import CSV / XLSX**, inspect the source cells and
compatibility ledger, explicitly select column types, and accept the import.
CSV begins as Text; leading zeros and ambiguous dates remain Text unless the
user selects a valid conversion. Missing cells remain absent optional values.
Extra typed output columns can be declared during inspection for subsequent
split or conversion without destroying the source cells.

Hidden worksheets, rows, columns or default rows are blocked explicitly because
the profile cannot preserve their visibility; they never become visible
silently through export. Invalid XML, including out-of-root CDATA, is rejected.
Time-only and elapsed-time formats cannot become canonical Date or Number
values. Inherited row/column styles are blocked when this profile cannot apply
them. Workbook/worksheet children and row/column attributes are inventoried
against the bounded mapping: known source-only presentation has explicit export-loss
findings, and unknown constructs block acceptance.
Source formula caches are evidence only: supported formulas are rebound and
calculated by Rust even when caches are absent or stale errors. This does not
admit unsupported formulas or bypass validation of their XML structure.
Currency display is bounded to the declared JPY/USD patterns, including Excel
locale tokens. Other currency/locale patterns use ordinary Number display;
the complete source number format is retained unless explicitly changed.
Numeric sections with different currency/percentage markers, or value-dependent
conditions, also use ordinary Number display without choosing an active section.
Mixed Number/Date/time semantics across numeric format sections are blocked;
the adapter does not choose conditional sections to infer a canonical type.
Worksheet names must satisfy the same bounded XLSX name rules at inspection
and export, including case-insensitive uniqueness.

The existing table and Rust formula tools edit the imported collections.
Sorting/filtering change only the view and target stable identities. Cleanup
supports trim, literal replacement, split into two selected existing Text
columns, conversion into an existing typed output, missing-value fill and
stable first-row deduplication by a selected key. Each operation previews an
atomic, revision-pinned change. Successful cleanup clears session Undo/Redo;
the UI discloses this before commit. Formula authoring/copy retains the same
history and scalar-replacement limitations as Budget.

Save/Save As stores canonical project bytes, source mappings, original source
bytes, compatibility ledger and table views in the existing atomic private
Browser host record. Opening validates the sidecar against the admitted
candidate before replacing the resident. This browser-local record is not a
portable backup or a public storage format.

Export rebuilds values and numeric formulas from the current Rust snapshot.
XLSX emits all mapped sheets, supported basic styles and widths. No-header CSV
projects receive an explicit output header conversion, with A1 formulas shifted
consistently; the original source/header setting is unchanged. CSV exports only
the selected sheet's values and loses formulas, formatting and other sheets.
CSV refuses formula-leading literal Text or headers that another spreadsheet
reader could activate; typed XLSX keeps those strings literal.
Both download paths require review and acknowledgement of the structured loss
ledger. Unsupported original parts stay only in the retained source and are
never executed or silently advertised as editable/exported equivalents.

Fixtures under `tests/fixtures/interop` include synthetic ordinary data actually
opened/saved by LibreOffice and synthetic hostile inventories. These establish
a bounded test profile, not compatibility with every Excel feature or a
Microsoft Excel certification. The source provenance note distinguishes real
reference-tool evidence from constructed hostile inputs.
