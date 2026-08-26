# Git-native, not Git-shaped

Status: Discussion

Origin: 2026-08-26 architecture discussion comparing Git-friendly spreadsheet/document approaches with Tachiko Work's existing semantic and storage decisions.

Authority: ADR-0003, ADR-0011, ADR-0015.

Tracking issue: #44 for the near-term Git-facing adapter and CI workflow.

Related: #12, #46, #47, #49, #50, #93.

This document is a non-authoritative synthesis of rationale, risks, and unresolved boundaries. It does not reopen the Accepted Git-native representation direction and must not be used to override the ADRs or the focused issues above.

## Why preserve this discussion

A recurring design temptation is to equate "Git-native" with mapping Tachiko concepts directly onto Git concepts. That would make the architecture initially look simple:

```text
Workbook/Project = repository
Revision         = commit
Workspace        = branch
Undo             = revert
Change           = diff
Collaboration    = merge
```

The mapping is attractive, but it is too strong. Git is excellent durable storage, branching, review, and distribution infrastructure. It does not understand Tachiko semantic identity, typed references, formulas, validation, or semantic conflict rules.

The useful shorthand from this discussion is:

> Git-native, not Git-shaped. Git carries Tachiko state and history; it does not define Tachiko semantics.

## Accepted baseline that this discussion does not reopen

The following points are already governed by Accepted authority:

1. The semantic model owns meaning.
2. `.roproj` is the canonical editable, deterministic, Git-native materialization.
3. `.ro` is a derived portable artifact, not a competing source of truth.
4. Git is storage/collaboration infrastructure, not the user interface.
5. Durable semantic identity is independent from human names, UI coordinates, storage paths, serialization order, and mutable content.
6. Authoritative three-way reconciliation happens at the semantic model level. Raw text merge is not sufficient to establish semantic correctness.

This discussion therefore concerns boundaries and follow-up questions, not whether Tachiko Work should be Git-native.

## Layer model

The architecture should continue to preserve this direction:

```text
Semantic Model / Engine
        |
        | owns meaning, identity, validation, calculation
        v
Canonical semantic state / revision
        |
        v
.roproj deterministic materialization
        |
        +------ Git raw diff / branch / commit / PR / CI
        |
        +------ Tachiko semantic diff / merge / validation
```

Git sits below the semantic contract. A Git host may carry Tachiko state and review it, but Git object identity and Git operations do not become semantic truth merely because the repository uses them.

## Important boundaries

### Raw Git diff is useful but not semantic authority

`.roproj` should make ordinary line diffs localized and readable where practical. This is a developer-experience property, not a proof that line diff equals semantic diff.

A semantic change can be small while its physical representation changes in several places. Conversely, a physically small edit may violate references, formulas, schema rules, or other semantic constraints.

The desired workflow is complementary:

```text
Git raw diff     -> inspect physical materialization
Tachiko diff     -> inspect semantic meaning
Tachiko validate -> establish semantic admissibility
```

Issue #44 owns the near-term Git/CI integration of these views.

### Git commit identity must not become semantic identity

A commit SHA identifies a repository object and can be useful provenance. It must not become the durable identity of a Document, Entity, Field, semantic revision, or other domain object by implication.

Operations such as rebase, squash, filter-repo, repository migration, or reconstruction can change Git commit identity without changing the semantic meaning of the project snapshot.

The same separation applies to branches. A branch is a repository/history concept unless a future product requirement independently proves that branch identity belongs in the semantic model.

### Semantic revision is not automatically a Git commit

The architecture should not assume:

```text
semantic operation == semantic revision == Git commit
```

Editing workflows may need distinct layers such as:

```text
operation
  -> transaction/batch
  -> accepted semantic revision
  -> durable checkpoint/publication
  -> optional Git commit
```

The exact contract remains unresolved and belongs to #12, #48, and #49 rather than this discussion record.

This separation is important for autosave, undo/redo, AI operation batches, interactive authoring, and future collaboration. Otherwise either Git history becomes unusably noisy or meaningful intermediate semantic history is discarded merely to keep Git clean.

### A Git working tree is not automatically an admissible semantic state

Canonical `.roproj` is a Git-trackable representation. That does not imply that every transient filesystem state during Git or editor activity is a valid Tachiko snapshot.

Examples include:

- checkout replacing several files non-atomically from the viewpoint of a filesystem watcher;
- a tool writing a multi-file materialization in stages;
- an interrupted operation leaving a mixed revision on disk;
- ordinary textual merge introducing conflict markers;
- a user hand-editing only one side of a cross-file invariant.

A host/runtime should admit a materialization only through the applicable parse/canonicalization/validation/revision boundary. Filesystem events by themselves must not silently publish semantic state.

