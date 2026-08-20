# Game Dev MVP Specification

## Product thesis

The first product is not an Office replacement.

It is a Git-native computational data platform for game development.

## Problem

Game teams commonly use spreadsheets for:

- enemies
- weapons
- skills
- economy
- progression
- quests

But spreadsheets are poor Git citizens.

Common workaround:

Excel / Google Sheets
→ CSV export
→ Engine import
→ Separate version control

## MVP Goals

### Semantic data and computation

Support:

- numbers
- strings
- booleans
- references
- formulas

### Git-friendly developer workflow

Support:

- deterministic serialization
- versioned `.ro` documents
- schema validation
- formula calculation
- semantic diff
- CLI operations
- AI semantic read/query

### Game balance proof

Provide a game balance example covering:

- characters
- weapons
- items
- economy

## Deferred after the MVP

Spreadsheet or React UI, semantic merge, realtime collaboration, cloud SaaS,
Office/Excel compatibility, engine plugins, and enterprise permissions remain
future layers over the semantic model.

## Example MVP workflow

Designer changes enemy HP.

The CLI and AI read/query interface provide:

- data diff
- balance impact report
- validation result
- model and formula explanation

## Success criteria

The MVP succeeds if a developer or technical designer can create game balance
data, define formulas, calculate derived values, validate the document, review
semantic changes through Git-friendly diff, and ask AI to explain the model and
impact.
