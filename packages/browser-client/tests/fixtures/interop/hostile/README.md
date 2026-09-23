# Synthetic unsupported and bounded-input fixtures

These eight archives derive from `../reference-two-sheet.xlsx`, with minimal targeted mutations. Its SHA-256 is `81cce4bdfecc8e9832a48cfff4b6e83818c1f169d84d339d9793954655800d63`. The subsequently namespace-prefixed `../ordinary-two-sheet.xlsx` is a different artifact and is not their construction source. They have not been opened or evaluated in LibreOffice, Excel or a browser. Added parts can be structurally incomplete and are not claimed to be complete standards-valid workbooks. Binary macro/ActiveX/OLE parts contain inert synthetic markers only.

`inventory.json` records each exact SHA-256, compressed/expanded size, targeted construct/location, expected safe behavior, construction-check status and this limitation. The original construction used fixed ZIP timestamps and sorted entries. Its temporary generator was not committed; these checked-in archives and the inventory identify the preserved construction outputs.

| Fixture | Purpose |
| --- | --- |
| 01-shared-selfclosing.xlsx | Shared formula master plus self-closing follower; follower cached 777 must not be admitted as scalar truth. |
| 02-external-dde.xlsx | External workbook reference, DDE formula and external relationship to reserved example.invalid; never fetch or execute. |
| 03-disabled-parts.xlsx | Inert macro/ActiveX/OLE markers plus synthetic pivot/chart XML inventory. |
| 04-table-validation-names.xlsx | Synthetic table part, actual worksheet dataValidation/autoFilter elements and workbook definedName. |
| 05-date-boundaries.xlsx | Date-formatted 1900 serial 60 and 46270.5 time fraction; never produce fictitious Date or truncate noon. |
| 06-oversize-address.xlsx | Cell reference ZZZZZZZZZZZZ999999999; reject before allocation based on coordinates. |
| 07-duplicate-zip-entry.xlsx | Two xl/workbook.xml entries; reject archive ambiguity. |
| 08-expanded-limit.xlsx | Unknown `xl/synthetic-expanded.xml` part: 524311 expanded bytes / 549 compressed bytes; the whole archive expands to 549384 bytes, below the shipped 8 MiB limit. The shipped adapter inventories `unknown_package_part` as blocking. This fixture does not prove shipped expanded-limit rejection. |

Construction checks reopen archives with Python zipfile/XML only, assert the intended mutations, count duplicate entries and inspect advertised sizes. They do not assert any Tachiko runtime outcome or reference-tool admission.

The 256 KiB limit previously associated with fixture 08 belonged only to an earlier synthetic experiment, not the shipped profile. The separate runtime test `expanded_limit_is_checked_before_xml_materialization` generates an archive above `MAX_EXPANDED_BYTES` and checks the actual shipped rejection.

The TypeScript fixture-manifest test verifies the explicit source path, source and all eight archive SHA-256 hashes and archive byte lengths using `node:crypto`. The Rust adapter test verifies ZIP entry counts and expanded/compressed sizes using the existing ZIP dependency. No adversarial source formulas or embedded content are executed.
