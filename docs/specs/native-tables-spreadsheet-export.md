# Native tables spreadsheet export

Bounded acceptance contract for [Issue #545](https://github.com/nurockplayer/tachiko-work/issues/545),
under Accepted [ADR-0027](../decisions/ADR-0027-open-format-and-interoperability-policy.md).
This is an outbound adapter over the existing semantic document. It does not
change semantic types, storage formats, imported-workbook exports, or the
separate Tracker/Budget presentation-bound exports.

## Entry and authority

The public experimental client supplies this required method (also an optional
capability on the lightweight `DesignerClient` interface):

```ts
exportNativeTablesSpreadsheet(
  expectedRevision: string,
  collection: string,
  format: "csv" | "xlsx",
): Promise<SpreadsheetExport>
// SpreadsheetExport = { revision: string; bytes: ArrayBuffer; ledger: FidelityFinding[] }
```

The host and WASM spreadsheet request is
`{type: "export_native_tables", expected_revision, collection, format}`.
`collection` accepts the runtime collection key or stable schema ID, as existing
table operations do. It must identify an existing collection in either format.
Rust supplies all names, values and ordering from the exact revision. There is
no client-provided data, presentation, identity mapping or ordering input.
Unknown fields in the request are refused. Rust also exposes
`export_native_tables_workbook` and `export_native_tables_csv_workbook` for the
corresponding bounded source projections; encoding and its byte limits are
applied by the host/WASM operation.

A native mapping requires at least one declared field, each Text, Number,
Boolean or Gregorian Date, and only directly stored values of those types.
Absent optional fields are supported. Reference fields, formula values and
other unmapped collections are refused. This mapping includes scalar stock
Tracker, imported optional scalar tables, Sales and New table documents;
formula-bearing Budget/imported documents retain their existing export route.

## Output and deterministic names

XLSX exports every collection in the runtime's canonical collection-key order,
with one headered worksheet per collection. Column order follows the runtime's
canonical field order (including its fixed stock Tracker field order); rows
follow canonical entity identity order. View sorting and filtering do not affect
output. Empty tables still emit their declared header.

Each worksheet starts with its table display name. Replace controls, XML-invalid
characters and `[]:*?/\` with `_`, trim surrounding whitespace and boundary
apostrophes, use `Table` if empty, and take at most 31 Unicode scalar characters
without splitting UTF-8. Trim an apostrophe exposed at the truncated end.
Allocate names in canonical order. Compare complete candidate names using
`to_uppercase()` followed by `to_lowercase()`, without a new dependency. This
conservative caseless key makes `σ`/`ς`/`Σ` and `ß`/`SS`/`ss` collide. Turkish
`I`/`ı`/`i` share a key; `İ` retains its distinct `i` plus combining dot key.
Over-collision adds a deterministic, ledgered suffix; it never changes data.
Caseless collisions use ` (2)`,
` (3)`, etc.; reserve suffix space inside the 31-character limit, and test the
complete candidate against all previously allocated names, including names
which already contain suffix text. Every worksheet mapping has a `Converted`
ledger finding with the stable schema ID and original/output names.

Headers start with field display names. Replace controls/XML-invalid characters
with `_`, use `Column` for an empty/whitespace-only header, and disambiguate
with the same uppercase-then-lowercase key and suffix allocation. Headers are **never
truncated**. Every changed header has a `Converted` finding with schema/field
identity and original/output names. Worksheet names are internal for CSV; CSV
has no worksheet-name mapping claim.

Cells use the existing typed XLSX writer: inline Text, finite numeric Number,
Boolean, and Gregorian Date with `yyyy-mm-dd`. No formulas or execution-capable
parts are produced by this operation. Absent optional fields map to `Empty`,
not zero or false, preserving every rectangular position. Empty Text remains
Text in XLSX.

Native-tables XLSX refuses finite, non-zero subnormal Numbers with
`abs(value) < f64::MIN_POSITIVE`: Excel cannot retain them and would change them
to zero. The blocking finding identifies the worksheet, row and column.
`f64::MIN_POSITIVE` and its negative counterpart are accepted. Both `0.0` and
`-0.0` are emitted as numeric `0`. These rules do not change other XLSX profiles.

Numbers whose shortest round-trip decimal needs more than **15 significant
decimal digits** are exported unchanged, with a non-blocking `LossyOnExport`
finding stating that Excel may round them to 15 digits. Count digits in the
decimal significand, excluding sign, decimal point, exponent and insignificant
leading/trailing zeros. Thus `0.30000000000000004` and `1234567890123456` have a
finding, while `0.1`, `123456789012345` and powers such as `1e20` do not.

For these precision warnings, emit at most the first **20 cell locations** in
canonical worksheet/row/column order, followed by one count finding whenever
any cell is affected. The count states the total affected cells, the number of
locations shown and the number omitted. This cap applies across the entire
workbook. No value is rounded or dropped to satisfy the ledger cap.

Native-tables XLSX refuses literal SpreadsheetML escape sequences matching
`_x[0-9A-Fa-f]{4}_` in Text cell values, headers and worksheet names. Hex digits
may use either case; the literal prefix is `_x`. Excel decodes these sequences,
so emitting them unchanged would silently alter the source Text. Check source
table/column display names before mapping and all emitted names and Text after
mapping: truncation must not hide a source sequence, and sanitization must not
introduce one. Near-misses such as `_x00D_` and `_x000G_` remain literal and
exportable. The shared XLSX text encoder and reader are unchanged.

Native-tables XLSX also refuses Text cell values and headers longer than
**32,767 UTF-16 code units**, Excel's per-cell text limit. ASCII and BMP
characters each use one unit; an astral character such as 😀 uses two. Headers
count as cells and are never truncated to fit. Both refusals carry a blocking
fidelity finding: cell locations name the worksheet, row and column; label
locations identify the header/field or worksheet name. These checks do not
apply to CSV or other XLSX output profiles. This preserves truthful fidelity
instead of producing an apparent success that Excel reinterprets or truncates.

CSV exports only the requested collection using scalar lexical values and the
existing UTF-8/RFC4180 writer. Every other collection has an individual exclusion
finding, including any unmapped collection. Typing and formatting losses and
CSV's inability to distinguish missing values from empty Text are explicit.
Its ledger also states that CSV retains full binary64 Number precision through
shortest round-trip decimal text, including subnormals. The CSV numeric writer
is unchanged. A spreadsheet application may still infer types or round those
values when opening CSV; full precision here describes the file's contents.
Headers and Text beginning with `=`, `+`, `-` or `@` after leading whitespace or
controls are refused; no apostrophe prefix is added. Check source headers before
sanitization as well as the encoded headers. Typed XLSX remains the escape for
formula-like literal Text. XML-illegal cell Text fails closed rather than being
silently changed.

XLSX refuses the entire request if any collection lacks the native mapping.
CSV may export an eligible selected collection from that mixed document.

## Dedicated outbound bounds

| Resource | Maximum |
| --- | ---: |
| Worksheets | 32 |
| Fields per worksheet | 32 |
| Data rows per worksheet | 128 |
| Total data rows | 1,024 |
| XLSX Text cell or header length | 32,767 UTF-16 code units |
| Encoded CSV or XLSX bytes | 8,388,608 (8 MiB) |
| Expanded XLSX XML part bytes | 25,165,824 (24 MiB) |

The existing complete response-envelope byte limit also applies (64 KiB,
with the existing Text-capacity exception unchanged). Over-limit receipts are
refused with a native-export finding before either transport returns bytes.

Geometry matches native admission (which also bounds per-collection stored Text
at 64 KiB and aggregate identity/profile strings at 64 KiB). The byte caps leave
room for up to 32 such Text budgets, XML entity escaping, 32,768 typed cells,
coordinates, headers and package overhead. Caps are hard refusal boundaries,
never truncation. The native table output profile is explicitly selected and
does not borrow `NativeTracker` or widen default writers.

The incoming typed spreadsheet profile remains 4 sheets, 16 columns and 64 data
rows per sheet, with 2 MiB source/8 MiB expanded bounds. Its separate restricted
Text-capacity exception is unchanged. Within incoming bounds, typed readback
and explicit CSV reimport are tested. Larger outbound workbooks are not promised
reimportable. Source admission also enforces complete-publication and refresh
projection bounds: these can refuse nominally in-range geometry such as 1,024
one-field rows. Tests exercise the native 32-collection, 32-field and 128-row
boundaries independently, and the writer directly proves 1,024 total rows and
+1. Conversely, the restricted Text-capacity path can admit 129 rows; this native
export still refuses that request at 128 rather than inheriting its exception.
The writer independently enforces every outbound geometry and byte bound.

## Fidelity, refusal and state

The ledger names excluded keyed-grouped-sum definitions, their calculated
results, reports/charts, session Undo/Redo history, and Sheet presentation.
Saved grouped-sum definitions additionally have individual findings by stable
ID. No calculated snapshots are included. Retain a Tachiko source copy to
preserve bindings, computation and native meaning.

Stale revisions return the existing `stale_revision` failure. Native mapping,
output bounds and encoding failures return `native_tables_export_refused` with
a blocking `UnsupportedSafeDisabled` Rust fidelity finding, projected through
the existing failure diagnostics (`code`, `message`, `path`); refused requests
return no output bytes. No resident revision, content, Undo/Redo state or other
session state changes on success or refusal.

## Acceptance evidence

- `runtime/tests/native_tables_export.rs`: typed readback via `import_xlsx`,
  explicit CSV reimport, independent ZIP/XML structure assertions, empty/missing
  values, stable order, collision chains, geometry limits/+1, mixed documents,
  stale revisions, source/history preservation and ledger coverage; literal
  SpreadsheetML escape/near-miss locations and native-only XLSX Text refusal;
  deterministic Greek/German/Turkish/ASCII caseless mappings, located subnormal
  refusal, full-precision CSV and the bounded XLSX precision warning ledger.
- `interop_adapter::native_tables_output_tests`: independent outbound geometry,
  encoded CSV byte and expanded XLSX byte limits, including exact/+1 boundaries;
  Text/header lengths of 32,767 and 32,768 UTF-16 units for ASCII, multi-byte BMP
  and astral characters. The expanded-XLSX fixture reaches exactly 24 MiB and
  +1 across many valid cells, each within the per-cell limit. Direct native
  writer tests additionally check caseless uniqueness, minimum-normal/adjacent
  subnormal boundaries and numeric `0` output for both zero signs, preserving
  the previous behavior of other profiles.
- `wasm::native_tables_parity_tests`: the actual WASM bridge's request/response
  logic executed natively, compared byte-for-byte with host receipts, bytes,
  findings and refusals. It never dereferences WASM pointer offsets; this is
  bridge logic parity, not execution of a compiled wasm32 artifact.
- `tests/experimental-client-public-entry.test.ts`: public Worker request,
  opaque bytes/ledger receipt and revision refusal for both formats.

Independent spreadsheet-application reopening, actual wasm32 execution and the
Sheet re-pin/download journey are additional integration evidence; native bridge
parity and Work's own parser do not replace those claims. The typed acceptance
test can write its exact artifact to `TACHIKO_NATIVE_TABLES_EXPORT_EVIDENCE` for
independent-reader inspection without changing or skipping its assertions.
