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

Issue #44 implements the optional adapter around that standalone boundary.
Read-only semantic CLI commands accept an exact `.roproj/v1` directory without
repository discovery. Root `.gitattributes` force only canonical project JSON
and JSONL members to LF text with ordinary line diffs. The provider-neutral
Git/CI journey proves one-record scalar diffs, byte-identical structured
semantic results inside and outside Git, canonical/workspace rejection, and
read-only generated-package drift detection. It uses no Git host API and does
not install a diff or merge driver.

## Accepted representation and implementation split

ADR-0003 is Accepted and defines `project.roproj/` as the canonical editable/source representation for the mature Git-native workflow, with `.ro` as a portable artifact. ADR-0023 fixes the `.roproj/v1` canonical tree and entity-sharding contract. ADR-0025 fixes portable-package v1 as a deterministic envelope over those exact source bytes.

The production `.roproj/v1` pure codec and native host workflow are implemented
by #123, not merely selected as representation direction. Issue #3 implements
the packaged `.ro` codec, exact pack/unpack, read-only comparison, and real
atomic no-replace destination publication. Issue #44 composes Git attributes,
raw/semantic review, CI validation, and explicit generated-package consistency
without changing these representation roles. Broader hostile source/path races,
full durability/recovery, and host hardening remain separately Deferred.

In v1, Git paths and JSONL line positions are materialization coordinates only.
Entity identity comes from the stable ID inside the decoded record. A
layout-only change has no semantic delta; canonical rematerialization restores
the one Accepted tree. Issue #44 implements ordinary text attributes and
provider-neutral validation/review/package-consistency composition. Custom diff
or merge drivers and semantic merge protocol work remain outside that adapter.

ADR-0025 permits a read-only consistency comparison between a verified
portable package and canonical tracked source by calculating the same root
over the tracked tree's exact paths and bytes. Equal roots mean `consistent`
and cause no write. Different roots mean explicit source mismatch; the tracked
`.roproj` remains authoritative, neither side is mutated, and no timestamp or
filename selects a winner. Issue #44 composes that comparison in Git/CI without
moving it into the package contract.

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
