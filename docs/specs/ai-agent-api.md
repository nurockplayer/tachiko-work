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

The provider-free `tachiko-ai-api` crate is an adapter over
`tachiko-workspace-engine`, its only workspace-crate dependency. Document
description remains AI-facing projection, while formula analysis, semantic
comparison, typed proposal construction, validation, and calculation use the
shared workspace-engine application policy. The current Rust surface is not an
external stability or versioning commitment; #10 owns that decision.

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
- return an inert, validated workspace-engine candidate and mark the AI-facing
  proposal as requiring approval,
- require a separate approved host write before execution,
- never replace formulas with scalars automatically (formula-to-scalar suggestions
  are rejected).

The `requires_approval` flag preserves the implemented v0.1 safety behavior; it
does not define the capability, approval, provenance, or execution protocols
owned by #27/#28.
