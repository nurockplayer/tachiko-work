# Native tables spreadsheet export

Bounded acceptance contract for [Issue #545](https://github.com/nurockplayer/tachiko-work/issues/545),
under Accepted [ADR-0027](../decisions/ADR-0027-open-format-and-interoperability-policy.md).
This is an outbound adapter over the existing semantic document. It does not
change semantic types, storage formats, [imported-workbook exports](imported-workbook-spreadsheet-export.md), or the
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
Absent optional fields are supported. Every field must have `FieldConstraint::None`:
`TextLiteralSet`, `NumberInclusiveRange`, and any other non-None constraint have
no export mapping and are refused, even in an empty collection. The blocking
`native_tables_field_constraint` finding identifies the stable collection and
field IDs. XLSX refuses the whole document; CSV checks only the selected
collection and can exclude constrained collections individually.
Reference fields, formula values and other unmapped collections are refused.
This mapping includes scalar stock Tracker, imported optional scalar tables, Sales and New table documents;
formula-bearing Budget/imported documents retain their existing export route.

## Output and deterministic names

XLSX exports every collection in the runtime's canonical collection-key order,
with one headered worksheet per collection. Column order follows the runtime's
canonical field order (including its fixed stock Tracker field order); rows
follow canonical entity identity order. View sorting and filtering do not affect
output. Empty tables still emit their declared header.

Each worksheet starts with its table display name. Refuse XML-illegal characters
and literal SpreadsheetML escapes before allocation. Replace XML-legal controls
and `[]:*?/\` with `_`, repeatedly trim surrounding whitespace and boundary
apostrophes until neither remains, use `Table` if empty, and take at most **31 UTF-16 code units**, never splitting
an astral character. Repeat boundary normalization after truncation. The
reserved caseless name `History` is unavailable: allocate `History (2)`, then
`History (3)`, etc. if necessary, respecting existing suffix collisions.
Allocate names in canonical order. Compare complete candidate names using a
bounded fixed point: start with the full candidate and repeatedly apply
`to_uppercase()` followed by `to_lowercase()`, stopping when unchanged or after
**four iterations**, returning the last key. Allocation and final preflight/
writer validation use this same NativeTables-only helper without a new dependency.
This conservative caseless key makes `σ`/`ς`/`Σ` and `ß`/`ẞ`/`SS`/`ss` collide:
`ẞ` first becomes `ß`, then `ss`, which is stable. Turkish
`I`/`ı`/`i` share a key; `İ` retains its distinct `i` plus combining dot key.
Over-collision adds a deterministic, ledgered suffix; it never changes data.
Caseless collisions use ` (2)`,
` (3)`, etc.; reserve suffix space inside the 31-unit limit, and test the
complete candidate against all previously allocated names, including names
which already contain suffix text. Every worksheet mapping has a `Converted`
ledger finding with the stable schema ID and original/output names.

Headers start with field display names. XLSX refuses XML-illegal characters,
preserves XML-legal whitespace, uses `Column` for an empty/whitespace-only header,
and disambiguates with the same bounded fixed-point caseless key and suffix
allocation. Headers are **never truncated**. Every changed header has a `Converted` finding with schema/field
identity and original/output names. Worksheet names are internal for CSV; CSV
has no worksheet-name mapping claim.

Cells use the existing typed XLSX writer: inline Text, finite numeric Number,
Boolean, and ISO-typed Gregorian Date (`t="d"`) with `yyyy-mm-dd`. Dates must be
within **1900-01-01..9999-12-31**, Excel's supported 1900-system range; otherwise
`native_tables_xlsx_date_range` blocks export at the worksheet/row/column.
The existing ISO Date encoding avoids inventing a fictitious 1900-02-29 and
retains correct pre-March-1900 Dates in LibreOffice as well as the Work reader.
Equivalent Excel serials are 1 for 1900-01-01, 59 for 1900-02-28, 61 for
1900-03-01 and 2,958,465 for 9999-12-31. Serial 60 has no Gregorian Date.
No formulas or execution-capable parts are produced by this operation. Absent optional fields map to `Empty`,
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
finding, including any unmapped collection. Typing and formatting losses are
explicit. Under [ADR-0045](../decisions/ADR-0045-optional-native-text-and-number-fields.md),
emit one non-blocking `LossyOnExport` finding with code
`native_tables_csv_blank_empty_text` at the selected stable schema ID when
**a declared optional Text field has at least one absent value or present empty
Text value in the exported rows**. Either state produces an empty CSV field
and cannot be distinguished on readback; both states need not coexist in the
snapshot. Retain the Tachiko source to preserve presence. Do not emit this
finding for XLSX, excluded collections, empty tables, required Text fields,
optional Number fields, or optional Text fields containing only nonempty Text:
those cases have no blank/empty-Text value distinction to lose. This conditional
finding is separate from `native_tables_csv_values_only`, which continues to
disclose CSV typing and formatting losses on every CSV export.
Its ledger also states that CSV retains full binary64 Number precision through
shortest round-trip decimal text, including subnormals. The CSV numeric writer
is unchanged. A spreadsheet application may still infer types or round those
values when opening CSV; full precision here describes the file's contents.
Headers and Text beginning with `=`, `+`, `-` or `@` after leading whitespace or
controls are refused; no apostrophe prefix is added. Check source headers before
sanitization as well as the encoded headers. Typed XLSX remains the escape for
formula-like literal Text. Selected-collection Text and headers with XML-illegal
characters are refused before CSV sanitization or encoding, with a located
blocking `native_tables_csv_xml_character` finding. TAB/LF/CR remain legal.
This shares XLSX's XML-character rule; CSV does not inherit XLSX's escape,
Text-length, line-feed-count, Date-range or subnormal limits.

XLSX refuses the entire request if any collection lacks the native mapping.
CSV may export an eligible selected collection from that mixed document.

## Excel compatibility inventory

This is the complete compatibility inventory for this operation's scalar-only
XLSX surface, not a claim to implement Excel's other features. The single
`native_tables_compatibility` policy is used by source-name allocation and
final workbook/writer preflight. It checks original labels and emitted labels;
all refusals below are blocking `UnsupportedSafeDisabled` findings with a
worksheet-name/header location or worksheet/row/column location. Precision
warnings remain non-blocking and bounded. The reader and shared text encoder
are unchanged. CSV keeps its existing names, lexical values and injection
refusal. CSV Text/headers share the XML-character refusal; the other XLSX-only
constraints do not restrict CSV.

| Constraint | Source | Our behavior |
| --- | --- | --- |
| Worksheet name: 1–31 UTF-16 code units | [Microsoft worksheet naming][excel-names]; conservative UTF-16 measurement required by the lead disposition | Rename empty to `Table`; truncate only worksheet names on scalar boundaries, reserving suffix units. `Converted` mapping for every sheet. Test 0/1/31/32 units, including astral characters. |
| Worksheet name: no `[]:*?/\` | [Microsoft worksheet naming][excel-names] | Replace each forbidden character with `_`, then validate and ledger. |
| Worksheet name: no leading/trailing apostrophe | [Microsoft worksheet naming][excel-names] | Repeatedly trim boundary whitespace and apostrophes until stable, including boundaries exposed by truncation. `Foo`, `' Foo '`, `'' Foo ''`, and ` ' Foo ' ` collide in canonical order. Empty after normalization becomes `Table`; ledger mappings retain source/output names. |
| Worksheet name: not blank | [Microsoft worksheet naming][excel-names] | Empty/whitespace-only sources become `Table`; direct unallocated blank output is refused. |
| Worksheet name: reserved `History`, in any case | [Microsoft worksheet naming][excel-names] | Reserve its caseless key; allocate `History (2)` or next free suffix with a `Converted` mapping. |
| Worksheet names: caseless uniqueness | [Microsoft Office sheet implementation][excel-sheet] | Bounded fixed point of uppercase then lowercase: repeat until unchanged, at most four iterations, then use the last key for both allocation and final preflight/writer validation. `ß`/`ẞ` collide in either order; deterministic suffixes and ledger mappings. Greek/Turkish/ASCII/German and key idempotence tested. |
| Headers: nonempty and unique | [Issue #545](https://github.com/nurockplayer/tachiko-work/issues/545), adapter contract (stronger than plain Excel cells) | Empty becomes `Column`; use the same caseless suffix allocator; changed mappings are `Converted`; never truncate. An added suffix exceeding the cell limit is refused. |
| Text/headers: at most 32,767 UTF-16 units | [Excel limits][excel-limits]; conservative UTF-16 measurement required by the lead disposition | Refuse 32,768; accept 32,767 for ASCII, BMP and astral values and headers. |
| Text/headers: at most 253 line feeds | [Excel limits][excel-limits] | Refuse 254; preserve all 253 at the boundary. Applies to cells/headers, not allocated worksheet names. |
| Text/headers/names: literal `_x[0-9A-Fa-f]{4}_` | ECMA-376 Part 1 §22.9.2.19 `ST_Xstring`; [lead fidelity disposition](https://github.com/nurockplayer/tachiko-work/pull/546#issuecomment-6262364908) | Refuse source and emitted escapes because consumers decode them; accept near-misses. No encoder/reader changes. |
| Text/headers/names: XML-legal characters | [XML 1.0 §2.2][xml-characters] | Refuse forbidden controls, U+FFFE/U+FFFF. CSV applies the same rule to selected Text and original/emitted headers, with a located finding. Rust strings cannot contain surrogates. Accept TAB/LF/CR and other XML-legal scalars, including astral characters. |
| Text/headers: leading/trailing whitespace | [XML 1.0 §2.10][xml-space] | Inline Text with `xml:space="preserve"`; CR encoded as `&#13;`. Verify exact Work readback and independent-reader spaces/TAB/LF. Worksheet names retain their explicit rename rules. |
| Numbers: finite | [Microsoft numeric behavior][excel-numbers] | Refuse NaN/infinities with a cell finding; emit finite numeric cells. |
| Numbers: no nonzero subnormals | [Microsoft numeric behavior][excel-numbers] | Refuse magnitude below `f64::MIN_POSITIVE`; accept both minimum-normal signs. Emit both zero signs as `0`. |
| Numbers: precision above 15 significant decimal digits | [Excel limits][excel-limits] | Export the shortest round-trip value unchanged; `LossyOnExport` at first 20 locations plus total count. CSV ledger states full precision. |
| Dates: 1900-01-01..9999-12-31 (1900 system) | [Excel limits][excel-limits] | Refuse outside range or invalid Gregorian dates; accept endpoints; typed ISO Dates with `yyyy-mm-dd`. |
| Dates: Excel's fictitious 1900 leap day | [Microsoft leap-year explanation][excel-leap] | No 1900-02-29. Typed file readback and equivalent numeric fixtures verify Feb 28/Mar 1 map to 59/61, skipping serial 60. |
| Booleans | [SpreadsheetML Boolean cell type][excel-booleans] | Emit `t="b"` and `0`/`1`; read back false/true as Boolean, not Text or Number. |
| Grid: Excel 1,048,576 rows / 16,384 columns | [Excel limits][excel-limits] | Native profile is smaller: 128 data rows plus header, 32 columns, 32 sheets and 1,024 total data rows. Exact/+1 tests; refuse excess rather than truncate. |
| Field constraints: no declared spreadsheet mapping | [Fail-closed fidelity profile](../../packages/browser-client/interop-profile.json), [lead disposition](https://github.com/nurockplayer/tachiko-work/pull/546#issuecomment-6265455373) | Require `FieldConstraint::None`; refuse `TextLiteralSet`, `NumberInclusiveRange` and all other non-None variants at the stable collection/field location. Whole XLSX refuses; only the selected CSV collection is checked, including when empty. |
| Rectangular missing values | [Issue #545](https://github.com/nurockplayer/tachiko-work/issues/545) | Missing optional values emit `Empty` in their original position; empty Text stays typed Text in the source XLSX. |
| Package/receipt resource bounds | [Issue #545](https://github.com/nurockplayer/tachiko-work/issues/545), dedicated bounds below | Refuse above 8 MiB encoded / 24 MiB expanded or the existing receipt limit; exact/+1 byte tests. |

[excel-names]: https://support.microsoft.com/en-gb/excel/rename-a-worksheet
[excel-sheet]: https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/ebf12ea5-2bb4-4af5-ab26-563f22d3f895
[excel-limits]: https://support.microsoft.com/en-us/excel/excel-specifications-and-limits
[excel-numbers]: https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/excel/floating-point-arithmetic-inaccurate-result
[excel-leap]: https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/excel/wrongly-assumes-1900-is-leap-year
[excel-booleans]: https://learn.microsoft.com/en-us/office/open-xml/spreadsheet/how-to-retrieve-the-values-of-cells-in-a-spreadsheet
[xml-characters]: https://www.w3.org/TR/REC-xml/#charsets
[xml-space]: https://www.w3.org/TR/REC-xml/#sec-white-space

The independent torture workbook includes maximum-length ASCII/BMP/astral
Text and headers, XML-legal scalar boundaries, whitespace, escape near-misses,
minimum-normal Numbers, both Boolean values, Date endpoints and the leap-bug
boundary, allocated reserved/Unicode names, and the native grid boundaries.
Set `TACHIKO_NATIVE_TABLES_TORTURE_EVIDENCE` when running its unit test to write
both XLSX and the expected source projection JSON. LibreOffice ODS readback
checks typed values and names independently of Work's importer. External
applications may round Numbers already covered by the precision ledger, and
may collapse empty Text to an empty cell. Those limitations are not represented
as exact-value preservation claims. Direct writer tests cover labels rejected
by resident admission (for example untrimmed/control-bearing display names).

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
  deterministic Greek/German/Turkish/ASCII caseless mappings, including both
  `ß`/`ẞ` allocation orders as worksheets and headers with source/output ledger
  mappings, located subnormal
  refusal, full-precision CSV and the bounded XLSX precision warning ledger.
- `interop_adapter::native_tables_output_tests`: independent outbound geometry,
  encoded CSV byte and expanded XLSX byte limits, including exact/+1 boundaries;
  Text/header lengths of 32,767 and 32,768 UTF-16 units for ASCII, multi-byte BMP
  and astral characters. The expanded-XLSX fixture reaches exactly 24 MiB and
  +1 across many valid cells, each within the per-cell limit. Direct native
  writer tests additionally enumerate the compatibility inventory, reserved and
  UTF-16 worksheet names, XML characters and whitespace, Date endpoints and
  1900-system serial fixtures, caseless uniqueness (including unsuffixed `ß`/`ẞ`
  rejection in both orders), fixed-point key idempotence across all Unicode
  scalars and contextual strings, minimum-normal/adjacent
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
