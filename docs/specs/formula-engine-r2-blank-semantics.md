# R2 decision note: optional Number blanks in formulas

State: task-scoped formula roadmap R2 amendment, for normal lead/PR review.
Owner direction: 2026-10-25 JST; implementation note: 2026-10-26 JST.

An absent optional Number field remains absent in the semantic document. Reading
that field as a numeric formula operand now yields semantic positive zero.
This supports ordinary spreadsheet work such as an expense total whose inputs
have not all been filled. No `Value::Blank`, placeholder Number, or persisted
calculation cache is introduced.

This is the narrow formula-language exception to ADR-0018's no-blank-coercion
rule and ADR-0045's formula/scalar-min/max missing-input rule. Clearing an optional
Number input may therefore publish when its surviving formula consumers remain
valid with zero; it still refuses atomically if the resulting formula fails
(for example, a zero denominator). Required missing values, undeclared targets,
and non-Number arithmetic retain their typed failures. Field constraints apply
to present stored values and computed results as before.

ISBLANK(reference) is a non-value condition based on stored presence. It is true
for an absent declared field, false for numeric zero, empty Text, or a formula,
and accepts any declared field type. It does not evaluate a referenced formula.
Its reference still participates in static dependencies and cycle detection.
Comparisons read optional Number blanks as zero using the same Number operand
rule. This does not change the separate typed Query filter semantics.

This amendment does not change keyed grouped-sum missing-input behavior, future
aggregate blank rules, query grouping/filtering, storage absence, or CSV's
existing blank-versus-empty-Text disclosure. Existing documents with blank
Number formula inputs can calculate differently when opened by an R2-capable
engine. No stored values or formulas are migrated automatically. New bound
operations require canonical `.roproj/v3` or direct `.ro/v2`; frozen project
v1/v2 writers refuse those operations rather than dropping them.

See the [formula specification](formula-engine-spec.md#type-surface-and-conversion)
and [authoring reference](../engineering/formula-authoring-reference.md).
