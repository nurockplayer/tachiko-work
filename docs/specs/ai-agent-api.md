# AI Agent API Specification

## Principle

AI should interact with Tachiko Work through semantic operations.

It should not simulate mouse and keyboard usage.

## Operations

Implemented v0.1 operations are read, explain, and suggest-only:

```
describe_document(document)
explain_formula(document, field_ref)
explain_impact(before, after)
suggest_field_change(document, field_ref, value)
```

No API writes the document directly.

Suggestions require an approval path before any write. The API is explicitly
inert by design: it returns an explicit proposal object plus validation and
calculation outcomes.

## Safety

AI operations should be:

- typed
- validated
- reviewable
- approval-gated
- deterministic
- non-persistent

## Goal

The document model becomes a safe API for intelligent analysis and proposal.

AI is a native participant in the workspace rather than an external assistant attached to a file editor.

Formula suggestions:

- accept only typed `Value::Formula` proposals that target numeric fields,
- apply the shared complexity limit (`256` nodes, `64` post-desugar depth,
  `4096` canonical bytes),
- require explicit approval before execution via the CLI/workflow layer,
- never replace formulas with scalars automatically (formula-to-scalar suggestions
  are rejected).
