# Tachiko Work Product Constitution

Status: Foundational

This document defines the highest-level constraints that should guide product, architecture, research, and implementation decisions.

It is intentionally small. Tachiko Work should not turn every current preference into constitutional law.

## 1. Why Tachiko Work Exists

Tachiko Work exists so that people can create, understand, version, migrate, automate, compute over, and preserve their work without permanent dependence on a single application vendor or historical document format.

The founding motivation came from dissatisfaction with deep dependence on Microsoft Office and OOXML, reinforced by the open-document ideas encountered through Italo Vignoli's COSCUP 2017 talk and by direct software-development experience with spreadsheets, game data, Git workflows, and lightweight collaborative tools.

The project is therefore fundamentally about user ownership and better foundations for digital work, not about reproducing an existing office suite in Rust.

## 2. Constitutional Principles

### 2.1 Users own their work

The application must not become the only practical way to understand or preserve the user's data and documents.

### 2.2 Meaning outranks representation

Historical file representations may be important for interoperability, but they must not automatically define the internal semantic model.

### 2.3 Compatibility belongs at explicit boundaries

Legacy behavior should be detected, preserved, emulated, converted, or explained deliberately. It should not silently become permanent core architecture.

### 2.4 Migration must be progressive

Tachiko Work should provide bridges from existing Office and spreadsheet workflows. Adoption should not require a flag-day rewrite of an organization's working documents.

### 2.5 Versionability is a first-class property

Meaningful changes should be inspectable, reviewable, reproducible, and automatable. Git is an important protocol and proving ground for this property, even when end users never interact with Git directly.

### 2.6 AI operates on capabilities and meaning

AI should manipulate explicit semantic objects and operations rather than depend on UI simulation as the primary architecture.

### 2.7 The stable core stays small

Only high-confidence, expensive-to-reverse invariants should become stable core contracts. Other capabilities should preserve replaceable seams and extension points.

### 2.8 The ecosystem may be larger than the application

Editors, adapters, importers, exporters, domain integrations, AI providers, and workflow extensions should be able to evolve without requiring the core project to own every use case.

### 2.9 Practical spreadsheet interoperability is first-class

Decision issue: [#275](https://github.com/nurockplayer/tachiko-work/issues/275)

Tachiko Work must provide practical interoperability with established spreadsheet workflows, especially Microsoft Excel. Architectural independence from Excel, OOXML, or legacy spreadsheet behavior must not be used to justify poor real-world interoperability.

When compatibility conflicts with modern internal semantics, Tachiko Work should preserve the modern semantic model while providing explicit boundaries that can import, export, translate, emulate, preserve, or explain legacy behavior as appropriate.

Historical Tachiko Work implementation choices do not gain permanent compatibility protection merely because they already exist. When an early design materially obstructs an Accepted interoperability requirement, prefer an explicit migration or supersession path over institutionalizing the mistake, while protecting user data and durable external contracts through the normal compatibility process.

## 3. What Tachiko Work Is Not

Tachiko Work is not constitutionally committed to:

- recreating Microsoft Office feature by feature
- perfect OOXML fidelity inside the semantic core
- exposing Git as the primary end-user interface
- forcing every document type into a spreadsheet or block-editor metaphor
- making every current implementation choice permanent
- replacing Excel or Word overnight
- treating the game-development wedge as the final market boundary

## 4. Current Strategic Shape

The current product strategy uses game development as the first proving ground because the pain is concrete:

- structured design data lives in spreadsheets
- source code lives in Git
- changes need review, validation, merge, CI, and engine integration

This wedge is useful because it tests the deeper thesis under demanding engineering conditions.

Success in this wedge should validate reusable foundations, not narrow the long-term mission to game development.

## 5. Decision Classification

Project discussions should distinguish between different levels of commitment.

### Foundational Principle

A durable statement about why the project exists or what it must protect.

Changing one requires explicit reconsideration of the product mission.

### Accepted Direction

A direction with strong current confidence, but whose implementation may evolve.

### Provisional Decision

A practical choice made to unblock current work. It should retain an escape hatch where possible.

### Hypothesis

An idea that still requires research, prototyping, market evidence, or technical validation.

### Open Question

A known decision gap that should not be silently resolved by implementation convenience.

### Superseded Direction

A previous direction retained in history so future contributors can understand why it changed.

## 6. Architecture Change Rule

When a decision is expensive to reverse, ask:

1. Is this truly a semantic or persistence invariant?
2. What external data, plugins, Git history, or APIs will depend on it?
3. Can the same requirement be placed behind an adapter or extension boundary?
4. What evidence justifies freezing it now?
5. What migration path exists if the assumption later proves wrong?

The default should be to freeze less, not more.

## 7. Research and Implementation Rule

Important unresolved questions should normally flow through:

Canonical context
→ focused research
→ explicit recommendation
→ ADR or specification when needed
→ implementation-ready issue
→ Codex implementation
→ tests and review
→ revision when evidence changes

The purpose of documentation is not to eliminate iteration.

It is to make iteration cumulative instead of repeatedly rediscovering the same reasoning.

## 8. Relationship to Other Documents

- `mission.md` explains the purpose and long-term vision.
- `design-principles.md` translates the constitution into product and architecture guidance.
- `docs/discussions/` preserves reasoning history and the evolution of the thesis.
- ADRs record concrete architectural decisions.
- Specs define implementable contracts.
- GitHub Issues track unresolved work and research questions.

When these documents appear to conflict, the conflict should be reconciled explicitly rather than resolved by whichever file was edited most recently.
