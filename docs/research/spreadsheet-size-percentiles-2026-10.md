# Spreadsheet size percentiles from the FUSE corpus

Status: research evidence, 2026-10-26 JST. This note records a measurement; it
does not change an accepted contract, an ADR, or this repository's capacity
gates.

## Question

`capacity-research-2026-10-26.md` established p50/p75 and means for spreadsheet
size but no measured p90/p95/p99 for an intended business population.
`capacity-plan-2026-10-26.md` §4 asks for those percentiles and a coverage
table against the proposed Stage A–D profiles. This note reports what the FUSE
metadata corpus can and cannot answer, so the capacity targets can be compared
against measured data instead of extrapolated quartiles.

## 1. Source

- Corpus: FUSE: A Reproducible, Extendable, Internet-Scale Corpus of
  Spreadsheets (Barik, Lubick, Smith, Slankas, Murphy-Hill, MSR 2015 Data
  Showcase, <https://go.barik.net/msr2015>), downloaded from its official page
  <https://static.barik.net/fuse/index.html> on 2026-10-26 at 04:04 JST.
- File used: `fuse-bin.analysis.dedup.poi-dec2014.json.gz`,
  <https://blob.barik.net/fuse/fuse-bin.analysis.dedup.poi-dec2014.json.gz>.
- Size 311,756,140 bytes (297 MiB). SHA-1
  `9aea74288ce99129282d8dfed78aeed11dc911ee`, matching the checksum published on
  the download page; the analysis command verifies it before parsing.
- 249,376 records, one JSON object per line inside the gzip stream, one record
  per FUSE-deduplicated spreadsheet binary.

Why the POI-enriched deduplicated file: the smaller 213 MB
`fuse-bin.analysis.dedup-dec2014.json.gz` (SHA-1
`50f880258ac2ebbd407e3f718e57ac2ab23ca03e`, 222,995,401 bytes, also 249,376
records) was downloaded first and inspected. Despite its name it contains only
WARC provenance, Tika file facts and LingPipe tokens; it carries no spreadsheet
analysis fields at all. It does carry `Tika.Length`, so file-byte percentiles
could be computed from it, but worksheet counts and cell counts require the
`.poi` variant, which adds the Apache POI workbook analysis. The undecorated
`fuse-bin.analysis-dec2014.json.gz` has the same provenance-only shape in its
first records.

The 6.9 GB `fuse-binaries-dec2014.tar.gz` (the spreadsheet binaries themselves)
was not downloaded: it is the only FUSE artifact with per-worksheet geometry,
and parsing 249k workbooks is a separate, much larger job (see §7).

## 2. Method

- Analysis script: `tools/corpus/fuse_metadata.py` version 1.0.0, Python 3.11.2,
  standard library only.
- Execution: inside an OrbStack container through `orb-test`
  (`--memory 1024m --cpus 1 --image tachiko-work-rust:1.99.0`) on
  2026-10-26 08:32 JST, with the dataset on a scratch mount and the
  machine-readable result written as JSON next to it. The dataset and that JSON
  were deleted after the run; this report and the script are the only retained
  artifacts.
- Percentiles use the nearest-rank definition on sorted values: the value at
  rank `ceil(p/100 × n)`, 1-indexed. For small `n` this selects an observed
  value rather than interpolating between two.
- Inclusion (the "analyzable" set): the record's POI analysis reports
  `errorNotification == "OK"`, and it has a positive `Tika.Length`,
  `numSheets > 0`, and non-negative `totalInputCells` and `totalFormulas`.
  Numeric fields must be finite integers; a fractional or non-finite value
  excludes the record rather than being rounded.
- Deduplication: the source file is already FUSE's deduplicated derivative (one
  record per unique binary, 249,376 of them). The script additionally drops any
  repeated `Tika.Digest` SHA-1 it encounters among analyzable records and
  counts the drops. Deduplication runs after the readability check so that an
  unreadable duplicate cannot shadow a readable copy.
- Exclusions are counted rather than silently skipped; see §5.

## 3. Field definitions

FUSE's POI analysis is workbook-level. The report uses these fields:

