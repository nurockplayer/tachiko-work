# Tachiko Work Design Principles

These principles translate the project's founding motivation into durable product and architecture guidance.

They are intentionally higher-level than implementation details. When a short-term feature request conflicts with them, the conflict should be made explicit rather than silently weakening the core direction.

The detailed derivation behind the semantic-first direction is recorded in [Why Tachiko Work Has a Semantic Core](../architecture/semantic-core-rationale.md). That rationale is explanatory; Accepted ADRs and the Product Constitution remain the authority for concrete commitments.

## 1. User Ownership Before Application Ownership

People should own their work independently of the application that created it.

Tachiko Work should make it practical to inspect, version, migrate, automate, preserve, and extend documents and data without requiring permanent dependence on one vendor, one UI, or one historical file format.

Open formats are not merely an import/export feature. They are part of the product's reason for existing.

## 2. Not an Office Clone

Tachiko Work is not a Rust rewrite of Microsoft Office or LibreOffice.

Familiar Office-like workflows may be useful, especially for progressive adoption, but they should remain views over a more modern foundation rather than define that foundation.

The goal is a semantic workspace that can represent documents, spreadsheets, structured data, and computation without inheriting every historical constraint of the tools it interoperates with.

Office is an interoperability target, not the ontology of the semantic core.

## 3. Semantic Core First

The core model stores meaning, not historical implementation accidents.

Semantic architecture is a means, not a slogan. It is justified when it enables stable identity, structured references, computation, validation, meaningful diff and merge, migration, multiple views, or AI reasoning.

Presentation coordinates, storage paths, and physical serialization layout must not silently become durable semantic identity.

The project should avoid inventing semantic structure where it adds no practical value.

## 4. Legacy Compatibility at the Boundary

Legacy formats such as DOCX and XLSX are compatibility boundaries, not architectural authorities.

Historical bugs should not automatically become permanent core semantics.

For example:

- Excel's 1900 leap-year behavior should be detected during migration.
- The conversion layer may preserve, emulate, or explain old behavior when compatibility requires it.
- The modern internal model should not inherit a legacy bug merely because an importer encounters it.

Compatibility tooling should create escape paths from historical constraints, not reproduce them indefinitely inside the core.

## 5. Progressive Migration Instead of Flag-Day Replacement

Users and organizations should not be required to abandon Excel, Word, or existing document estates all at once.

Tachiko Work should support coexistence and gradual migration:

- import existing work
- inspect and explain dependencies
- preserve meaning where possible
- convert into open and versionable representations
- allow legacy and Tachiko Work workflows to coexist during transition
- reduce dependence on legacy formats over time

Adoption should be a bridge, not a cliff.

## 6. Multiple Views, Shared Meaning

Markdown, visual editing, spreadsheet interfaces, structured-data views, graphs, timelines, domain-specific tools, and AI operations may be different projections of shared semantic structures.

A view is not merely decoration over data. It is an instrument for thinking with shared meaning: a table may make comparison and aggregation easy to see; a graph may expose dependency and relationship structure; a timeline may expose sequence, delay, and duration; a document-like view may make context and rationale easier to understand.

The underlying semantic state remains authoritative while the user changes how that state is selected, arranged, filtered, grouped, or presented. A view may own presentation state such as layout, grouping, ordering, formatting, or interaction state without becoming a competing source of semantic truth.

Not every surface must expose every primitive, and not every document type must be forced into one visual metaphor. Different representations are valuable when they make different questions easier to ask or different relationships easier to perceive.

Before adding a new first-party view, ask:

> What does this view make easier to see, understand, decide, or do that existing views do not?

If there is no meaningful answer, the new surface may be cosmetic variation rather than a useful cognitive instrument.

The shared model exists to preserve meaning across views, not to make all views identical.

## 7. Git as Storage and Collaboration Protocol

Git should be able to understand persisted work as structured, deterministic data.

Users should not need to manually edit Git representations, but the format and tooling should support:

- deterministic serialization
- meaningful textual changes where practical
- semantic diff
- semantic merge
- stable identity across harmless presentation changes
- reviewable history
- CI and automation

Git is a storage and collaboration protocol, not the user interface.

## 8. AI-Native Architecture

AI should operate on typed semantic objects and explicit capabilities instead of depending on simulated mouse and keyboard actions.

The system should make important operations addressable, permissionable, reviewable, and reversible where practical.

AI should benefit from the same structured meaning that supports humans, version control, validation, and automation.

The same semantic behavior should be reusable by CLI, AI, native, WASM, and future graphical clients where applicable rather than being reimplemented as UI-specific logic.

## 9. Small Stable Core, Replaceable Surroundings

The project should keep the set of hard-to-change invariants as small as possible.

Only decisions with strong evidence and high switching cost should become stable core contracts.

Editors, importers, exporters, engine integrations, AI providers, workflow features, collaboration mechanisms, host technologies, and other application-level capabilities should remain replaceable or extensible whenever possible.

Core hardening is not an attempt to predict every future feature. It is an attempt to make the few truly foundational contracts reliable while preserving escape hatches everywhere else.

A useful review question is:

> If this capability were removed or replaced, would the remaining semantic model still be Tachiko Work?

If yes, it probably does not belong in the semantic kernel without stronger evidence.

## 10. Correctness and Explainability Over Hidden Magic

When Tachiko Work transforms, migrates, validates, computes, or merges user work, important behavior should be explainable.

The system should prefer explicit semantics, deterministic behavior, inspectable transformations, and actionable diagnostics over opaque convenience that makes future debugging or migration impossible.

## 11. Open Ecosystem Over Monolithic Ownership

Tachiko Work should be capable of supporting extensions, adapters, integrations, and domain-specific tools without requiring every useful capability to be implemented by the core project.

The long-term goal is not a single giant application that owns every workflow.

It is an open foundation on which many workflows can be built.

## 12. Generalize From Real Pressure

Do not begin with a universal ontology, type system, formula language, plugin ABI, or collaboration mechanism merely because the architecture could support one.

Prefer concrete workflows first, introduce the minimum semantic abstraction they require, pressure-test it against additional domains, and generalize only when evidence demands it.

If evidence invalidates an assumption, revise the model through an explicit Decision Issue/ADR/reconciliation instead of preserving an elegant abstraction by redefining its meaning.
