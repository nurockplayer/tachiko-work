# Why Tachiko Work Has a Semantic Core

Authority: Explanatory architecture rationale

Decision state: Mixed. The semantic-first direction is Accepted through ADR-0001 and related principles; detailed mechanisms named here remain Provisional or Open Question unless separately accepted.

## Why this document exists

This document explains why Tachiko Work arrived at a semantic-first architecture. It does not independently create architectural authority.

If this rationale conflicts with the Product Constitution, an Accepted ADR/policy, or an accepted normative specification, the higher-authority record wins and the conflict must be reconciled explicitly under `docs/governance/knowledge-authority.md`.

## Thesis

Tachiko Work did not begin by choosing `semantic` as a product label and then designing around it.

The direction emerged from independent product pressures that converged on the same boundary:

- game-development data needs durable domain meaning beyond spreadsheet coordinates;
- Git-native work needs stable identity, deterministic materialization, and reviewable changes;
- AI-native work needs operations over meaning rather than simulated UI actions;
- documents, tables, Markdown, computation, APIs, and future views should not become incompatible sources of truth;
- Office migration must be possible without making Office's historical representation the ontology of the new core.

The accepted architectural direction is therefore a typed semantic model whose meaning is independent of any single UI or storage layout.

## 1. The rejected starting point: another Office suite

The original exploration asked whether a modern Rust project, accelerated by AI coding agents, could build an open Office alternative.

A direct clone would naturally organize the system around existing Office abstractions and compatibility behavior: workbooks, sheets, cells, pages, application-specific documents, macros, and historical file semantics.

That can produce useful software. LibreOffice demonstrates the value of an open Office ecosystem, while Microsoft Office has enormous practical compatibility obligations because decades of documents, macros, templates, and enterprise workflows depend on historical behavior.

Tachiko Work's architectural opportunity is different. It can choose meaning as the source of truth first, then treat Office-style formats and workflows as interoperability surfaces.

> Office is an interoperability target, not Tachiko Work's ontology.

This is not a claim that Microsoft Office or LibreOffice cannot implement semantic diff, typed data, AI operations, Git integration, validation, or similar capabilities. The distinction is which layer is allowed to define identity and meaning.

## 2. Game-development data exposes the presentation/meaning split

The game-development wedge makes the problem concrete.

A spreadsheet may visually contain:

```text
row 27, column F = 135
```

but a team may actually mean:

```text
weapon.iron_sword.damage = 135
```

The row, column, sheet, and visual position are presentation choices. They are not necessarily the durable identity of the domain object.

This produces a high-confidence invariant:

> Presentation coordinates must not become durable semantic identity merely because the first editor is table-shaped.

The same reasoning applies beyond games. A requirement, budget category, policy rule, customer record, document section, or computed field can retain meaning while its view/layout changes.

ADR-0015 now fixes the durable contract: independently addressable mutable
objects use typed, opaque, stable identity distinct from mutable human keys.
The generator and persisted encoding remain replaceable mechanisms owned by
their creation and storage boundaries.

## 3. Git-native work needs semantic stability, not merely text files

Making a file textual is not enough to make a workflow Git-native.

A useful review workflow needs to answer questions such as:

- what semantic object changed;
- which field or relationship changed;
- which formulas or constraints are affected;
- whether references remain valid;
- whether concurrent changes actually conflict;
- whether equivalent semantic state produces deterministic persisted output.

If identity is derived from row position, UI coordinates, file path, or serialization order, harmless representation changes become semantic noise.

The high-confidence requirement is therefore:

> Identity must survive presentation and storage-layout changes, and equivalent semantic state needs deterministic canonical persistence behavior.

ADR-0015 fixes typed opaque identity but deliberately leaves the identifier
generation algorithm replaceable. Storage DTO, directory sharding,
JSON/container format, and canonical encoding rules remain separate decisions
in #25/#37/#38.

