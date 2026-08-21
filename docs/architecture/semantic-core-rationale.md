# Why Tachiko Work Has a Semantic Core

Status: Architecture rationale

This document explains the reasoning behind Tachiko Work's semantic-first architecture. It does not replace Accepted ADRs. If this rationale conflicts with an Accepted ADR, specification, or verified implementation contract, the more authoritative record wins and the conflict should be reconciled explicitly.

## Thesis

Tachiko Work did not begin by choosing `semantic` as a product label and then designing around it.

The direction emerged from several independent requirements that all pushed toward the same architectural boundary:

- game-development data needs stable domain meaning beyond spreadsheet coordinates;
- Git-native work needs deterministic, reviewable changes and identity that survives presentation changes;
- AI-native work needs operations over meaning rather than simulated UI actions;
- documents, tables, Markdown, computation, and future views should not become separate incompatible sources of truth;
- Office migration must be possible without making Office's historical representation the new core.

The resulting conclusion is that the durable center of Tachiko Work should be a typed semantic model whose identity and meaning are independent of UI layout and storage representation.

## 1. The rejected starting point: another Office suite

The original exploration began with a reasonable question: can a modern Rust project build a better open Office alternative?

A direct clone would naturally organize the product around familiar abstractions such as workbooks, sheets, cells, pages, paragraphs, and application-specific file formats. That path can produce useful software, but it also inherits the ontology and compatibility obligations of the products being cloned.

LibreOffice demonstrates the value of an open Office ecosystem. Its mission requires strong interoperability with Office-style documents and ODF, so Office-compatible document categories remain central to the product. Microsoft has an even larger compatibility obligation because decades of spreadsheets, documents, macros, templates, integrations, and enterprise workflows depend on historical behavior.

This is not a claim that Microsoft or LibreOffice are technically incapable of adding semantic diff, typed data, AI operations, Git integration, validation, or other individual features. They can.

The architectural distinction is that Tachiko Work is free to make semantic meaning the source of truth from the beginning rather than adding semantic features on top of an existing Office ontology.

**Office is an interoperability target, not Tachiko Work's ontology.**

## 2. Game-development data exposes the presentation/meaning split

Game-development balance data is the first commercial wedge because it makes the architectural problem concrete.

A spreadsheet may visually contain:

```text
row 27, column F = 135
```

but the team does not think of that value as `F27`. The durable meaning may be:

```text
weapon.iron_sword.damage = 135
```

The row, column, sheet, and visual position are useful presentation choices. They are not the business identity of the data.

This leads to a core rule:

**presentation coordinates must not be semantic identity.**

If an entity moves from one row to another, changes view, or is rendered as a document instead of a table, it should remain the same semantic object.

The same rule applies outside games. A budget category, requirement, policy rule, customer record, paragraph role, or computed field has meaning that should not disappear when the user changes layout.

## 3. Git-native work requires stable semantic identity

Making a file text-based is not enough to make a workflow Git-native.

A useful Git-native workspace needs changes that can be reviewed in terms users care about:

- what semantic object changed;
- which field changed;
- which computed values are affected;
- whether references are still valid;
- whether two changes actually conflict;
- whether equivalent semantic state produces stable canonical output.

If identity is derived from row position, UI coordinates, file path, or serialization order, harmless presentation changes become noisy deletes and additions.

A semantic identity layer makes it possible for rename, move, reorder, and alternative physical layouts to preserve object identity. Deterministic serialization then gives Git a stable materialization of that semantic state.

The exact identifier algorithm and exact physical encoding remain design choices. The invariant is stronger and simpler:

**identity must survive presentation and storage-layout changes, and equivalent semantic state must have a deterministic canonical representation when persisted.**

This is why Milestone 02 treats semantic identity and canonical serialization as foundation work rather than file-format polish.

## 4. AI-native work requires a meaning-level API

An AI system can operate traditional software by simulating a user:

```text
click B27
read C31
type a formula
select a paragraph
```

That can be useful automation, but it is fundamentally UI-dependent and brittle. It also makes impact analysis, permissions, validation, and human review harder because the system is acting on presentation state rather than declared meaning.

