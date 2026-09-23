# Issue 259 local reference fixture evidence

All files in this estate are synthetic test inputs. Repository copies are `ordinary-two-sheet.xlsx`, `reference-two-sheet.xlsx` (the original `reference/ordinary-two-sheet.xlsx`), `messy-utf8.csv`, `expected.json`, and `hostile/`. Other paths below describe the historical local verification estate, not checked-in files. No repository file, browser session, Excel window, global configuration or installed dependency was modified. The LibreOffice user profiles used here belong only to this temporary directory.

Reference tool, observed from `soffice --version`:

`LibreOfficeDev 26.8.0.0.alpha0 2c87e51eeaa2b413ff4ae097b2705eea1995d8e5`

## Files

- `ordinary-two-sheet.xlsx`: artifact-tool authored input; worksheets Budget and Rates, 54 populated cells, 12 numeric formulas.
- `reference/ordinary-two-sheet.xlsx`: actual LibreOffice headless open/save of the input; recommended ordinary XLSX importer fixture.
- `reopen/ordinary-two-sheet.xlsx`: second independent LibreOffice profile open/save of the reference output.
- `messy-utf8.csv`: raw UTF-8 CSV, CRLF records, embedded LF, quoted comma, leading-zero identifiers, trim/split fields, duplicate rows and ambiguous date.
- `expected.json`: exact cell raw/type/formula/result/format evidence, CSV cell evidence, limitations and SHA-256 hashes.
- `cache-challenge.xlsx`: test copy of reference output with all 12 formula cached values replaced by -999, leaving formulas and inputs unchanged.
- `reopen/cache-challenge.xlsx`: forced-reference-recalculation result, all 12 caches restored to independently specified expected values.

## Commands actually executed successfully

```sh
soffice --version
/Users/tachikoma/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node /tmp/tachiko-259-fixtures/build.mjs
soffice -env:UserInstallation=file:///tmp/tachiko-259-fixtures/lo-profile-one --headless --convert-to 'xlsx:Calc MS Excel 2007 XML' --outdir /tmp/tachiko-259-fixtures/reference /tmp/tachiko-259-fixtures/ordinary-two-sheet.xlsx
uv run --no-project --python /Users/tachikoma/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 python /tmp/tachiko-259-fixtures/verify.py
soffice -env:UserInstallation=file:///tmp/tachiko-259-fixtures/lo-profile-two --headless --convert-to 'xlsx:Calc MS Excel 2007 XML' --outdir /tmp/tachiko-259-fixtures/reopen /tmp/tachiko-259-fixtures/reference/ordinary-two-sheet.xlsx /tmp/tachiko-259-fixtures/cache-challenge.xlsx
soffice -env:UserInstallation=file:///tmp/tachiko-259-fixtures/lo-profile-recalc --headless --convert-to 'xlsx:Calc MS Excel 2007 XML' --outdir /tmp/tachiko-259-fixtures/reopen /tmp/tachiko-259-fixtures/cache-challenge.xlsx
uv run --no-project --python /Users/tachikoma/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 python /tmp/tachiko-259-fixtures/verify.py
```

The second-profile conversion alone retained deliberately incorrect formula caches. Therefore it was not accepted as evaluation evidence. Only the third profile, configured locally with `/org.openoffice.Office.Calc/Formula/Load` property `OOXMLRecalcMode=0`, forced recalculation and restored every cache. The property and its purpose were checked against the installed LibreOffice registry schema. Logs are saved in `reference-command.log`, `reopen-command.log`, and `recalc-command.log`.

The final verifier passed: reference scalar/formula checks, second-profile reopen checks, and poisoned-cache recalculation checks. Formula result expectations were explicit constants computed from the fixture specification, not values copied from the tool output.

## Limitations

- Reference verification uses a LibreOffice development build, not Microsoft Excel.
- No claim of macros, external links, chart, pivot or validation-rule coverage is made for this ordinary estate.
- Date 2026-09-05 is represented in XLSX by 1900-system serial 46270 plus a date format. Serial identity is adapter-only.
- Currency and percent are Number plus format, not independent semantic types.
- The artifact-tool PNG preview shows leading-zero text 00123 as 123. The authored XLSX raw cell, reference output and second-profile output were independently checked to retain Text 00123. PNG is not the scalar authority.
- CSV source records 2 and 4 intentionally duplicate each other. 09/05/2026 remains ambiguous; no locale interpretation is asserted.
- The raw authoring workbook uses namespace-prefixed OOXML elements; the importer must respect namespace/local-name semantics rather than match only unprefixed tag spellings.
