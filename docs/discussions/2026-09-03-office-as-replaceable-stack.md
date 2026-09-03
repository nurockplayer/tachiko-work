# Office as a Replaceable Stack

Status: Discussion history / founder architecture hypothesis. **Not authoritative by itself.**

Captured: 2026-09-03 JST

## Why this note exists

A useful mental model emerged from ordinary backend engineering experience.

A server can look, from the outside, like one machine doing one job. Once the system is built and operated, the apparent single thing separates into layers with different responsibilities and replacement boundaries:

```text
client
  ↓
web / reverse proxy
Nginx / Apache / Caddy
  ↓
application runtime
Python / Go / Rust / Node
  ↓
persistence / services
PostgreSQL / MySQL / SQLite / cache / queue / object storage
```

The important lesson is not that every implementation is interchangeable. It is that the system becomes easier to evolve when responsibilities stop being welded into one indivisible product.

The same inversion is useful when thinking about Office-style software.

From a user's point of view, Excel, Word, or PowerPoint can appear to be one application. Internally, however, such a product bundles many different concerns: user interface, document or spreadsheet representation, calculation, validation, automation, persistence, import/export, collaboration, integration, and compatibility behavior.

Tachiko Work should pressure-test whether structured work can be treated more like a software stack than an application monolith.

## The working hypothesis

> **Office-style products should be decomposable into product surfaces, semantic contracts, runtime behavior, and host/adaptor layers, so that surrounding implementations can evolve without creating competing semantic truths.**

In shorter form:

> **Turn Office from an application monolith into a layered work stack.**

This is explanatory founder framing, not a new Accepted architectural contract. The durable authority remains the Product Constitution, Design Principles, Accepted ADRs, and specifications.

## Conceptual decomposition

```text
Product experiences
spreadsheet │ document │ presentation │ inventory │ CRM │ other domain UI
                              │
                              ▼
Client / interaction boundary
native │ web │ mobile │ CLI │ AI │ automation
                              │
                              ▼
First-class Semantic API
same operations │ same validation │ same authority rules
                              │
                              ▼
Tachiko semantic/application runtime
identity │ types │ relationships │ formulas │ validation
operations │ revision/history │ diff/conflict │ explanation
                              │
                              ▼
Host / adapter boundaries
filesystem │ browser storage │ Git │ import/export │ network │ external systems
```

This decomposition is already directionally consistent with existing repository authority:

- `docs/vision/design-principles.md` says Tachiko is not an Office clone, supports multiple views over shared meaning, keeps a small stable core with replaceable surroundings, and prefers an open ecosystem over monolithic ownership.
- `docs/architecture/frontend-backend-boundary.md` makes the UI a projection/client layer rather than the owner of semantic truth.
- ADR-0020 establishes a first-class headless Semantic API boundary.
- ADR-0022 separates the resident semantic/application runtime from host composition.
- Issue #234 and `docs/architecture/postgresql-like-engine-spreadsheet-interface.md` already frame the spreadsheet as a privileged client of the engine rather than the engine itself.

The new value of this note is the **stack analogy**: it explains why those decisions belong together.

## Excel, Word, and PowerPoint become product surfaces, not sovereign engines

The hypothesis does not require every Office-shaped experience to use the same UI, component library, or interaction model.

For example:

```text
Spreadsheet product
  dense grid, formulas, filtering, aggregation

Document product
  narrative editing, structure, references, review

Presentation product
  spatial composition, sequencing, speaker-oriented workflow

Inventory product
  stock-centric workflows, alerts, forms, operational views
```

These products may need very different frontend architectures and visual languages.

A shared React component library is optional.

A shared semantic authority, where the underlying meaning is genuinely shared, is not optional.

That distinction matters because "one platform" must not accidentally become "one frontend forced to serve every domain." A specialized product should be allowed to have a specialized frontend.

The desired relationship is closer to:

```text
many applications
      ↓
one governed semantic/runtime foundation where applicable
```

than to:

```text
one universal UI
      ↓
every possible workflow
```

## Replaceable seams, not arbitrary interchangeability

The web-stack analogy is useful only if its limits remain explicit.

Tachiko should not infer that every layer must support arbitrary hot-swapping or that all implementations are equivalent.

Examples:

- A native and web frontend may use different rendering technology while remaining Semantic API clients.
- A domain-specific frontend may expose a completely different interaction model while preserving the same semantic operation meaning where it shares state with other clients.
- Import/export adapters may evolve independently without making XLSX or DOCX the semantic ontology.
- Host technologies may differ where Accepted contracts permit it.
- Extensions and connectors may add optional capability without redefining core truth.

But the analogy does **not** currently authorize:

