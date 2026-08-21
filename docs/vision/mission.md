# Tachiko Work Mission

## Purpose

Tachiko Work exists to help people regain ownership of their documents, data, and computation.

This project is not only about building a better Office application.

It is about creating an open workspace where individuals, teams, and organizations are no longer permanently dependent on closed document ecosystems, proprietary representation choices, or one vendor's application stack.

## The Problem

For decades, Word, Excel, and related Office tools have become the default language of productivity.

They are extremely successful products, but the world built around them creates long-term constraints:

- Important information becomes tightly coupled to vendor-specific document ecosystems.
- Documents are difficult to version-control and merge with software-engineering quality.
- Spreadsheets are forced to act as databases, programs, configuration systems, and knowledge systems at the same time.
- Collaboration workflows often depend on centralized platforms.
- Legacy compatibility behavior accumulates until historical accidents become permanent constraints.
- AI systems must interpret human-oriented files and interfaces instead of operating on structured meaning directly.

The problem is not that Office tools are bad.

The problem is that no application or historical file format should become the permanent owner of human work.

## Founding Motivation

Tachiko Work grew from a long-standing desire to reduce dependence on Microsoft Office and OOXML and to make open document ecosystems a practical default rather than an ideological luxury.

A COSCUP 2017 talk by Italo Vignoli was an important influence on this motivation. It reinforced the importance of document freedom, interoperability, and the ability to preserve work independently of the vendor that created the software.

The motivation was later reinforced by direct engineering experience:

- game and application data living in spreadsheets while code lives in Git
- spreadsheet changes that are difficult to diff, review, merge, test, and validate
- the weight and historical complexity of traditional Office software
- the appeal of lightweight tools such as HackMD, combined with their inability to cover richer structured-data, computation, extensibility, and version-control workflows

These experiences led to a broader question:

> If productivity software were designed again today, what should remain invariant, and what historical assumptions could be discarded?

People should not need to fight their tools in order to create, understand, version, migrate, and preserve their own work.

## The Vision

Tachiko Work aims to create a new foundation for digital work:

- Open and inspectable representations instead of format lock-in.
- Semantic models instead of historical file representations owning the truth.
- Version control instead of opaque binary changes.
- Computation as a native capability instead of spreadsheet-specific hacks.
- AI agents operating on meaning instead of simulating user actions.
- Migration paths that let existing Office users move gradually rather than forcing a flag day.
- A small, stable semantic core surrounded by replaceable and extensible capabilities.

## Beyond Office Replacement

The goal is not to recreate Microsoft Office feature by feature.

A simple clone would inherit many of the same limitations and would still allow legacy formats to dictate the architecture.

Instead, Tachiko Work aims to create a platform where documents, spreadsheets, Markdown, structured data, computational documents, diagrams, AI workflows, and domain-specific tools can share semantic foundations where doing so creates real value.

Familiar Office-like surfaces may exist. They are views and workflows, not the constitutional center of the system.

## User Ownership

A central mission principle is:

> People should own their work independently of the application that created it.

In practical terms, users should be able to:

- inspect their data and document structure
- version and review meaningful changes
- migrate between tools and representations
- automate workflows without UI simulation
- preserve work over long time horizons
- extend the system without requiring every capability to enter the core

Compatibility is important, but compatibility should provide an escape path from lock-in rather than reproduce lock-in inside a new implementation.

## Long-Term Mission

The long-term mission is to make productivity software more open, understandable, versionable, computational, and extensible.

Microsoft Office does not need to disappear because someone builds a copy of Word or Excel.

It should become less necessary because people have a better foundation for creating and managing knowledge and data.

Tachiko Work is an attempt to build that foundation.
