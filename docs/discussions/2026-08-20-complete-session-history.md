# Tachiko Work Founding Discussion History

Date: 2026-08-20
Updated: 2026-08-21

## Overview

This document records the evolution of the Tachiko Work thesis during the initial architecture and product discussions.

It preserves the reasoning path rather than only the final architecture. Accepted ADRs and current specifications remain authoritative for concrete implementation contracts.

## Phase 0: The motivation before the product thesis

Tachiko Work did not begin with the phrase "semantic workspace."

The deeper motivation was a long-standing desire to reduce people's dependence on Microsoft Office and OOXML and to make document ownership, interoperability, and long-term freedom practical.

A COSCUP 2017 talk by Italo Vignoli was a major influence on the founder. It helped crystallize the importance of open document ecosystems and the danger of allowing one vendor-specific document stack to become the default substrate of human work.

That motivation combined with direct engineering experience:

- game and application data living in spreadsheets while source code lives in Git
- spreadsheet changes that are difficult to diff, review, merge, validate, and test
- traditional Office software feeling heavy and burdened by decades of compatibility behavior
- lightweight collaborative tools such as HackMD being attractive but insufficient for richer structured-data, computation, extensibility, and version-control needs

The original emotional goal can be stated plainly:

> Help liberate people from deep dependence on the Microsoft Office ecosystem and OOXML.

The engineering form of the same goal became:

> People should own their work independently of the application that created it.

This is the origin. The semantic architecture came later as an answer to the resulting engineering questions.

## Phase 1: Office replacement analysis

The first explicit project investigation asked whether Rust and AI coding agents made it realistic to build an open-source Office alternative.

Investigation focused on:

- Microsoft Word and Excel dominance
- LibreOffice as the largest open-source desktop Office suite
- the difficulty of rewriting Office software
- whether modern implementation tools materially changed the feasibility boundary

Key conclusion:

A direct LibreOffice or Microsoft Office rewrite is not the right goal.

The difficult parts are not only UI and file parsing. The true challenges include:

- layout engines
- spreadsheet semantics
- compatibility behavior
- international text handling
- deterministic rendering
- round-trip fidelity

More importantly, a feature-for-feature rewrite would allow historical Office assumptions to dictate the architecture of the new system.

## Phase 2: New platform thesis

The project direction changed from:

> Build an open-source Office clone.

into:

> Build a new semantic document and computational platform where Office formats are interoperability boundaries rather than the architectural source of truth.

Core principles emerged:

- The semantic model owns meaning.
- File formats do not define the internal architecture.
- Legacy behavior belongs at explicit compatibility boundaries.
- AI should operate on semantic objects instead of simulating user interaction.

The important historical point is that "semantic" was derived from concrete requirements. It was not the original ideology of the project.

## Phase 3: Legacy compatibility philosophy

A major line of reasoning concerned historical Excel behavior.

Example:

The Excel 1900 leap-year behavior should not automatically become part of the new platform's permanent internal semantics.

The preferred pattern became:

1. import legacy files
2. detect historical dependencies
3. explain compatibility implications
4. preserve or emulate behavior where explicitly required
5. provide migration paths toward modern semantics

Legacy problems should live in conversion and compatibility tooling rather than silently occupying the next hundred years of core architecture.

## Phase 4: Unified semantic model

Documents, spreadsheets, Markdown, charts, formulas, and structured data were explored as views over shared semantic foundations.

Possible primitives discussed included:

- document blocks
- tables
- records
- formulas
- references
- assets
- views
- history
- agent operations

The goal is not to force every kind of work into one visual editor. The value of a shared model is preservation of identity and meaning across appropriate views, tools, computation, version control, and AI operations.

## Phase 5: Game development as the first market wedge

One of the founder's own engineering pain points produced the strongest initial wedge.

Typical workflow:

Designer
→ Excel / Google Sheets
→ CSV or custom export
→ Engine import

while code follows:

Developer
→ source files
→ Git
→ review / CI

This separation causes problems:

- spreadsheet files are difficult to diff meaningfully
- merges are painful
- code and data branches diverge
- design changes lack software-engineering-quality review
- validation and CI require custom glue

Tachiko Work's first concrete product thesis became:

> The spreadsheet that belongs in your Git repository.

Capabilities explored and implemented around this thesis include:

- typed schemas
- formulas
- semantic diff
- semantic merge
- validation
- CI checks
- structured export
- future engine integration

Game development is a proving ground, not the constitutional limit of the product.

