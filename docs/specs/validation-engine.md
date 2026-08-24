# Validation Engine Specification

Decision state: Mixed. The staged validation semantics, candidate/finalization
boundary, full-validation oracle, deterministic extension rules, and diagnostic
meaning follow [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md).
Stable identity and formula semantics remain governed by
[ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md) and
[ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md).
ADR-0020 makes authoritative validation/gate meaning part of the first-class
Semantic API result contract without changing these validation stages. Exact
Rust APIs and incremental mechanisms remain Provisional.

Implementation state: implemented for the Milestone 02 first-party boundary.
`semantic-core` emits generic semantic-first diagnostic primitives and core
rules; `formula-engine` exposes the complete ADR-0018 node-keyed failure oracle;
and `workspace-engine` composes one authoritative deterministic
`ValidationReport` for validation, queries, mutations, and merge finalization.
Incremental validation, external wire encoding, and future extension
registration remain Provisional or Deferred.

See the [diagnostics contract](diagnostics-contract.md), the
[Semantic API specification](semantic-api.md), and the
[canonical reconciliation register](../governance/canonical-reconciliation-register.md).

## Purpose

Define one authoritative semantic validation model that can serve CLI, CI, AI,
and future graphical clients without creating client-specific semantic rules.

Validation is not a second source of schema meaning. Schema declarations and
Accepted semantic contracts define meaning; validators derive deterministic
findings from that meaning.

## State boundary

Validation operates after hard admission has produced a structurally admissible
semantic candidate.

```text
raw authoring / representation
        |
        | admission / parse / bind / construction
        v
structurally admissible semantic candidate
        |
        | validation / operation gate
        v
validated / finalized semantic snapshot
```

Raw authoring may be incomplete or malformed and is not semantic state.
Accepted intrinsic representability barriers such as coherent stable semantic
identity, finite `Number`, and bounded valid bound expressions are not weakened
into ordinary editor diagnostics.

A structurally admissible candidate may contain higher-level semantic failures
that are useful to diagnose during an interactive workflow. Existing strict
workspace commands may still reject such candidates atomically.

ADR-0020's Atomic Command Batch may use a structurally admissible working
candidate across ordered internal command steps without applying the final
operation gate after every step. Intermediate higher-level invalidity is allowed
only when intrinsic admission/representability invariants remain satisfied and
the final batch candidate passes the authoritative gate required for
publication.

Resident editor state, recovery/autosave, and invalid-draft persistence are not
defined here.

## Authoritative stages

The validation authority/prerequisite order is:

### Stage 0: Admission / representation prerequisites

Owned by the relevant parser, constructor, or representation boundary.
Examples include UTF-8/JSON/version admission, formula source parsing/binding,
and Accepted primitive construction limits.

Failure may occur before a semantic candidate exists. Storage preserves its
ADR-0017 representation-local error precedence and does not become the
universal diagnostics model.

For the Semantic API, a newly authored command that fails these prerequisites
is a pre-candidate admission/command failure rather than a `ValidationReport`
claim that an admissible candidate exists.

### Stage 1: Intrinsic semantic declaration invariants

Validates semantic facts that must be interpretable independently of a
particular schema instance or client projection, including Accepted stable
identity/coherence requirements and deterministic human-address ambiguity where
applicable.

This stage must not invent new durable schema vocabulary.

### Stage 2: Schema-instance conformance

Validates the relationship between an entity and its declared schema, including
currently represented requirements such as:

- referenced schema existence;
- required field presence;
- unexpected fields where the current closed semantic model requires it; and
- declared field type compatibility.

If the schema prerequisite is unavailable, dependent field checks are
suppressed rather than guessed.

### Stage 3: Semantic relationship validation

Validates typed semantic relationships, including stored entity references and
their target-schema compatibility.

Missing targets remain identified by stable semantic identity. A reused human
key does not silently retarget a relationship.

### Stage 4: Formula static / graph validation

Owned by ADR-0018 formula semantics. The semantic precedence is:

```text
structural
  -> binding / type / stale target
  -> cycle
```

Formula graph cycle meaning is ADR-0018 SCC membership. A particular DFS cycle
witness is not the semantic contract.

Generic reference cycles outside the formula dependency graph are not declared
invalid merely by this specification.

### Stage 5: Formula evaluation

Owned by ADR-0018. After earlier eligible formula checks, evaluation failures
follow:

```text
failed dependency
  -> local evaluation
```

The accepted full-recompute oracle, node-keyed failures, direct failed
dependency sets, and no-partial-`CalculationState` publication remain formula
authority. `calculate_complete()` exposes that authority; the fail-first
`CalculationError` family remains only a compatibility projection and is not a
new validation or Semantic API failure contract.

### Stage 6: Domain validation

Pure deterministic rules may validate domain meaning such as game-balance or
business constraints when those rules have an explicit semantic authority.

Domain rules do not implicitly become schema-core vocabulary and may not
reinterpret earlier Accepted identity/type/reference/formula semantics.

### Stage 7: Extension validation

Future deterministic extensions may add findings through the shared diagnostic
contract. Registration/runtime mechanics remain outside this specification.

## Accumulation and cascade suppression

Validation follows this rule:

> Fail when a prerequisite makes a subject unavailable; otherwise accumulate
> independent findings and continue validating independent subjects.

