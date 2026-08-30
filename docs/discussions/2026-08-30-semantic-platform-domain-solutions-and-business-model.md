# Semantic Platform, Domain Solutions, and Business-Model Synthesis

Status: Discussion history / Hypothesis synthesis. **Not authoritative by itself.**

Captured: 2026-08-30  
Repository baseline at capture: `2c3f72676a408f1086f6a989397b757b307a1959`

## Authority boundary

This document preserves the reasoning path that connected several recent product,
ecosystem, enterprise, and business-model discussions. It is intentionally broader
than any one Decision or Strategy Issue.

Do not treat the prose below as Accepted product, architecture, licensing,
commercial, ecosystem, or roadmap authority. Durable parts are already routed to
owning records:

- #192 owns the strategic hypothesis that vertical applications may become
  composable domain solutions over the common Tachiko semantic platform;
- #176 and #177-#185 own the layered ecosystem contribution model;
- #18 owns gradual enterprise migration and Japan-enterprise research strategy;
- #15 owns licensing and open/commercial boundaries;
- #135 owns future public extension distribution, trust, compatibility, and
  support policy;
- Accepted ADRs/specifications remain authority for semantic identity, formulas,
  validation, Semantic API, runtime, authorization, and storage.

The purpose of this memo is to preserve *how the ideas connected* so future
research does not have to reconstruct the path from chat history.

## Starting question: how software ecosystems compound

The discussion compared several families of productivity and platform businesses:

- Microsoft, Google, and Apple as large integrated ecosystems;
- LibreOffice as an open productivity suite with a different sustainability
  model;
- Notion, HackMD, and newer SaaS products that grew through low-friction sharing,
  templates, collaboration, and creator/community loops;
- extension/package/creator ecosystems such as VS Code, Figma, dbt, Obsidian,
  Shopify, Atlassian, and related platform models.

The useful conclusion was not that Tachiko should copy one company. The stronger
hypothesis is a hybrid growth model whose layers compound:

```text
open/local semantic foundation
        ↓
usable vertical wedge
        ↓
reusable templates / semantic assets / Skills
        ↓
creators / developers / consultants
        ↓
team and enterprise deployments
        ↓
distribution / governance / services / optional managed infrastructure
```

This is not a commitment to build SaaS, a Marketplace, a plugin store, or a
partner program now. Current ecosystem decisions deliberately avoid making those
a cold-start prerequisite.

## Why "plugin ecosystem" is too narrow

Mature ecosystems create value at several contribution thresholds. Requiring
every contributor to write executable code would make Tachiko unnecessarily
developer-centric.

A more useful contribution ladder is:

```text
ordinary user
  → reusable project / template / view

power user / domain expert
  → declarative semantic assets / queries / procedures / Skills

developer
  → connectors / executable extensions / renderers / integrations

consultant / company
  → domain solutions / migration / integration / support / training

core contributor
  → generic semantic/runtime changes
```

Contribution friction and execution authority should rise together. A reusable
data-only asset should not pay the same trust, capability, security, and lifecycle
cost as arbitrary executable code.

## The deeper product question: what is Tachiko actually storing?

A spreadsheet, ERP screen, dashboard, report, or SaaS application often contains
more than visible values. It may encode:

- entities and relationships;
- formulas and derived values;
- constraints and validation;
- business rules and approval expectations;
- operational procedures;
- implicit provenance and historical workarounds;
- mappings to external systems;
- presentation choices that expose only one view of the underlying work.

Traditional software frequently stores this meaning indirectly inside a mixture
of database schemas, application code, formulas, macros, UI logic, workflow
configuration, documentation, and human memory.

Tachiko's existing architectural direction suggests a different center of
gravity: keep domain meaning in an inspectable semantic model and let multiple
clients, projections, agents, adapters, and integrations operate over the same
meaning without becoming alternate semantic authorities.

This led to the working phrase:

> **semantic work substrate**

The phrase is discussion shorthand, not a product rename or new architectural
layer. The durable point is that Tachiko may be more valuable as a common semantic
runtime for structured work than as a clone of any single productivity
application.

## Before Tachiko: the "compiler" from domain knowledge to software was people

A domain expert may state rules in ordinary language:

```text
inventory below safety stock requires attention
Japanese supplier lead time is normally 14 days
these parts are incompatible
class-A purchases require two approvals
```

In a conventional project, those statements may pass through:

```text
domain expert
  ↓
requirements / spreadsheets / documents
  ↓
PM / BA / systems analyst
  ↓
data model + workflow + application specification
  ↓
engineers
  ↓
database + backend + frontend + integrations + tests
  ↓
running software
```

Every boundary can lose meaning. Later maintainers may only see
`approval_count >= 2` in code and have to rediscover why that condition exists.

