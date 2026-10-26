# Formula authoring reference

Status: task-scoped engineering reference for callers of the experimental
Designer runtime (`updateFormula`, `copyFormula`). The Accepted authoring
language is owned by
[ADR-0014](../decisions/ADR-0014-computational-formula-authoring.md) and the
[formula engine specification](../specs/formula-engine-spec.md); if this page
and those authorities disagree, the authorities win.

A formula is submitted as **source text** against an expected revision. The
runtime parses it, binds every reference to stable IDs in one document
snapshot, checks numeric references and condition types, rejects cycles, and
evaluates it. A successful update publishes the new formula; a refused update
publishes nothing and leaves the previous formula source, calculated value, and
revision unchanged.

## Accepted syntax

| Form | Example |
| --- | --- |
| Number literal (finite decimal or scientific) | `0.1`, `80000`, `1.5e3` |
| Field reference | `[rent.expense]` |
| Same-row field reference (the formula owner's own row) | `[.expense]` |
| Binary `+`, `-`, `*`, `/` with standard precedence | `[rent.expense] * [settings.rate]` |
| Unary `+` and `-` | `-[enemy.defense] + 2` |
| Parentheses | `([rent.expense] + 100) * 2` |
| Two-argument minimum and maximum | `min(0, [attacker.power] - [target.armor])` |
| Number comparisons (below arithmetic in precedence) | `=`, `<>`, `<`, `<=`, `>`, `>=` |
| Conditional Number | `IF([.expense] > 0, [.expense], 0)` |
| Logical conditions | `IF(AND([.expense] > 0, NOT([.expense] >= 100)), 1, 0)` |
| Decimal rounding, half away from zero | `ROUND(2.675, 2)` yields `2.68` |
| Percent literal | `50%` is `0.5` |
| Blank predicate | `IF(ISBLANK([.expense]), 0, [.expense])` |

IF, AND, OR, NOT, ROUND and ISBLANK accept upper- or lowercase names; canonical
source uses uppercase. Existing binary min/max retain their lowercase form.
A formula source has no leading `=`.

Conditions are not values: they may appear only as IF's condition or inside
AND/OR/NOT. Numeric truthiness and comparisons between non-Numbers are refused
as `formula_type_mismatch`. ROUND's second argument must be an integer literal
in `0..=10`; negative, fractional, calculated, and reference precisions are
refused with the same typed code. AND/OR require at least one condition; NOT
and ISBLANK require one argument and IF requires three.

Only the selected IF branch is evaluated, but every reference in both branches
remains a dependency and participates in cycle checks. AND/OR evaluate all their
arguments left-to-right. An absent optional Number reads as zero in numeric
expressions; its stored cell stays blank. ISBLANK accepts any declared field
reference, including Text; empty Text and zero are present. Unknown fields and
non-Number numeric operands remain failures. See the
[blank semantics note](../specs/formula-engine-r2-blank-semantics.md).

Aggregates, lookups, and user-defined functions remain deferred.

## References use keys, not labels

`[entity.field]` is an **entity key plus a field key** in the document — the
stable canonical lowercase spelling, never a display name and never a row
label. A table whose column is labelled `Expense` stores the key `expense`, so
the reference is `[rent.expense]`. The row item text (`Rent`) is ordinary data,
not an address.

Binding uses the current document snapshot. Renaming a key invalidates an old
source spelling until it is rewritten; renaming only a display name does not
affect meaning. A reference to a missing entity or field, or to a non-Number
field used as a Number, refuses the whole update.

## Same-row shorthand `[.field]`

`[.expense]` means "this row's `expense`" — a field on the entity that **owns
the formula**. It exists so one formula can describe a per-row
computation without naming the row:

```text
rent.reserve  [.expense] * [settings.rate]   80000 * 0.1 = 8000
```

The shorthand names a field **key**, never a row label or display name. Numeric
uses require a Number field on the owner's schema; ISBLANK may inspect any field. A missing key refuses the update
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

## Canonical source projection

Accepted updates return the stored formula through `field.formula.source`
(the shared canonical printer's source string), which is a **fully parenthesised**
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