## 4. AI-native work needs a meaning-level capability boundary

An AI system can automate traditional software through actions such as:

```text
click B27
copy C31
enter a formula
select a paragraph
```

That can be useful compatibility automation, but it is UI-dependent and makes permission, validation, impact analysis, and review harder to express.

Tachiko Work instead wants semantic requests such as:

```text
reduce damage for legendary weapons by 8%
exclude boss-only items
validate the candidate
show affected formulas and constraints
prepare a reviewable change
```

This pressure aligns with ADR-0007: AI is a client of semantic capabilities, not a special UI robot with secret authority.

The same semantic operation should be reusable by CLI, graphical clients, automation, and agents where appropriate.

## 5. Multiple views should be projections, not competing truths

One semantic object may need several useful presentations:

- a row in a spreadsheet/table view;
- a section in a document-like view;
- Markdown or structured text;
- an API object;
- a game-engine export;
- an AI-readable object;
- a canonical Git materialization.

These should not become independent competing sources of truth.

> Multiple views, shared semantic meaning.

A view can own presentation state such as grouping, column order, formatting, layout, or interaction state. That presentation state must not silently redefine the underlying object's identity or meaning.

This principle does not require every future document type to share one universal object taxonomy.

## 6. Progressive escape from Office dependency

The anti-lock-in mission does not imply rejecting existing Office assets.

A credible adoption path can keep Excel, ODF, CSV, DOCX, and other legacy representations at explicit boundaries while Tachiko Work gradually assumes responsibilities that are painful in the legacy workflow:

```text
legacy file/workflow
        ↓
import / analysis adapter
        ↓
semantic candidate
        ↓
validation / computation / diff / merge / review / AI
        ↓
export / compatibility adapter
```

Early adopters may continue using familiar tools as input, output, or presentation surfaces while selected workflows move toward an open semantic authority.

The product strategy is progressive migration, not a big-bang replacement event.

## 7. Small core, strong semantics

Keeping the core small does not mean making it semantically empty.

The core should contain only the invariants every Tachiko Work client and extension must agree on. Candidate responsibility areas include the minimum concepts needed for:

- semantic identity;
- typed values and relationships;
- validation/diagnostic meaning;
- formula/reference semantics;
- diff/merge semantics;
- the boundary between domain meaning and persistence representation.

Replaceable concerns should remain outside the stable kernel where practical:

- graphical UI technology;
- native/browser hosting technology;
- Office/ODF/CSV/game-engine adapters;
- AI model providers;
- cloud providers;
- Git hosting integrations;
- realtime collaboration mechanisms;
- plugin hosts and package registries;
- enterprise policy surfaces.

A useful test is:

> If this capability were removed or replaced, would the remaining semantic model still be Tachiko Work?

If yes, it probably should not become a semantic-core invariant without stronger evidence.

The reverse failure mode also matters. Stable identity, reference meaning, validation meaning, and other core invariants must not be independently redefined by plugins or clients until Tachiko Work has no coherent model left.

> Small core, strong semantics.

## 8. Lock invariants, defer mechanisms

Core & Format Hardening should not try to predict the perfect implementation a decade from now.

High-confidence invariants suitable for hardening include:

- semantic authority is independent of UI technology;
- presentation coordinates are not durable semantic identity;
- storage paths and physical wire layout are not semantic identity;
- the domain model is not defined by a specific storage DTO;
- equivalent semantic state requires deterministic persistence behavior;
- legacy compatibility lives at explicit system boundaries;
- semantic operations must not depend on one graphical client.

Mechanisms that should remain replaceable until evidence is stronger include:

- the stable-ID generation algorithm;
- the final `.ro` encoding or public format name;
- the production `.roproj` codec and host-persistence mechanisms, plus
  physical layouts for versions after the separately Accepted `.roproj/v1`;