Excel and VBA shorten the chain by letting power users encode rules directly,
but often trap business knowledge in workbook layout, formulas, macros, or one
employee's memory. Low-code/no-code systems shorten the engineering chain further,
but usually ask users to learn a simplified application-development model.

AI coding shortens implementation time again:

```text
domain knowledge
  ↓
prompt
  ↓
AI
  ↓
TypeScript / Python / SQL / application code
```

That is a major improvement, but the knowledge is still commonly compiled into
another implementation whose meaning must later be reconstructed from code.

## Hypothesis: domain knowledge can become inspectable semantic assets

The more ambitious possibility is:

```text
domain knowledge
        ↓
human + AI-assisted modeling
        ↓
inspectable Tachiko semantic material
        ↓
multiple deterministic / projected / integrated behaviors
```

Conceptually, domain knowledge may become some combination of:

- schemas and typed entities;
- relationships and references;
- formulas and calculations;
- validation and deterministic constraints where supported;
- saved semantic queries/analyses;
- workflows or Skills over existing authorized operations;
- views and projections;
- connectors/extensions only where new computation or host effects are genuinely
  required.

AI may lower the cost of producing those assets by translating domain-language
statements into proposals. The human/domain expert remains authority for business
truth; deterministic Tachiko engines remain authority for admitted semantic
behavior; AI remains a client/proposer rather than a new source of truth.

This may be more consequential than "AI writes an Excel file" or "AI writes CRUD
code" because the reusable artifact may preserve domain meaning at a level that
several tools and agents can inspect directly.

## Application-to-domain-solution inversion

The strongest strategic deduction was that some vertical software categories
might eventually invert from standalone semantic products into compositions over
the common Tachiko runtime.

Traditional vertical application:

```text
product-specific domain model
+ product-specific rules
+ product-specific workflow
+ product-specific UI
+ product-specific integrations
+ product-specific persistence/runtime
= one application
```

Possible Tachiko model:

```text
common Tachiko semantic/runtime primitives
        ↓
domain model + rules + reusable assets
        ↓
domain-specific solution experience
```

This produced the provocative phrase:

> **ERP from product to template**

That phrase is deliberately incomplete. #192 records the more precise strategic
hypothesis:

```text
ordinary project / reusable template
+ declarative semantic packs
+ views / projections
+ Skills / workflows
+ connectors / executable extensions where required
+ optional services / support
= domain solution
```

The architectural implication is not "build ERP now." It is that `Customer`,
`Invoice`, `PurchaseOrder`, `InventoryItem`, `GameWeapon`, `ResearchClaim`, and
other domain nouns should not automatically become Tachiko core concepts. They
should remain domain content whenever generic primitives can honestly represent
the required meaning and behavior.

## ERP is a pressure test, not a current product claim

ERP makes the hypothesis concrete because ERP-class systems combine:

- customers, suppliers, products, inventory, orders, invoices;
- formulas, validations, and relationships;
- approvals and procedures;
- reports and role-specific views;
- external systems and migrations;
- permissions, audit, governance, and compliance expectations.

If Tachiko can represent a substantial portion of those concerns generically, an
ERP-like solution might increasingly be domain content rather than a separate
semantic engine.

But real ERP-class products also require hard behavior that must not be waved away
as "configuration":

- transactions and concurrency;
- accounting and regulatory semantics;
- segregation of duties and authorization;
- audit/retention obligations;
- manufacturing/inventory-specific invariants;
- high-volume operational workloads;
- long-lived migrations and compatibility;
- organization-wide deployment and support.

Some may generalize into future Tachiko primitives; some may require extensions or
services; some may remain specialized products. Evidence must be allowed to narrow
or reject the hypothesis.

## Presentation inversion follows the same pattern

Excel-like, Word-like, PowerPoint-like, dashboard, form, or report experiences
need not each own a separate semantic document model.

Where underlying meaning is shared, those interfaces can be projections or
materializations of common semantic state. Renderer-specific state should remain
renderer/presentation state unless a separate continuity requirement earns
first-class semantic authority.

This does not imply that all presentation modes have identical UX or that every
view can be generated trivially. It preserves the separation between domain
meaning and one particular way of showing it.

## Business-model hypothesis: monetize coordination around open meaning

The discussion produced a commercial hypothesis that fits the product principles
better than data/format lock-in:

> **Keep semantic meaning, portable project ownership, and standalone value open
> enough to remain credible; monetize coordination, scale, governance,
> distribution, managed execution, and expertise where users receive incremental
> service value.**

Potential long-term revenue surfaces include:

1. managed sync, backup, sharing, and collaboration;
2. team/enterprise identity, policy, audit, private distribution, managed
   deployment, and support;
3. hosted AI/compute/automation where managed resources have operating cost;
4. future creator distribution/commerce if real supply and demand justify it;
5. migration, integration, training, managed support, consulting, OEM/embedding,
   and related professional services.

This is not a pricing plan, SaaS commitment, open-core decision, or license
selection. #15 remains the decision owner.

