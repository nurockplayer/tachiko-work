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

### Typed data

Support:

- numbers
- strings
- enums
- references
- formulas
- localized text

### Git workflow

Support:

- deterministic serialization
- semantic diff
- semantic merge
- pull request review
- CI validation

### Engine integration

Initial targets:

- Unity
- Unreal Engine
- Godot

## Example future workflow

Designer changes enemy HP.

RustOffice generates:

- data diff
- balance impact report
- validation result
- pull request summary

## Success criteria

The MVP succeeds if a game team can replace:

"Spreadsheet + CSV export + manual review"

with:

"Version-controlled computational data workflow."
