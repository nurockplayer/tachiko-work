# Rendering System Direction

## Principle

Rendering is a projection of semantic content.

The document model should not depend on a specific renderer.

## Possible Targets

```
Semantic Model
      |
      +-- Web Renderer
      +-- Desktop Renderer
      +-- PDF Renderer
      +-- Markdown Renderer
      +-- DOCX Renderer
```

## Requirements

- deterministic layout
- accessibility support
- Unicode-first design
- international text support
- reusable rendering primitives

## Long Term Goal

The same document should be able to move between:

- visual editor
- Markdown view
- printed document
- collaborative workspace
- AI-generated output

without changing its underlying meaning.
