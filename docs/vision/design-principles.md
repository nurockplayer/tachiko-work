# Tachiko Work Design Principles

## 1. Not an Office Clone

Tachiko Work is not a Rust rewrite of Microsoft Office or LibreOffice.

The goal is a new semantic workspace that can represent documents, spreadsheets, structured data, and computation.

Office is an interoperability target, not the ontology of the core model.

See [Why Tachiko Work Has a Semantic Core](../architecture/semantic-core-rationale.md) for the architecture derivation behind this principle.

## 2. Semantic Core First

The core model stores meaning, not historical implementation accidents.

Legacy formats such as DOCX and XLSX are compatibility boundaries.

Presentation coordinates, storage paths, and physical serialization layout must not become durable semantic identity.

## 3. Legacy Compatibility at the Boundary

Historical bugs should not become permanent architecture.

Example:

- Excel 1900 leap-year behavior should be detected during migration.
- The conversion layer may preserve or explain old behavior.
- The internal model should use correct modern semantics.

Migration and compatibility are product requirements, but they should be implemented through explicit adapters and migration policy rather than by making legacy behavior the source of truth.

## 4. Multiple Views, One Model

Markdown, visual editing, spreadsheets, document-like editing, APIs, and AI operations should be views or projections over the same semantic structure.

Presentation-specific state may differ between views. It must not silently redefine the identity or meaning of the underlying semantic objects.

## 5. Git as Storage and Collaboration Protocol

Git should understand documents as structured data.

Users should not need to manually edit Git representations, but the format and semantic model should support:

- deterministic ordering and canonical persistence
- human-readable, reviewable changes
- semantic diff
- semantic merge
- stable identity across harmless presentation changes

Git is infrastructure for history and review, not the end-user interface.

## 6. AI-Native Architecture

AI should operate on typed semantic objects and semantic operations instead of controlling a UI through simulated actions.

The same semantic behavior should be reusable by AI, CLI, native, WASM, and future graphical clients where applicable.

AI mutation remains subject to explicit permission and human-review policy defined by ADR-0007 and later safety decisions.

## 7. Small Core, Strong Semantics

Keep the semantic kernel as small as practical, but do not make it semantically empty.

The core should own only the invariants that every Tachiko Work capability must agree on. Replaceable concerns such as UI technology, external formats, cloud providers, game-engine integrations, AI providers, collaboration mechanisms, and plugin hosts should remain outside the core unless evidence proves otherwise.

A useful review question is:

> If this capability were removed or replaced, would the remaining semantic model still be Tachiko Work?

If yes, the capability probably does not belong in the semantic kernel.

The reverse is also true: stable identity, reference meaning, and other fundamental semantic rules must not become independently redefined by extensions.

## 8. Lock Invariants, Defer Mechanisms

Early architecture should lock high-confidence invariants and preserve replacement space for lower-confidence mechanisms.

Examples of high-confidence invariants include:

- semantic authority is independent of UI technology;
- presentation coordinates and storage layout are not semantic identity;
- persistence representation does not define the domain model;
- equivalent semantic state needs deterministic canonical persistence behavior;
- legacy compatibility belongs at boundaries;
- shared semantic operations must not depend on one graphical client.

Examples that should remain replaceable until stronger evidence exists include the final ID-generation algorithm, complete formula language, plugin ABI, collaboration protocol, final graphical component model, and final public file-format syntax or name.

Do not solve hypothetical future requirements by prematurely freezing public contracts.

## 9. Progressive Migration, Not Big-Bang Replacement

Tachiko Work should create value before an organization abandons its existing Office assets.

Excel, ODF, CSV, DOCX, and other legacy representations may remain inputs, outputs, or familiar surfaces while Tachiko Work progressively takes over semantic validation, computation, Git review, migration, automation, and AI-assisted understanding.

The strategic goal is to reduce dependency on legacy document ecosystems over time, not to require an all-at-once migration before the first benefit appears.

## 10. Generalize From Real Pressure

Do not begin with a universal ontology or abstraction intended to model every possible form of work.

Start with concrete workflows, introduce the minimum semantic abstraction they require, test it against additional domains, and generalize only when evidence demands it.

A model that cannot survive real domain pressure should be revised through explicit ADRs or reconciliation rather than defended for elegance.
