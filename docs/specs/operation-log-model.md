# Semantic Operation Log Model

Decision state: Open Question

Implementation state: No first-class persisted operation log in v0.1

Decision owners: #12 and #48

## Overview

Tachiko Work has accepted the principle that meaningful changes should be expressed semantically rather than reduced to opaque raw-file replacement where a semantic operation exists.

What remains unresolved is whether a persisted operation log is authoritative history, optional audit metadata, collaboration infrastructure, or unnecessary for some workflows.

Current runtime edits are made through explicit typed CLI/workflow operations that produce validated output documents. Those operations are not persisted as a first-class canonical log in v0.1.

## Example vocabulary

Instead of reasoning only from:

```text
file changed
```

A semantic change vocabulary may express intent such as:

```text
UpdateField
 entity: Dragon
 field: hp
 old: 8000
 new: 9000
```

The concrete command / operation / transaction / semantic-event taxonomy is intentionally not frozen here. #48 owns that later distinction.

## Potential benefits

A semantic history layer may enable:

- meaningful history
- semantic review
- AI explanation
- conflict explanation
- provenance and auditability
- collaboration workflows

These benefits justify research; they do not by themselves require event sourcing or a permanently retained operation log.

## Future questions

#12 and #48 must determine:

- whether semantic operations are the canonical mutation vocabulary;
- whether an operation log is persisted and, if so, whether it is authoritative or optional;
- how operations differ from commands, atomic transactions, state deltas, and committed semantic events;
- how any semantic history composes with Git;
- whether current state must be reconstructable from retained operations;
- how history can be omitted or compacted without changing current semantic meaning.

Until those questions are accepted through an ADR/specification, operation-log persistence remains an Open Question rather than a storage requirement.
