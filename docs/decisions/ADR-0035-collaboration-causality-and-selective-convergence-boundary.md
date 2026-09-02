# ADR-0035: Collaboration causality and selective convergence boundary

## Status

Accepted

Decision issue: [#50](https://github.com/nurockplayer/tachiko-work/issues/50)

Specified by: [Collaboration Model Specification](../specs/collaboration-model.md)

Related authority:
[ADR-0011](ADR-0011-semantic-three-way-merge.md),
[ADR-0018](ADR-0018-bound-formulas-and-deterministic-binary64.md),
[ADR-0019](ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0024](ADR-0024-revision-pinned-semantic-patch.md),
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md),
[ADR-0029](ADR-0029-current-state-authority-and-optional-history.md),
[ADR-0031](ADR-0031-semantic-merge-conflict-protocol.md),
[ADR-0032](ADR-0032-semantic-execution-and-transition-taxonomy.md),
[ADR-0033](ADR-0033-snapshot-first-semantic-history-and-checkpoints.md), and
[ADR-0034](ADR-0034-team-workspace-policy-and-recovery-boundary.md)

## Context

Tachiko Work must preserve a path to offline and realtime collaboration without
making the semantic model universally CRDT-shaped. The Accepted baseline already
keeps complete current-state snapshots authoritative, distinguishes revision and
optional history evidence, defines deterministic semantic merge and conflict
evidence, and requires every semantic publication to pass exact-base,
authorization, validation, and publication gates.

Offline collaborators still need to distinguish known causal succession from
concurrent work. Wall-clock timestamps and Git ordering cannot establish that
relationship safely, while automatic convergence is not valid for every semantic
datatype. Missing or corrupt collaboration history must also have a truthful
recovery path that does not invalidate a valid authoritative snapshot or invent
continuity.

This decision fixes only the logical causality and selective-convergence
boundary. It does not select a clock encoding, public or wire DTO, server or
transport topology, CRDT/OT library, compaction engine, or production
collaboration implementation.

## Decision

### 1. Causality is collaboration evidence, not another authority

Collaboration causality is optional evidence about the known ordering and
concurrency of collaboration activity inside one declared collaboration scope.
It is not semantic `Document` state, semantic object identity, mutation intent,
authorization, Approval, current-state admission, revision identity, retained-
history authority, wall-clock order, or Git identity.

A collaboration profile that claims offline causal continuity MUST distinguish:

- known causal succession, where one collaboration activity incorporates or
  descends from another within that profile's declared scope;
- causal concurrency, where sufficient valid evidence within the declared
  coverage establishes that neither activity descends from the other; and
- an unknown or broken relationship, where required evidence is missing,
  corrupt, unsupported, or outside declared coverage.

Unknown or broken continuity MUST NOT be guessed into succession or concurrency.
The profile fails or downgrades that collaboration guarantee and uses the
resynchronization boundary below. This ADR fixes those logical outcomes, not a
parent shape, DAG representation, vector or hybrid clock, replica identifier,
counter, epoch, or serialization.

### 2. Wall clock and Git do not establish causal order

A wall-clock timestamp alone MUST NOT establish global order, causal succession,
conflict precedence, or last-writer authority. Clock equality or comparison does
not resolve concurrent semantic intent. Timestamps may remain presentation,
diagnostic, retention, or provenance evidence only.

Git commits, parents, refs, branch names, repository order, and hosting facts are
likewise optional provenance or transport evidence. They MUST NOT become
collaboration causal identity or silently establish causal order. Rebase,
squash, mirroring, repository migration, ref movement, or Git absence therefore
does not by itself rewrite Tachiko collaboration causality or semantic meaning.

A collaboration adapter may associate genuine causal evidence with Git objects,
but the identities and guarantees remain independently interpretable. Bytes
obtained through Git re-enter ordinary Tachiko admission and validation.

### 3. The authoritative snapshot is the resynchronization root

The complete admitted current snapshot remains authoritative for semantic
meaning and is the root for full collaboration resynchronization. Missing,
corrupt, unsupported, compacted, or discontinuous causal evidence may reduce or
remove a collaboration-continuity guarantee, but it MUST NOT invalidate an
independently valid authoritative snapshot, fabricate history, or advance that
snapshot automatically.