A missing schema, for example, may prevent meaningful field-type checks for that
entity while leaving other entities eligible for validation.

The goal is useful complete evidence, not maximum diagnostic count.

## Full-validation correctness oracle

A complete deterministic validation over a semantic snapshot is the correctness
oracle.

For the same semantic snapshot and deterministic validator configuration,
stable validation observations must be equivalent across supported native/WASM
targets.

Incremental validation is permitted only as an optimization. It must produce
observationally equivalent stable results to the full oracle. If affected state
cannot be classified safely, the implementation falls back to full validation.

The following are part of stable equivalence where applicable:

- diagnostic code meaning;
- machine-readable classification concept;
- semantic subjects;
- semantically relevant related subjects/facts;
- validator provenance; and
- formula semantic outcomes already fixed by ADR-0018.

Localized prose, source spans, rendered human paths, UI grouping, terminal
formatting, and selected cycle witness paths are not semantic-equivalence
criteria.

Specific dependency indexes, dirty-set algorithms, caches, or schedulers remain
Provisional.

## Operation gating

A validation report describes semantic findings. A gate decides whether a
specific operation may finalize/publish a candidate.

These concepts are distinct:

- an interactive editor may eventually retain a diagnosable semantic candidate;
- a strict semantic mutation may reject that same candidate atomically;
- runtime export requires the semantic/calculation facts required by the export
  contract;
- an approved AI mutation must not bypass the deterministic gate used by the
  corresponding first-party semantic operation; and
- CI/workflow policy may impose stricter treatment of advisory findings without
  changing diagnostic identity.

Under ADR-0020, the Semantic API exposes the authoritative gate outcome wherever
client control flow needs to know whether a proposed/executed semantic operation
may publish. The client must not re-derive that decision from severity, report
emptiness, localized messages, or a duplicated validation policy.

`Propose` and `Execute` share the same semantic command and gate meaning. Execute
must evaluate the authoritative gate for the state it actually acts on rather
than trusting a stale earlier client-side gate result.

This specification does not make physical Git commit rejection a semantic
invariant.

It also does not declare that every diagnostic Error automatically blocks every
storage save API. ADR-0017 requires semantic validation as required by the
operation; representation-specific save behavior and future invalid-draft
persistence remain separately owned.

## Atomic batch finalization

ADR-0020 defines an Atomic Command Batch as one candidate transition with
all-or-nothing semantic publication.

Validation/finalization therefore applies to the final candidate according to
the operation's authoritative gate. A conforming implementation may evaluate
validation during intermediate steps for diagnostics or optimization, but that
must not turn a temporary higher-level invalid working candidate into partial
published semantic state.

The batch contract does not define resident transaction sessions, concurrency,
revision conflict resolution, or persistence rollback; those remain #26 and
host/representation concerns.

## Deterministic domain/extension validator boundary

A validator participating in authoritative semantic validation must be:

- read-only over semantic state;
- deterministic for the same semantic snapshot and deterministic
  configuration;
- independent of implicit filesystem access;
- independent of implicit network access;
- independent of wall clock;
- independent of locale;
- independent of process environment;
- independent of ambient randomness; and
- independent of thread completion order.

It may add findings but may not mutate semantic state or redefine Accepted core
semantics.

External or nondeterministic checks may be useful, but they belong to a separate
host/workflow result family unless their inputs become explicitly pinned by a
future Accepted protocol.

## Diagnostics

All first-party semantic validation consumers share the semantic diagnostic
contract in [diagnostics-contract.md](diagnostics-contract.md).

Clients must not define semantic validity by parsing human error strings.
Presentation and transport adapters may render/project the same semantic
finding differently.

ADR-0020 makes the stable diagnostic observations and gate relationship part of
the transport-neutral Semantic API result meaning while leaving the concrete
wire mapping to #26.

## Current implementation boundary

The Milestone 02 implementation now:

- uses one `workspace-engine::ValidationReport` for first-party semantic
  validation and formula outcomes;
- accumulates independent findings while suppressing checks whose subject
  prerequisites are unavailable;
- orders stable machine observations deterministically and treats human paths,
  messages, and cycle witnesses as presentation;
- derives formula diagnostics from the complete ADR-0018 outcome; and
- keeps canonical authoring projection and other output/finalization preflights
  as explicit operation gates layered after shared semantic validation.

This is strong implementation evidence for ADR-0020 but does not make the exact
Rust report, `WorkspaceError`, or current function signatures the public API.

Incremental scheduling/caching, concrete external wire mapping, invalid-draft
lifecycle, extension registration, and resident-runtime delivery remain owned
by their existing deferred decisions rather than this implementation.

## Explicitly not defined here

- generic schema validation-rule DSL;
- generic enum/range/pattern/default semantics;
- progressive/freeform typing (#13);
- exact Semantic API Rust/wire representation (`semantic-api.md` / #26);
- native/WASM/IPC delivery protocol or resident runtime (#26);
- plugin ABI/runtime/sandbox (#17);
- `.roproj` invalid-draft persistence (#41);
- diagnostic suppression, baselines, fingerprints, or fix-it protocol; and
- cross-document/project validation.

## Goal

Keep semantic truth strong while allowing future editing workflows to diagnose
and repair temporarily invalid candidates through one deterministic validation
model and one first-class semantic operation boundary.
