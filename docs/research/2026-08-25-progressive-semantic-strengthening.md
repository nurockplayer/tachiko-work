# Issue #13 progressive semantic strengthening synthesis

Decision state: Research / decision evidence. ADR-0021 is the architecture authority.

## Outcome

Issue #13 accepts progressive typing only as **progressive semantic strengthening**, not as a weakly typed universal core.

Accepted direction:

- semantic-first does not imply schema-first;
- legitimate weak/freeform semantic content is distinct from malformed raw authoring;
- mixed-strength content may coexist;
- stronger declarations are explicit semantic transitions rather than inferred reinterpretation;
- established stable identity/meaning survives strengthening;
- fragments that never had first-class identity do not receive fabricated retroactive identity;
- durable references and formulas require stable declared endpoints;
- deterministic or AI inference may propose structure but does not become semantic authority;
- conversion/promotion must expose exact, lossy, and unresolved cases rather than silently coerce;
- schema evolution is an explicit reviewable semantic migration;
- validation stages remain ADR-0019 authority and apply according to semantic facts actually declared; and
- the current Game Development MVP remains strongly typed.

## Narrowing from research wording

The accepted architecture does **not** require every future row, cell, block, or fragment to have stable identity underneath. ADR-0015 remains the threshold: independently addressable mutable semantic objects receive stable identity when concrete product semantics require continuity/referenceability.

The accepted architecture also does not freeze a source-selection mechanism such as `stable parent + base-bound selector`. Pre-identity source fragments may participate as proposal/migration evidence, but exact selectors and source-to-target mapping DTOs remain Provisional.

Probabilistic AI/schema inference is advisory evidence, not an authoritative Semantic API Query fact. Only deterministic semantic facts produced under Accepted authority can be treated as authoritative Query results.

## Relationship to ADR-0020

A future strengthening operation is a semantic mutation under ADR-0020. Where exposed, it uses typed Command/AtomicBatch meaning, may be evaluated through Propose, and only Execute may request semantic publication. Exact command names, proposal/result DTOs, mapping representation, temporary handles, and runtime/session mechanics remain Deferred.

## Explicit non-goals

This decision does not authorize immediate implementation of a general freeform editor, simple-table runtime, universal `AnyValue`, optional-schema Entity, `BlockId`/`RowId`/`CellId`, inference engine, AI extraction pipeline, generalized Office import framework, generic migration engine, generic constraint DSL, or new foundation crate.

## Exit-criteria disposition

- typing policy: progressive strengthening accepted;
- identity before typing: only existing first-class identity is guaranteed; no universal pre-identity graph;
- mixed typed/untyped behavior: allowed with explicit crossing rules;
- schema inference: advisory/proposed, explicit acceptance required for semantic change;
- migration/evolution: explicit, deterministic, reviewable, identity-preserving, loss/ambiguity visible;
- validation: ADR-0019 stages unchanged; applicability follows declared semantic facts and operation gates;
- MVP: strongly typed workflow remains current implementation; freeform runtime is Deferred.
