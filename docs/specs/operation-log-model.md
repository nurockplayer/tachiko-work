# Semantic Operation Log Model

Decision state: Mixed — ADR-0029 history boundary and ADR-0032 transition
taxonomy Accepted; retained-history profiles and mechanics Open Question

Implementation state: No first-class persisted semantic operation/history log

Authority:
[ADR-0029](../decisions/ADR-0029-current-state-authority-and-optional-history.md)
and
[ADR-0032](../decisions/ADR-0032-semantic-execution-and-transition-taxonomy.md)

Decision owner for retained-history mechanics: #49

## Overview

Meaningful changes are requested through ADR-0020 typed `Command | AtomicBatch`
and may be proposed through an ADR-0024 SemanticPatch. `Operation` is only an
umbrella/conversational word at this layer. It does not name another mutation
DTO or a persistable apply language, and `transaction` does not extend
AtomicBatch into host or distributed transaction semantics.

An Execute attempt publishes zero or one semantic state installation. Only an
actual non-no-op installation creates a semantic revision occurrence.
Pre-publication failure and `NoChange` create none.

ADR-0029 makes any general retained history optional and non-authoritative. A
complete snapshot remains sufficient to open and use current semantic meaning
without an operation log, retained transition stream, Git, checkpoint, or
replay.

## Reconciled vocabulary

```text
Command | AtomicBatch
    -> optional SemanticPatch proposal
    -> gated Execute attempt
    -> zero or one semantic state installation
       -> if no installation: failure or NoChange; no revision/event
       -> if installed: revision occurrence
          + canonical A-to-B Semantic Delta evidence
          + required security/provenance receipt where ADR-0026 applies
          + optional retained semantic transition/event
```

The concepts are not interchangeable:

- `Command | AtomicBatch` is typed semantic intent.
- SemanticPatch is an immutable exact-base proposal occurrence.
- Execute attempt is a request to evaluate and possibly publish that intent.
- `NoChange` is a non-publication outcome.
- `RevisionOccurrenceRef` is opaque occurrence identity scoped to one owning
  revision context/domain and continuing `DocumentId`.
- Semantic Delta is canonical direct A-to-B state evidence, not intent or a
  mutation program.
- A retained semantic transition, also called a semantic event, is optional
  immutable evidence of one actual non-no-op publication.
- An ADR-0026 receipt is independent security/provenance evidence and remains
  required where that authority applies even if general history is disabled.

Current runtime Commands and internal revision tokens are implementation
evidence. They are not a canonical persisted log, globally meaningful revision
identity, or a public retained-transition DTO.

## Potential optional-history benefits

A retained semantic-history profile may support:

- meaningful history and semantic review;
- AI explanation and conflict explanation;
- audit, recovery, or verification guarantees; and
- collaboration workflows.

These benefits justify bounded profiles; they do not require event sourcing or
a permanently retained operation log.

## Deferred mechanics

Issue #49 must define any retained-history profile, including:

- the exact logical contract/version and DTO/wire mapping for retained
  transitions;
- durable storage and retention/redaction guarantees;
- parent/history structure, checkpoints, replay/verification, compaction, and
  crash recovery;
- how incomplete post-install evidence is reconciled truthfully; and
- optional Git association without making Git semantic identity.

Issue #50 owns offline causal metadata, DAG/clock mechanics,
resynchronization, and selective CRDT/OT. Issue #11 owns multi-document, host,
external-effect, durability, rollback, and recovery transaction semantics.

Until those decisions are Accepted, operation-log persistence remains an Open
Question and may not reopen ADR-0029's current-state authority or complete
standalone snapshot boundary.
