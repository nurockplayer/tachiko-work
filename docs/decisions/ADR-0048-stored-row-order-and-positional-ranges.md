# ADR-0048: Stored row order and positional ranges

## Status

Accepted

Decision issue: [#668](https://github.com/nurockplayer/tachiko-work/issues/668)

Authority: [Work lead's design note](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6690046800)
and [sheet-lead's decisions on it](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6697638099),
2026-11-07 JST, including the positional range as an explicit Constitution §2.3
and §2.9 construct, under the window in
[ADR-0046](ADR-0046-pre-1-0-roproj-v3-revision-window.md).

Amends: ADR-0046's closed list, adding the deleted-reference marker;
[ADR-0047](ADR-0047-auto-columns-tagged-results-and-cell-failures.md) §3, adding one
failure code. Current v2 successors of
[ADR-0030](ADR-0030-canonical-semantic-delta.md) and
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md) gain the facts in §6.

Relies on, without changing: [ADR-0015](ADR-0015-stable-semantic-identity.md)
(`EntityId` stays record identity), [ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md)
§7 and §8 (static dependencies, full recomputation as the oracle) and
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md) (undo).

Specified by: [v3 DTOs](../specs/roproj-format-v3.md#row-rank-and-positional-references--adr-0048).

## Context

`=SUM(A2:A9)` in an empty sheet has to mean something. A1 translation today maps a
range only when it covers a whole data column, and there's no stored row order: an
entity record is `id`, `key`, `schema`, `fields`, and rows are listed in ID order.

Endpoint record IDs can't express this range. An empty sheet has no records for A2
or A9, and a sort can reverse the endpoints. Excel's meaning is positional, so the
range has to be too. Record identity stays with `EntityId`, and nothing durable is
addressed by position except the references this ADR defines. The translator still
prefers a whole-column aggregate or `[entity.field]` whenever one is exact.

## Decision

### 1. Row order

An entity gains an optional `rank`. A table's row order is `(rank, EntityId)` in
unsigned byte order, with unranked rows after ranked ones in `EntityId` order. So
two collaborators generating one rank still get one defined order, and a table with
no ranks keeps today's order and its exact bytes.

A rank matches `[0-9A-Za-z]{1,64}` and doesn't end in `0`, so a rank exists between
two unequal ranks. An insert rebalances the table's ranks in that command when the
new rank would exceed the length, or when its neighbours hold one rank, which a merge
can produce and no rank can split. Ranks carry no meaning beyond order, and nobody
reads them.

The first positional insert or sort in an unranked table ranks every row in that
command. Appending to an unranked table adds no rank when the new row already sorts
last. Row order is what `window`, `locateInView` and a view's final tie-break use.
Cost at 10,000 rows is measured in stage 8 of #678.

### 2. Positions

A position is 1-based among a table's rows in row order. The A1 row number maps to
a position through the table's header state, which the translator already holds.
A position is valid from 1 to 1,048,576, Excel's sheet limit, whether or not a row
is there. A position past the end reads as blank and creates nothing, so
`=SUM(A2:A10000)` creates no rows.

### 3. Two references

- **Range aggregate:** a function (SUM, AVERAGE, COUNT, MIN or MAX) over one field
  from `first_row` to `last_row`, with `first_row <= last_row`. It follows ADR-0047's
  range rules, and over a declared column the column's own rules.
- **Position read:** one field at one row, for `=A5` where row 5 doesn't exist. Where
  row 5 does exist, `=A5` binds to the record, as today.

The translator takes the first that's exact: a same-row reference, a whole-column
aggregate, `[entity.field]`, then these. A reference to a column that doesn't exist
yet adds a declared Auto column, up to `table_fields`, because that creates no
records.

Conditional aggregates over a partial range aren't in this ADR. Their import
boundary stays as it is.

### 4. What moves a reference

| Command | Effect on bounds |
|---|---|
| Insert a row at `p` | Each bound at or past `p` moves down by one, so a range containing `p` grows |
| Delete the row at `p` | `first_row` moves up by one when it's past `p`, and `last_row` when it's at or past `p`, so a range containing `p` shrinks |
| Delete every row of a range, or the row of a position read | The reference becomes the deleted-reference marker |
| Sort or move rows | Nothing: the range stays and reads what's now there |
| Append a row | Nothing |

All of it happens in the same command as the row change, with one Undo. A read of a
record, `[entity.field]`, on a deleted row also becomes the marker when its owner is
an Auto cell.

The marker is an Expression with empty arguments. It evaluates to a new failure code,
`reference`, shown as `#REF!`, and exports as `#REF!`. So deleting a row can leave a
visible failure in an Auto cell, as in Excel, and it refuses when the owner is a
declared column, under ADR-0047 §4. Deleting a column keeps today's dependency-safe
refusal from ADR-0040; a later ADR can change that.

### 5. Dependencies and calculation

A range aggregate or position read depends on the cells it covers at the document's
row order. Its positions come from the bound formula and its rows from the revision,
so the edges are still derived without evaluating anything, as ADR-0018 §7 requires.
Full recomputation stays the oracle.

Cycles use these resolved cells. So `=SUM(A2:A5)` in A7 isn't a cycle, though both
are in column A. A command that would make a range cover its own owner, such as a
sort moving A7 into rows 2 to 5, is a cycle and refuses like any other, naming the
formula. Marking the whole field dirty, the column edge from stage 1 of #678 plus one
order edge, is a sound way to find what to recalculate.

### 6. Delta and merge

- **Delta:** `rank` is a direct entity fact, because it changes what positional
  references read. A rank change appears in the delta. Calculated results never do.
- **Same entity, unequal ranks:** an ordinary `concurrent_change` on the `rank` facet.
- **Different entities:** ranks merge freely, and `(rank, EntityId)` defines the
  result.
- **Bounds:** bounds are part of the formula, so two sides that each rewrote them
  meet as an ordinary conflict on that value.
- **Order against a new range:** one side changes a table's row order (a rank, an
  inserted row or a deleted row), and the other side creates or changes a positional
  reference into that table. That merge would shift what the reference reads with
  nobody told, which Constitution §2.3 rules out. It raises the new conflict kind
  `positional_order_change`, on the formula's value. The merge never renumbers a
  reference for the user.

The new kind is a specific structural relation, so the rule against inventing a
catch-all conflict for a failed candidate still holds. A candidate that merges
cleanly and then fails on a declared field stays a validation failure.

### 7. Format

`rank` follows `schema` and precedes `fields`, omitted when absent. Three Expression
forms join v3, in the existing `op` and `args` form, as the
[spec](../specs/roproj-format-v3.md#row-rank-and-positional-references--adr-0048)
sets out. Older-representation writers refuse any of them, and refuse a ranked
entity, under ADR-0046.

### 8. Fixtures this requires

Canonical-byte fixtures and strict-reader refusals for `rank` and each Expression
form. Behaviour fixtures, each including Undo: a range over an empty table; a sort
under a range; deleting each endpoint; deleting a whole range; inserting inside,
before and after a range; a rebalance, including an insert between equal ranks after
a merge; a range in the column of its owner; a sort that would make that a cycle. Merge fixtures: equal ranks on two entities; unequal
ranks on one; each side of `positional_order_change`. A round trip through XLSX for a
partial range, and for `#REF!`. Conformance runs on native and WASM.

## Consequences

- **A view sort and a stored sort are different things.** A view sort is request
  state and changes no rank, so `=SUM(A2:A9)` keeps reading stored row order. Excel's
  Sort is stored: it rewrites ranks as one command with one Undo. Sheet decides which
  one its sort controls run in each kind of table, and has to label row numbers
  honestly under a view sort.
- **`=A5` follows its record through a sort,** because it's bound to the record while
  row 5 exists. Excel's stays at row 5. Same-row references behave as in Excel.
  The export is correct either way, since it writes the record's current address.
  This is a disclosed departure.
- **A range that's exact when typed becomes a whole-column aggregate,** and so it
  takes in rows appended later, where Excel's wouldn't. This is the translator's
  existing preference, and a disclosed departure.
- A stored sort of 10,000 rows writes 10,000 facts into the delta. That's accurate,
  and its size is measured with stage 8.

## Alternatives rejected

- **Endpoint record IDs.** They can't address rows that don't exist yet, and a sort
  can reverse them.
- **A row index stored on each entity.** Every insert would rewrite every later row,
  and two collaborators would always conflict.
- **Refusing a delete that empties a range.** Deleting a row is ordinary, and a refusal
  naming a formula elsewhere doesn't feel like a spreadsheet. The marker keeps the
  formula and shows what happened.
- **Merging renumbered bounds automatically.** It would guess which rows the other
  side meant, and a wrong guess is a silent change of meaning.