## Phase 6: Git-native representation and format strategy

The project explored `.ro` and `.roproj` as different physical representations around the same semantic source of truth.

ADR-0003 records the architectural direction:

- the semantic model remains the source of truth
- `.roproj/` is intended as a canonical editable project representation optimized for Git workflows
- `.ro` is intended as a portable packaged artifact
- packaging must remain semantically lossless

The implementation state is intentionally narrower than the long-term architecture. The current v0.1 product persists canonical, versioned `.ro` documents and does not yet expose the full `.roproj` workflow described by ADR-0003.

This is an example of an important project pattern:

> Architecture may preserve an intended seam before every representation is implemented in the product.

The `.ro` name itself is still provisional before a formal release identity is frozen. Naming should not be confused with semantic-format invariants.

Git remains storage and collaboration infrastructure, not the end-user interface.

## Phase 7: AI-native direction

The project concluded that AI should not primarily control Tachiko Work by finding buttons and simulating a human operator.

If the system already has typed semantic identity, formulas, references, validation, and explicit operations, AI can work through those capabilities directly.

This creates a common substrate for:

- human editing
- automation
- validation
- Git review
- AI reasoning
- permissions and approval workflows

The semantic model therefore became valuable not only for file architecture, but also as the natural API boundary for agents.

## Phase 8: Progressive migration instead of an Office replacement event

The anti-lock-in mission does not imply demanding that organizations immediately abandon Excel or Word.

That would replace one form of tool-driven coercion with another.

The product direction therefore moved toward progressive migration:

- accept existing documents at interoperability boundaries
- inspect dependencies and problematic legacy semantics
- preserve meaning where possible
- convert into open, versionable representations
- allow old and new workflows to coexist during migration
- gradually reduce dependence on Office-specific behavior

This is particularly relevant to organizations with long-lived spreadsheet workflows and institutional knowledge embedded in existing files.

## Phase 9: Japanese enterprise pain as a long-term product problem

Discussion of Japanese organizations highlighted a broader class of Office dependence:

- critical workflows embodied in old spreadsheets and macros
- process knowledge tied to individual employees
- undocumented monthly or operational procedures
- difficulty understanding why a workbook behaves as it does
- migration risk that makes replacement projects politically and operationally expensive

This reinforced the value of history, explainability, semantic structure, Git-style change tracking, and progressive migration.

The opportunity is not merely to offer another spreadsheet program. It is to help organizations turn fragile, person-dependent Office workflows into understandable and evolvable assets over time.

## Phase 10: Small stable core, extensible surroundings

As the issue backlog expanded, an important architectural principle emerged.

The project cannot and should not decide every future capability during early milestones.

The preferred shape is closer to a small framework core surrounded by extensions than a monolithic Office suite.

Only high-confidence, expensive-to-reverse invariants should be stabilized early, especially where persisted data, external tools, Git history, public APIs, or plugin ecosystems would later make migration costly.

Likely core-hardening concerns include:

- semantic identity
- canonical serialization
- schema compatibility semantics
- formula semantics
- storage and semantic boundaries
- core versus extension authority

Many other concerns should remain replaceable:

- editors
- UI metaphors
- importers and exporters
- AI providers
- engine integrations
- domain-specific workflows
- collaboration surfaces

The purpose of architectural flexibility is not to make all mistakes cheap. It is to ensure that most future choices retain an escape hatch while the truly expensive invariants receive deliberate attention.

## Phase 11: Business and ecosystem thinking

The project explored how an open foundation could remain commercially sustainable.

Possible revenue areas include:

- managed cloud services
- team collaboration
- enterprise deployment
- migration services
- AI services
- support
- marketplace or ecosystem services
- commercial licensing where appropriate

The long-term moat should not rely only on closed code.

Potential durable assets include:

- ecosystem
- format adoption
- integrations
- community
- migration expertise
- trust

Licensing and governance remain subjects for explicit decisions rather than assumptions baked into technical architecture.

## Current thesis

Tachiko Work aims to create an open, semantic, computational foundation for digital work.

Its first usable product is a Git-native computational data workflow for game development.

Its long-term mission is broader: reduce the degree to which human work is trapped inside application-specific formats and workflows by providing a foundation that is open, versionable, computational, explainable, extensible, and AI-addressable.

The project should not attempt to predict every future workflow.

It should protect a small set of durable principles while keeping enough architectural seams that research, implementation, and product evidence can revise everything else.