When incremental continuity cannot be proved, a collaborator resynchronizes
from an exact complete admitted snapshot and establishes a new declared
collaboration boundary. An implementation may recover genuine causal evidence,
but it MUST NOT synthesize predecessors, clocks, activities, semantic revisions,
retained transitions, receipts, or Git associations merely to preserve a
continuity claim.

Resynchronization does not resurrect a prior runtime revision token or imply
that the new collaboration context is causally continuous with unavailable
history. ADR-0033 history coverage and gaps remain truthful independently of
the collaboration guarantee.

### 4. Automatic convergence is selective and capability-declared

Tachiko Work adopts no universal CRDT or OT model. Automatic convergence is
eligible only for presence or transient UI collaboration, or for an explicitly
declared and versioned text or ordered datatype whose later Accepted contract
defines:

- scope and identity;
- supported collaboration behavior;
- deterministic convergence and canonical projection where applicable;
- treatment of concurrent intent and user-visible conflict;
- validation and semantic-publication interaction;
- resynchronization and unsupported-version failure; and
- any retention or compaction guarantee needed by that capability.

An unrecognized capability, datatype, or version fails closed for that
collaboration behavior. Capability declarations do not become semantic fields,
authorization scopes, or transport negotiation formats by implication.

Presence and transient UI collaboration may use automatically convergent
techniques because they do not publish semantic `Document` meaning. Text or
ordered datatypes may become eligible only when a later Accepted decision names
the exact datatype and fixes the requirements above. This ADR accepts the
eligibility boundary, not any particular text or ordered datatype, algorithm,
or implementation.

### 5. Structured semantic meaning keeps ordinary merge and conflict

Structured semantic fields, schemas, formulas, references, and disputed human
intent are not automatically convergent datatypes under this decision.
Concurrent changes to those semantic surfaces continue to use the existing
typed semantic merge, deterministic Semantic Conflict evidence, and semantic
validation/calculation authorities.

Authorization and Approval state remain outside semantic `Document` state. They
are neither automatically convergent nor inputs to semantic merge; every use
remains subject to the live ADR-0026 authorization/Approval and ADR-0034 host-
authority boundaries.

Independent or same-final-value semantic facts may still compose under the
Accepted merge laws. A genuine semantic dispute is not erased by last-writer-
wins, timestamp order, replica order, Git order, or a generic convergence rule.
Cross-fact invalidity remains a semantic finalization failure rather than a new
causal or CRDT conflict kind.

Future Accepted work may promote one narrowly named text or ordered semantic
datatype only by proving that its convergence preserves Tachiko semantic
identity, invariants, conflict visibility, and validation. It MUST NOT use that
promotion to redefine the rest of the semantic model.

### 6. Synchronized semantic publication passes the existing gates

Collaboration activity, causal evidence, and convergent adapter state are not
Execute authority. Any result that would change canonical semantic meaning MUST
enter the existing semantic publication boundary as accepted typed intent or
state reconciliation against the exact current base.

Every synchronized semantic publication therefore continues to require:

- exact-base admission and stale-base handling;
- live authorization and Approval where required;
- ordinary deterministic semantic merge/conflict behavior where branches are
  reconciled;
- full semantic validation and calculation; and
- one truthful publication outcome under ADR-0032 and ADR-0034.

Admitted branch states may undergo the existing three-way reconciliation before
a new publication attempt. A stale SemanticPatch or Execute proposal itself
fails closed under ADR-0024; it is never implicitly rebased or merged, and a
proposal against a new base is a new proposal occurrence. Convergence validity
cannot grant authorization, bypass Approval, publish an invalid candidate,
manufacture `NoChange`, or rewrite a known publication outcome.

### 7. Causal, semantic, history, authorization, and Git identities stay separate

The following meanings MUST remain distinct:

- collaboration scope, activity, replica/participant, causal-boundary, and
  capability-version concepts introduced by a future concrete profile;
- `DocumentId`, semantic object identity, and snapshot/content identity;
- revision context/domain and `RevisionOccurrenceRef`;
- retained transition, history segment, and checkpoint identity;
- principal, Grant, Approval, receipt, and authorization-policy identity; and
- Git object, ref, repository, and host identity.

This ADR does not mint any identifier or version string for the first group.
No value in another group may be substituted for collaboration causal identity
merely because an implementation stores or displays them together.

### 8. Concrete mechanisms remain separately owned

This decision authorizes no production implementation.

