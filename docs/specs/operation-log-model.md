# Semantic Operation Log Model

Decision state: Mixed — ADR-0029 boundary Accepted; retained-history mechanics Open Question

Implementation state: No first-class persisted operation log in v0.1

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)

Decision owners: #48 and #49

## Overview

Tachiko Work has accepted the principle that meaningful changes should be expressed semantically rather than reduced to opaque raw-file replacement where a semantic operation exists.

ADR-0029 resolves that any general retained operation/history log is optional
and cannot be authoritative state or required to reconstruct a complete
snapshot. What remains unresolved is whether a particular future profile
retains transition records for audit, recovery, or collaboration and which
guarantees that profile declares.

Current runtime edits are made through explicit typed workspace-engine
operations composed by the CLI into validated output documents. Those
operations are not persisted as a first-class canonical log in v0.1.

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

Issues #48 and #49 must determine:

- whether a particular profile persists transition records and what bounded
  guarantee it provides;
- how operations differ from commands, atomic transactions, state deltas, and committed semantic events;
- how any semantic history composes with Git;
- how retained history can be compacted while disclosing its changed coverage
  and preserving current semantic meaning.

Until those questions are accepted through an ADR/specification,
operation-log persistence remains an Open Question rather than a storage
requirement. It may not reopen ADR-0029's current-state authority or complete
standalone snapshot boundary.
