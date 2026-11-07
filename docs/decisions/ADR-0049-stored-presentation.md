# ADR-0049: Stored presentation: cell styles, display, rules, footer and title

## Status

Accepted

Decision issue: [#668](https://github.com/nurockplayer/tachiko-work/issues/668)

Authority: [Work lead's design note](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6690046800)
and [sheet-lead's decisions on it](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6697638099),
2026-11-07 JST, under the window in
[ADR-0046](ADR-0046-pre-1-0-roproj-v3-revision-window.md).

Amends: [ADR-0042](ADR-0042-first-public-portable-package-over-roproj-v3.md) only by
admitting one more payload entry, under ADR-0046.

Relies on, without changing: [ADR-0015](ADR-0015-stable-semantic-identity.md) (stable
identity), [ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md)
(undo), [ADR-0047](ADR-0047-auto-columns-tagged-results-and-cell-failures.md) (values
and failures), [ADR-0048](ADR-0048-stored-row-order-and-positional-ranges.md) (row
order) and the #686 display-text contract.

Specified by: [v3 layout](../specs/roproj-layout-v3.md#presentation-member--adr-0049),
[v3 DTOs](../specs/roproj-format-v3.md#optional-date-display-format--adr-0049) and
[portable package v1](../specs/portable-package-v1.md#presentation-entry--adr-0049).

## Context

Someone used to Excel expects a bold header, a filled cell and a red overdue date to
stay with their row through a sort. Today those styles exist only in interop
metadata, as a per-row copy the host carries, and a native table can't hold them.
Benchmark P0-12 asks for rules on top (overdue, negative, low stock) as a v1.0 item,
sequenced after style.

These are presentation. Changing a fill doesn't change what the document means, so it
shouldn't appear as a semantic change or be able to block a merge. It still has to
be saved, undone and exported.

## Decision

### 1. One optional member, separate from `Value`

The tree gains `presentation.jsonl`, present only when it has content. Its absence is
canonical, so a document without presentation keeps the exact nineteen-file tree and
its hashes. One file is enough. Records sort by identity and one record is one line,
so a single style edit is a one-line diff, and sharding would add paths without
helping. 100,000 cell records come to about 12 MB (arithmetic, at 120 bytes a record),
which a column style (§2) avoids for a uniformly styled column.

The [layout spec](../specs/roproj-layout-v3.md#presentation-member--adr-0049) defines
each record's exact JSON. Records appear in this order: `style`, `table`, `rule`,
`cell`. A reader refuses any other order, a dangling reference, an unused style or a
style whose ID doesn't match its content, and repairs nothing.

### 2. Cell styles

A `style` record is one palette entry: `bold`, `italic`, `fill`, `text_color`,
`alignment`, `wrap`, `borders`, `number_format` and `date_format`, each optional.
Absent means no opinion. Colours are `#RRGGBB`. Alignment is `left`, `center` or
`right`. `borders` names any of the four sides, each a thin line in the default
colour. Excel's other line styles map to this and get a ledger finding.

A style's ID is the first 32 hexadecimal digits of the SHA-256 of its canonical JSON.
So the same style has the same ID on every branch, two people's new styles can't
collide, and a merge takes the union and drops what nothing uses.

A column can hold a style, in its `table` record, which applies to every cell in the
column including rows added later. A `cell` record is `(entity, field) -> style`,
anchored to identity, so it survives sort, filter and insert with no extra work. It
layers over the column's style attribute by attribute. Removing a row or column removes
its records in the same command, and there are no tombstones.

### 3. Display

The column holds the default: today's `number_format`, and a new `date_format`
(§4). A cell's style can override it, which is how `$1,234.50` keeps its currency
next to a plain `1234.50` in an Auto column (ADR-0047 §2). Work's `display_text`
reads the override, so the grid, filters, Find and distinct values can't disagree.
The original spelling isn't kept.

### 4. Column Date format

FieldDefinition gains optional `date_format`, after `number_format`, on Date and Auto
fields. It's a closed set: `iso` (`2026-11-07`), `mdy` (`11/7/2026`), `dmy`
(`7/11/2026`), `month_day_year` (`Nov 7, 2026`) and `day_month_year` (`7 Nov 2026`).
Absent means `iso`, today's display. English month names, as with en-US numbers. Like
`number_format`, it's a direct field fact in the delta, because it lives on the
definition and is edited with a revision-pinned command. It's a fact about display
and it changes no value.

### 5. Rules for conditional formatting

P0-12 needs a stored construct, and this is it, so it lands before #579 even if Sheet's
rule UI is cut.

A `rule` record has a stable ID, a table, the fields it paints, a condition, a style
and a rank that places it in the table's rule order, in the grammar and tie-break of
ADR-0048 §1, so two sides' orderings converge. The condition is a bound Expression,
the same language as a formula, so overdue is `[due] < TODAY()`, negative is
`[amount] < 0` and low stock is `[stock] < [reorder_at]`, with no second language.

The row being painted takes the place of a formula's owner. So `[due]` is the existing
`same_row_reference`, reading that row's field, and one rule covers every row. A
condition reads only same-row references, whole-column aggregates, literals, functions
and `TODAY()`. It can't name a record or a position, so deleting or sorting rows never
leaves a rule pointing at nothing.

- **It isn't a formula.** A rule has no owner cell and adds no edge to the dependency
  graph, so it can't form a cycle or slow an edit. It's evaluated when a window is
  read, for the rows in that window.
- **It never refuses.** A condition that fails counts as false. It can read `TODAY()`
  with no restriction from ADR-0047 §4, because nothing depends on the result.
- **Layering.** The cell's static style comes first, then each matching rule in order.
  The earliest rule wins for each attribute, as in Excel.
- **Bounds.** 64 rules a table, under the existing expression limits.
- **Interop.** XLSX `expression` and `cellIs` rules map both ways. Any other kind
  (colour scales, data bars, top-N) gets a ledger finding.
- **Removal.** Removing a column removes the rules that paint or read it, and the
  command's result names them.

### 6. Footer and title

A `table` record holds both.

- **Footer:** one aggregate per field, from SUM, COUNT, AVERAGE, MIN and MAX. The
  record stores the choice, never a result. It's computed over the rows the current
  view shows, like Excel's Total Row with a filter on, through `queryViewTotals`. It
  sits outside the records: it's no row, formulas can't read it and sorting can't move
  it. XLSX maps it to `SUBTOTAL`.
- **Title:** optional text, under the display-name spelling rules in ADR-0043. It maps
  to a row above the header and spans the table's width. Import keeps a recognized
  title, which it skips today.

Other merged cells stay a disclosed import boundary with a ledger finding.

### 7. Not semantic, still saved

- **Semantic delta and conflict:** these records aren't facts in either, apart from
  `date_format`, and a presentation edit is never a semantic change. A semantic
  document comparison ignores them, and the package root covers their bytes.
- **Merge:** three-way, per identity. A cell record's identity is its `(entity, field)`;
  a rule's is its ID; and a `table` record's title, each footer choice and each column
  style are separate identities, so one person's title doesn't overwrite another's
  footer. Concurrent unequal edits to one identity take the byte-larger value, so both
  sides converge, and the merge report lists it. A colour must not block a merge. A
  style carrying a number or date override follows the same rule, and because the value
  is unchanged, what's listed is enough to correct it.
- **After a merge:** the candidate's presentation is pruned before it's written. A record
  whose row, column or table is gone, a rule that paints or reads a missing field, and a
  style nothing uses are all dropped, and the merge report lists them. So a rule added
  on one side and a column deleted on the other merge cleanly and never reach the strict
  reader as a dangling reference.
- **History:** a presentation command is an ordinary revision with one Undo.
- **Capacity:** cell records count up to the profile's `populated_cells`, and the
  palette holds up to 4,096 styles. Editing past a bound refuses with a typed capacity
  failure. An import past a bound keeps its values and drops the style with a ledger
  finding, as #678 decided.
- **A merge past a bound:** two valid sides can add up to more than a bound, and that
  must not block the merge. The prune above also drops, in canonical order, rules past
  64 by rank and styles past 4,096 by ID with the records that use them. The merge
  report lists what went, and the values stay.
- **Older representations:** direct-ro/v2 and roproj/v1 and v2 writers refuse, under
  ADR-0046. CSV carries no styles and keeps its existing disclosure.
- **Interop metadata:** once import and export use this store, metadata stops carrying
  cell styles. Sheet names, header state and widths remain until a later ADR moves them.

### 8. Fixtures this requires

Canonical-byte fixtures for the member, including its absence, and for `date_format`.
Strict-reader refusals for record order, a dangling reference, an unused style and a
style ID that doesn't match its content. Package fixtures with and without the entry,
including the twenty-leaf payload root. Behaviour fixtures,
each including Undo: a styled cell through sort, insert and delete; a rule on a
failing condition; rule layering; `TODAY()` across two host inputs; a column removal
with rules attached; a rule painting many rows from one `same_row_reference`; a merge
of unequal styles from each side; two sides creating different new styles; a new rule
against a concurrent column deletion. An XLSX round trip for
every mapping and finding.

## Consequences

- Styles, rules, footer and title are stored vocabulary from the start, so under
  decision 4 on #668 Sheet can cut any of their UI without a format change.
- A window reply gains resolved styles, and they count against its byte budget in
  stage 3 of #678. Rules cost read time for the rows in a window, never edit time.
- `display_text` becomes per-cell where a style overrides the column, and the #686
  corpus extends to that.
- A byte-larger pick is arbitrary between two people's colours. That's acceptable for
  presentation, and the merge report says so.

## Alternatives rejected

- **Style inside `Value`.** A fill would become a semantic change, and every value
  comparison would have to ignore it.
- **Runtime-only styles, kept in interop metadata.** A native table couldn't have them,
  and the host would carry them forever.
- **A rule language of its own.** Excel users already write these as formulas, and a
  second language means a second parser, evaluator and export path.
- **Rules as formula-graph nodes.** Every rule would slow every edit in its column,
  and a failing rule could refuse an edit over a colour.
- **A presentation conflict.** Asking someone to resolve two fill colours before they
  can merge costs more than a wrong colour does.
