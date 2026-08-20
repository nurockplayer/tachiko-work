# Tachiko Work Founding Discussion History

Date: 2026-08-20

## Overview

This document records the evolution of the Tachiko Work thesis during the initial architecture and product discussions.

The project started from a question:

> Can a Rust-based open-source Office platform challenge Word and Excel?

The conclusion evolved into a different thesis:

> Tachiko Work is not an Office clone. It is a semantic document and computational workspace platform.

## Phase 1: Office replacement analysis

Initial investigation focused on:

- Microsoft Word and Excel dominance
- LibreOffice as the largest open-source desktop Office suite
- The difficulty of rewriting Office software
- Whether Rust and AI coding agents make a new implementation realistic

Key conclusion:

A direct LibreOffice or Microsoft Office rewrite is not the right goal.

The difficult parts are not only UI and file parsing. The true challenges are:

- layout engines
- spreadsheet semantics
- compatibility behavior
- international text handling
- deterministic rendering
- round-trip fidelity

## Phase 2: New platform thesis

The project direction changed from:

"Build an open-source Office clone"

into:

"Build a new semantic document platform where Office formats are only import/export formats."

Core principles:

- The semantic model owns meaning.
- File formats do not define the internal architecture.
- Legacy behavior belongs at the compatibility boundary.
- AI should operate on semantic objects instead of simulating user interaction.

## Phase 3: Legacy compatibility philosophy

A major decision was made around historical Excel behavior.

Example:

The Excel 1900 leap-year bug should not become part of the new platform's internal model.

Correct approach:

Import legacy files.
Detect historical dependencies.
Generate migration reports.
Convert to modern semantics.

Legacy problems should live in conversion tooling, not in the next hundred years of core architecture.

## Phase 4: Unified semantic model

Documents, spreadsheets, Markdown, charts, formulas, and structured data should share one underlying model.

Possible primitives:

- Document blocks
- Tables
- Records
- Formulas
- References
- Assets
- Views
- History
- Agent operations

Word, Excel, Markdown, and HTML become projections of the same model.

## Phase 5: Game development as first market

The strongest initial pain point was discovered in game development.

Current workflow:

Designer
→ Excel / Google Sheets
→ CSV export
→ Engine import
→ Git separation

Problems:

- binary spreadsheet files are difficult to diff
- merges are painful
- code and data branches diverge
- design changes lack reviewability

Tachiko Work opportunity:

A spreadsheet that belongs in the Git repository.

Features:

- typed schemas
- formulas
- semantic diff
- semantic merge
- validation
- CI checks
- Unity / Unreal / Godot integration

## Phase 6: Git-native format

The project explored .ro and .roproj.

Final direction:

They are not competing formats.

They are two representations of the same semantic model.

.ro

Portable package format.

.roproj/

Git working representation.

The working tree should be:

- deterministic
- canonical
- UTF-8
- human readable
- Git diff friendly

Git is a storage and collaboration protocol, not the user interface.

## Phase 7: Business model

The conclusion was that open source can be commercially viable.

Possible revenue:

- managed cloud
- team collaboration
- enterprise deployment
- migration services
- AI services
- support
- marketplace
- commercial licensing

The moat should not only be code.

Important assets:

- ecosystem
- format adoption
- integrations
- community
- trust

## Final thesis

Tachiko Work aims to create an open computational document ecosystem.

The first product is not an Office replacement.

The first product is a Git-native computational data workflow for game development.

From there the platform can expand toward:

- technical documentation
- AI knowledge work
- scientific workflows
- operational data
- general productivity
