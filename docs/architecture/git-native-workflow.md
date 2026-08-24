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

ADR-0003 is Accepted and defines `project.roproj/` as the canonical editable/source representation for the mature Git-native workflow, with `.ro` as a portable artifact. ADR-0023 fixes the `.roproj/v1` canonical tree and entity-sharding contract.

The production `.roproj` codec remains deferred, not the representation
decision itself. The current `.ro`-based workflow is therefore a validated
transitional product path while implementation catches up.

In v1, Git paths and JSONL line positions are materialization coordinates only.
Entity identity comes from the stable ID inside the decoded record. A
layout-only change has no semantic delta; canonical rematerialization restores
the one Accepted tree. Operational Git attributes, diff drivers, CI policy,
generated-artifact checks, and merge integration remain #44 and later protocol
work.

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
