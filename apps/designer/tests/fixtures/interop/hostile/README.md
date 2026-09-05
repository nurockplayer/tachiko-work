# Synthetic unsupported and bounded-input fixtures

These eight archives derive from the ordinary reference workbook, with minimal targeted mutations. They have not been opened or evaluated in LibreOffice, Excel or a browser. Added parts can be structurally incomplete and are not claimed to be complete standards-valid workbooks. Binary macro/ActiveX/OLE parts contain inert synthetic markers only.

`inventory.json` records each exact SHA-256, compressed/expanded size, targeted construct/location, expected safe behavior, construction-check status and this limitation. `../build-hostile.py` regenerates the estate using fixed ZIP timestamps and sorted entries.

| Fixture | Purpose |
| --- | --- |
| 01-shared-selfclosing.xlsx | Shared formula master plus self-closing follower; follower cached 777 must not be admitted as scalar truth. |
| 02-external-dde.xlsx | External workbook reference, DDE formula and external relationship to reserved example.invalid; never fetch or execute. |
| 03-disabled-parts.xlsx | Inert macro/ActiveX/OLE markers plus synthetic pivot/chart XML inventory. |
| 04-table-validation-names.xlsx | Synthetic table part, actual worksheet dataValidation/autoFilter elements and workbook definedName. |
| 05-date-boundaries.xlsx | Date-formatted 1900 serial 60 and 46270.5 time fraction; never produce fictitious Date or truncate noon. |
| 06-oversize-address.xlsx | Cell reference ZZZZZZZZZZZZ999999999; reject before allocation based on coordinates. |
| 07-duplicate-zip-entry.xlsx | Two xl/workbook.xml entries; reject archive ambiguity. |
| 08-expanded-limit.xlsx | A 524311-byte synthetic XML part compressing below 2048 bytes; under an explicitly configured 256 KiB expanded limit, reject before full materialization. This is a bounded half-MiB concept input, not a large bomb. |

Construction checks reopen archives with Python zipfile/XML only, assert the intended mutations, count duplicate entries and inspect advertised sizes. They do not assert any Tachiko runtime outcome or reference-tool admission.

Rebuild command:

```sh
uv run --no-project --python /Users/tachikoma/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 python /tmp/tachiko-259-fixtures/build-hostile.py
```
