# ADR-0047: Auto columns, tagged formula results and published cell failures

## Status

Accepted

Decision issue: [#668](https://github.com/nurockplayer/tachiko-work/issues/668)

Authority: [Work lead's design note](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6690046800)
and [sheet-lead's decisions on it](https://github.com/nurockplayer/tachiko-work/issues/668#issuecomment-6697638099),
2026-11-07 JST, under the window in
[ADR-0046](ADR-0046-pre-1-0-roproj-v3-revision-window.md).

Amends: [ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md) §2, §9
and §10, and the [R2 blank note](../specs/formula-engine-r2-blank-semantics.md),
only for formulas that read or live in an Auto column. A formula over declared
columns alone keeps exactly its current meaning.

Relies on, without changing: [ADR-0045](ADR-0045-optional-native-text-and-number-fields.md)
(blank is an absent field), [ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md)
and [ADR-0020](ADR-0020-first-class-headless-semantic-api.md) (atomic publication).

Specified by: [v3 DTOs](../specs/roproj-format-v3.md#auto-field-type-and-tagged-literals--adr-0047),
[formula engine](../specs/formula-engine-spec.md#type-surface-and-conversion).

## Context

Someone used to Excel types into cells without declaring a type, and expects
`=A2+B2` to work whatever is there, showing an error in the cell when it can't.
Tachiko's declared columns do the opposite on purpose: a formula has one result
type, nothing coerces, and an edit that would make any formula fail is refused.

Both are right for different cells. A declared column is a promise, and it should
keep holding. A blank workbook and an imported workbook shouldn't require anyone
to make that promise first. This ADR adds a column kind for cells without the
promise, and defines how formulas behave there. A failure stays a calculated
outcome and doesn't become a stored value.

## Decision

### 1. The `auto` field type

`FieldType` gains the tag `auto`. An Auto field is always optional and has
constraint `none`. A cell is absent (blank), or holds a Number, Text, Boolean,
Date or Formula `Value`, never a Reference. `Value` is already tagged per cell, so
cells need no new encoding. `number_format` is admitted and applies only to cells
showing a Number.

A column moves from Auto to a stricter kind only through the retype in #660, with
a preview. Nothing rewrites a declared column silently, and no declared column
changes to Auto implicitly. An XLSX import makes a formula column Auto, along with
any column whose values a formula reads in a way its declared kind would refuse.

Keyed grouped-sum definitions and field constraints don't accept Auto fields.

### 2. Input classification belongs to Work

One Rust function takes what the user typed and returns the `Value` and any
display it implies. Sheet calls it and doesn't reimplement it. The first row that
matches decides:

| Input | Result |
|---|---|
| Empty | Blank |
| Starts with `=` | Formula, through the existing binder |
| `TRUE` or `FALSE`, any ASCII case | Boolean |
| The numeric grammar in §5, no leading zero before another digit, at most 15 significant digits | Number |
| The same with comma groups of three, such as `1,000` | Number, with a thousands display |
| The same with a leading currency mark, such as `$1,234.50` or `NT$1,200` | Number, with that currency's display: `$` USD, `NT$` TWD, `¥` and `￥` JPY, `€` EUR, `£` GBP |
| The same with a trailing `%`, such as `50%` | That Number over 100, with a percent display |
| A valid `YYYY-MM-DD`, `YYYY/M/D` or `YYYY年M月D日` | Date, with the display it was typed in |
| `M/D/YYYY`, or `D/M/YYYY`, by the host's date order | Date, when the host gives an order and the date is valid |
| Anything else | Text, exactly as typed |

Before matching, full-width digits and the marks `．`, `，`, `％`, `＋` and `－` read as
their ASCII forms, because a Japanese or Chinese IME often produces them, so `１２３` is
the Number 123. Input that matches no row keeps its exact spelling as Text. `¥` and `￥`
mean JPY because the function reads no locale. The value is the same either way, and
the display shows `¥` as typed.

An implied display holds at most 10 decimals, R3's limit; the value stays exact.

So `00123`, a digit run over 15 significant digits and a value outside binary64
such as `1e309` stay Text, because a Number couldn't give the spelling or the digits
back, and `1/2/2026` stays Text without a
date order. The host's order comes from the OS region and is visible in settings.
The function reads no clock and no locale: it depends only on its arguments.

The classifier runs at authoring and its output is an ordinary stored value, so
widening this table later changes no existing document.

A display the table implies lands in the presentation ADR's per-cell display record,
which is what lets `$1,234.50` keep its currency next to a plain `1234.50` through a
save.

### 3. Three layers, kept apart

- **Definition:** the bound formula. It's stored. Parse, binding, resource and
  cycle errors still refuse the edit, because there's no meaningful formula to
  keep.
- **Calculated outcome:** per cell, either a tagged result (Number, Text, Date or
  Boolean) or a failure with a code. It's never stored, as ADR-0018 requires, and
  it's the same on every client for the same document and host inputs.
- **Valid value:** absent or a tagged `Value`. There's no `Value::Error`, and no
  way to type or store a failure as a literal.

A formula's result is never blank: `=A1` with A1 blank is the Number 0, as in Excel.

Failure codes are closed: `division_by_zero`, `value`, `number` and, from
[ADR-0048](ADR-0048-stored-row-order-and-positional-ranges.md), `reference`, shown
as `#DIV/0!`, `#VALUE!`, `#NUM!` and `#REF!`. A dependent formula carries the first failure
found evaluating left to right, with its code. `explainField` names the cell it
came from.

### 4. Where a failure publishes

- **Auto cell:** a failure publishes, reopens and exports as that failure, and
  refuses nothing.
- **Declared column:** today's guarantee stands. An edit that would make a declared
  field's formula fail, or give a result of another kind, is refused atomically.
  This includes a declared formula reading a failed Auto cell. The refusal names
  the typed cell and offers the Auto route.
- **Reopen and merge:** admission recalculates. Auto failures admit. A merge
  result is admitted by the same rule.
- **`TODAY()`:** a host input, given explicitly for each snapshot and shown in the
  projection. It never reads an ambient clock, so ADR-0018 §9's purity holds over
  document plus inputs. Two clients either side of midnight can differ, as in Excel.
  A declared field can't depend on it, directly or through an Auto cell, and the
  bind refuses. Otherwise a date change alone could stop a saved document reopening.

### 5. Evaluation over Auto

These rules apply when an operand is a value read from an Auto cell, or a tagged
result flowing from one. A declared Text operand stays strict and refuses at bind.

**Arithmetic.**

| Operand | Becomes |
|---|---|
| Number | Itself |
| Blank | 0 |
| Boolean | 1 or 0 |
| Text in the numeric grammar below | That Number; the stored value stays Text |
| Other Text | `value` failure |
| Date | See below |
| Failure | Propagates |

The numeric grammar is locale-free: optional ASCII spaces, an optional sign, digits
with an optional `.`, an optional exponent, optional ASCII spaces. At least one
digit is required. A result outside binary64 is a `number` failure.

**Dates.** A Date isn't a serial number, and only the XLSX boundary converts.
`Date - Date` is a Number of days. `Date + n`, `n + Date` and `Date - n` are Dates
when `n` is a whole number, and a result outside the Date range is a `number`
failure. A Date holds no time of day, so a fractional `n` is a `value` failure. Any
other arithmetic on a Date is a `value` failure too.

**Ranges and reference arguments.** SUM and AVERAGE skip Text, Boolean and blank.
A failure in the range propagates. COUNT counts Numbers and Dates and skips
everything else, including a failure, as in Excel. MIN and MAX skip
the same cells; over Dates they return a Date, and over Dates mixed with Numbers
they're a `value` failure. SUM or AVERAGE over a range with a Date is a `value`
failure. A reference argument to MIN or MAX follows the range rule, as in Excel.
Empty ranges give SUM 0, COUNT 0, MIN and MAX 0, and AVERAGE `division_by_zero`.

**Comparison.** `=` and `<>` across types are false and true, and don't coerce. A
blank equals 0, `""` and FALSE. Ordering across types is a `value` failure, and a
blank orders as the other operand's zero value. Case rules for Text arrive with
the first text function, and a Text comparison in an Auto cell can't land before
them. A comparison outside a condition gives a Boolean result.

**Conditions.** Boolean is itself. Number is true unless 0. Blank is false. A Date
is true. Text is a `value` failure. A failed condition propagates, and the branch
not taken isn't evaluated.

**Views.** Typed filters match by tag, and a failure matches none. A sort orders
Number, Date, Text, Boolean, failure, then blank. A failed cell's display text is
its code, so `display_in` and Find can reach it, which replaces the "counts as
blank" boundary in #686.

### 6. Excel at the boundary

A workbook exported from Auto cells must calculate the same in Excel. Results are
exported with their cached value, and a failure as a cached error. Where the rules
above knowingly differ, export adds a ledger finding when the workbook's values
actually reach the difference:

- SUM or AVERAGE over a Date, which Excel sums as serials;
- MIN or MAX over Dates mixed with Numbers;
- Date arithmetic other than the forms in §5, including a fractional `n`, which
  Excel keeps as a time of day;
- ordering across types, where Excel orders Number, Text, Boolean;
- Text outside the numeric grammar that Excel would still coerce, such as
  `1,000`, `50%` or a date-like string.

### 7. Format, delta and merge

- **Stored:** the `auto` tag, and Text, Boolean and Date literals in Expression.
  New function tags come with each function's own PR, under the spec's limits.
- **Delta and conflict:** the current v2 successors carry `auto` as one more field
  type. Value facts are already tagged. A failure is calculated and is never a
  delta fact. Frozen v1 writers refuse.
- **Older representations:** direct-ro/v2 and roproj/v1 and v2 writers refuse any
  document with an Auto field or a new literal, under ADR-0046.

### 8. Fixtures this requires

Canonical-byte fixtures for an Auto field and each literal, with a strict-reader
refusal for each malformed pairing. A classifier corpus covering every row of §2, including
full-width input such as `￥１，２００` and `２０２６年１１月７日`, with each implied display
surviving a save and reopen. A conformance corpus, run on native and WASM,
covering every row of §5, including `"00123"+1` = 124, `=A1` over a blank, and
each failure code propagating through a dependent. An atomicity test where a
declared formula reading a failed Auto cell refuses with nothing published. A
round trip through XLSX for every §6 finding.

## Consequences

- A blank workbook and an import behave like Excel. Declared columns stay the
  user's stricter choice, and every existing document keeps its meaning.
- `CalculationOutcome::Failed` can no longer mean "the whole document failed" once
  Auto cells exist. The engine carries an outcome per cell and the publication gate
  looks at declared fields. Stage 7 of #678 must be built with this in mind.
- A declared column reading Auto data can refuse edits made elsewhere. That's the
  promise working, and the refusal has to explain it, naming the typed cell.
- `IFERROR` becomes definable: it catches a failure value. It stays on hold with
  the other formula slices behind #678.

## Alternatives rejected

- **Every formula cell publishes failures.** `=100/B1` in a typed column would stop
  refusing B1 = 0, and "a declared column is a promise" would no longer hold.
- **Never coerce Text.** `"00123"+1` would fail here and calculate in Excel, so an
  exported workbook would change its answer.
- **`Value::Error` as a stored value.** A failure would become data that outlives
  its cause, and every reader would need to handle it. ADR-0018 keeps results out
  of storage for good reasons.
- **Sum Dates as serials, as Excel does.** It makes Date a Number with a display,
  which the Date model rejects. It's a disclosed departure.
