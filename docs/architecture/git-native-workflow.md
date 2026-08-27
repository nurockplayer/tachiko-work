# Git-Native Workflow

## Principle

Git is a storage and collaboration protocol, not the user interface.

## Current implementation

The implemented CLI currently reads and writes deterministic single-file `.ro` documents:

```text
project.ro
```

That path is already used for branch-based editing, semantic diff, semantic three-way merge, validation, calculation, CI smoke journeys, and reviewable data changes.

Issue #123 also implements the production `.roproj/v1` pure codec and native
standalone exact-tree workflow: explicit direct `.ro` materialization,
canonical-only validation, and explicit bounded canonicalization to a distinct
absent output. These operations preserve their source and require no Git
repository or Git configuration.

## Accepted representation and implementation split

ADR-0003 is Accepted and defines `project.roproj/` as the canonical editable/source representation for the mature Git-native workflow, with `.ro` as a portable artifact. ADR-0023 fixes the `.roproj/v1` canonical tree and entity-sharding contract. ADR-0025 fixes portable-package v1 as a deterministic envelope over those exact source bytes.

The production `.roproj/v1` pure codec and native host workflow are implemented
by #123, not merely selected as representation direction. The packaged `.ro`
ZIP codec and CLI pack/unpack remain #3 work, while optional Git attributes,
diff/CI integration, and generated-artifact policy remain #44 work. Hostile
filesystem races, full durability/recovery, and broader host hardening remain
separately Deferred; #123's staged absent-destination publication does not
resolve them.

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
