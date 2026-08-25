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

ADR-0003 is Accepted and defines `project.roproj/` as the canonical editable/source representation for the mature Git-native workflow, with `.ro` as a portable artifact. ADR-0023 fixes the `.roproj/v1` canonical tree and entity-sharding contract. ADR-0025 fixes portable-package v1 as a deterministic envelope over those exact source bytes.

The production `.roproj` codec remains deferred, not the representation
decision itself. The production package codec and CLI also remain deferred,
not the package decision itself. The current direct-JSON `.ro` workflow is
therefore a validated transitional product path while implementation catches
up.

In v1, Git paths and JSONL line positions are materialization coordinates only.
Entity identity comes from the stable ID inside the decoded record. A
layout-only change has no semantic delta; canonical rematerialization restores
the one Accepted tree. Operational Git attributes, diff drivers, CI policy,
generated-artifact checks, and merge integration remain #44 and later protocol
work.

ADR-0025 permits a read-only consistency comparison between a verified
portable package and canonical tracked source by calculating the same root
over the tracked tree's exact paths and bytes. Equal roots mean `consistent`
and cause no write. Different roots mean explicit source mismatch; the tracked
`.roproj` remains authoritative, neither side is mutated, and no timestamp or
filename selects a winner. Automating that comparison in Git or CI remains
#44 rather than part of the package contract.

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
