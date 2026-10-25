# Formula authoring reference

Status: task-scoped engineering reference for callers of the experimental
Designer runtime (`updateFormula`, `copyFormula`). The Accepted authoring
language is owned by
[ADR-0014](../decisions/ADR-0014-computational-formula-authoring.md) and the
[formula engine specification](../specs/formula-engine-spec.md); if this page
and those authorities disagree, the authorities win.

A formula is submitted as **source text** against an expected revision. The
runtime parses it, binds every reference to stable IDs in one document
snapshot, checks that each reference is a Number field, rejects cycles, and
evaluates it. A successful update publishes the new formula; a refused update
publishes nothing and leaves the previous formula source, calculated value, and
revision unchanged.

## Accepted syntax

| Form | Example |
| --- | --- |
| Number literal (finite decimal or scientific) | `0.1`, `80000`, `1.5e3` |
| Field reference | `[rent.expense]` |
| Same-row field reference (the formula owner's own row) | `[.expense]` |
| Column total (sum of one Number column over a table) | `[plan.reserve:total]` |
| Binary `+`, `-`, `*`, `/` with standard precedence | `[rent.expense] * [settings.rate]` |
| Unary `+` and `-` | `-[enemy.defense] + 2` |
| Parentheses | `([rent.expense] + 100) * 2` |
| Two-argument minimum and maximum | `min(0, [attacker.power] - [target.armor])` |

Conditional expressions, comparisons, lookups, user-defined functions, and
aggregates beyond the saved column total are not part of this language; the
[specification](../specs/formula-engine-spec.md#deferred-language-features)
lists them as deferred. Reference targets must be Number fields.

## References use keys, not labels

`[entity.field]` is an **entity key plus a field key** in the document — the
stable canonical lowercase spelling, never a display name and never a row
label. A table whose column is labelled `Expense` stores the key `expense`, so
the reference is `[rent.expense]`. The row item text (`Rent`) is ordinary data,
not an address.

Binding uses the current document snapshot. Renaming a key invalidates an old
source spelling until it is rewritten; renaming only a display name does not
affect meaning. A reference to a missing entity or field, or to a non-Number
field, refuses the whole update.

## Same-row shorthand `[.field]`

`[.expense]` means "this row's `expense`" — a field on the entity that **owns
the formula**. It exists so one formula can describe a per-row
computation without naming the row:

```text
rent.reserve  [.expense] * [settings.rate]   80000 * 0.1 = 8000
```

The shorthand names a field **key**, never a row label or display name, and it
must be a Number field on the owner's schema. A missing key refuses the update
as `formula_unknown_reference`; a Text or other non-Number key refuses it as
`formula_type_mismatch`. Referring to the formula's own field — `[.reserve]`
inside `reserve` — is a dependency cycle and refuses as `formula_cycle`.

The bound expression stores only the field identity; the entity is resolved
from the formula that contains it. The canonical source projection keeps the
written form (`([.expense] * [settings.rate])`) and follows a field-key rename.
Because the entity is implicit:

- `copyFormula` with `relative_rows` accepts the shorthand unchanged — there is
  no per-row address to shift, and each destination interprets it against its
  own row;
- `duplicateCollection` keeps it unchanged as well, so a copied table reads the
  copy's rows rather than the source table's.

The shorthand is stored in `.roproj/v3` and direct-ro/v2 as a distinct
same-entity reference. The frozen `.roproj/v1` and `.roproj/v2` formats refuse
to encode it rather than silently dropping the row meaning.

## Column totals `[schema.field:total]`

`[plan.reserve:total]` is the sum of the `reserve` column over **every row of
the `plan` table**. It names the column, not the rows, so it keeps meaning the
same column when rows, row keys, the table key, or the field key change:

```text
summary.total  [plan.reserve:total] + [plan_copy.reserve:total]
```

Binding resolves the table key and the field key in the current snapshot and
requires a Number column. An unknown table or field key refuses the update as
`formula_unknown_reference`; a Text or other non-Number column refuses it as
`formula_type_mismatch`. Blank cells are skipped, and an empty or all-blank
table totals `0`. A row whose value is non-Number or failed makes the total
unavailable instead of summing the rest.

The total depends on every row of that column, so a formula that reads the
column of its own row — `[plan.reserve:total]` inside a `plan.reserve` cell —
is a `formula_cycle`.

- `copyFormula` leaves a column total unchanged: it is not a per-row address.
- `duplicateCollection` moves a total of the copied table's own column onto the
  copy (`[plan_copy.reserve:total]`), and keeps a total of any other table on
  the original.
- Removing a row of the table recomputes the total over the remaining rows;
  removing the table or the column field is refused while a formula names it.

The form is stored in `.roproj/v3` and direct-ro/v2 as a column identity; the
frozen `.roproj/v1` and `.roproj/v2` formats refuse to encode it.

Every table projection also reports these numbers in a `column_totals` map from
column ID to `number | null`, observed at the same revision as its rows, so a
footer row can show them without authoring a formula. Blank cells are skipped
there too, and a null entry means a contributing value is unavailable rather
than zero.

## Canonical source projection

Accepted updates return the stored formula through `field.formula.source`
(the string `projectExpression` renders), which is a **fully parenthesised**
projection of the bound expression through the document's current human keys:

```text
[rent.expense] * [settings.rate]   ->  ([rent.expense] * [settings.rate])
min(0, [a.b] + 1)                  ->  min(0, ([a.b] + 1))
```

This projection is a copyable authoring aid, not persisted numeric-token
authority and not a stability contract for the exact spacing. It can be absent
when a bound reference cannot prove a unique round-trip through the current
human addresses (for example, an entity deleted while a formula still targets
it); the calculation diagnostics describe that state instead.

## Copying a formula

`copyFormula` copies one field's formula to other destinations and rewrites
references according to two axes:

- `fixed_references` lists dependencies that stay bound to their original
  targets — typically a shared settings or summary row.
- `relative_rows` / `relative_columns` shift the remaining references by the
  destination's row/column offset within the same collection.

Each destination receives its own canonical source string. For a monthly plan
where `rent.reserve` is `[rent.expense] * [settings.rate]`, copying to
`ads.reserve` and `tools.reserve` with `settings.rate` fixed and
`relative_rows` enabled produces:

```text
ads.reserve    ([ads.expense] * [settings.rate])     30000 * 0.1 = 3000
tools.reserve  ([tools.expense] * [settings.rate])   10000 * 0.1 = 1000
```

`duplicateCollection` also remaps references: a reference is rewritten only
when both its entity and its field belong to the copied collection, so a copy
of the plan reads the copy's rows while still reading the original shared
`settings.rate` row.

A same-row shorthand skips both rewrites. If `rent.reserve` is
`[.expense] * [settings.rate]`, copying it (with `relative_rows`) and
duplicating the table both leave every destination's source as
`([.expense] * [settings.rate])`, because each formula resolves the field from
its own owner row.

## Limits

The source text limit is 4,096 UTF-8 bytes; an expression has at most 256
nodes and a post-desugaring depth of 64, and its canonical projection is also
limited to 4,096 bytes. Exceeding a structural limit refuses the update.
