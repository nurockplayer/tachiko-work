# Game Development Wedge

## Problem

Game designers frequently use Excel or Google Sheets for balancing.

The rest of the production pipeline uses Git.

Typical workflow:

```
Excel / Sheets
      ↓
CSV export
      ↓
Git
      ↓
Game engine
```

This creates disconnected versions of the truth.

## Tachiko Work Opportunity

Create a semantic balance workflow that fits cleanly in a Git repository.

Features:

- typed data
- schemas
- references
- formulas
- semantic diff
- three-way semantic merge
- pull-request friendly review
- balance validation CI

Current release boundary: this is CLI-first with `.ro` persistence.

## Long Term Expansion

Game data is the first vertical, not the final product.

The same foundation can expand into:

- technical documentation
- data workflows
- research documents
- enterprise spreadsheets
- AI-native knowledge work

Non-implemented work in this release remains:

- spreadsheet-native GUI editing
- `.roproj`/working-copy workflow
- realtime/online collaboration
- engine plugins and hosted multiplayer pipelines
