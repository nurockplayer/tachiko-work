# Tachiko Work Design Principles

## 1. Not an Office Clone

Tachiko Work is not a Rust rewrite of Microsoft Office or LibreOffice.

The goal is a new semantic workspace that can represent documents, spreadsheets, structured data and computation.

## 2. Semantic Core First

The core model stores meaning, not historical implementation accidents.

Legacy formats such as DOCX and XLSX are compatibility boundaries.

## 3. Legacy Compatibility at the Boundary

Historical bugs should not become permanent architecture.

Example:

- Excel 1900 leap-year bug should be detected during migration.
- The conversion layer may preserve or explain old behavior.
- The internal model should use correct modern semantics.

## 4. Multiple Views, One Model

Markdown, visual editing, spreadsheets and AI operations should be views over the same semantic structure.

## 5. Git as Storage Protocol

Git should understand documents as structured data.

Users should not manually edit Git representations, but the format should support:

- deterministic ordering
- human-readable changes
- semantic diff
- semantic merge

## 6. AI-Native Architecture

AI should operate on typed semantic objects instead of controlling a UI through simulated actions.
