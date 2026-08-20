# Migration Framework Strategy

## Principle

Legacy compatibility belongs at the boundary, not in the core.

Tachiko Work should support importing existing formats without inheriting their historical mistakes.

## Supported Sources

Future adapters may include:

- XLSX
- DOCX
- CSV
- Markdown
- JSON
- proprietary game data formats

## Migration Flow

```text
Legacy File
    ↓
Import Adapter
    ↓
Compatibility Analysis
    ↓
Migration Report
    ↓
Modern Semantic Model
```

## Example

Excel workbook:

- legacy date behavior detected
- unsupported VBA detected
- formulas requiring conversion detected

The user receives:

- detected issues
- affected data
- recommended migration
- validation result

## Principle

Historical compatibility is a service.

It must not become the foundation of future data models.
