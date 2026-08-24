# Unified Semantic Document Model

Decision state: Accepted direction; detailed object model is Provisional

Implementation state: Stable identity and the Milestone 02
schema/entity/field/formula aggregate are implemented; richer block/view graph
remains Provisional

Hardening owner: #21. Progressive semantic strengthening is Accepted by ADR-0021; concrete future freeform object kinds remain Deferred.

## Authority note

Tachiko Work has accepted the semantic-first direction: documents, structured data, formulas, references, and future views should share meaning through the semantic model rather than letting historical file/UI representations own the truth.

Semantic-first does **not** mean every semantic object must carry a domain schema at creation. ADR-0021 accepts legitimate weaker semantic content and explicit strengthening when additional structure creates value. A paragraph or future simple table may therefore be semantic content without claiming the stronger schema-instance semantics of the current Entity model.

The exact long-term graph shape, set of block/object types, containment rules,
and view model are not all frozen by this document. ADR-0015 fixes the
implemented stable-identity/key boundary; ADR-0021 preserves that threshold and
does not authorize universal graph IDs or IDs for future object kinds without a
concrete independent-addressability/continuity need.

Examples below illustrate the direction. A named example type is not automatically a required v1 primitive.

## Overview

Tachiko Work is based on the idea that documents, spreadsheets, Markdown files, structured data, and computational notebooks should not require separate incompatible foundations.

They can be different views over shared semantic structures where that unification creates practical value.

The core object is not a Word document or an Excel workbook.

The architectural direction is a typed, structured, executable semantic graph where stronger declarations are added only when they earn their cost. The current Game Development aggregate is strongly typed; future weaker semantic kinds may coexist without weakening that aggregate.

## Design Principles

### Meaning over formatting

Traditional Office formats encode application-specific and historical representation choices.

Tachiko Work stores semantic meaning first where that meaning is useful to computation, validation, references, versioning, migration, multiple views, or AI reasoning.

Possible semantic concepts include:

- headings
- paragraphs
- tables
- formulas
- references
- datasets
- assets
- computations
- relationships

Rendering is a projection of this model rather than the owner of semantic truth.

A weaker semantic claim is still meaning. A freeform paragraph need not be forced into a schema merely to qualify as semantic content. Conversely, inferred structure does not become durable meaning until an explicit semantic transition accepts it.

## Candidate long-term objects

### Document

A richer future document may contain or reference semantic content such as:

```text
Document
├── Heading
├── Paragraph
├── Table
├── Spreadsheet/Table View
├── Chart
├── CodeBlock
├── Diagram
└── Metadata
```

This is illustrative. The current Developer MVP is narrower and centers on typed schemas, entities, fields, references, and formulas.

### Block

Blocks are a candidate abstraction for richer document content.

Possible future blocks include:

- Text
- Heading
- List
- Table
- Formula
- Image
- Diagram
- Code
- Query
- Embedded dataset

Block identity, ordering, containment, and persistence remain part of later semantic-model design rather than an implemented v0.1 contract.

ADR-0021 explicitly rejects pre-allocating universal `BlockId`, `RowId`, or `CellId` merely to preserve a hypothetical future strengthening path. Stable identity is introduced when a concrete semantic object becomes independently addressable/continuity-bearing under ADR-0015.

## Progressive semantic strengthening

ADR-0021 accepts this long-term principle:

> Write first; strengthen semantics when the structure earns its cost; preserve established identity and meaning across the transition.

Future content may therefore move from weaker declared meaning toward stronger contracts such as schema conformance, typed relationships, constraints, or computation through explicit semantic transitions.

Strengthening is not read-time reinterpretation and is not equivalent to a global `typing_level`. AI or deterministic analysis may propose structure, but inferred structure is advisory until a semantic transition is accepted through the ADR-0020 Semantic API laws.

Already-established stable identity and meaning must survive strengthening. A source fragment that never had first-class identity must not be given fabricated retroactive identity merely to make migration history look continuous.

## Spreadsheet as a Semantic View

A spreadsheet-like surface should not reduce all meaning to a grid of coordinates.

Structured work can contain:

- schema
- records/entities
- formulas
- references
- constraints
- views
- calculations

Example semantic shape:

```text
Enemy
├── id: EnemyId
├── hp: Health
├── attack: Damage
├── speed: Speed
└── drops: ItemReference[]
```

The visual table can be one representation over this meaning.

A future simple table may exist before it is promoted into a typed entity collection. Until stronger semantics are explicitly declared, coordinates, labels, or display order are not durable semantic identity and cannot silently become formula/reference targets.

## Formula Model

Formulas are semantic expression trees rather than opaque calculator strings in the core model.

For illustration:

```text
Attack * Speed
```

can correspond to a typed expression such as:

```text
Multiply
├── Reference(Attack)
└── Reference(Speed)
```

This supports:

- static analysis
- AI understanding
- dependency tracking
- safer refactoring

The durable binding/reference/numeric semantics are Accepted in ADR-0015 and
ADR-0018 and implemented for the current formula subset.

ADR-0021 does not weaken those rules for freeform content. Durable computation requires stable, declared endpoints; labels, cell coordinates, guessed types, or weak source fragments do not become formula bindings.

## References

References are typed semantic relationships rather than unvalidated application strings.

Current durable entity references store `EntityId`; bound formula references
store `EntityId + FieldId`. Schema, field, and entity keys are mutable human
addresses resolved through deterministic derived indexes. Rename changes a key
without changing an ID or rewriting a bound reference. Normal creation uses a
replaceable UUIDv7 host seam, while deterministic legacy migration uses a
version-scoped UUIDv5 mechanism; neither UUID family is generic semantic
meaning.

Progressive strengthening preserves every already-established durable identity/reference. A future source fragment without first-class identity may participate as proposal/migration evidence, but it cannot be treated as a durable reference target until admitted as an appropriate identity-bearing semantic object.

## AI-Native Operations

AI should operate on semantic capabilities.

Not primarily:

```text
click cell B17
copy text
move cursor
```

But operations such as:

```text
Update balance data
Explain affected systems
Generate migration proposal
Create review summary
```

ADR-0007 governs AI authority; ADR-0020 governs shared Semantic API behavior; ADR-0021 permits AI to propose semantic strengthening but does not make probabilistic inference semantic truth or permit silent schema/type promotion.

## Future Direction

This semantic foundation can support:

- document-like editing
- spreadsheet/table views
- Markdown workflows
- Git-native review
- AI agents
- collaborative workspaces

without requiring separate incompatible sources of truth or requiring every content kind to be fully typed before it becomes useful.