| Report metric | FUSE field | Definition |
| --- | --- | --- |
| Source file bytes | `Tika.Length` | Size of the spreadsheet file as downloaded, in bytes. For `.xlsx` this is the compressed zip container; it is not expanded size or import memory. |
| Worksheets per workbook | `POI.numSheets` | Number of worksheets in the workbook. |
| Input cells per workbook | `POI.totalInputCells` | Cells holding a constant (non-formula) value. Blank and format-only cells are not counted. |
| Formula cells per workbook | `POI.totalFormulas` | Cells holding a formula. |
| Populated cells per workbook | sum of the two | The closest FUSE analogue of Tachiko's non-empty scalar cell count. |
| Formula-free / formula-bearing | derived from `POI.totalFormulas` | Formula-free means `totalFormulas == 0`. |

The POI analysis partitions both totals by cell type (`integer`, `nonInteger`,
`string`, `boolean`, `error`, `dateTime`, `blank`). The script checks that the
subtype sums equal the reported totals and reports any mismatch (§5); a
mismatch would mean the totals and the type breakdown describe different
populations, and would weaken any inference drawn from the types.

## 4. Results

Run 2026-10-26T08:32:27+09:00 over all 249,376 records:

| Population | Workbooks | Share of analyzable |
| --- | --- | --- |
| Records in the file | 249,376 | — |
| Analyzable (POI `OK`, usable fields) | 220,760 | 100% |
| Formula-free (`totalFormulas == 0`) | 205,978 | 93.30% |
| Formula-bearing | 14,782 | 6.70% |
| Multi-sheet (>1 worksheet) | 36,745 | 16.64% |

The analyzable and formula-free counts match FUSE's published figures exactly:
249,376 unique files, 28,616 unreadable, 205,978 analyzable files with zero
formula cells. Independent agreement on all three numbers confirms this reader
selects the same population FUSE's own tools did.

### 4.1 Workbook-level distributions

| Metric | n | p50 | p75 | p90 | p95 | p99 | max | mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Source file bytes | 220,760 | 11,267 | 26,115 | 127,488 | 254,976 | 778,240 | 1,048,576 | 52,593.41 |
| Worksheets per workbook | 220,760 | 1 | 1 | 3 | 3 | 10 | 147 | 1.57 |
| Input (non-formula) cells per workbook | 220,760 | 184 | 685 | 2,652 | 7,361 | 31,232 | 252,642 | 1,618.09 |
| Formula cells per workbook | 220,760 | 0 | 0 | 0 | 5 | 762 | 58,389 | 48.82 |
| Populated cells per workbook | 220,760 | 184 | 702 | 2,826 | 7,477 | 32,035 | 252,642 | 1,666.91 |

Reading these: half of the FUSE workbooks have at most 184 populated cells, 90%
at most 2,826, and 99% at most 32,035. The distributions are long-tailed (the
mean sits far above p50) but the tail stops at 252,642 cells, consistent with
the corpus's 1 MiB file-size ceiling (see §7). For comparison, Sheetpedia's
published medians are 300 cells per worksheet and 270 per workbook; FUSE's
workbook median of 184 describes a smaller-file corpus, and neither is a
business population.

### 4.2 Formula-free versus formula-bearing workbooks

| Metric | n | p50 | p75 | p90 | p95 | p99 | max | mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Populated cells, formula-free workbooks | 205,978 | 184 | 637 | 1,857 | 7,008 | 30,134 | 252,642 | 1,505.49 |
| Populated cells, formula-bearing workbooks | 14,782 | 905 | 4,678 | 8,543 | 15,625 | 41,671 | 160,478 | 3,916.19 |
| Formula cells, formula-bearing workbooks | 14,782 | 44 | 326 | 1,492 | 3,173 | 14,786 | 58,389 | 729.06 |

Formula-bearing workbooks are roughly five times larger at the median (905
versus 184 populated cells) and larger at every percentile above it. Across all
workbooks, 90% contain no formula at all (the p90 of formula cells is 0); among
the 6.7% that do, the median workbook carries 44 formula cells and the p99
carries about 14,800.

This is the clearest difference from the Enron business sample, where 58% of
workbooks contained formulas. FUSE is dominated by published flat data:
sports results, government reports, downloadable tables. Neither corpus is the
intended business population, so the two are complementary rather than
contradictory.