- a complete formula language/standard library;
- a public plugin ABI;
- a realtime/CRDT collaboration algorithm;
- a universal ontology;
- a final graphical component model;
- speculative crate splits without an independent lifecycle/dependency need.

The rule is:

> Make semantic commitments strong and implementation commitments weak until evidence justifies freezing them.

## 9. Storage is a boundary, not semantic authority

The semantic model and its persisted representation have different responsibilities.

Domain structures express meaning and invariants. Storage DTOs/codecs express versioned physical representation, compatibility, and migration concerns.

That separation is what makes it possible to evolve `.ro`, implement `.roproj`, reuse standards, add adapters, or revise canonical encoding without making every persistence mistake a semantic-model mistake.

The first format does not need to be perfect. It needs an escape hatch.

## 10. Formula, schema, validation, and graph structure should generalize from real pressure

Semantic-first is not permission to invent a universal ontology or type/formula system before users need it.

Prefer this loop:

```text
concrete use case
    ↓
minimum semantic abstraction
    ↓
second use case
    ↓
pressure test
    ↓
generalize only when evidence requires it
```

The current game-development wedge provides real pressure through characters, weapons, items, balance constraints, formulas, references, and review workflows.

New domains should test the abstractions rather than being forced into them.

## 11. The semantic direction is falsifiable

`Semantic` is not a sacred word. It is the current architectural conclusion from the product requirements above.

The project should revisit assumptions if evidence shows, for example, that:

- multiple real domains cannot share useful semantic foundations without excessive universal abstraction;
- durable identity creates materially more cost than value in ordinary workflows;
- Git-native review does not benefit from semantic identity and deterministic representation;
- progressive legacy migration requires so much embedded historical behavior that the boundary model becomes unrealistic;
- different views repeatedly require incompatible notions of meaning rather than projections over shared state;
- AI workflows cannot usefully share semantic operations with human-facing clients.

A failed assumption should produce an explicit new Decision Issue/ADR/reconciliation record. It should not be hidden by silently changing the meaning of earlier documents.

## 12. Core & Format Hardening guardrails

ADR-0015 requires identity and typed references to survive rename, move, view
changes, and storage-layout changes without committing to collaboration
machinery or a universal graph model. Its implementation migration remains
follow-up work.

For #25/#37/#38, persistence/version/canonicalization should preserve the semantic boundary without assuming current v0.1 JSON details are eternal.

For #24/#23, formula/schema/diagnostic rules should become durable only where the semantic contract genuinely requires it.

For #20, crate layering should reflect those responsibilities instead of creating a crate for every future roadmap noun.

For #26, native/WASM boundaries should follow the hardened semantic/runtime ownership rather than force host constraints into the core.

## Non-goals

This rationale does not claim that:

- Microsoft Office or LibreOffice cannot implement similar features;
- Office interoperability is undesirable;
- every value must be strongly typed at first entry;
- every capability should be a plugin;
- the first `.ro` representation is permanent;
- Tachiko Work needs a universal ontology before a useful product;
- spreadsheet-like, document-like, or Markdown-like interfaces are undesirable.

The distinction is architectural authority: interfaces project or manipulate semantic state rather than becoming the only place where identity and meaning exist.

## Related authority

- [Product Constitution](../vision/product-constitution.md)
- [ADR-0001: semantic platform, not an Office clone](../decisions/ADR-0001-semantic-platform-not-office-clone.md)
- [ADR-0003: `.roproj` / `.ro` representation relationship](../decisions/ADR-0003-ro-and-roproj-representation.md)
- [ADR-0007: AI semantic interaction](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [ADR-0015: stable semantic identity](../decisions/ADR-0015-stable-semantic-identity.md)
- [Design Principles](../vision/design-principles.md)
- [Knowledge Authority](../governance/knowledge-authority.md)
- [Canonical Reconciliation Register](../governance/canonical-reconciliation-register.md)
- GitHub issues #19, #20, #21, #23, #24, #25, #26, #37, #38
