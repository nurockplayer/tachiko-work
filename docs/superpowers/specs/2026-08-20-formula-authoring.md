# Computational Formula Authoring Design

## Objective

Make formulas a first-class authoring workflow while preserving Tachiko Work's
semantic model, deterministic calculation, Git review, and AI approval boundary.

## Language contract

The formula engine exposes:

```rust
parse_expression(input: &str) -> Result<Expression, FormulaParseError>
format_expression(expression: &Expression) -> String
```

The grammar is:

```text
expression     = additive
additive       = multiplicative (("+" | "-") multiplicative)*
multiplicative = unary (("*" | "/") unary)*
unary          = ("+" | "-") unary | primary
primary        = number | reference | function | "(" expression ")"
reference      = "[" identifier "." identifier "]"
function       = ("min" | "max") "(" expression "," expression ")"
```

ASCII whitespace is optional around tokens. Numeric literals use finite `f64`
decimal/scientific syntax. Unary minus on a literal produces a negative number;
on another expression it maps to `0 - expression`. Function names are
case-sensitive. References contain exactly one dot and no interior whitespace;
both components use `is_valid_identifier`.

The parser rejects empty input, unknown functions/tokens, malformed numbers or
references, missing delimiters/operands, non-finite literals, trailing content,
input over 4,096 bytes, more than 256 AST nodes, or nesting beyond 64. Errors
include the byte position where parsing became impossible and a stable message.

The canonical formatter uses the existing stable number rendering, bracketed
references, parentheses around every binary operation, and `min`/`max` calls.
Every successfully parsed expression must format and parse to the same AST.

## Workflow contract

`set_formula(document, field, input)` returns `EditPreview` and never mutates
its input. It returns explicit errors for a missing entity/field/schema,
non-numeric target, invalid expression, unchanged formula, invalid candidate,
calculation failure, or diff failure. A stored number may become a formula; an
existing formula may change. Text, boolean, and reference fields are refused.

The shared edit finalizer remains validation → calculation → semantic diff.
Missing/non-numeric references, cycles, division by zero, and non-finite results
therefore fail before persistence through existing typed diagnostics/errors.

## CLI contract

`tachiko formula set INPUT FIELD EXPRESSION --output OUTPUT` is a nested Clap
surface parallel to `tachiko entity`. The command:

- parses `FIELD` as one `entity.field` path;
- rejects the same input/output path;
- loads a canonical validated source;
- invokes `set_formula`;
- serializes canonical output and uses exclusive creation;
- prints direct formula change and derived impact; and
- recommends `tachiko explain OUTPUT FIELD`.

Parsing, validation, calculation, no-op, and existing-output failures create no
file and preserve every existing byte.

## AI integration

`suggest_field_change` accepts `Value::Formula` only for schema-numeric fields.
It may replace a stored number or another formula, validates and calculates the
candidate, and remains inert with `requires_approval = true`. An existing
formula still rejects a proposed scalar value so AI cannot silently erase
computational intent.

## Verification

- Parser tests lock precedence, associativity, unary behavior, all AST shapes,
  identifier forms, scientific numbers, whitespace, round trips, diagnostics,
  resource limits, and deterministic repeated results.
- Workflow tests cover numeric-to-formula, formula-to-formula, no-op, wrong
  target type, parse error, missing/non-numeric reference, cycle, division by
  zero, and source immutability.
- AI tests cover inert valid formula proposals and refusal of formula-to-scalar.
- CLI tests cover help, success/impact/explain guidance, parse and semantic
  errors, same-path and existing-output protection, and no-write failures.
- A formula-authoring smoke edits the Moonfall DPS formula, verifies a result of
  45, proves repeat canonical bytes, and exercises parse/reference/cycle
  rejection before validate, diff, explain, and export.
- Completion requires the full release-equivalent gate and independent review.

## Non-goals

- New evaluation operations beyond the existing AST.
- Formula clearing, schema authoring, or field creation.
- Spreadsheet syntax or Office compatibility.
- Effects, network/filesystem access, nondeterminism, or a scripting runtime.