The accepted host/runtime architecture keeps filesystem and Git capabilities outside the authoritative resident semantic session. #93 is the current implementation seam for revision-safe resident commands; narrower file-watch/open/reload behavior should be split from the owning host/runtime work when implementation evidence requires it.

### Git conflict markers are not Tachiko conflict objects

Text such as:

```text
<<<<<<< ours
...
=======
...
>>>>>>> theirs
```

is malformed or unresolved physical materialization, not canonical semantic state.

A future Tachiko conflict object must come from the semantic merge contract and preserve typed base/left/right meaning as defined by #46. A Git merge driver may adapt between Git and Tachiko merge, but it must not redefine merge correctness.

### Realtime collaboration does not need to be Git-shaped

Git is well suited to durable snapshots, branching, review, and repository history. It is not automatically the right live transport for concurrent keystrokes, presence, partially authored values, or fine-grained offline causality.

Future collaboration may use operation streams, sessions, or selectively applied CRDT techniques and still publish deterministic semantic revisions into a Git-native project workflow. #50 intentionally preserves this question without making CRDT a universal core dependency.

### Cross-version branches need semantic migration rules

Git can retain two branches that use different representation or semantic versions, but that fact does not define whether those branches can be semantically diffed or merged.

Explicit migration, write-version pinning, loss classification, and cross-version merge behavior remain owned by #47. Git must not silently choose a migration target merely because files can be textually combined.

### History rewrite can preserve meaning while changing provenance

Squash and rebase may preserve final semantic state while changing the Git evidence chain. Future provenance/history work therefore needs to distinguish at least:

- semantic meaning/state;
- semantic history or event identity, if retained;
- repository commit identity;
- external review/approval provenance.

The exact retention and reconstruction model remains owned by #12 and #49.

### Assets, generated state, and secrets need explicit Git policy

Git-native does not mean every byte touched by Tachiko belongs in canonical tracked state.

The project should keep distinguishing:

- canonical semantic state;
- canonical assets when they are part of project meaning;
- derived indexes/caches;
- generated `.ro` artifacts;
- host-local state;
- credentials and secrets.

Secrets are especially important because deleting the current value does not remove previously committed history. Canonical project formats should not encourage storing credentials or service tokens as ordinary semantic state.

## Lessons from spreadsheet/document version-control approaches

The prior-art discussion highlighted several general lessons without making another office format part of the Tachiko core:

1. A textual format alone does not guarantee a useful Git diff. Serialization noise can still swamp a one-cell or one-field semantic change.
2. Embedded document version history is not the same thing as repository history with branches and distributed review.
3. A generic textual three-way merge cannot establish domain validity for formulas, references, schemas, and typed values.
4. The durable solution is stable semantic identity plus deterministic materialization plus semantic diff/merge, with Git used as infrastructure around that model.

These lessons reinforce existing ADRs rather than introducing a new format decision.

## Open questions and existing owners

Do not create duplicate architecture owners for these questions.

| Question | Existing owner |
| --- | --- |
| Raw Git diff, semantic review, CI, `.roproj` / `.ro` consistency | #44 |
| Relationship among semantic operations, history, event sourcing, CRDT, and Git | #12 |
| Machine-readable authoritative semantic merge and conflict objects | #46 |
| Cross-version branch and migration behavior | #47 |
| Snapshot/checkpoint/replay/compaction and retained history profiles | #49 |
| Offline causality and selective CRDT boundaries | #50 |
| Resident runtime revision safety and host capability separation | #93 / #26 |

A new issue is warranted only when implementation or research exposes a narrower problem that none of these owners can absorb without widening its intended scope.

## Near-term guidance

For the current `.roproj` / Git workflow work:

- optimize canonical materialization for deterministic, localized raw diffs;
- keep semantic diff and validation independently authoritative;
- do not introduce branch, commit, repository, or Git-host IDs into semantic-core merely for integration convenience;
- do not make Git configuration a prerequisite for semantic correctness;
- keep GitHub/GitLab/Gitea-specific features in adapters rather than format semantics;
- treat generated `.ro` as derived material unless a workflow explicitly chooses to track it;
- preserve the later seams for semantic history, collaboration, and provenance rather than solving them prematurely in #44.

## Research timing

No new deep-research task is required to preserve the current Accepted direction. The next evidence-heavy research should be triggered by a concrete owning issue, especially:

- #12 when Team Workspace history semantics becomes active;
- #46 when first-class semantic conflict protocol work begins;
- #47 when cross-version branch behavior needs an implementable migration contract;
- #50 when real offline/realtime collaboration pressure exists.

At those points, focused research into Git data-model behavior, structured-data merge systems, CRDT/operation-log approaches, and spreadsheet/document prior art can test the specific unresolved contract instead of reopening the Git-native product premise in the abstract.
