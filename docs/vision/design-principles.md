# Tachiko Work Design Principles

These principles translate the project's founding motivation into durable product and architecture guidance.

They are intentionally higher-level than implementation details. When a short-term feature request conflicts with them, the conflict should be made explicit rather than silently weakening the core direction.

## 1. User Ownership Before Application Ownership

People should own their work independently of the application that created it.

Tachiko Work should make it practical to inspect, version, migrate, automate, preserve, and extend documents and data without requiring permanent dependence on one vendor, one UI, or one historical file format.

Open formats are not merely an import/export feature. They are part of the product's reason for existing.

## 2. Not an Office Clone

Tachiko Work is not a Rust rewrite of Microsoft Office or LibreOffice.

Familiar Office-like workflows may be useful, especially for progressive adoption, but they should remain views over a more modern foundation rather than define that foundation.

The goal is a semantic workspace that can represent documents, spreadsheets, structured data, and computation without inheriting every historical constraint of the tools it interoperates with.

## 3. Semantic Core First

The core model stores meaning, not historical implementation accidents.

Semantic architecture is a means, not a slogan. It is justified when it enables stable identity, structured references, computation, validation, meaningful diff and merge, migration, multiple views, or AI reasoning.

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

Markdown, visual editing, spreadsheet interfaces, structured-data views, domain-specific tools, and AI operations may be different projections of shared semantic structures.

Not every surface must expose every primitive, and not every document type must be forced into one visual metaphor.

The shared model exists to preserve meaning across views, not to make all views identical.

## 7. Git as Storage and Collaboration Protocol

Git should be able to understand persisted work as structured, deterministic data.

Users should not need to manually edit Git representations, but the format and tooling should support:

- deterministic serialization
- meaningful textual changes where practical
- semantic diff
- semantic merge
- reviewable history
- CI and automation

Git is a storage and collaboration protocol, not the user interface.

## 8. AI-Native Architecture

AI should operate on typed semantic objects and explicit capabilities instead of depending on simulated mouse and keyboard actions.

The system should make important operations addressable, permissionable, reviewable, and reversible where practical.

AI should benefit from the same structured meaning that supports humans, version control, validation, and automation.

## 9. Small Stable Core, Replaceable Surroundings

The project should keep the set of hard-to-change invariants as small as possible.

Only decisions with strong evidence and high switching cost should become stable core contracts.

Editors, importers, exporters, engine integrations, AI providers, workflow features, and other application-level capabilities should remain replaceable or extensible whenever possible.

Core hardening is not an attempt to predict every future feature. It is an attempt to make the few truly foundational contracts reliable while preserving escape hatches everywhere else.

## 10. Correctness and Explainability Over Hidden Magic

When Tachiko Work transforms, migrates, validates, computes, or merges user work, important behavior should be explainable.

The system should prefer explicit semantics, deterministic behavior, inspectable transformations, and actionable diagnostics over opaque convenience that makes future debugging or migration impossible.

## 11. Open Ecosystem Over Monolithic Ownership

Tachiko Work should be capable of supporting extensions, adapters, integrations, and domain-specific tools without requiring every useful capability to be implemented by the core project.

The long-term goal is not a single giant application that owns every workflow.

It is an open foundation on which many workflows can be built.