Tachiko Work instead wants AI-facing operations such as:

```text
reduce damage for legendary weapons by 8%
exclude boss-exclusive weapons
validate the result
show affected formulas and constraints
prepare a reviewable change
```

That requires typed objects, references, formulas, constraints, and semantic operations that exist independently of a particular editor.

ADR-0007 therefore makes the semantic layer the AI interaction boundary and keeps direct mutation behind explicit approval.

AI-native architecture is not an additional reason to invent a second model. It is another pressure toward sharing one semantic authority between humans, CLI workflows, future graphical clients, and agents.

## 5. Multiple views should be projections, not competing document models

A single semantic object may need several representations.

For example, one weapon record can appear as:

- a row in a spreadsheet-like view;
- a section in a document-like view;
- structured data in an API;
- a game-engine export;
- an AI-readable object;
- a canonical Git representation.

These should not become six independent sources of truth.

The design principle is therefore:

**multiple views, one semantic state.**

A view may have presentation-specific state such as column order, grouping, formatting, or layout. That state must not silently redefine the identity or meaning of the underlying data.

## 6. Progressive escape from Office, not a forced migration event

The semantic direction does not imply that existing Office assets should be rejected.

A realistic migration path can keep Excel, CSV, ODF, DOCX, or other legacy representations at the boundary while Tachiko Work gradually assumes responsibilities that are painful in the legacy workflow:

```text
Excel / CSV / ODF
        |
        v
import / migration adapter
        |
        v
semantic model
        |
        +--> validation
        +--> computation
        +--> semantic diff / merge
        +--> Git review
        +--> AI analysis
        |
        v
export / compatibility adapter
```

Early users may continue using Excel as an input, output, or familiar presentation surface while the semantic model becomes the durable authority for selected workflows.

This reduces switching cost and avoids requiring an organization to replace decades of files, macros, training, and integrations before receiving value.

The product strategy is therefore a **progressive escape from Office dependency**, not a big-bang Office replacement.

## 7. Small core, strong semantics

Tachiko Work should keep the core small, but `small` must not mean semantically empty.

The core exists to define the invariants that every client and capability must agree on. Candidate responsibilities include the minimum concepts needed for:

- stable semantic identity;
- typed values and objects;
- typed references;
- document/entity relationships;
- deterministic diagnostics and semantic operations;
- the contracts required by schema, formulas, validation, diff, merge, and persistence boundaries.

Capabilities that can be replaced without changing what Tachiko Work fundamentally means should remain outside the semantic kernel whenever practical. Examples include:

- graphical UI technology;
- Tauri or browser hosting;
- Excel, ODF, DOCX, CSV, and game-engine adapters;
- cloud providers;
- AI model providers;
- GitHub integrations;
- realtime collaboration mechanisms;
- enterprise policy systems;
- plugin hosts and ecosystem tooling.

A useful review question is:

> If this capability were removed or replaced, would the remaining semantic model still be Tachiko Work?

If yes, it is probably not a semantic-core invariant.

The opposite failure mode is also dangerous. Making identity, reference semantics, validation meaning, or mutation semantics plugin-defined would create many extensions without a coherent Tachiko Work model.

The principle is therefore:

**Small core, strong semantics.**

## 8. Lock invariants, defer mechanisms

Milestone 02 should not attempt to predict the perfect 2035 implementation.

The safest architectural strategy is to lock only high-confidence invariants and keep lower-confidence mechanisms replaceable.

### High-confidence invariants

These are suitable for early hardening:

- semantic authority is independent of UI technology;
- presentation coordinates are not durable semantic identity;
- storage paths and wire-format layout are not semantic identity;
- the semantic/domain model is not defined by a particular persistence DTO;
- equivalent semantic state requires deterministic canonical persistence behavior;
- legacy compatibility belongs at migration/import/export boundaries;
- formulas, schema, validation, diff, merge, and AI operations must not depend on one graphical client;
- semantic operations should be reusable across CLI, AI, native, WASM, and future clients where applicable.

### Decisions that should remain replaceable until evidence is stronger

Examples include:

