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
| Rename column | An explicit current `display_name` replaces the source header. A surviving imported field with no explicit display name keeps its metadata header, rather than exposing its generated `column_N` key. Undo restores that fallback. |
| Add optional column, blank or filled | Append after surviving source columns, in current canonical field order. Read present typed values from the snapshot; absence is Empty. No source width or style is invented. |
| Remove column | Omit its header, values, width and cell styles. Historical slots remain available for Undo. |
| Insert row, including blanks | Append after surviving source rows, in current canonical entity order. New cells receive default presentation. |
| Delete row | Omit it. Metadata cannot recreate the entity. The existing “deleted rows are omitted” ledger statement remains accurate. |
| Undo / Redo | Recompute the mapping against current identities. Surviving or restored original cells recover their original widths/styles. |
| Save/reopen, v3 and package | With the same host metadata, apply exactly the same mapping to the reopened content. Session history is not saved. Metadata-aware v3 admission uses the same reconciliation. |

Source column order and surviving source row order remain presentation authority;
new slots append deterministically. No frontend sort/filter changes the export.
Every current schema must have exactly one metadata sheet. New/foreign/unmapped
schemas, duplicate mappings, invalid source style dimensions and invalid widths
refuse. Non-native sheets still require complete current identity mappings;
structural reconciliation never authorizes new native operations on them.
A current field missing from metadata must have an explicit display name;
otherwise its spreadsheet header cannot be recovered and the whole export refuses.
Historical missing field/row IDs in native sheets are presentation tombstones:
they are ignored, never used to infer provenance or create values. Newly present
identities get default presentation; the host must preserve the original metadata
to preserve original presentation across history and saved copies.

## Compatibility, formulas and blanks

Column widths decode through the same JSON Number representation on host and
WASM, including fractional widths in tagged metadata requests.

Current headers pass the existing native compatibility name policy: preserve
legal spelling, allocate deterministic caseless collision suffixes without
truncating headers, and refuse XML-illegal or literal SpreadsheetML escape text.
Renames and mapped spellings have `converted` findings at collection/field IDs.
Structural changes have `imported_current_structure` at the stable collection.
Source worksheet names remain unchanged and must satisfy the existing shared
profile. XLSX inserts headers for headerless input as before, with disclosure.

Unchanged cells retain their supported styles and typed values. Formulas are
rebuilt from stable references using current absolute A1 coordinates, including
column removal and row deletion; missing references or unsupported formulas
refuse the whole export. Calculation and formula-editing rules do not change.

An absent optional value is an Empty cell. Present empty Text remains Text in
XLSX; CSV represents both with an empty field and discloses this in
`csv_values_only`. CSV also discloses lost formula definitions, types,
presentation and non-selected worksheets. It refuses formula-leading literal
Text/headers; it does not rewrite them. XLSX covers every mapped worksheet.

## Limits and atomic refusal

The shared output profile remains 4 sheets, 16 columns, 64 data rows per sheet,
32 formulas, 2 MiB encoded and 8 MiB expanded XLSX. The existing separately
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