Separately Ready decision or implementation Issues must own any concrete:

- causal parent, DAG, clock, replica, activity, epoch, or boundary encoding;
- public API, wire DTO, persistence record, transport protocol, or SDK contract;
- peer-to-peer, client/server, relay, presence, session, or service topology;
- text/ordered CRDT/OT datatype, algorithm, library, contract, or adapter;
- causal retention, garbage collection, compaction, checkpoint, or repair
  engine;
- synchronization, realtime collaboration, conflict-resolution UI, or
  operational tooling; or
- new `.ro`, `.roproj`, checkpoint, history, or Git-mapping format.

Issue #47 retains cross-version migration and branch behavior. Issue #53 retains
exact commitment bytes, digest/signature algorithms, and trust semantics.
Concrete collaboration realization must preserve ADR-0029 through ADR-0034 and
requires separately Ready work.

## Required pressure tests

Future concrete contracts and implementations must preserve these outcomes:

1. **Concurrent structured value edits** — different offline edits to one
   stored semantic value produce ordinary deterministic semantic conflict
   evidence; timestamp, replica, or Git order does not select a winner.
2. **Independent semantic edits** — causally concurrent edits to independent
   semantic facts may compose only through the existing semantic merge and
   finalization laws.
3. **Presence remains non-semantic** — converging cursors or participant
   presence does not create a semantic revision or grant publication authority.
4. **Causal gap with a valid snapshot** — corrupt or missing causal evidence
   downgrades incremental collaboration while the independently valid snapshot
   remains usable and becomes the resynchronization root.
5. **Stale synchronized publication** — a stale SemanticPatch or Execute
   proposal fails closed without implicit rebase; admitted branch states may
   reconcile separately before a new exact-base proposal and publication
   attempt.
6. **Invalid converged candidate** — structural convergence that violates a
   schema, formula, or reference invariant fails semantic finalization and is
   not published.
7. **Authorization remains live** — causal descent or convergence cannot replace
   a live Grant, required Approval, or publication-bound authorization check.
8. **Git rewrite is non-causal** — squash, rebase, ref movement, or repository
   migration changes Git evidence without silently changing collaboration
   causality or semantic meaning.
9. **Unsupported capability fails closed** — an unknown datatype or capability
   version does not receive generic automatic convergence.
10. **No fabricated continuity** — resynchronization after a gap starts a
    declared new boundary unless genuine evidence proves continuity.

## Consequences

- Offline collaboration can distinguish causal succession, concurrency, and
  broken continuity without defining a universal global clock.
- Complete authoritative snapshots remain independently usable and provide a
  deterministic full-resynchronization boundary.
- Presence and future narrowly accepted text or ordered datatypes may evolve
  without making the whole semantic model a CRDT.
- Structured semantic disputes keep the existing deterministic merge,
  conflict, authorization, and validation behavior.
- Git remains compatible as optional transport/provenance without becoming
  causal identity.
- Production collaboration mechanics remain separately gated.

## Rejected alternatives

- **Universal CRDT/OT semantic state:** rejected because convergence alone does
  not preserve semantic validity, authorization, or disputed intent.
- **Last-writer-wins for structured semantic disputes:** rejected because
  timestamp, replica, or transport order would silently discard intent.
- **Wall-clock timestamps as global causal order:** rejected because clock order
  does not prove causal descent and cannot safely classify concurrency.
- **Git history as collaboration causality:** rejected because Git is optional,
  many-to-many evidence that may be rewritten, migrated, or absent.
- **Causal history as current-state authority:** rejected because a gap or
  corrupt collaboration log must not invalidate or replace a valid snapshot.
- **Convergence as authorization or validation:** rejected because adapter
  agreement cannot grant authority or make an invalid semantic state publishable.
- **Selecting concrete clocks, DTOs, topology, libraries, or compaction here:**
  rejected because Issue #50 authorizes only the logical authority boundary.

## Related

- [Issue #50](https://github.com/nurockplayer/tachiko-work/issues/50)
- [Collaboration Model Specification](../specs/collaboration-model.md)
- [Distributed Collaboration Architecture](../architecture/distributed-collaboration.md)
- [Conflict Resolution Specification](../specs/conflict-resolution.md)
- [Semantic Operation Log Model](../specs/operation-log-model.md)
- [Decision traceability protocol](../governance/decision-traceability.md)
