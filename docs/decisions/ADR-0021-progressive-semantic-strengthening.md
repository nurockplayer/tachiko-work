# ADR-0021: Progressive semantic strengthening

## Status

Accepted

Decision issue: [#13](https://github.com/nurockplayer/tachiko-work/issues/13)

Related authority: ADR-0015, ADR-0018, ADR-0019, ADR-0020

## Context

The current Game Development wedge is intentionally strongly typed: schemas, entities, typed values, stable references, bound formulas, validation, semantic diff/merge, and runtime export provide concrete value.

That successful narrow workflow does not decide that every future Tachiko Work content kind must have a domain schema before it can exist. Product Constitution and Design Principles require meaning to outrank representation while also keeping the stable core small and avoiding semantic structure that has not earned its cost.

Issue #13 asks whether users may begin with legitimate freeform or weakly structured content and add stronger semantics later without weakening the current typed core, breaking established identity/references, or turning AI inference into semantic authority.

ADR-0019 already distinguishes malformed/incomplete raw authoring from structurally admissible semantic candidates. ADR-0015 already limits durable opaque identity to independently addressable mutable semantic objects rather than requiring a universal node graph. ADR-0020 already defines typed semantic Commands, Propose/Execute, stable targeting, authoritative gates, and atomic semantic publication.

The remaining decision is therefore a product-architecture rule for **how semantic guarantees may strengthen over time**, not a new universal weak data model.

## Decision

### 1. Semantic-first does not imply schema-first

Tachiko Work MAY contain legitimate semantic content whose declared meaning is weaker than the current schema/entity model.

A paragraph can semantically mean “this text content exists.” A simple table can semantically mean “these rows, columns, and cell contents exist” without yet claiming that a column is a Number, a row is a typed Entity, or a relationship is a durable Reference.

This is different from malformed or incomplete raw authoring. Input that has not satisfied the admission/representation prerequisites for its semantic kind remains outside semantic truth under ADR-0019.

### 2. Progressive typing is explicit semantic strengthening

The accepted long-term principle is:

> Write first; strengthen semantics when the structure earns its cost; preserve established identity and meaning across the transition.

Adding a schema, declared type, durable relationship, constraint, or computation is an explicit semantic transition. It MUST NOT be a silent reinterpretation of existing content merely because a parser, heuristic, or AI model inferred a plausible structure.

The persisted model does not gain a universal `typing_level`. Semantic strength is expressed by the actual semantic facts that have been explicitly declared and accepted.

### 3. Mixed-strength semantic content is legal

A future document MAY contain semantic content with different declared guarantees, for example freeform narrative beside a strongly typed entity collection.

Cross-region semantics are constrained by the stronger operation:

- durable typed references require stable semantic identity at the referenced endpoint;
- bound formulas require the stable typed field/reference contracts Accepted by ADR-0018; and
- a weak region MUST NOT become a durable formula/reference target through a label, coordinate, display order, storage path, or guessed type.

Weak content may be presented alongside or may mention stronger semantic objects without thereby acquiring their guarantees.

### 4. Identity is introduced only when the object has earned first-class identity

ADR-0015 remains unchanged.

If an independently addressable mutable semantic object already has an Accepted stable identity, strengthening its semantics MUST preserve that identity and MUST NOT model the transition as delete-and-recreate merely for implementation convenience.

A fragment that has not yet become a first-class identity-bearing semantic object MUST NOT be given fabricated retroactive identity. It may participate as proposal/migration source evidence, and new first-class target objects may receive new stable identities during the accepted transition.

Exact source-selection, source-to-target mapping, row/block/cell identity thresholds, and proposal locator mechanisms remain Provisional until concrete product pressure justifies them. This ADR does not require a universal `BlockId`, `RowId`, `CellId`, or node/edge graph.

### 5. Inference is advisory; declaration is authority

Deterministic analysis and AI may inspect content and propose candidate structure, including possible schemas, field types, relationships, mappings, normalization, or conversion plans.

Inference does not itself change canonical semantic meaning.

A structure inferred from values or natural language becomes durable semantic fact only through an explicitly accepted semantic transition. Probabilistic AI inference is advisory evidence and provenance, not an authoritative Query fact merely because it is machine-readable or schema-valid.

Current AI-originated semantic mutation remains subject to ADR-0007 approval requirements.

### 6. Promotion must expose ambiguity and loss

A freeform-to-stronger transition MUST distinguish at least conceptually between:

- exact/lossless mappings or conversions;
- transformations whose loss or coercion is explicitly accepted; and
- unresolved/ambiguous source content.

The system MUST NOT silently convert ambiguous content into a stronger claim by inserting defaults such as zero/null/empty string, parsing strings into numbers, guessing relationship targets by name, or otherwise introducing spreadsheet-like implicit coercion that conflicts with existing semantic/formula guarantees.

An unresolved source value may remain source evidence or may cause the proposed transition to remain diagnosable/rejected according to the operation's gate. This decision does not add a universal `Invalid`, `Unknown`, `Any`, or dynamic-value variant to the current typed `Value` model.

### 7. Strengthening uses the first-class Semantic API laws

A concrete strengthening operation is a semantic mutation and therefore follows ADR-0020.

Where such operations are eventually exposed, they are represented as typed semantic Commands or Atomic Command Batches and may be evaluated through Propose before Execute. Propose and Execute use the same semantic meaning and authoritative gates; only Execute may request semantic publication.

Review projections may expose proposed mappings, semantic impact, diagnostics, and exact/lossy/unresolved evidence. The exact promotion command catalogue, proposal/result DTOs, source selectors, mapping representation, and intra-batch temporary-object mechanics are Deferred.

Pre-identity source fragments are proposal/migration evidence, not durable semantic reference targets.

### 8. Validation follows declared semantic facts; stages do not change

ADR-0019 remains the validation authority.

Validation applies according to the semantic facts a subject actually declares:

- a semantic kind that does not claim schema-instance semantics is not invalid merely because it lacks a schema;
- once schema-instance semantics are declared, existing schema conformance rules apply;
- typed relationships add relationship validation;
- bound formulas add ADR-0018 formula graph/evaluation validation; and
- domain/extension rules apply only when they have explicit authority.

An interactive workflow may eventually retain a diagnosable candidate only if a separately accepted operation/runtime contract permits it. This ADR does not create such a retention/autosave contract and does not weaken the final operation gates required by ADR-0020.

### 9. Schema evolution is an explicit semantic migration

Changing established schema meaning is not merely a UI toggle.

A schema evolution that changes existing semantic claims MUST be explicit, deterministic, reviewable, and identity-preserving where identity is already established. Ambiguity, loss/coercion, changed constraints, relationship effects, and affected computation MUST be visible to the transition rather than silently reinterpreted at read time.

The exact schema-migration operation family remains Deferred. This decision does not merge semantic schema evolution with storage-format migration under ADR-0017.

### 10. Import and progressive strengthening share principles, not necessarily one engine

Legacy import and freeform promotion both benefit from the same high-level discipline:

```text
source content
  -> analyze candidate structure
  -> expose mapping / ambiguity / loss
  -> form semantic candidate
  -> deterministic validation / operation gate
  -> explicit review/authorization where required
  -> semantic publication
```

This conceptual reuse does not require a universal migration framework, new foundation crate, or common implementation pipeline today.

### 11. The current strongly typed core remains intact

This ADR does not weaken the current `Entity { schema: SchemaId, ... }` contract, typed `Value` model, stable Reference semantics, bound formula semantics, validation stages, storage representation, or ADR-0016 crate layering.

Future freeform/simple-table content should be introduced as additive semantic kinds/stores when real product pressure justifies their object model, identity, persistence, and editing semantics. The current typed Entity model MUST NOT be converted into an optional-schema dynamic property bag merely to implement progressive typing.

### 12. Game Development MVP remains strongly typed

This is an architecture non-exclusion and long-term product decision, not an implementation authorization for a general-purpose editor.

The current Game Development MVP continues to use the existing strongly typed workflow.

This ADR does not authorize immediate implementation of:

- a general freeform block/document engine;
- simple-table editing;
- `BlockId`, `RowId`, `CellId`, or universal fragment identity;
- optional-schema Entity;
- `AnyValue`, DynamicValue, or universal property bags;
- a schema-inference service;
- an AI extraction/promotion pipeline;
- generalized XLSX/ODS/CSV/Markdown import;
- a general schema-migration engine;
- a generic constraint DSL;
- a new schema/validation/migration/foundation crate; or
- an Office-like editing projection.

## Rejected alternatives

### Require schemas before all future content exists

Rejected as a universal product rule. It is appropriate for the current game-data wedge but imposes schema-design cost where stronger structure has not yet produced practical value.

### Make all semantic data a dynamic `Map<String, Any>` with optional schemas

Rejected. It would weaken the strongly typed identity/reference/formula/validation guarantees already hardened in ADR-0015 through ADR-0019 and recreate semantics as convention.

### Treat freeform content as an external staging world that must be copied into Tachiko semantics

Rejected as the long-term architecture. It can be an implementation simplification, but legitimate freeform content may itself be Tachiko semantic content with weaker declared meaning.

### Give every future fragment stable identity up front

Rejected. ADR-0015 requires stable identity where independent addressability/continuity earns it, not a speculative universal identity graph.

### Let deterministic or AI inference silently commit stronger meaning

Rejected. Inference is derived/advisory evidence; semantic strengthening is an explicit authoritative transition.

## Consequences

Positive:

- users are not forced to perform database design before ordinary content becomes useful;
- Tachiko retains its stronger differentiation when structure is added: stable identity, typed references, deterministic formulas, validation, semantic diff/merge, AI-addressable operations, and safe export;
- the current strongly typed Game Development wedge remains simple and hardened;
- AI can assist structure discovery without becoming semantic authority; and
- the architecture remains open to mixed-content future products without prebuilding an Office-scale inner platform.

Costs:

- future mixed-content implementation must define concrete semantic kinds and their identity thresholds;
- promotion/schema evolution require explicit reviewable migration semantics rather than silent coercion;
- UI authoring state and invalid-candidate retention remain separate problems; and
- future commands must be designed deliberately rather than exposing arbitrary dynamic property mutation.

## Required follow-up

- Reconcile `document-model.md` so semantic-first is not misread as universal schema-first.
- Reconcile `schema-system.md` so schema declarations are required only for semantic kinds that claim schema-instance semantics while the current Entity contract remains schema-required.
- Reconcile `validation-engine.md` so validation applicability follows declared semantic facts without adding stages.
- Reconcile `ai-agent-api.md` so AI inference remains advisory and strengthening uses the ADR-0007/ADR-0020 proposal/execution boundary.
- Keep concrete freeform kinds, source selectors, promotion command catalogues, identity thresholds, storage, and editor runtime Deferred until real implementation pressure exists.
- Close #13 with a Decision Capsule after the documentation reconciliation is merged.

## Related

- Product Constitution
- Design Principles
- ADR-0007
- ADR-0015
- ADR-0016
- ADR-0017
- ADR-0018
- ADR-0019
- ADR-0020
- Issues #13, #14, #18, #26, #27, #28
