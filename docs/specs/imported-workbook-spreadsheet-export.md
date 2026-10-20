# Imported workbook spreadsheet export

Status: Provisional implementation contract for [#557](https://github.com/nurockplayer/tachiko-work/issues/557),
under [ADR-0027](../decisions/ADR-0027-open-format-and-interoperability-policy.md)
and [ADR-0045](../decisions/ADR-0045-optional-native-text-and-number-fields.md).
This specifies the existing `exportSpreadsheet` / `export` metadata route;
[native table export](native-tables-spreadsheet-export.md) is a separate route.
The machine-readable profile is
[`interop-profile.json`](../../packages/browser-client/interop-profile.json).

## Identity and current structure

Imported tables qualify for native authoring structurally when every field is
scalar and every optional field is Text or Number. Origin is irrelevant. Optional
Date/Boolean imports remain non-native. Classification changes no stored field,
requiredness or value.

Export reads one revision-pinned immutable snapshot. Historical metadata is
presentation bound to stable schema, field and entity IDs, never content authority.
It is retained by the host alongside saved content; neither canonical v3 nor the
portable package gains spreadsheet presentation or a new storage version.

| Change | Metadata export decision |
| --- | --- |
| Rename column | An explicit current `display_name` replaces the source header. Table columns expose `display_name_explicit: true` exactly when that stored name exists, including a rename equal to the generated key; omission means fallback. A surviving imported field with no explicit display name keeps its metadata header, rather than exposing its generated `column_N` key. Undo restores that fallback and removes the projection flag. |
| Add optional column, blank or filled | Append after surviving source columns, in current canonical field order. Read present typed values from the snapshot; absence is Empty. No source width or style is invented. |
| Remove column | Omit its header, values, width and cell styles. Historical slots remain available for Undo. |
| Insert row, including blanks | Append after surviving source rows, in current canonical entity order. New cells receive default presentation. |
| Delete row | Omit it. Metadata cannot recreate the entity. The existing “deleted rows are omitted” ledger statement remains accurate. |
| Undo / Redo | Recompute the mapping against current identities. Surviving or restored original cells recover their original widths/styles. |
| Save/reopen, v3 and package | With the same host metadata, apply exactly the same mapping to the reopened content. Session history is not saved. Metadata-aware v3 admission uses the same reconciliation. |

Source column order and surviving source row order remain presentation authority;
new slots append deterministically. No frontend sort/filter changes the export.
Every current schema must have exactly one metadata sheet. New/foreign/unmapped
schemas, duplicate mappings and invalid source style dimensions refuse.
Presentation compatibility is checked only where that presentation reaches the
selected format: surviving XLSX widths/styles are checked; CSV omits them. Non-native sheets still require complete current identity mappings;
structural reconciliation never authorizes new native operations on them.
A current field missing from metadata must have an explicit display name;
otherwise its spreadsheet header cannot be recovered and the whole export refuses.
A field or entity ID present in another current schema is a foreign current
mapping and refuses, with that stable collection/field or collection/entity
location. Schema IDs must exist and be mapped once; swapped sheet-to-schema
mappings cannot smuggle another schema's current fields or rows into tombstones.
Only IDs absent from every current owner are historical tombstones:
they are ignored, never used to infer provenance or create values. Newly present
identities get default presentation; the host must preserve the original metadata
to preserve original presentation across history and saved copies.

## Compatibility, formulas and blanks

Column widths decode through the same JSON Number representation on host and
WASM, including fractional widths in tagged metadata requests.

Current headers use deterministic caseless collision suffixes without truncation.
XLSX uses the shared native compatibility name/Text policy. CSV uses the same
collision key and suffix allocation with its own XML-character and formula-leading
Text policy; literal SpreadsheetML escape text and XLSX-only string limits do not
restrict CSV headers.
Renames and mapped spellings have `converted` findings at collection/field IDs.
`imported_current_header` is emitted whenever the written spelling differs from
the current requested spelling **or** the historical header. A collision that
returns to the historical header still discloses its requested/output mapping.
Structural changes have `imported_current_structure` at the stable collection.
XLSX source worksheet names remain unchanged and pass the shared native name
policy: at most 31 UTF-16 units, legal characters/boundaries, no reserved `History`,
no literal SpreadsheetML escapes and no fixed-point caseless duplicates. Refuse
rather than allocate a new worksheet name and change formula references. XLSX
inserts headers for headerless input as before, with disclosure.

Every cell of every worksheet in an XLSX export, including untouched source
cells, newly inserted rows, added columns and calculated formula results, passes
the shared `native_tables_compatibility` policy before shared profile validation
or file encoding. Native tables and this route call the same
`validate_xlsx_compatibility` inventory; profile-specific grid bounds remain
separate:

- XML-illegal Text and literal `_x[0-9A-Fa-f]{4}_` sequences refuse; strings are
  never silently changed or Office-escaped by this route.
- Text may contain at most **32,767 UTF-16 code units** (astral characters count
  as two) and **253 LF characters**. Refuse excess without truncation.
- Numbers must be finite and either zero or at least `f64::MIN_POSITIVE` in
  magnitude. Nonzero subnormals refuse. The file retains the round-trip decimal
  for Numbers needing over 15 significant digits, with bounded `lossy_on_export`
  precision findings: first 20 affected scalar/formula/number-format locations
  and a total count. One formula or format location counts once even if several
  literals there need disclosure. Scalar-only native receipts keep their wording.
- Dates must be valid Gregorian dates in **1900-01-01..9999-12-31**. Boolean and
  Empty keep their existing mappings.

The format is selected **before reconstruction**. CSV builds only its selected
stable collection, checks headers only when `has_header` emits them, emits
authoritative scalar results, and never serializes its
formulas, worksheet names, widths or styles. Unselected cell values, formulas and
presentation cannot block a valid selected-sheet CSV. Global bounded metadata
shape and current identity ownership still apply; CSV selection must be a stable
schema ID. The independently admitted Text-capacity metadata rules remain intact.

XLSX inventories formula **tokens independently of cached results**. Its closed
emitted grammar consists of finite Number literals, stable references projected
to quoted worksheet names plus absolute A1 coordinates, arithmetic and MIN/MAX.
Every numeric literal passes the same Cell Number policy, including subnormals
and precision disclosure. Semantic Numbers normalize either zero sign to `0`;
NaN/infinity cannot enter the typed document. String and Boolean formula literals
have no AST variant and fail import/authoring; any future token outside the output
grammar refuses until governed. Numeric-looking text in a quoted worksheet name
is a name, not a literal. References resolve only through current mapped identities.
Formula source retains the existing 4,096-byte bound and 32-formula workbook bound;
no formulas or expression grammar are added by this repair.

All retained presentation also passes the shared policy. Number-format codes use
the existing closed classifier; numeric conditions come from that **same tokenizer**
and pass the shared Decimal/Cell Number checks. Retained condition spelling is
checked before binary64 conversion can hide nonzero underflow or extra decimal
precision. Format codes refuse XML-illegal or literal `_xHHHH_`
text and codes of 255 or more UTF-16 units. XML attribute TAB/LF/CR use character
references, preserving their actual contents. Alignment remains left/center/right/
general; fill remains six/eight hex digits; bold/wrap/border are typed booleans.
Widths must be finite, positive, at most 255, and exact multiples of 1/256 character;
refuse widths Excel would quantize rather than rounding them. The writer allocates
one format/font/fill/border per distinct style, so permit at most **200 styles
including the generated default**, below Office's format/fill resource ceilings.
No style is merged or discarded to fit. These compatibility decisions follow
Microsoft's [number-format constraints](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/17d11129-219b-4e2c-88db-45844d21e528),
[column-width representation](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.column?view=openxml-3.0.1)
and [Excel resource limits](https://support.microsoft.com/en-us/excel/excel-specifications-and-limits).

Failures carry the shared policy's specific code and current worksheet/row/column
location, with formula/literal, number-format/condition, sheet-name or width context
where applicable. No bytes are returned. CSV keeps its lexical representation and existing
formula-leading refusal; XLSX-only length, escapes, line-feed, subnormal and Date
rules do not restrict CSV cells. CSV XML-character refusals also identify the
current row/column. The format-neutral Rust `export_workbook` projection is not
an XLSX compatibility claim; both host and WASM file exports use the format-aware
preflight. Canonical v3/package reopen and its existing format-neutral source
closure remain unchanged: source Text can reopen exactly even when Excel cannot
represent it. Each subsequent XLSX file export independently refuses, and a
representable CSV export remains available. Internal source reconstruction is
not a claim of Excel compatibility.

Supported untouched cells retain their styles and typed values. The new refusals
and precision disclosures affect only previously unfaithful or undisclosed XLSX
output. Supported values/formulas/styles keep their meaning and presentation;
attribute character references repair previously unfaithful whitespace encoding.
New worksheet/style/format/width refusals affect output that could previously be
changed, truncated or rounded by Excel, or refused only with a generic workbook
diagnostic. Scalar precision
policy remains disclosure, without rounding the file's decimal text. Formulas are
rebuilt from stable references using current absolute A1 coordinates, including
column removal and row deletion; missing references or unsupported formulas
refuse the whole export. Calculation and formula-editing rules do not change.

An absent optional value is an Empty cell. Present empty Text remains Text in
XLSX; CSV represents both with an empty field and discloses this in
`csv_values_only`. CSV also discloses lost formula definitions, types,
presentation and non-selected worksheets. It refuses formula-leading literal
Text/headers; it does not rewrite them. XLSX covers every mapped worksheet.

### Issue #547 boundary

This repair guards **every emitted imported-route text surface**: cells,
headers, worksheet/reference names and number-format codes. Generated formulas
cannot contain string literals. It widens batch 1's imported-cell boundary and
removes imported export from the known unguarded `_xHHHH_` writer routes, by
whole-file refusal. It does not close [#547](https://github.com/nurockplayer/tachiko-work/issues/547):
native Tracker and native Budget still use the general shared writer without
this compatibility preflight; that writer still lacks Office literal escaping.
The reader still lacks SpreadsheetML escape decoding and its fidelity ledger,
and independent-reader qualification across every export profile remains open.
Native tables already uses the same refusal policy. Attribute whitespace encoding
is repaired in the shared writer; no reader decoding or global Office escaping
change is claimed.

## Output-element coverage

All test names below are in `runtime/tests/imported_structural_export.rs` unless
qualified otherwise. Their recorded host operations and opaque file bytes are
required captures replayed through the fresh actual WASM ABI by the parity gate.
No ledger string is embedded in either spreadsheet file: serde JSON encodes the
bounded response receipt. Generated coordinates, IDs, ZIP member names, XML tags,
relationship URIs and fixed Arial/font/border/fill defaults are constants or bounded
integers, never raw source markup.

| Element | Check / emitted representation | Test name |
| --- | --- | --- |
| CSV scope | Select stable schema before reconstruction; only selected emitted headers/scalars; headerless names neither checked as file text nor disclosed as written; global identity map still checked | `imported_csv_checks_only_selected_emitted_values_and_headers`, `foreign_current_fields_entities_and_schema_mappings_are_not_tombstones` |
| Text scalars, including untouched/inserted/added | XLSX Cell XML/escape/32,767 UTF-16/253 LF checks; CSV CsvCell/CsvText XML and formula-leading refusal, RFC quoting | `csv_import_every_current_cell_has_format_specific_compatibility`, `xlsx_import_every_current_cell_has_format_specific_compatibility`, `untouched_imported_text_is_checked_before_xlsx_encoding` |
| Number scalars and formula caches | Finite semantic Number; canonical zero; XLSX normal-or-zero and 15-digit disclosure; CSV round-trip scalar decimal | `imported_numbers_and_dates_follow_the_shared_cell_policy_without_changing_csv`, `imported_formula_literals_and_cached_results_share_the_number_policy` |
| Boolean, Date, Empty scalars | Typed XLSX boolean 0/1, Gregorian Date range, Empty distinct from Text; CSV lexical/empty fields with disclosure | `imported_boolean_values_use_typed_xlsx_and_lexical_csv`, `imported_numbers_and_dates_follow_the_shared_cell_policy_without_changing_csv`, `csv_insert_blank`, `xlsx_insert_blank` |
| Formula text/operators/functions | Closed numeric/reference/arithmetic/MIN/MAX wire grammar; 4,096-byte / 32-formula bounds; XML text escaping | `imported_formula_grammar_and_nonfinite_numbers_cannot_reach_file_output`, `structural_export_preserves_untouched_formulas_styles_and_workbook_coverage` |
| Every formula numeric literal | Same Cell Number policy, independently of cache; includes subnormal, minimum normal, canonical -0, precision | `imported_formula_literals_and_cached_results_share_the_number_policy` |
| Formula string/Boolean/NaN/infinity literals | No corresponding AST variant or nonfinite Number; reject before publication; unknown future wire tokens also refuse | `imported_formula_grammar_and_nonfinite_numbers_cannot_reach_file_output` |
| References and names inside formulas | Stable binding, current absolute coordinates; quoted/doubled apostrophe names; shared worksheet-name policy; never scan name digits as numbers | `imported_reference_names_headers_and_csv_quoting_preserve_spelling`, `structural_export_preserves_untouched_formulas_styles_and_workbook_coverage` |
| Headers | Current/historical fallback and caseless allocation; format-specific text checks; XML/CSV escaping and header-insertion disclosure | `imported_reference_names_headers_and_csv_quoting_preserve_spelling`, `header_mapping_discloses_requested_spelling_even_when_output_equals_history`, `unrepresentable_structures_refuse_whole_files_with_located_findings` |
| Worksheet names | Name policy: 31 UTF-16 units, illegal/reserved/escape/caseless checks; no rename; CSV does not emit them | `imported_xlsx_presentation_surfaces_have_located_shared_policy_checks`, `imported_reference_names_headers_and_csv_quoting_preserve_spelling` |
| Number-format codes and literal display text | Closed classifier; ST_Xstring/XML refusal, <255 UTF-16 units; attribute character references preserve TAB/LF/CR | `imported_xlsx_presentation_surfaces_have_located_shared_policy_checks` |
| Numeric format conditions / locale and color tokens | Same format tokenizer inventories numeric conditions for Cell checks; existing closed color/locale grammar; precision disclosure | `imported_xlsx_presentation_surfaces_have_located_shared_policy_checks`; `interop_number_format::tests` |
| Fill/alignment/bold/wrap/border and generated font/style attributes | Hex/enumeration/typed boolean checks; fixed generated Arial/size/border vocabulary; preserve supported presentation | `imported_xlsx_presentation_surfaces_have_located_shared_policy_checks` |
| Style/format/font/fill/border resource counts | At most 200 distinct styles including default; matches writer deduplication, no merging | `imported_style_inventory_refuses_before_excel_format_resource_limits` |
| Width attributes | Width policy: finite, (0,255], exact 1/256 unit; omit for CSV | `imported_xlsx_presentation_surfaces_have_located_shared_policy_checks` |
| Precision findings and response bounds | Shared first-20 location budget plus total across scalar/formula/format locations; accurate CSV consequence | `imported_formula_precision_disclosures_share_the_bounded_location_budget` |
| Ledger strings / source names in receipts | Response-only JSON escaping and existing complete-receipt size bound; never XLSX/CSV payload; deleted rows statement retained | `imported_reference_names_headers_and_csv_quoting_preserve_spelling`, all structural matrix cases |
| Geometry/coordinates/generated XML/ZIP members and byte totals | Existing per-format shared/Text-capacity bounds and deterministic bounded writer; no raw source markup/ZIP paths; whole-file refusal | `unrepresentable_structures_refuse_whole_files_with_located_findings`, all structural matrix cases; `interop_adapter::native_tables_output_tests` |

## Limits and atomic refusal

The shared output profile remains 4 sheets, 16 columns, 64 data rows per sheet,
32 formulas, 2 MiB encoded and 8 MiB expanded XLSX. The compatibility inventory
also enforces the Office presentation limits above. The existing separately
admitted Text-capacity exception keeps its exact bounds and metadata rules.
Native authoring capacity does not widen metadata export capacity. Crossing any
output, style, formula, character or mapping limit refuses the entire operation;
no truncated workbook or partial bytes are returned. Spreadsheet export does not
mutate content, revision, history or source metadata.

Mapping/output failures use `imported_spreadsheet_export_refused` with a blocking
`unsupported_safe_disabled` finding projected as a typed diagnostic: a stable
collection/field path where known, otherwise `workbook`. Compatibility findings
retain their specific codes; other unmappable output uses
`imported_spreadsheet_unrepresentable`. Stale revisions keep the existing typed
stale failure. CSV selection must identify a mapped stable schema ID.

## Qualification

`runtime/tests/imported_structural_export.rs` exercises real CSV/XLSX imports,
all six native operations (blank and partially filled additions separately), both
export formats, Undo/Redo, and edited/undone v3 and package reopen. Independent
expected cell arrays are compared with decoded files. The optional native-fields
parity gate captures and replays these host operations and opaque output bytes
through the fresh actual WASM ABI. Boundary and multi-sheet formula/style cases
cover whole-file refusal and surviving presentation.

Repair-batch qualification also covers every-cell compatibility boundaries,
untouched-cell refusal, Number/Date policy, foreign current field/entity/schema
mappings and collision allocation returning to a historical header. These
fixtures are required captures in the same actual host/WASM parity gate.

Root-cause qualification inventories all output elements, including formula literals,
selected-sheet CSV scope, quoted/numeric-looking references, closed formula input,
Boolean scalars, style/format/width boundaries, attribute normalization and bounded
precision receipts. No TS/e2e fixture or seed format changes are needed.

Capacity acceptance applies the same format distinction to Unicode/XML-sensitive
metadata. CSV preserves quoted header cell text and does not emit a worksheet
name, so a control-bearing metadata name does not cause this CSV export to
refuse. XLSX refuses LF/TAB/CR in worksheet names with `native_tables_xlsx_name`
at `/sheet-name`, no partial file and unchanged resident state. A separate
31-character Excel-valid name containing Unicode and XML-sensitive characters
roundtrips with the same 39-byte header row. Promoted-capacity Worker probes and
the independent XML oracle require the valid name and retain the negative
control-name case; they do not claim Excel can retain control-bearing names.
