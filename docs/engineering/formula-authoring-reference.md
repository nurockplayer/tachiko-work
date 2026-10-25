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
| Binary `+`, `-`, `*`, `/` with standard precedence | `[rent.expense] * [settings.rate]` |
| Unary `+` and `-` | `-[enemy.defense] + 2` |
| Parentheses | `([rent.expense] + 100) * 2` |
| Two-argument minimum and maximum | `min(0, [attacker.power] - [target.armor])` |

Aggregates, conditional expressions, comparisons, lookups, and user-defined
functions are not part of this language; the
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

## Limits

The source text limit is 4,096 UTF-8 bytes; an expression has at most 256
nodes and a post-desugaring depth of 64, and its canonical projection is also
limited to 4,096 bytes. Exceeding a structural limit refuses the update.
