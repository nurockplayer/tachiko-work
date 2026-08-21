# Tachiko Work Origin Discussion

Date: 2026-08-20

## Origin

The initial question was whether it was realistic to create an open-source Office alternative using Rust and AI coding agents.

The discussion evolved away from building a Rust clone of Microsoft Office or LibreOffice.

The conclusion became:

> Tachiko Work is not an Office clone. It is a semantic document and computational workspace platform.

## Evolution

### 1. Office replacement question

Microsoft Word and Excel are the dominant productivity formats. LibreOffice is the largest community-driven open-source Office suite.

However, directly replacing Office means inheriting decades of historical compatibility problems.

### 2. Semantic core idea

The platform should store meaning instead of historical file-format accidents.

Documents, spreadsheets, Markdown, structured data, formulas, charts, and AI operations should be different views over a shared semantic model.

### 3. Legacy compatibility boundary

Historical compatibility problems should be handled by migration tools and import/export adapters.

Example:

Excel 1900 leap-year behavior should not exist inside the modern calculation engine.

The system should:

- detect legacy behavior
- explain migration impact
- provide conversion tools
- keep the modern core clean

### 4. Game development wedge

A major pain point was identified in game development:

- designers use Excel or Google Sheets
- code lives in Git
- balancing data cannot be properly version controlled
- teams export CSV as a workaround

The first product opportunity:

> The spreadsheet that belongs in your Git repository.

Features:

- typed schemas
- semantic diff
- semantic merge
- balance validation
- CI integration
- Unity / Unreal / Godot adapters

### 5. Long-term platform

The game development use case is a starting point, not the final destination.

The long-term goal is a unified platform combining:

- Office documents
- spreadsheets
- Markdown workflows
- computational documents
- AI-native editing
- Git-native collaboration

## Core philosophy

Legacy formats are accepted at the boundary, never inherited by the core.

Git is a storage/versioning protocol, not the user interface.

## Follow-up architecture rationale

A 2026-08-21 follow-up discussion pressure-tested this direction against several questions:

- whether Microsoft Office or LibreOffice could implement the same capabilities;
- whether LibreOffice's Office-compatible direction is actually wrong or simply solving a different problem;
- whether Tachiko Work should use a small-core / extension-oriented architecture;
- how to avoid freezing the wrong Milestone 02 decisions too early;
- whether the semantic direction was chosen first or derived from concrete product requirements.

The result did not change ADR-0001. It clarified why the existing semantic-first decision follows from game-development data, Git-native review, AI-native operations, multiple projections, and progressive Office migration.

The detailed derivation and explicit revisit conditions are recorded in [Why Tachiko Work Has a Semantic Core](../architecture/semantic-core-rationale.md). The corresponding architecture guardrails are summarized in [Design Principles](../vision/design-principles.md).
