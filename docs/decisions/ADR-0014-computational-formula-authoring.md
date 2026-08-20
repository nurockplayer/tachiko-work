# ADR-0014: Bounded computational formula authoring

## Status

Accepted

## Context

Tachiko Work evaluates, explains, diffs, merges, and exports formulas, but a
user cannot create or revise one without editing the tagged expression AST in
canonical JSON. The product therefore exposes computation as a result while
keeping computational authoring representation-dependent.

The current expression model is deliberately small: finite numbers, numeric
field references, arithmetic, minimum, and maximum. A human authoring language
should project exactly that model, remain deterministic, and preserve the same
validation and approval boundaries as other semantic edits.

## Alternatives considered

### Expose the JSON expression tree in a CLI flag

This would be machine-readable but would not be a usable authoring experience.
It would also duplicate the representation coupling that the workflow layer is
meant to hide.

### Adopt a general-purpose scripting language

A scripting runtime would add effects, sandboxing, versioning, and portability
questions far beyond the current numeric expression model. It would weaken the
deterministic computational-document contract.

### Add a small expression language that maps one-to-one to the AST

This provides familiar authoring without expanding execution semantics. This
is the selected approach.

## Decision

The formula engine owns a bounded, deterministic expression parser and
canonical formatter. The language supports:

- finite decimal and scientific numeric literals;
- semantic field references written as `[entity.field]`;
- `+`, `-`, `*`, and `/` with standard precedence and left associativity;
- unary `+` and `-`;
- parentheses; and
- two-argument `min(left, right)` and `max(left, right)`.

Bracketed paths remove ambiguity between decimal literals, subtraction, and
semantic identifiers that may contain `-`. Both path components must satisfy
the semantic-core identifier grammar. The formatter emits fully parenthesized
binary expressions and bracketed references, so output from `tachiko explain`
can be pasted back into the authoring command.

Parsing is side-effect free and limited to 4,096 input bytes, 256 expression
nodes, and 64 nested constructs. Failures report a stable byte position and an
actionable reason. The parser does not perform document lookup; semantic-core
validation remains the authority for reference existence and numeric type.

The workflow adds a validated formula edit that:

1. requires an existing schema-declared numeric field;
2. parses the expression;
3. refuses an unchanged formula;
4. validates and calculates the full candidate; and
5. produces a semantic diff before returning an immutable preview.

The CLI exposes:

```text
tachiko formula set INPUT.ro ENTITY.FIELD EXPRESSION --output OUTPUT.ro
```

It exclusively creates a distinct output, prints semantic impact, and suggests
`explain` as the next step. Replacing a formula with a stored scalar remains an
explicitly separate operation and is not smuggled through scalar `set`.

The AI API may propose a typed `Value::Formula` for a numeric field through its
existing inert suggestion model. Formula-to-formula and numeric-to-formula
suggestions require approval; formula-to-scalar suggestions remain refused.
The AI API still performs no write.

## Consequences

Positive:

- Users can author computation without understanding the wire AST.
- Parsed formulas retain typed references, deterministic evaluation, semantic
  diff, Git review, and AI readability.
- Canonical display and accepted input use the same syntax.
- Resource limits make malformed or generated input fail predictably.

Negative:

- The language intentionally lacks conditionals, comparisons, strings,
  collections, named variables, and user-defined functions.
- Canonical formatting is more explicit than the shortest human input.
- Formula removal and conversion back to a scalar remain separate future UX.

## Deferred work

- Conditional and lookup expressions justified by game-balance use cases.
- Formula clearing or conversion to a stored input.
- Schema-level computed-field declarations and defaults.
- Interactive completion, syntax highlighting, or a graphical formula editor.
- Importing formulas from spreadsheet languages.
