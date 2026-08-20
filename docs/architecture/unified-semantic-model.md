# Unified Semantic Model

## Principle

Tachiko Work stores meaning, not historical file format behavior.

Documents, spreadsheets, Markdown, charts, and computational blocks are different views over a shared model.

## Conceptual model

```
Semantic Document Model
        |
        +-- Visual Editor
        +-- Markdown View
        +-- Spreadsheet View
        +-- Code / Formula View
        +-- Export Formats
```

## Goals

- Structured data first
- Type awareness
- AI-readable representation
- Deterministic serialization
- Collaboration-ready design

## Non-goal

The system should not reproduce every historical accident from legacy formats.

Compatibility belongs at import/export boundaries.
