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

Current release boundary: this is CLI-first with direct `.ro` as the ordinary
persistence path plus Issue #123's explicit standalone `.roproj/v1`
materialize, canonical-only validate, and bounded canonicalize workflow, and
Issue #3's portable package pack/unpack/compare workflow. Issue #44 adds
optional `.roproj` raw Git review, semantic review, canonical validation, and
generated-package consistency composition for provider-neutral CI without
changing the standalone product boundary.

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
- broader hostile source/path hardening, full durability/recovery, and host work
- realtime/online collaboration
- engine plugins and hosted multiplayer pipelines
