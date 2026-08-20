# Git-Native Workflow

## Principle

Git is a storage and collaboration protocol, not the user interface.

## Current v0.1 workflow

The implemented CLI currently reads and writes deterministic single-file `.ro` documents:

```text
project.ro
```

That path is already used for branch-based editing, semantic diff, semantic three-way merge, validation, calculation, CI smoke journeys, and reviewable data changes.

## Accepted target representation

ADR-0003 is Accepted and defines `project.roproj/` as the canonical editable/source representation for the mature Git-native workflow, with `.ro` as a portable artifact.

`.roproj` implementation is deferred, not the decision itself. The current `.ro`-based workflow is therefore a validated transitional product path while the semantic model and authoring contracts stabilize.

The model itself is the compatibility boundary for future representations.

## Benefits already validated

- deterministic semantic-document serialization
- branch-based workflows
- semantic diff
- semantic three-way merge
- CI validation
- reviewable data changes

## Goal

Make non-programmers first-class participants in Git-based workflows without making Git itself the user interface.
