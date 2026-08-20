# Game Development User Stories

## Goal

Validate Tachiko Work through a painful real-world workflow before expanding into a general document platform.

The first market is game development data.

## Designer

### Current Pain

A game designer adjusts balance values in Excel or Google Sheets.

Problems:

- no meaningful diff
- difficult branch workflows
- unclear ownership
- manual CSV exports
- hidden formulas

### Desired Workflow
  
v0.1 target is CLI-first, not GUI-first.

The designer uses deterministic commands and canonical text output:

The system provides:

- typed fields
- validation
- references
- impact analysis
- review history

### Future Interface

Spreadsheet-style editing and richer designer tooling are planned as future
workflow layers and are not part of this release.

## Programmer

### Current Pain

Game code and game data evolve separately.

### Desired Workflow

A programmer can review data changes like code changes.

Example:

```
Enemy.Goblin.hp
1000 → 1200

Affected systems:
- Forest Stage
- Tutorial Battle
- Quest Reward
```

## Producer

Needs:

- balance change visibility
- release impact understanding
- confidence before patches

## QA

Needs:

- changed data detection
- regression targets
- reproducible versions

## Core Promise

Tachiko Work makes game data a first-class software artifact.

Code, data, and history belong in the same workflow.
