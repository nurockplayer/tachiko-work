# Tachiko Work Origin Discussion

Date: 2026-08-20
Updated: 2026-08-21

## Why this document exists

This document preserves the founding motivation and the reasoning path that led to Tachiko Work's current direction.

It is intentionally more personal and historical than an ADR or specification. The goal is to preserve not only what the project decided, but why the project exists at all.

## The actual origin

Tachiko Work did not begin as a market exercise around the phrase "semantic workspace."

The founder's strongest original motivation was a desire to help free people from deep dependence on Microsoft Office and OOXML. In deliberately unfiltered terms, the emotional starting point was:

> Liberate people from the Microsoft Office empire and OOXML lock-in.

That motivation was strongly influenced by a COSCUP 2017 talk by Italo Vignoli. The talk helped crystallize the importance of open document ecosystems, user ownership, interoperability, and the long-term cost of allowing a vendor-specific document stack to become the default substrate of human work.

This conviction later combined with the founder's own software-development experience.

Several recurring frustrations became important inputs:

- Game and application data often lives in spreadsheets while source code lives in Git.
- Spreadsheet data is difficult to diff, review, merge, validate, and evolve with code.
- Microsoft Office is powerful, but carries substantial weight, latency, historical complexity, and compatibility baggage.
- HackMD demonstrates the appeal of lightweight, text-oriented, web-native collaboration, but cannot satisfy all of the structured-data, computation, extensibility, version-control, and richer workspace needs behind this project.
- Existing tools repeatedly force people to adapt their work to the assumptions of the application or file format.

These experiences produced the original engineering question:

> If modern productivity software were designed again today, without treating Office's historical constraints as laws of nature, what should the foundation look like?

Rust became the preferred implementation language because the project wanted a modern, efficient, portable, strongly typed foundation with enough control to support native applications, reusable core libraries, and eventually multiple runtimes.

## The first explicit project question

The initial concrete project question was whether it was realistic to create an open-source Office alternative using Rust and AI coding agents.

The discussion then evolved away from building a Rust clone of Microsoft Office or LibreOffice.

The resulting direction became:

> Tachiko Work is not an Office clone. It is a semantic document and computational workspace platform.

This statement describes where the reasoning arrived. It was not the original premise.

## Evolution

### 1. Office replacement question

Microsoft Word and Excel are dominant productivity tools, and LibreOffice is the largest community-driven open-source Office suite.

However, directly replacing Office feature by feature would mean inheriting decades of historical compatibility problems and allowing legacy representation choices to dictate the new architecture.

The project therefore shifted from "rebuild Office" toward "build a better substrate for work."

### 2. Semantic core emerged as an answer

The semantic direction was not chosen as an abstract ideology.

It emerged from repeated requirements that are difficult to satisfy when presentation formats own the truth:

- structured data should be understandable without reverse-engineering a UI file
- formulas should refer to stable meaning
- changes should be reviewable and mergeable
- AI should manipulate concepts rather than simulate mouse and keyboard actions
- multiple editors and views should be able to share the same underlying work
- migration should not permanently contaminate the modern core with legacy accidents

The platform should therefore store meaning instead of historical file-format accidents.

Documents, spreadsheets, Markdown, structured data, formulas, charts, and AI operations can become different views over a shared semantic model where that unification is useful.

### 3. Legacy compatibility boundary

Historical compatibility problems should be handled by migration tools and import/export adapters.

For example, Excel's 1900 leap-year behavior should not automatically become a permanent rule of the modern calculation engine.

The system should be able to:

- detect legacy behavior
- explain migration impact
- preserve compatibility where explicitly required
- provide conversion tools
- keep the modern core clean

### 4. Game development wedge

A concrete pain point from software and game development provided the first strong product wedge:

- designers use Excel or Google Sheets
- code lives in Git
- balance and configuration data cannot be version controlled with the same quality as code
- teams export CSV or build custom pipelines as workarounds

The first product opportunity became:

> The spreadsheet that belongs in your Git repository.

Useful capabilities include:

- typed schemas
- semantic diff
- semantic merge
- balance validation
- CI integration
- engine adapters

Game development is the first proving ground, not the final product boundary.

### 5. Progressive migration, not forced replacement

The goal is not to demand that individuals or organizations abandon Excel, Word, or existing document estates overnight.

A credible path out of lock-in must include bridges.

Tachiko Work should therefore support progressive migration:

1. accept legacy formats at the boundary
2. inspect and explain their semantics
3. convert work into open, versionable representations
4. allow old and new workflows to coexist during transition
5. reduce dependence on legacy tools over time

### 6. Small stable core, extensible ecosystem

The project should not try to freeze every future product decision into the core.

The desired shape is closer to a small framework core with extension points than a monolithic Office suite.

Only high-confidence invariants should become difficult-to-change contracts. Editors, importers, exporters, engine integrations, AI providers, workflow features, and many application-level behaviors should remain replaceable or extensible.

The purpose of core hardening is therefore not to predict the future. It is to identify the few decisions for which future escape hatches would otherwise become prohibitively expensive.

### 7. Long-term platform

The long-term direction is a unified, open workspace foundation that can support:

- documents
- spreadsheets and structured data
- Markdown workflows
- computational documents
- Git-native review and history
- AI-native manipulation
- domain-specific extensions

The surface may resemble familiar tools when useful. The substrate should not be constrained by their historical formats.

## Core philosophy

The founding motivation can be summarized in two layers.

The emotional origin:

> People should not have to surrender ownership of their work to an Office ecosystem or a vendor-controlled format.

The engineering translation:

> Users should be able to create, inspect, version, migrate, automate, compute over, and preserve their work independently of the application that first created it.

Legacy formats are accepted at the boundary, not allowed to define the semantic core.

Git is a storage and versioning protocol, not the user interface.

Semantic architecture is a means to preserve meaning and interoperability, not an end in itself.

## Follow-up architecture rationale

A 2026-08-21 follow-up discussion pressure-tested the semantic direction against whether Office/LibreOffice could add similar capabilities, whether a small-core/extension architecture better fits the project, how Milestone 02 should avoid freezing mechanisms prematurely, and whether `semantic` was a premise or a derived answer.

The result did not replace ADR-0001. It clarified the derivation and added explicit falsification/revisit conditions so semantic-first does not become dogma.

See [Why Tachiko Work Has a Semantic Core](../architecture/semantic-core-rationale.md). Its role is explanatory; current authority remains governed by the Product Constitution, Accepted ADRs/policies, and the knowledge-authority rules.