- the final stable-ID generation algorithm;
- the final `.ro` syntax or eventual public file-format name;
- detailed `.roproj` physical layout beyond the relationship already established by ADR-0003;
- a complete formula language and standard library;
- a public plugin ABI or extension SDK;
- a realtime collaboration algorithm or CRDT protocol;
- a universal ontology intended to model every possible domain;
- a final graphical workspace component model;
- crate splits that do not yet have an independent lifecycle or dependency need.

An implementation can change without invalidating the semantic direction if its surrounding boundary is preserved.

## 9. Storage is a boundary, not semantic authority

The semantic model and its persisted representation have different responsibilities.

The domain model should express meaning and invariants. Storage DTOs and codecs should express versioned physical representation and migration concerns.

That separation makes it possible to evolve `.ro`, introduce `.roproj`, add adapters, or change canonical encoding without forcing persistence details into every semantic API.

The requirement is not that the first format be perfect. The requirement is that a format mistake remain cheaper to replace than a semantic-model mistake.

## 10. Formula, schema, and validation should harden by pressure from real use cases

Semantic-first does not justify inventing a universal type system, ontology, or formula language before users require it.

The preferred evolution loop is:

```text
concrete use case
    -> minimum semantic abstraction
    -> second use case
    -> pressure test
    -> generalize only when evidence requires it
```

For the current game-development wedge, weapons, characters, items, balance constraints, references, and computed fields provide concrete pressure on the model.

New domains should test whether the abstractions remain natural. If they do not, Tachiko Work should revise the model through explicit decisions rather than stretching an elegant abstraction until it becomes a universal but meaningless graph.

## 11. The semantic direction is falsifiable

`Semantic` is not a sacred word. It is the current architectural conclusion from the product requirements above.

The project should revisit the direction if evidence shows, for example, that:

- multiple real domains cannot share the model without excessive universal abstraction;
- stable semantic identity creates more complexity than value for ordinary workflows;
- Git-native review does not materially benefit from semantic identity and deterministic representation;
- legacy migration requires so much embedded legacy behavior that the proposed boundary becomes unrealistic;
- different views repeatedly require incompatible notions of meaning rather than projections of shared state;
- AI workflows cannot usefully operate on the same semantic operations as human-facing clients.

A failed assumption should result in a new ADR or reconciliation record. It should not be hidden by silently changing the meaning of earlier decisions.

## 12. Milestone 02 guardrails

The current Core & Format Hardening milestone should use this rationale as a review lens, not as permission to over-design.

For issue #20, crate layering should prefer a minimal dependency kernel with strong semantic contracts and replaceable capabilities. A crate should not move into the core merely because it may be useful later.

For issue #21, identity and typed references should be stable across rename, move, view changes, and storage-layout changes without prematurely committing to collaboration machinery or a universal graph system.

For serialization, formula, schema, validation, native/WASM, and future plugin work, the same rule applies:

> make semantic commitments strong and implementation commitments weak.

## Non-goals of this rationale

This document does not claim that:

- Microsoft Office or LibreOffice can never implement similar features;
- Tachiko Work should refuse Office compatibility;
- every piece of information must be strongly typed on first entry;
- every capability should become a plugin;
- the first `.ro` representation is permanent;
- Tachiko Work needs a universal ontology before the first usable product;
- a graphical spreadsheet-like experience is undesirable.

A spreadsheet-like, document-like, or Markdown-like experience can be an excellent interface. The architectural distinction is that the interface projects semantic state rather than defining the only identity and meaning of that state.

## Related authority and context

- [ADR-0001: Tachiko Work is a semantic platform, not an Office clone](../decisions/ADR-0001-semantic-platform-not-office-clone.md)
- [ADR-0003: `.ro` and `.roproj` representation](../decisions/ADR-0003-ro-and-roproj-representation.md)
- [ADR-0007: AI Semantic Interaction Model](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [Design Principles](../vision/design-principles.md)
- [Mission](../vision/mission.md)
- [Unified Semantic Document Model](document-model.md)
- [Git-Native Workflow](git-native-workflow.md)
- [Rust Crate Architecture](rust-crate-architecture.md)
- [Origin Discussion](../discussions/2026-08-20-origin-discussion.md)
- GitHub issues #19, #20, and #21
