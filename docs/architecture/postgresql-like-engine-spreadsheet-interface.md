# PostgreSQL-like Engine, Spreadsheet-first Human Interface

Authority: Explanatory product and architecture synthesis

Decision state: Founder framing / Strategy under GitHub Issue [#234](https://github.com/nurockplayer/tachiko-work/issues/234). This document does not independently create Accepted architecture authority.

Recorded: 2026-09-03 JST (2026-09-02 UTC)

## Why this document exists

Several Tachiko Work discussions reached the same conclusion from different directions:

- the semantic core should stay small, typed, and independent from any one user interface;
- vertical applications should usually be composed as domain solutions above the shared runtime;
- ordinary users should receive a complete, familiar product before they are asked to understand the platform underneath;
- advanced users and builders should still be able to inspect, modify, automate, and extend that same foundation; and
- PostgreSQL provides useful architectural prior art for building a durable, extensible engine without absorbing every application domain into its core.

Those conclusions already appear separately in [the Mission](../vision/mission.md), [Design Principles](../vision/design-principles.md), [the semantic-core rationale](semantic-core-rationale.md), ADR-0001, ADR-0020, and Issues #192, #215, #217, and #220.

This document connects them into one product-architecture model.

## The model in one sentence

> **Tachiko Work should have a PostgreSQL-like architectural shape: a small, typed, extensible engine that many clients can operate, while its default human interface feels like a spreadsheet rather than SQL or REST.**

In plain language:

> **Underneath, an extensible semantic work engine. On top, a familiar spreadsheet-first product.**

The comparison is about architectural shape, not database compatibility.

## What the PostgreSQL comparison means

PostgreSQL is not identical to the `psql` prompt or to SQL text typed by a person.

One PostgreSQL engine can be used through:

- SQL clients;
- language drivers;
- ORMs;
- administration tools;
- application-specific services; and
- REST or GraphQL gateways built over the database boundary.

Those interfaces differ greatly, but they operate over one database authority and one set of typed behavior, constraints, transactions, and extension mechanisms.

Tachiko should aim for the corresponding separation at the work-semantics layer:

- the spreadsheet is the default human interface;
- CLI, API/SDK, AI, and specialized domain views are other clients;
- all clients use the same Semantic API and typed operations where applicable; and
- no client creates a competing semantic truth.

This is the useful analogy:

```text
PostgreSQL
engine != SQL prompt

Tachiko Work
semantic engine != spreadsheet UI
```

## What the PostgreSQL comparison does not mean

This framing does not imply that Tachiko should:

- become a relational database;
- expose SQL as its semantic language;
- use PostgreSQL as a required dependency;
- copy PostgreSQL OIDs, MVCC, WAL, query planning, system catalogs, or native extension ABI;
- introduce a universal runtime registry merely because PostgreSQL has catalogs; or
- build a REST server only to complete the analogy.

Issue [#220](https://github.com/nurockplayer/tachiko-work/issues/220) owns the detailed PostgreSQL comparison. Its conclusion was to transfer a few architectural principles selectively while refusing database machinery that does not answer current Tachiko pressure.

## Product and architecture layers

```text
Driver experience
spreadsheet-first GUI + familiar packaged domain solutions
                         │
Other first-class clients
CLI │ API / SDK │ AI agent │ specialized domain views
                         │
                 same Semantic API
                 same typed operations
                 same semantic state
                         │
Tachiko semantic work engine
identity │ types │ relationships │ formulas │ validation
operations │ revisions │ diff/conflict │ explanation
                         │
Hosts and representations
.roproj │ .ro │ Git integration │ future adapters
```

Each layer answers a different question.

| Layer | Question it answers |
| --- | --- |
| Human/product surface | How does a person complete a recognizable job? |
| Client/interface | How is semantic state selected, shown, proposed, or operated? |
| Semantic API/runtime | What operation is requested, validated, authorized, evaluated, and published? |
| Semantic model | What objects, relationships, values, rules, and identities mean? |
| Host/representation | How is the state executed, persisted, transported, reviewed, or integrated? |

The layers may evolve independently only where their contracts allow it. They must not redefine each other's authority.

## Spreadsheet-first, not spreadsheet-shaped at the core

A spreadsheet is a strong default human interface because it is familiar, information-dense, flexible, and already acts as the working language for many structured workflows.

That is a product decision. It is not permission to make the cell the center of the semantic universe.

For example:

```text
What a table displays
row 27, column F = 135

What the work may mean
weapon.iron_sword.damage = 135
```

The row, column, sheet, and coordinate help a person see and edit the value. They do not necessarily identify the durable domain object.

This distinction preserves several existing Tachiko commitments:

- moving a row or column must not automatically change semantic identity;
- a graph, document-like view, API object, or game-engine export may project the same meaning differently;
- AI should request meaning-level operations rather than depend on clicking cells;
- Git materialization should preserve semantic identity across harmless presentation changes; and
- frontend state must not become a second canonical model.

The spreadsheet is therefore a privileged product surface, not a privileged source of truth.

## Spreadsheet-first is not spreadsheet-only

The default interface should make ordinary structured work feel familiar, but Tachiko should not force every job into rows and columns.

Different views can remain useful cognitive instruments over shared meaning:

- tables expose comparison, sorting, grouping, and aggregation;
- document-like views expose context, argument, and narrative;
- graphs expose relationships and dependencies;
- timelines expose sequence, duration, and delay;
- dashboards expose selected operational signals;
- domain-specific surfaces expose the concepts and actions that matter to one job.

A specialized view is justified when it makes something materially easier to see, understand, decide, or do. Its presentation state can differ without becoming another semantic authority.

## One operation, multiple clients

The strongest proof of this architecture is not that multiple interfaces can read the same file. It is that they can participate in the same semantic operation lifecycle.

Conceptually:

```text
Spreadsheet user
edits the displayed stock quantity
                    │
API client
submits an inventory adjustment
                    │
AI agent
proposes "receive 20 units of Item A"
                    │
                    ▼
          one typed semantic intent
                    ▼
validation + authorization + impact/explanation
                    ▼
       one authoritative publication result
                    ▼
       one authoritative canonical state
                    ▼
each client receives an authorized,
revision-consistent projection of that state
```

The canonical semantic state remains singular, but disclosure is grant-scoped: different clients may legitimately receive different projections or execution facts. The visible workflow may differ by client, while meaning, validation, authorization, revision, and publication rules must not be reimplemented as unrelated UI logic.

This consumes the accepted direction in [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md), [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md), [the semantic-authorization specification](../specs/semantic-authorization.md), and the [frontend/backend boundary](frontend-backend-boundary.md).

## Core, reusable capability, domain solution, or interface

A recurring architecture failure is to ask only whether a feature is useful. Useful features can belong at very different layers.

Before adding a new concept, classify it:

| Test | Likely owner |
| --- | --- |
| Must every conforming Tachiko client agree on this meaning or invariant? | Semantic core or Semantic API authority |
| Is it reusable but optional behavior, declaration, integration, or lifecycle-owned capability? | Project/template asset, semantic asset, Skill, connector, or extension according to its trust and execution class |
| Does it package domain meaning and workflows into a recognizable job outcome? | Domain solution |
| Does it only change how existing meaning is selected, arranged, edited, or displayed? | Spreadsheet or another interface projection |

A practical ordering rule is:

```text
ordinary project semantics
        ↓ if reusable packaging is needed
template / declarative semantic asset
        ↓ if a human or agent procedure is needed
Skill / workflow guidance
        ↓ if external effects or new executable behavior are required
connector / bounded extension
        ↓ only if every client must share a new invariant
semantic core primitive
```

Core is considered last, not because it should be weak, but because it is the most expensive place to put a concept.

## Small core does not mean empty core

There are two symmetric failure modes.

```text
core too thin
→ each solution rebuilds identity, types, rules, validation,
  operations, history, and explanation independently
→ Tachiko becomes a format or database wrapper
```

```text
core too broad
→ Customer, Invoice, Inventory, Approval, Project,
  and every vertical workflow enter the generic engine
→ Tachiko becomes a domain-aware monolith
```

The target is a small but semantically strong center.

The core should own the minimum invariants needed for clients and solutions to agree on meaning. Domain applications should compose those invariants without redefining them.

## Domain solutions above the engine

Issue [#192](https://github.com/nurockplayer/tachiko-work/issues/192) records the hypothesis that vertical applications may often become domain solutions over one common semantic runtime.

```text
Tachiko semantic/runtime primitives
        ↓
domain model + rules + views + procedures + integrations
        ↓
complete inventory / CRM / project / game-design experience
```

For example, an inventory solution may contain concepts such as:

```text
Product
InventoryItem
Location
Receipt
Shipment
ReorderPolicy
```

Those nouns should normally remain domain-solution content. The generic core should instead provide the reusable identity, type, relationship, formula, validation, operation, revision, conflict, and explanation capabilities required to express them.

This does not mean that every production ERP or CRM can be reduced to a template. Specialized transactions, permissions, compliance, runtime services, integrations, scale, or user experience may require declarative assets, extensions, separate services, or a dedicated product. Evidence decides the boundary.

## Driver, Enthusiast, and Builder use the same engine

Issue [#215](https://github.com/nurockplayer/tachiko-work/issues/215) describes a stock-simple, tuner-deep product model.

This architecture gives that model a concrete shape:

```text
Driver
uses a spreadsheet-first surface or packaged domain solution
without learning the engine

Enthusiast
opens the hood to inspect and modify types, relationships,
formulas, validation, views, templates, and procedures

Builder
uses APIs, reusable assets, connectors, and bounded extensions
to create new solutions over the same engine
```

The three levels must not become three separate products with incompatible models.

Progressive disclosure changes how much structure the user sees. It does not change which semantic state is authoritative.

## The proof burden

The project should not treat this framing as proven merely because the diagram is elegant.

The central falsifiable question is:

> **Can materially different work domains reuse one small set of strong semantic/runtime primitives without domain leakage into core or hidden reimplementation outside it?**

Evidence should eventually demonstrate:

1. at least two materially different domains reuse the same primitives;
2. a spreadsheet-like client and at least one headless client use the same semantic operations and authoritative state, with disclosure remaining grant-scoped;
3. domain-specific UX remains strong without putting domain truth into frontend state;
4. reusable assets, Skills, connectors/extensions, domain solutions, and specialized services retain understandable boundaries; and
5. counterexamples are recorded where a behavior cannot honestly fit the shared-engine model.

A failed test is useful. If each serious domain repeatedly needs an incompatible semantic engine, the hypothesis should be narrowed rather than protected through vague abstractions.

## Non-normative architecture review checklist

The questions below are a review aid for applying the framing. They do not independently create acceptance criteria or override Accepted ADR/spec authority.

Before approving a new feature, abstraction, or public contract, ask:

1. Is this meaning, optional reusable capability, packaged domain experience, or presentation?
2. Would GUI, CLI, API, and AI clients need exactly the same contract?
3. Does the proposal make a cell coordinate, file path, serialization detail, or frontend state semantic authority?
4. Is a domain-specific noun entering the generic engine for one vertical's convenience?
5. Does the extension point answer demonstrated independent evolution, or only speculative flexibility?
6. Can the capability be removed or replaced while the remaining semantic model is still Tachiko?
7. What second domain pressure-tests the proposed primitive?
8. What second client proves that the behavior is not trapped in one interface?
9. What deterministic evidence explains the result or effect?
10. What migration path exists if the boundary is wrong?

## Non-goals

This document does not authorize:

- SQL compatibility or relational semantic authority;
- a PostgreSQL dependency;
- a REST API implementation;
- a universal Semantic Catalog;
- a public plugin ABI or marketplace;
- a generic cost-based planner;
- database MVCC or WAL as Tachiko history;
- a universal `Cell`, `Capability`, or `DomainSolution` core type;
- Excel feature-by-feature parity;
- forcing all work into a spreadsheet interface; or
- claiming current production support for ERP, CRM, inventory, accounting, or other vertical applications.

## Relationship to existing authority

This document explains how existing records fit together. It does not supersede them.

- [Product Constitution](../vision/product-constitution.md) owns the highest-level product constraints.
- [Mission](../vision/mission.md) owns user ownership and the long-term foundation for digital work.
- [Design Principles](../vision/design-principles.md) already establish semantic core first, multiple views over shared meaning, a small stable core, an open ecosystem, and generalization from real pressure.
- [ADR-0001](../decisions/ADR-0001-semantic-platform-not-office-clone.md) establishes a semantic platform rather than an Office clone.
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md) establishes the first-class headless Semantic API boundary.
- [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md) establishes shared runtime authority and host separation.
- [Semantic Core Rationale](semantic-core-rationale.md) explains why UI, storage, Git, AI, and domain views must not become competing semantic authorities.
- [Frontend/Backend Boundary](frontend-backend-boundary.md) explains how graphical clients project and operate shared semantic state.
- Issue [#192](https://github.com/nurockplayer/tachiko-work/issues/192) owns the domain-solution strategy hypothesis.
- Issue [#215](https://github.com/nurockplayer/tachiko-work/issues/215) owns the Driver, Enthusiast, and Builder product model.
- Issue [#217](https://github.com/nurockplayer/tachiko-work/issues/217) records the layered distinction among semantic state, user outcome, domain solution, creator artifact, and Market offering.
- Issue [#220](https://github.com/nurockplayer/tachiko-work/issues/220) records the detailed PostgreSQL research and bounded lessons.
- Issue [#234](https://github.com/nurockplayer/tachiko-work/issues/234) tracks this founder framing and its evidence/promotion criteria.

If this explanatory framing conflicts with an Accepted ADR or normative specification, the higher-authority record wins and the contradiction must be reconciled explicitly.