### 4.3 Coverage against the Stage A–D profiles

The stages are the profiles in `capacity-plan-2026-10-26.md` §2 and the
research note's §4.2: Stage A 10,000 rows per worksheet / 100,000 populated
cells per workbook; Stage B 50,000 / 500,000; Stage C 100,000 / 2,000,000;
Stage D 1,000,000 / 20,000,000.

Workbooks fitting the populated-cell ceiling of each profile:

| Profile | Cell ceiling | All (n=220,760) | Formula-free (n=205,978) | Formula-bearing (n=14,782) |
| --- | ---: | ---: | ---: | ---: |
| Stage A | 100,000 | 220,668 (99.9583%) | 205,893 (99.9587%) | 14,775 (99.9526%) |
| Stage B | 500,000 | 220,760 (100%) | 205,978 (100%) | 14,782 (100%) |
| Stage C | 2,000,000 | 220,760 (100%) | 205,978 (100%) | 14,782 (100%) |
| Stage D | 20,000,000 | 220,760 (100%) | 205,978 (100%) | 14,782 (100%) |

Only 92 workbooks (0.042%) exceed 100,000 populated cells; none exceeds
500,000. The same table cannot be produced for the row ceilings — §6 explains
why — so these percentages are upper bounds on real coverage, and the 1 MiB
corpus ceiling (§7) biases them upward by excluding large workbooks before
analysis. They show that the workbook cell ceilings are not the binding
constraint for web-crawled spreadsheets; they do not show that Stage A covers
99.96% of business spreadsheets.

## 5. Exclusions and data quality

| Exclusion | Count |
| --- | ---: |
| JSON parse errors | 0 |
| Malformed records (not an object, non-string digest) | 0 |
| Missing POI analysis | 0 |
| Missing length/sheet/cell fields | 0 |
| Invalid size or dimensions | 0 |
| Repeated `Tika.Digest` after deduplication | 0 |

The POI reader's own status field is the only exclusion class that fires:

| `POI.errorNotification` | Records |
| --- | ---: |
| `OK` | 220,760 |
| `BIFF5` | 17,643 |
| `OTHER` | 10,782 |
| `CORRUPT` | 129 |
| `ENCRYPTED` | 62 |

The 28,616 non-OK records equal FUSE's published count of files its analysis
tools could not read. The subtype consistency check found no mismatches: the
per-type formula counts sum to `totalFormulas` and the per-type input counts sum
to `totalInputCells` for every analyzable workbook, so the totals and the type
breakdowns describe the same cells.

By content type, the analyzable population is 210,061 `application/vnd.ms-excel`
(`.xls`), 10,551 `.xlsx` and 148 `.xltx`. The POI analysis only covers Excel
formats, so no ODS or CSV files appear, and the corpus says nothing about them.

The deduplicated file contained no repeated digests: FUSE's own deduplication
had already collapsed identical binaries.

## 6. Metrics FUSE metadata cannot provide

The following requested metrics are **unavailable**, not estimated:

| Requested metric | Status |
| --- | --- |
| Highest used row per worksheet | Unavailable |
| Populated columns per worksheet | Unavailable |
| Non-empty cells per worksheet | Unavailable |
| Highest used row or column per workbook | Unavailable |

Evidence: the POI analysis object has 498 keys, and none of them is a geometry
field; the only shape fields are the workbook aggregate `numSheets` and the
`numCharts`/`containsMacros` flags. The analysis records `countSUM`, `countIF`
and similar per-function formula counts, not worksheet dimensions. Scans of the
deduplicated analysis files for sheet/row/column field names found only
free-text occurrences (LingPipe vocabulary tokens), never fields. FUSE's own
paper reports "more than 450 metrics per file" on this same per-workbook basis.

Consequence for the capacity question: the row ceilings of the Stage A–D
profiles cannot be checked with FUSE metadata at all. Only the workbook
populated-cell ceilings are measurable here, and a workbook that satisfies the
cell ceiling can still exceed a per-worksheet row ceiling (for example a
20,000-row single-column sheet is under 100,000 cells but over 10,000 rows).
The coverage figures in §4.3 are therefore **upper bounds** on real coverage.
Producing row percentiles needs either the 6.9 GB FUSE binary archive, the
Enron workbook archive, or the Sheetpedia dataset, each of which requires
opening every workbook.