A recurring principle is that commerce may govern acquisition, updates, or
service, but should not become semantic authority. A local project should not
lose its meaning merely because a cloud subscription, Marketplace listing,
creator account, or vendor disappears.

## Why services and consultants may be part of the moat

Large ecosystems compound partly because independent people and companies can
build careers on top of them. A future Tachiko service ecosystem might include:

- Excel/Office migration specialists;
- domain modeling consultants;
- systems integrators;
- data/model auditors;
- template/Skill studios;
- training providers;
- connector/extension developers;
- support and managed-service providers;
- OEM/embedding partners.

A particularly attractive flywheel is:

```text
customer engagement
  ↓
customer-owned working solution
  ↓
reusable template / pack / Skill / connector where rights permit
  ↓
next deployment becomes cheaper
  ↓
more users and more domain evidence
  ↓
better reusable primitives and assets
```

The consultant's output can then compound the ecosystem instead of remaining only
opaque one-off custom code. #183 records the corresponding service/partner
boundaries.

## A six-sided growth loop

Several participant classes may reinforce one another:

```text
Users
  create projects and expose real problems

Creators / domain experts
  publish reusable domain knowledge

Developers
  add connectors and capabilities that genuinely require code

AI agents
  reduce modeling, migration, and operational friction while remaining clients

Consultants / integrators
  turn difficult deployments into reusable solution assets

Companies
  create enterprise requirements, budgets, support demand, and ecosystem
  opportunities
```

The possible moat is accumulated usable domain meaning, workflows, integration
knowledge, conformance evidence, and expertise around an open semantic substrate,
not merely the number of plugins or Marketplace listings.

## Possible standardization moat

One long-term thought worth preserving, but far too early to claim, is that the
strongest ecosystem signal would not be "Tachiko has many UI features." It would
be third-party demand for compatibility with Tachiko semantic artifacts and
operations.

A hypothetical mature-market question might be:

> "Is this workflow / asset / tool Tachiko-compatible?"

That would imply the format/runtime/semantic contract has become useful beyond the
first-party application. Current protocol, conformance, licensing, and trademark
decisions do not justify such a claim today. The strategic point is only that open
interoperability could eventually become a moat rather than merely a concession.

## What should remain outside GitHub Issues for now

These useful ideas are intentionally kept here rather than promoted into separate
backlog items:

- ranking Microsoft/Google/Apple/LibreOffice/Notion/Obsidian/Figma/dbt/Shopify/
  Atlassian as business-model analogues;
- the metaphor that the historical "compiler" from domain knowledge to software
  was a team of people;
- the broad `knowledge → software` thesis;
- the phrase `semantic work substrate`;
- the six-sided ecosystem flywheel;
- speculation about future overlap with low-code, ERP, CRM, data, workflow, or
  enterprise platforms;
- the idea that compatibility itself may become a standards/network moat.

These may guide future research but do not need Issues until they produce a
concrete question, bounded evidence need, decision owner, or implementation
dependency.

## What would falsify or narrow the thesis?

The strategy should become narrower if evidence repeatedly shows that:

- materially different verticals require incompatible domain-specific semantic
  engines;
- generic composition creates a hidden, unreadable programming language;
- domain-specific UI cannot become excellent without duplicating semantic truth;
- asset composition creates unmanageable precedence, migration, dependency, or
  trust complexity;
- local/open project ownership prevents a sustainable business rather than
  strengthening adoption;
- enterprise-grade transaction, authorization, compliance, or scale needs cannot
  be separated cleanly from the common semantic substrate;
- AI-assisted modeling produces too much ambiguity or review cost to beat normal
  application development for real users.

Negative evidence is valuable. The product should not stretch the semantic core
merely to preserve an elegant platform story.

## Promotion rule

Promote material from this memo only when a concrete evidence path exists:

```text
discussion hypothesis
        ↓
focused Research / Strategy / Decision Issue
        ↓
prototype / user / implementation / external evidence
        ↓
Accepted ADR / policy / product document / specification when justified
```

Do not promote concepts merely because they sound strategically attractive.

## Related records

- #15 — licensing and commercial/open-source boundary
- #18 — Japan enterprise pain points and gradual migration from Excel
- #67 — presentation/projection boundary
- #104 — Project Memory as a domain model over generic primitives
- #134 — legacy Office/VBA migration and private enterprise extensions
- #135 — public extension distribution/signing/compatibility/support
- #176 — ecosystem contribution primitives and creator ladder
- #177 — reusable project/template asset contract
- #178 — declarative semantic asset-pack boundary
- #179/#180 — reusable Skills and agent hierarchy
- #181/#182 — discovery, creator identity, and creator economics
- #183 — service/consultant/integrator/partner model
- #184/#185 — cold-start and Git-hosted reusable-asset bootstrap
- #192 — vertical applications as domain solutions over the semantic platform
