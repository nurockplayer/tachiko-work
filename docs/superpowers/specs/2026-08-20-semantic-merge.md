# Semantic Merge Design

## Product outcome

A game team can combine independent balance changes from two branches into a
new canonical `.ro` document. Compatible edits merge automatically; conflicting
intent is reported at stable semantic paths; no conflicted or invalid output is
written.

## User workflow

```text
tachiko merge base.ro designer-a.ro designer-b.ro --output merged.ro
tachiko validate merged.ro
tachiko diff base.ro merged.ro
```

On success, `merge` prints the output path and a semantic summary from the base
to the merged result. On conflict it exits unsuccessfully, prints every conflict
in path order with base/ours/theirs values, and leaves the requested output
absent. Existing outputs are always preserved.

## Architecture

`tachiko-merge-engine` depends on `semantic-core` and `formula-engine`. It owns
only three-way semantic reconciliation and candidate validation; it has no
filesystem or terminal behavior. `tachiko-cli` loads three canonical documents,
renders conflicts, and persists successful results using the existing
exclusive-create boundary. `diff-engine` remains a sibling used by the CLI to
summarize successful combined impact.

The public API is:

```rust
pub fn merge(
    base: &Document,
    ours: &Document,
    theirs: &Document,
) -> Result<MergeOutcome, MergeError>;

pub enum MergeOutcome {
    Merged(Document),
    Conflicted(Vec<MergeConflict>),
}
```

`MergeConflict` contains a stable `path` plus typed optional
`base`/`ours`/`theirs` values. `MergeValue` distinguishes document identifiers,
text, schema identifiers, whole schemas/entities, field definitions, and field
values so future AI and UI clients do not need to parse rendered strings.

## Merge semantics

For any semantic value `(base, ours, theirs)`:

1. if `ours == theirs`, choose that value;
2. if `ours == base`, choose `theirs`;
3. if `theirs == base`, choose `ours`;
4. otherwise, report a conflict.

Existing schemas and entities recurse before applying the rule, allowing
different fields to merge independently. Map entry behavior is:

- one-sided addition: add;
- identical two-sided addition: add;
- different two-sided addition: conflict at the entry path;
- one-sided deletion with the other side unchanged: delete;
- deletion against modification: conflict at the entry path;
- two-sided deletion: delete.

Document IDs and titles merge as scalar values. Existing schema IDs and entity
IDs are fixed by their validated map keys; schema field definitions, entity
schema membership, and entity field values merge at their own paths.

## Safety and errors

- The engine validates and calculates all three inputs. `MergeError` identifies
  the failing `MergeSide::{Base, Ours, Theirs}` so direct API callers receive
  the same safety as CLI callers.
- Conflicts return `MergeOutcome::Conflicted`; they are not exceptional engine
  failures and never contain a partial document.
- A conflict-free candidate is checked with `validate_document` and `calculate`.
- Invalid inputs return `MergeError::InvalidInput`; input calculation failures
  return `MergeError::InputCalculation`; invalid combined semantics return
  `MergeError::InvalidMergedDocument`; and combined calculation failures return
  `MergeError::MergedCalculation`.
- The CLI writes only `MergeOutcome::Merged` and uses exclusive create.

## Test contract

Tests cover one-sided change, identical two-sided change, independent field and
schema-definition changes, same-field conflict, delete-versus-modify conflict,
different concurrent additions, deterministic conflict ordering, invalid input,
combined semantic validation failure, combined calculation failure, successful
CLI persistence, conflict output, and existing-output preservation. The collaboration smoke script
performs a real base/two-branch/merge/validate/diff journey.

## Deferred

- Git configuration and a Git merge-driver adapter
- interactive conflict resolution
- `.roproj` representation
- operation-log, CRDT, or realtime collaboration
- AI-autonomous conflict resolution or mutation