- a generic database-backend abstraction merely so PostgreSQL, MySQL, and SQLite can be swapped;
- replacing `.roproj` durable representation authority established elsewhere;
- treating every Office feature as a plugin;
- making every frontend implementation interchangeable;
- creating separate semantic engines for spreadsheet, document, and presentation products;
- exposing internal Rust implementation details as a public extension ABI; or
- declaring that all document/presentation semantics can already be represented by one proven model.

The right principle is narrower:

> **Preserve replaceable seams where responsibilities genuinely differ, while keeping the few authoritative invariants explicit and small.**

## Why this changes the Excel question

This framing sharpens Issue #240's research question.

If Tachiko were trying to reproduce Excel as one monolithic application, the natural failure mode would be feature-parity thinking: every Excel capability eventually becomes a first-party implementation obligation.

A layered product can instead separate several questions:

```text
Does ordinary spreadsheet work need this natively?
        ↓ yes
first-class spreadsheet product capability

Is it important only for existing workbook migration?
        ↓ yes
compatibility / conversion boundary

Is it valuable mainly to specialists?
        ↓ maybe
Enthusiast / Builder surface, reusable asset, Skill, connector, or extension

Does it define universal semantic meaning?
        ↓ only with strong evidence
core / Semantic API consideration
```

This does not predetermine #240's research results. Usage evidence still decides which Excel capabilities belong in the native Driver experience.

It does, however, remove the assumption that every useful Excel feature must eventually become core Tachiko behavior.

## Why this changes the frontend question

For frontend contributors, the architecture can be explained without requiring them to understand every internal semantic mechanism:

```text
Build the best product surface for the job.

Do not build a second source of truth.

Use the shared semantic contract for product meaning.

Keep presentation and interaction choices free to evolve.
```

That leaves room for multiple independently designed products:

```text
Excel-like frontend
Word-like frontend
presentation frontend
inventory frontend
CRM frontend
future experimental interfaces
```

They can diverge visually and technically. What they must not do is silently fork the meaning and rules of the same semantic state.

## Relationship to stock-simple, tuner-deep

Issue #215's Driver / Enthusiast / Builder model remains a hypothesis whose promotion is currently blocked pending Tachiko product evidence.

The stack model helps explain how that hypothesis could work without turning ordinary users into platform engineers:

```text
Driver
uses a complete product surface

Enthusiast
inspects and modifies deeper declarative structure

Builder
creates new surfaces, integrations, reusable assets, or bounded extensions

all operate over the same authoritative foundation where applicable
```

A Driver should not need to know the stack exists.

A Builder should not discover that the hood was welded shut.

## Architecture smell: the Office-monolith test

When reviewing a feature, a useful non-normative question is:

> **Are we adding this capability to the semantic foundation because every conforming client truly needs the invariant, or because the current application happens to bundle that behavior today?**

If the answer is the latter, the capability may belong in a product surface, domain solution, adapter, Skill, connector, extension, or host layer instead of the semantic core.

A second useful question is:

> **Could another frontend solve the same user job differently without changing the underlying meaning?**

If yes, UI structure should probably remain replaceable.

## What would falsify or narrow the hypothesis?

The framing should become narrower if real implementation and product evidence shows that:

- materially different products repeatedly require incompatible semantic authorities;
- the common contract becomes so broad that it is effectively another monolith;
- separating layers adds more compatibility and lifecycle cost than it removes;
- domain-specific UX cannot remain excellent without duplicating or bypassing semantic truth;
- extension boundaries become an unmaintainable hidden programming language; or
- ordinary workflows become slower or harder because architecture purity outranks product quality.

The goal is not decomposition for its own sake.

The goal is to let product surfaces evolve freely while protecting user-owned, inspectable, reusable meaning underneath.

## Promotion boundary

Do not promote this note into a new design principle merely because the analogy is memorable.

The current repository already contains the likely durable principles:

- Not an Office Clone
- Multiple Views, Shared Meaning
- Small Stable Core, Replaceable Surroundings
- Open Ecosystem Over Monolithic Ownership
- Generalize From Real Pressure

This note should remain explanatory history until Tachiko's own product evidence justifies stronger canonical wording.

## Related records

- `docs/vision/product-constitution.md`
- `docs/vision/design-principles.md`
- `docs/architecture/frontend-backend-boundary.md`
- `docs/architecture/postgresql-like-engine-spreadsheet-interface.md`
- ADR-0020: first-class headless Semantic API
- ADR-0022: resident semantic runtime and host boundary
- #18: gradual Office migration
- #192: domain solutions over the semantic platform
- #215: stock-simple, tuner-deep product model
- #234: PostgreSQL-like semantic engine with spreadsheet-first interface
- #240: Excel high-frequency feature scope