## 7. Caveats

- **The corpus has a 1 MiB file-size ceiling.** No record exceeds
  `Tika.Length == 1,048,576` bytes, and 155 records sit exactly at that value
  (the next sizes are also exactly 1,048,576), so either the crawl truncated at
  1 MiB or the analysis excluded larger files. Whatever the mechanism, FUSE
  holds no workbook above 1 MiB: large business workbooks are absent rather
  than measured, and every percentile here — especially the cell-count tail and
  the coverage percentages — understates real-world sizes. The 252,642-cell
  maximum is a consequence of that ceiling, not evidence that larger parsed
  workbooks are rare.
- FUSE is a 2014 Common Crawl sample of publicly reachable spreadsheets, not a
  business population. It over-represents published reports, downloadable
  forms and public-sector tables and contains no permissioned,
  game-development or active-workplace sample. It is evidence about the long
  tail of public spreadsheets, not about Tachiko's intended customers.
- The corpus is deduplicated by file content, so the population is unique
  files, not unique documents-in-use: two organisations shipping the same
  template count once, and a widely copied template does not dominate.
- FUSE's published figures for the same corpus are 249,376 unique files,
  28,616 unreadable by its analysis tools, and 205,978 analyzable files with
  zero formula cells. All three match this pass exactly (§4, §5), which is the
  main evidence that the population here is the paper's population. This pass
  defines "analyzable" from the record fields rather than from the original
  tool run, so a future re-derivation could still diverge at the margins.
- `Tika.Length` is source file size, not memory use, and not expanded size.
  The same content can have very different byte sizes depending on format and
  compression (a compressed `.xlsx` container, a binary `.xls`, an ODS zip),
  so file-byte percentiles describe downloaded bytes, not workbook content.
- The spreadsheet content dates from the crawl window (2014); the corpus says
  nothing about how spreadsheet sizes have changed since.
- Populated-cell semantics may differ from Tachiko's: POI counts cells with
  stored values, while a Tachiko document may additionally carry formatting,
  text fields, or table metadata that consume capacity.
- The measurement says nothing about import compatibility, recalculation cost
  or file fidelity; those need the workbook-binary corpora.

## 8. Reproduce

```sh
# 1. Download the dataset (public; no authentication).
curl -L -o fuse-bin.analysis.dedup.poi-dec2014.json.gz \
  https://blob.barik.net/fuse/fuse-bin.analysis.dedup.poi-dec2014.json.gz
shasum -a 1 fuse-bin.analysis.dedup.poi-dec2014.json.gz
# 9aea74288ce99129282d8dfed78aeed11dc911ee

# 2. Analyse (from the worktree root; the dataset is scratch input).
orb-test --memory 1024m --cpus 1 --image tachiko-work-rust:1.99.0 \
  --out target/corpus-scratch -- bash -c \
  'python3 tools/corpus/fuse_metadata.py \
     target/corpus-scratch/fuse-bin.analysis.dedup.poi-dec2014.json.gz \
     --sha1 9aea74288ce99129282d8dfed78aeed11dc911ee \
     --json-out target/corpus-scratch/fuse-result.json'

# 3. Delete the dataset and the JSON result afterwards (one-off data, Owner rule
#    2026-10-26); only the script and this report are kept.
```

The pass needs about one CPU-minute and under 1 GiB; on a normally loaded host
the same command should run under `--heavy`, which is the default route for
data processing. This run used a small non-heavy container because the
host-wide heavy lock was held by other projects' jobs for several hours on
2026-10-26 JST (see the pull request's Assumptions).

## 9. Follow-ups

- Worksheet-level row, column and cell percentiles need a corpus whose members
  can be opened per sheet: the FUSE binary archive (6.9 GB), the Enron
  spreadsheets archive, or Sheetpedia. That pass should also record import
  behaviour, not just geometry.
- The coverage table here can be combined with a row-percentile corpus to turn
  the Stage A–D profiles into measured coverage instead of upper bounds.
- A permissioned sample of modern business and game-development workbooks
  remains the only way to answer the population question the research note
  leaves open.
