# Rendering System Direction

Decision state: Hypothesis; future Designer MVP direction

Implementation state: Not implemented

Tracking: #67

## Principle

Rendering is a projection of semantic content.

The document model should not depend on a specific renderer, and renderer-specific state should not become the owner of semantic truth merely because a user ultimately needs a visual artifact.

The durable product direction is therefore not "one canonical Word/Excel/PowerPoint file." It is a shared semantic foundation that can support multiple useful projections where the underlying meaning is genuinely shared.

## Projection Model

Conceptually:

```text
Semantic Knowledge / Document Graph
              |
              +-- Document projection
              +-- Table / spreadsheet projection
              +-- Report projection
              +-- Presentation projection
              +-- Dashboard projection
              +-- AI context
```

These projections do not imply that every visual property or application-specific feature belongs in the semantic core.

The exact generic projection abstraction, if any, remains an Open Question for later design work. Do not freeze presentation, layout, or renderer primitives during Core & Format Hardening.

## Research-to-Presentation Dogfooding Case

Issue #67 records a concrete workflow discovered while building the Tokyo Mogu Mogu project:

```text
Official / Open Data sources
           ↓
AI-assisted research
           ↓
Reviewed semantic knowledge in Git
           ↓
Presentation projection
           ↓
PPTX / PDF / HTML / external presentation adapter
```

Today, the final transition is commonly performed by manually copying reviewed facts, metrics, claims, citations, and conclusions into a presentation. That creates a second materialization that can drift from the reviewed source of truth.

The product hypothesis is that Tachiko Work can eventually let users create and review knowledge once, then project it into a presentation without turning the exported deck into an independent semantic source of truth.

## Candidate Semantic Responsibilities

These are hypotheses and illustrative concepts, not required v1 object types:

- stable references to reusable facts, datasets, metrics, claims, evidence, and insights;
- dependency relationships between source knowledge, derived charts, narrative claims, and output views;
- provenance needed to trace rendered content back to reviewed sources;
- projection intent and semantic narrative structure where that meaning is renderer-independent;
- impact analysis when a source value or claim changes;
- deterministic generation where practical and where the target renderer permits it.

For example:

```text
Metric changed
   ↓
Chart affected
   ↓
Claim affected
   ↓
Presentation slides 4, 7, 11 affected
```

The ability to perform this safely depends on semantic identity, references, canonical persistence, and deterministic serialization rather than on a presentation file format alone.

## Renderer / Adapter Boundary

Tachiko Work should own semantic truth and reusable relationships.

Renderers or adapters should own target-specific realization such as:

- typography;
- exact positioning and layout;
- animations and transitions;
- host-application capabilities;
- renderer-specific metadata that has no cross-view semantic meaning.

A PPTX, Google Slides, Canva, PDF, or HTML integration should therefore be an adapter or materialization boundary, not the architectural center of the system.

This boundary is intended to prevent Tachiko Work from becoming a reimplementation of Microsoft Office while still allowing high-quality Office-like outputs.

## Possible Targets

Illustrative future targets include:

```text
Semantic Model
      |
      +-- Web Renderer
      +-- Desktop Renderer
      +-- PDF Renderer
      +-- Markdown Renderer
      +-- DOCX Renderer
      +-- Presentation Renderer
             |
             +-- PPTX adapter
             +-- HTML / Reveal.js adapter
             +-- external presentation-system adapter
```

Target selection is not decided here. Issue #67 owns the future validation work for presentation projection.

## Provenance and Citations

Presentation projection becomes substantially more valuable when rendered claims and metrics preserve traceability to their reviewed origins.

A future provenance chain may conceptually support:

```text
Slide / rendered block
        ↓
Claim or metric
        ↓
Dataset / evidence
        ↓
Original source metadata
        ↓
Git history / reviewed revision
```

This could enable generated citations, stale-data detection, and auditability without making citation formatting itself a semantic-core concern.

The exact provenance model remains unresolved and must be coordinated with semantic identity and future AI-operation provenance work.

## Requirements

Long-term rendering work should preserve these goals where applicable:

- semantic truth remains independent of a single renderer;
- deterministic output where practical;
- accessibility support;
- Unicode-first design;
- international text support;
- reusable rendering primitives;
- provenance and dependency traceability where projections reference shared knowledge;
- graceful degradation when a target format cannot express a semantic or visual feature exactly.

## Non-Goals

This direction does not mean:

- Git itself becomes a presentation format;
- Tachiko Work must reproduce the entire PowerPoint editing model;
- every slide-layout property becomes a semantic primitive;
- presentation support is part of `.ro` / `.roproj` v1 hardening;
- renderer-specific state automatically becomes canonical.

## Relationship to Core & Format Hardening

Presentation projection is future product work, but it explains why several current foundation decisions matter beyond game-development tables.

In particular, trustworthy cross-view projection depends on the foundations being hardened by:

- #21 semantic identity / document graph;
- #25 storage DTO / serialization boundary;
- #37 format/version envelope;
- #38 canonical value encoding / deterministic ordering.

Milestone 02 should preserve the architectural seams needed by future projections without prematurely designing presentation-specific contracts.

## Long-Term Goal

The same semantic knowledge should be able to move between useful surfaces such as:

- visual editor;
- Markdown or document view;
- table / spreadsheet view;
- report;
- presentation;
- printed/PDF artifact;
- collaborative workspace;
- AI-generated or AI-consumed context;

without requiring each surface to become a separate, drifting source of truth.
