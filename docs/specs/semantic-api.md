# Semantic API Specification

Decision state: Mixed. The first-class boundary and semantic laws are Accepted
under [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md).
The immutable, revision-pinned SemanticPatch proposal contract and exact-change
binding law are Accepted under
[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md).
The authorization, stable-ID scope, trusted footprint, exact Human Approval,
and provenance laws that consume this operation/proposal meaning are Accepted
under [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md).
Runtime ownership, resident interactive topology, host separation, explicit
snapshot boundaries, and native/WASM semantic parity are Accepted under
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).
The M04 logical formula-reasoning Query, read-only scenario Query, typed
formula-update Command, and bounded semantic analysis Query semantics are
Accepted by ADR-0020's Issue #32 and Issue #33 amendments.
Exact Rust APIs, complete operation catalogue, wire schemas, transports,
proposal/revision encodings, session mechanics, and several result/projection
shapes remain Provisional or Deferred as marked below.

Implementation state: partially implemented through `workspace-engine` as the
shared first-party application authority. Its provisional `patch_lifecycle`
module now implements Issue #29's immutable SemanticPatch envelope, the current
stable-ID field-value and FormulaUpdate Command families, ordered non-empty
AtomicBatch evaluation,
scoped preview, exact finite Human Approval, atomic publication/consumption,
verification against the immutable installed snapshot captured by the guarded
publication result, and receipts. Current Rust functions and result structures
are implementation evidence, not the versioned public product contract. The
lifecycle remains snapshot-style and receives opaque revision/current-state
publication mechanics from a trusted host seam; the concrete resident runtime,
revision token, concurrency, and state-installation implementation remains
later work under #93–#95. Issue #30's provisional `ai-api` boundary now obtains
effective identity/time only from trusted host context, requires the lifecycle
to prove an active Delegated occurrence, delegates typed proposal/execution to
it, and projects disclosure-safe machine outcomes while rejecting raw mutation
and host effects. The complete operation catalogue, authentication, transports,
and wire/SDK contract remain Provisional or Deferred.
Issue #144 supplies the first provider-neutral, Provisional workspace/CLI
implementation of the M04 formula-reasoning Query, exact-snapshot read-only
Number-override scenario Query, and typed formula-update Command. It reuses the
authoritative calculation, dependency, validation, diff, authorization,
SemanticPatch, Approval, and publication paths. Focused workspace/CLI tests and
the portable native/WASM corpus record the implementation evidence, including
bounded admission, disclosure projection, unchanged scenario source state,
atomic no-publication failures, complete bound FormulaUpdate meaning, and the
existing exact Human Approval path. The Rust types, CLI spelling, result JSON,
limits, content-derived opaque CLI revision tokens, and host composition remain
Provisional; this implementation does not establish a public wire or SDK
contract.
The Issue #33 amendment accepts only the logical bounded semantic analysis Query
contract below. Issue #150 supplies its first provider-neutral, Provisional
workspace/CLI implementation and native/WASM evidence without changing the
Accepted meaning or stabilizing the concrete Rust, CLI, result, or wire shapes.

Decision issues: [#10](https://github.com/nurockplayer/tachiko-work/issues/10),
[#27](https://github.com/nurockplayer/tachiko-work/issues/27),
[#28](https://github.com/nurockplayer/tachiko-work/issues/28),
[#32](https://github.com/nurockplayer/tachiko-work/issues/32),
[#33](https://github.com/nurockplayer/tachiko-work/issues/33)

## Purpose

Define the smallest transport-neutral semantic command/query/result contract
that GUI, CLI, AI, automation, and future first-party clients must share,
including the representation-neutral proposal contract that binds review to one
exact semantic change and semantic base.

This specification answers **what semantic operations mean**. ADR-0022 answers
the durable runtime/host ownership rules. Neither specification freezes how
requests/results are serialized over native, WASM, IPC, FFI, or network
transports or the concrete session/revision protocol.

## Contract boundary

Conceptually:

```text
GUI / Web / Tauri / Mobile
CLI / CI / Automation
AI / Agents
future first-party integrations
future first-party plugin host
        |
        v
+-----------------------------------+
| First-class Semantic API          |
|                                   |
| Queries                           |
| Semantic Commands                 |
| Propose / Execute                 |
| Revision-pinned SemanticPatch     |
| Atomic Command Batch              |
| ValidationReport + Gate Outcome   |
| Stable machine outcomes           |
| Capability-addressable operations |
+-----------------------------------+
        |
        v
workspace-engine
   /       |        \
semantic  formula   diff/merge
 core      engine    engines

storage / filesystem / IndexedDB / Git / IPC / WASM bridge
= representation/host/transport concerns,
  not alternate semantic authority
```

`tachiko-workspace-engine` currently implements the application boundary under
ADR-0016. The diagram does not make its source-level Rust API the external
contract.

For interactive clients, ADR-0022 places authoritative in-memory semantic state
in the shared Rust semantic/application runtime and forbids a second
independently authoritative frontend document model. Host/storage capabilities
remain beside that runtime rather than becoming semantic authority.

## Mandatory first-party client rule

A first-party client MUST use the Semantic API contract when it:

- reads product-semantic facts;
- requests semantic validation, explanation, calculation, comparison, or impact;
- proposes a semantic change; or
- requests publication of a semantic change.

A first-party adapter MUST NOT reproduce alternate semantic mutation,
validation, formula, diff, merge, or gating policy.

Different invocation/transport mechanisms are allowed when they preserve the
same semantic contract.

## Semantic context

Queries and commands operate against a semantic context/snapshot whose semantic
identity and content are governed by the existing semantic model and Accepted
ADRs.

This specification does not freeze a new public `Workspace`, `Project`, session,
or revision type. Milestone 02 semantic references remain document-local where
ADR-0015 says they are document-local. ADR-0024 requires every reviewable
proposal to bind one exact semantic context revision, while ADR-0022 accepts a
resident shared Rust runtime as the preferred interactive topology. The exact
session handle, revision/precondition representation, concurrency/conflict
policy, cancellation, and runtime state-installation mechanics remain Deferred
to #93 and related runtime work.

## Stable targeting

Durable API targeting uses stable semantic identity as defined by ADR-0015.

Human-facing keys, labels, authoring paths, formula-source addresses, source
spans, UI coordinates, storage paths, and collection indexes MAY be accepted or
returned as authoring/presentation projections when an operation explicitly
supports them. They MUST NOT silently become durable target identity.

Representation addresses such as JSON Pointer or `.roproj` paths are not the
generic semantic mutation API.

## Query contract

A Query:

1. reads semantic facts from a semantic context/snapshot;
2. MUST NOT publish a change to canonical semantic state;
3. is deterministic when its authoritative inputs/configuration are
   deterministic; and
4. returns use-case semantic results/projections rather than requiring clients
   to depend on internal Rust aggregate layout.

Queries MAY expose concepts such as:

- semantic object inspection/description;
- validation reports and gate inspection;
- calculated values;
- formula analysis/explanation;
- semantic diff/comparison/impact;
- merge/reconciliation inspection where the operation is read-only; and
- domain-specific or bounded analysis queries implemented above generic
  semantic foundations.

This list is illustrative. The complete externally Stable operation catalogue
is Provisional and is promoted operation-by-operation.

A generic `get(path)` / JSON-pointer query surface is not part of the Accepted
contract.

## Command contract

A Command expresses a typed semantic intent.

A conforming semantic command:

- targets semantic identity rather than storage/layout coordinates;
- supplies typed semantic input rather than arbitrary representation patches;
- is evaluated by the shared application authority;
- applies relevant semantic preconditions;
- forms a candidate transition according to Accepted semantic/formula rules;
- participates in authoritative validation/calculation/gating where required;
- and follows the atomic publication rules below.

An intent equivalent to changing a typed field value by `EntityId + FieldId` can
be a semantic command. An arbitrary mutation of an internal Rust field or JSON
path is not the stable product contract merely because an adapter can express
one mechanically.

## M04 formula reasoning and scenario Queries

ADR-0020 promotes the following logical M04 operations without freezing their
external names, DTOs, Rust layout, transport identifiers, or placement in a
complete API catalogue.

### Formula reasoning Query

A bounded formula-reasoning Query targets one formula-valued field by stable
semantic identity in one semantic context. Its structured result preserves, as
requested and applicable:

- the target stable identity;
- the complete typed bound expression meaning, including typed operators,
  normalized literals, and stable references, without exposing an internal
  Rust AST layout as public API;
- stable-ID direct inputs and direct dependents;
- the authoritative ADR-0018 calculation outcome;
- deterministic dependency and affected-subject facts; and
- applicable ADR-0019 validation/diagnostic facts.

Human keys and canonical authoring text MAY accompany that result as
presentation projections. They do not replace stable targeting or the bound
expression meaning, and ADR-0018 projection failure emits no fabricated
copyable source. Optional natural-language explanation consumes this
structured result; it is not an authoritative result field and may neither
recalculate nor override it.

The shared application authority obtains every calculation, dependency,
impact, and validation fact from the existing semantic/formula engines. A
first-party client or AI adapter MUST NOT maintain an independent evaluator,
dependency graph, or validation path.

### Read-only scenario Query

The logical M04 scenario request contains:

```text
exact source semantic revision/context, including deterministic validation context
+ bounded ordered collection of typed Number overrides
+ bounded set of requested stable result/inspection targets
```

Before resolving any semantic identity, the trusted application boundary MUST
perform bounded, disclosure-independent envelope admission using request-local
facts only. It enforces the applicable finite request profile, structural shape,
duplicate override targets, duplicate requested-target normalization, and
ADR-0018 Number representation/normalization. These checks MUST NOT dereference
a source or target, inspect semantic state, or expose anything beyond the
caller-supplied invalidity. Exact thresholds and representation remain
Provisional.

After envelope admission and before semantic classification or external
exposure, the boundary performs internal, non-disclosing resolution against the
exact source snapshot only as needed to derive the actual document-scope
occurrence and applicable ADR-0026 disclosure-scope atoms, including actual
`EntityField` schema membership. It derives and enforces Query authority from
those trusted facts; request identities or caller-supplied membership MUST NOT
establish scope. If the document-scope occurrence cannot be proven, the
boundary fails closed. If that occurrence is proven but a narrower actual scope
cannot be derived safely, it fails closed or requires broader explicit scope
within that same occurrence. Without sufficient actual or broader source and
override coverage, it returns one disclosure-safe scenario denial without
source- or override-specific facts and before source/target classification,
candidate derivation, calculation, or validation.

After authorization, each override is semantically classified and must identify
by stable semantic identity one existing field whose current semantic value is
a Number, not a Formula. Each supplies one admitted ADR-0018 `Number`;
normalization preserves request order. Duplicate override targets, non-finite
Numbers, and requests beyond the applicable finite profile fail envelope
admission. Missing or wrong-typed override targets fail only after authorized
semantic classification.

Requested result/inspection targets form a stable-identity set. Duplicate
requested-target occurrences normalize to one member, and target request order
has no semantic meaning. Exact result ordering is a Provisional projection
detail.

After the scenario envelope is admitted, each normalized requested target
follows the same internal non-disclosing resolution, actual-scope derivation,
and Query-authorization order above. Only sufficient actual scope, or broader
explicit scope within the same proven document occurrence when narrower actual
target scope cannot be derived safely, permits semantic classification or
external exposure. The target may then be classified against the exact source
snapshot. If it resolves to an existing semantic subject and supported M04
formula-reasoning result/inspection facet, it yields the applicable requested
structured facts; otherwise it yields structured unresolved-target or
unsupported-kind failure evidence preserving the requested stable identity and
expected/actual kind where applicable. An identity absent from the exact source
snapshot yields the unresolved-target family without claiming whether it never
existed or was removed elsewhere; target-history lookup is not authoritative
scenario input. Without sufficient actual or broader coverage, the target
instead yields one disclosure-safe denial without target-specific facts. Each
target therefore has exactly one outcome; one unsuccessful target does not
suppress outcomes for independently resolvable targets. The application
authority MUST NOT silently omit, retarget, or resolve a requested target
against another revision.

The source context pins the effective deterministic ADR-0019 validator
configuration. That same configuration governs baseline and transient-candidate
validation; ambient host validator state is not an authoritative scenario
input.

The application authority:

1. resolves the exact source snapshot without rebasing it;
2. derives one transient candidate and applies the normalized overrides as one
   hypothetical input set, not as sequential publications;
3. runs the authoritative full formula-calculation oracle and applicable full
   semantic validation needed for the baseline and candidate outcomes, using
   the pinned validator configuration for both; and
4. returns structured baseline/scenario evidence without publishing or
   persisting the candidate.

Before returning any scenario evidence, the trusted application boundary
derives disclosure requirements from the complete underlying outcome and the
trusted source/candidate relationships for every subject or fact the projection
would reveal. This includes baseline and scenario outcomes, changed or affected
subjects, dependencies, impact, validation/diagnostic facts, and their related
stable evidence. It enforces applicable ADR-0026 Query coverage for all of them.

Evidence outside live coverage MAY be safely reduced only when the reduction
reveals no unauthorized identity, existence, kind, relationship, diagnostic, or
other semantic fact and does not present an incomplete subject/fact set as
complete. If the required scenario or explanation meaning cannot survive that
reduction, the affected target outcome or entire scenario projection MUST be a
disclosure-safe denial. Exact redaction/projection mechanics and wire spelling
remain Provisional.

The authorized result projection preserves, as applicable:

- the exact source revision/context reference and validation
  configuration/provenance sufficient to distinguish any configuration change
  that can change returned validation facts;
- the normalized ordered stable-target/typed-Number override set;
- authoritative baseline and scenario formula outcomes for the requested
  subjects;
- deterministic changed and affected stable subjects available from the
  current engines;
- applicable validation/diagnostic outcomes; and
- dependency facts sufficient to explain why a requested result changed.

Before ADR-0026 disclosure projection, equal exact source revision, effective
deterministic validator configuration, normalized overrides, and requested
targets produce equal underlying semantic scenario outcomes. Live Query
authority may safely reduce exposed evidence or replace an affected target or
scenario outcome with a disclosure-safe denial; it MUST NOT change the transient
candidate or underlying outcome, and principal/Grant state is not scenario
meaning. The normalized override collection preserves override request order as
reproducibility evidence; it does not make the overrides sequential.
Requested-target order remains non-semantic, and output ordering is
projection-only. Exact normalization encoding, finite limits, validator
configuration/profile identifiers, revision-token encoding, result field
names, and wire representation remain Provisional.

Invalid scenario input, source formula failure or cycle, candidate calculation
failure, or validation failure returns the applicable structured failure facts
and publishes nothing. Requested-target failures return the per-target outcomes
defined above and likewise publish nothing. A scenario is neither canonical
state nor a SemanticPatch, branch, transaction, saved object, or mutation
proposal.

Formula/schema/structure mutation inside a scenario, parameter sweeps,
optimization, solver/statistical behavior, randomness, and persisted scenario
objects are Deferred.

Model-generated prose, provider/model identity, wall-clock time, Git metadata,
UI coordinates, and storage paths are not scenario meaning.

## M04 formula-update Command

One logical formula-update Command expresses normal typed semantic mutation.
Its exact semantic meaning contains:

- one stable formula target identity;
- the complete accepted typed bound formula expression;
- every stable reference in that expression; and
- every command-owned semantic precondition required by the Accepted formula
  contract.

Formula authoring source is bounded input to parse, bind, and type-check against
the exact semantic base. Failure at that boundary creates no admissible formula
Command. A successful authoring projection is not the proposal meaning: the
complete typed bound expression and stable references are fixed before ADR-0024
proposal identity is issued.

Bounded source-shape or syntax evidence that requires no semantic lookup MAY
describe request-local facts only. For target/reference resolution, binding, or
type-checking against the exact base, the trusted application boundary performs
the work internally without disclosure, derives the actual ADR-0026 Query
requirements for every semantic subject or fact in the resulting authoring or
admission evidence, and enforces live coverage before external projection.
Without sufficient coverage it MUST return a disclosure-safe admission denial
without subject-specific evidence. Internally retained diagnostics do not grant
their external disclosure, and an authoring/admission failure issues neither an
admissible Command nor a proposal occurrence. Propose authority never
substitutes for this Query coverage.

Review and execution use only the existing lifecycle:

```text
formula-update Command
  -> Propose
  -> immutable revision-pinned SemanticPatch
  -> candidate / semantic diff / dependency impact / validation
  -> authorization and exact Approval when required
  -> Execute
```

Changing the target, bound expression, any stable reference, command-owned
precondition, semantic base, or batch order changes ADR-0024 exact-change
meaning and cannot reuse the same proposal occurrence. Invalid, rebound,
stale-target, or cycle-inducing formula updates are rejected by the existing
formula, validation, stale-base, and operation-gate contracts and publish
nothing.

The trusted application layer applies the authoring/admission disclosure law
above, derives Formula-class write requirements, and derives every disclosure
scope exposed by diff, dependency, calculation, or validation evidence. Query
authority for formula reasoning or scenarios does not imply Propose or Execute.
Propose does not imply Query or Execute. Preview outside live Query authority is
denied or safely reduced. Delegated-origin or Delegated-authority Execute uses
ADR-0026's existing exact finite Human Approval; successful validation or
scenario evaluation grants no authority.

There is no `FormulaPatch`, formula-specific approval token, AI-only mutation
API, or second formula proposal vocabulary.

## M04 semantic analysis Query

ADR-0020's Issue #33 amendment promotes one bounded, typed, provider-neutral
Analysis Query family so population selection, predicate evaluation, grouping,
and the accepted reductions remain shared semantic behavior rather than client
or LLM reconstruction.

### Normalized analysis definition

An Analysis Query supplies one context-independent normalized analysis
definition and one exact semantic context as its execution input. The normalized
definition contains:

```text
one schema/type entity domain
+ optional bounded explicit stable-EntityId narrowing set
+ bounded AND-only typed field predicates
+ zero or one stable FieldId grouping key
+ one or more supported result requests
```

The exact semantic context is not part of normalized analysis definition
identity. The explicit EntityId set is optional. When supplied, it MUST narrow
the trusted schema/type population to the intersection with those stable
identities. The shared application boundary resolves every supplied identity
against the supplied exact semantic context and verifies that it belongs to the
declared schema/type domain. After sufficient Query authority, an unresolved
identity or an identity outside that domain yields the applicable structured
target/domain failure and MUST NOT be ignored as a non-match. Without sufficient
Query authority, the caller receives only a disclosure-safe denial. The
narrowing set MUST NOT establish membership, semantic scope, or Query authority.

Each predicate is a typed field/operator/operand constraint. It is evaluated
against authoritative effective semantic values only after the preauthorization
coverage described below succeeds. M04 accepts the bounded conjunction shape,
not a general boolean query AST. The exact finite supported predicate-operator
catalogue and request limits remain Provisional for the first implementation
slice; an implementation MUST NOT introduce representation-path matching,
untyped coercion, arbitrary expressions, or another evaluator as an
implementation shortcut.

For a predicate whose operator requires a Number on a Number-typed field, the
effective predicate value is the stored finite Number when present, or the
ADR-0018 authoritative calculated Number when the field stores a Formula. A
formula-backed predicate therefore reuses the same formula calculation and
failure semantics as a formula-backed result metric; it MUST NOT inspect formula
source text or evaluate through a second expression engine. Formula calculation
failure is a structured analysis failure, not a predicate `false` result and not
a silently skipped entity. The predicate's calculation/dependency facts are
part of the trusted disclosure footprint.

If an entity in the candidate domain omits the targeted optional predicate
field, that predicate evaluates `false` for that entity. Absence is not coerced
to a typed operand, exposed as a synthetic value, or treated as an error for
predicate selection.

The optional grouping key is one stable FieldId from the selected domain.
M04 grouping uses present, supported, non-Formula typed semantic values and
their authoritative equality meaning. A grouping field that stores a Formula
is unsupported: the grouped analysis returns a structured unsupported-grouping
failure and MUST NOT group by formula structure or by its calculated Number.
If any selected entity omits the grouping field, the grouped analysis returns a
structured missing-group-value failure; it MUST NOT drop the entity, synthesize
a null/absent group key, or silently place it in another group. Multi-key
grouping, grouping sets, query-defined bucketing, formula-valued grouping,
joins, subqueries, and windows are Deferred.

When a grouping key is present, grouping partitions the complete selected
population before result reduction. Every requested result primitive is then
evaluated independently **per group** over that group's members or Number
observations. A grouped request does not additionally return a global Count,
Min, Max, membership set, or per-member observation collection for the entire
selection. A caller that needs a global reduction issues the same normalized
analysis without a grouping key. When exact membership or per-member Number
observations are requested in a grouped analysis, they are returned within
their group result; the union of group memberships is the complete selected
membership.

### Supported result primitives

The Accepted M04 result primitives are:

- exact selected stable-EntityId membership when explicitly requested and
  authorized;
- exact `Count` of the selected membership;
- Number `Min` and `Max` over authoritative finite Number observations; and
- bounded per-member `(EntityId, effective Number)` observations that a
  renderer or client may use for charts or ranking inputs.

A requested Number metric may consume a stored Number field or an authoritative
calculated Number field. Formula-backed metrics MUST use ADR-0018 calculation
meaning and its deterministic failure semantics. Analysis MUST NOT introduce a
second expression or formula evaluator. Requested metric completeness is
operation-wide for one normalized Analysis Query. If any selected member or
group cannot supply a complete requested metric because the value is missing,
wrong-typed, unsupported, or calculation-failed, the entire operation returns
one structured analysis failure and no successful membership, group, `Count`,
`Min`, `Max`, or per-member observation payload. The member or group MUST NOT be
silently discarded from selection or reduction. This failure is distinct from
the separately defined empty-aggregate outcome for a valid zero-observation
selection.

`Count`, `Min`, and `Max` are Accepted in M04. `Sum`, `Mean`, weighted mean, and
other floating reductions remain Deferred until a deterministic reduction law
is separately accepted. Ranking/top-k, outliers, percentiles, statistical
semantics, optimization, and other higher analytics are Deferred. Returning
per-member numeric observations does not make a caller's ranking or chart
projection authoritative semantic analysis behavior.

An empty selected population has one deterministic meaning. `Count` returns
exactly zero. An ungrouped `Min` or `Max` request over zero authoritative Number
observations returns a structured empty-aggregate outcome; it MUST NOT return a
fabricated Number, `null` presented as a Number, omit the requested result, or
silently reuse a prior value. Grouping synthesizes no empty group: groups arise
only from selected members with present grouping values, so an empty selection
returns an empty group collection. Exact public error/result code spelling for
the empty-aggregate and missing-group-value outcomes remains Provisional.

Exact selected membership, grouped result collections, and per-member Number
observations are bounded collection results. After trusted selection/grouping
and sufficient Query authorization, if the complete requested collection would
exceed the applicable finite result profile, the operation returns a structured
result-too-large outcome. It MUST NOT truncate, sample, implicitly paginate, or
return a partial collection as complete. The exact finite limits and public
error/result spelling remain Provisional. This is distinct from request-envelope
admission failure: a small request may be admitted and later produce a complete
result that is too large for the current bounded profile.

### Two-context evaluation

The same context-independent normalized analysis definition MAY be evaluated
independently over two explicitly supplied exact semantic contexts. The logical
result is a paired A/B analysis result. A/B evaluation substitutes only the
execution context; it does not rewrite or renormalize the analysis definition.
Neither context is inferred from session history, Git, a branch, a resident
revision token, or current-state lookup.

Two-context analysis performs no rebasing, history traversal, or implicit change
attribution. If a consumer asks what changed semantically between contexts, the
existing semantic-diff authority remains the source of change facts.

The trusted boundary derives and checks the complete Query/disclosure footprint
independently in context A and context B, then checks the combined paired
lineage/result projection. If either context or the combined paired projection
lacks sufficient Query authority, the entire paired operation returns one
disclosure-safe denial. It MUST NOT return a one-sided result or reveal which
context failed authorization.

### Reproducibility and lineage

Before ADR-0026 disclosure projection, equal exact semantic context(s), equal
context-independent normalized typed analysis definition, and equal
deterministic configuration that can affect requested facts MUST produce equal
underlying analysis results.

The logical result preserves enough lineage to reproduce and review the result:

- exact source semantic context or paired A/B contexts as execution provenance;
- context-independent normalized typed analysis definition;
- stable schema, field, and explicitly targeted entity identities required by
  that definition;
- per-result derivation meaning for returned membership, groups, aggregates,
  and per-member observations;
- ADR-0018 calculation authority used by formula-backed metrics and predicates;
- relevant deterministic validation/configuration identity when it can change
  returned facts; and
- explicit A/B source provenance for two-context evaluation.

An aggregate need not disclose every contributor ID or the witness identity for
a Min/Max unless the request also asks for that membership/per-member evidence
and the caller holds sufficient Query authority. Git commit IDs, host paths,
wall clock, provider/model identity, UI coordinates, and transport metadata may
be adapter provenance but MUST NOT become semantic analysis identity.

### Authorization and disclosure

Analysis obeys ADR-0026 Query authority. Bounded request-envelope checks that
need only caller-supplied facts happen before semantic lookup. After admission,
the trusted application authority non-disclosingly resolves the actual source
occurrence, schema/type domain, optional explicit EntityId narrowing, and the
stable predicate/group/metric target scopes. This structural resolution derives
the **candidate domain** but does not evaluate predicate truth, expose target
types or values, or perform a reduction.

Before predicate evaluation or semantic target/type classification, the trusted
boundary derives a conservative preauthorization footprint. For the candidate
domain that footprint includes complete domain membership plus the requested
predicate, grouping, and metric field scopes for every candidate entity, along
with any dependency/calculation scopes required to evaluate a formula-backed
Number predicate or metric. Query authority must cover that complete footprint.
A Grant that covers only the entities that would happen to survive filtering is
not sufficient for a broader schema/domain query, because determining that
post-filter set would itself depend on protected predicate facts. A caller may
use the explicit bounded EntityId narrowing set to reduce the candidate domain
before this footprint is derived; the narrowing set itself still grants no
authority.

Only after this preauthorization succeeds may the application authority
classify semantic target types, calculate formula-backed predicate values,
evaluate the predicates, derive the selected membership, group selected members,
and perform the requested reductions. The final result footprint then includes
the exact selected membership, groups, aggregates/observations, lineage, and
other facts that the projection would reveal, and receives a final complete-
result Query disclosure check before projection.

The required ordering is:

```text
request-local bounded envelope admission
-> trusted non-disclosing source/domain/candidate-domain and target-scope resolution
-> conservative preauthorization-footprint derivation
-> Query authorization over candidate-domain membership and requested fact scopes
-> semantic target/type classification and authoritative predicate calculation/selection
-> authoritative grouping/reduction
-> final complete-result disclosure-footprint check
-> projection
```

Caller-supplied membership or scope claims grant nothing. For a schema-wide
analysis, the candidate-domain coverage rule above prevents predicate values,
formula dependencies, group keys, metric facts, or excluded membership from
being used as an unauthorized inference channel.

Grouped results and `Count`/`Min`/`Max` are **complete-or-denied** in M04. A
conforming implementation MUST NOT compute over only the visible subset and
present that value as the requested complete aggregate. If the complete
assertion cannot be disclosed safely, the result is a disclosure-safe denial.
This rule prevents unauthorized membership, empty-group, aggregate,
ranking-input, lineage, or cross-context facts from being inferred from a
partial projection. Result-too-large classification occurs only after the
trusted boundary has enough Query authority to classify the complete requested
collection; an unauthorized caller receives the disclosure-safe denial rather
than collection-size evidence.

### Failure and persistence boundary

The logical family distinguishes at least:

- malformed, oversized, or unsupported analysis request;
- unresolved or wrong-typed field, predicate, group, or metric target;
- unresolved or wrong-domain explicit EntityId narrowing target;
- selected entity missing a required grouping value;
- unsupported formula-valued grouping key;
- authoritative formula/calculation failure in a predicate or metric;
- operation-wide requested-metric incompleteness with no successful payload;
- invalid aggregate/type combination;
- empty aggregate for requested `Min`/`Max` with zero Number observations;
- complete bounded membership/group/per-member result exceeding the finite
  result profile;
- insufficient Query authority or disclosure-safe denial; and
- ambiguous or unsupported two-context comparison.

Exact error codes, Rust variants, DTO spelling, request/result limits,
normalized-definition encoding, output ordering, and internal execution-plan
shape remain Provisional.

M04 analysis is an ephemeral Query result. It creates no persisted `AnalysisId`,
saved semantic analysis block, analytics datastore, report authority, or
parallel revision/history axis. Report, chart, presentation, or AI explanation
layers may consume the structured result without becoming semantic authority.

## Query, Propose, and Execute

The Accepted semantic execution intents are:

```text
Query
  -> read semantic facts only

Propose(Command | AtomicBatch)
  -> evaluate the semantic intent and authoritative rules
  -> do not publish the semantic transition

Execute(Command | AtomicBatch)
  -> evaluate the same semantic intent and authoritative rules
  -> request authoritative semantic publication
```

### Shared semantics

`Propose` and `Execute` MUST share the same command meaning, validation authority,
and gate semantics. Propose is not a weaker alternate validation path.

### SemanticPatch proposal envelope

The reviewable output/input contract around Propose is conceptually:

```text
SemanticPatch
- proposal occurrence identity
- Semantic API compatibility contract
- semantic base reference
- change: Command | AtomicBatch
```

These are logical contract elements, not frozen source or wire field names.

A SemanticPatch:

- belongs to the same Semantic API/application boundary as Query, Command,
  Propose, and Execute;
- contains exactly one typed Command or one ordered AtomicBatch;
- does not itself become a Command;
- introduces no patch-operation vocabulary or independent patch version;
- publishes no semantic state;
- grants no authorization or approval; and
- performs no `.roproj`, filesystem, Git, network, or other host write.

The exact API call sequence and result container used to issue a proposal are
Provisional. A conforming mapping MUST preserve the logical envelope and the
laws below whether it models proposal construction and evaluation as one call
or as admission followed by evaluation.

### Proposal occurrence identity and immutability

Every reviewable proposal MUST have an opaque proposal occurrence identity.
The same identity MUST NOT refer to different proposal contents. Once issued,
the complete proposal record is immutable.

Changing any of the following requires a new proposal identity:

- Semantic API compatibility contract;
- semantic base;
- single-command versus AtomicBatch body;
- command content or batch order;
- stable targets or typed values;
- bound formulas;
- generated semantic IDs; or
- an immutable annotation stored inside the proposal record.

Two proposal occurrences MAY have identical semantic contents and different
identities. Proposal identity is not semantic object identity under ADR-0015,
not proof of content integrity, and not a content-equivalence or idempotency
claim.

Proposal-ID spelling, generation, issuer, namespace, collision handling, and
transport encoding remain Provisional.

### Exact-change binding

For proposal `P`, exact semantic review binds this logical value:

```text
ExactChangeBinding(P) =
    Semantic API compatibility contract
  + semantic base reference
  + body kind
  + complete typed command semantics
  + command order for AtomicBatch
```

Complete typed command semantics include every semantic input that can affect
candidate construction, including stable targets, typed operands,
command-owned semantic preconditions, bound formulas, and generated semantic
IDs.

Generated IDs required by the change MUST be fixed before proposal identity is
issued. A formula update MUST bind its accepted typed formula meaning and stable
references before the exact reviewable change is fixed. A later execution
cannot generate different IDs or rebind formula source while claiming to
execute the same proposal.

`ExactChangeBinding` is representation-neutral. It does not depend on Rust
layout, Serde shape, transport bytes, JSON formatting, UI coordinates,
provider metadata, rendered diff prose, storage paths, `.roproj` bytes, or Git
objects.

This specification selects no canonical proposal bytes, hash, digest,
signature, or MAC. ADR-0026 requires trusted structural verification of this
complete binding for exact Approval while deliberately deferring any canonical
bytes, digest, signature, MAC, or portable token. Proposal identity by itself
MUST NOT be treated as cryptographic proof of the expected binding.

### Semantic API compatibility binding

Every durable or transported proposal MUST carry or unambiguously derive the
Semantic API compatibility contract used to interpret its body.

A consumer that does not support that contract MUST reject before semantic
candidate construction. It MUST NOT reinterpret unknown command semantics or
fall back to representation CRUD.

SemanticPatch introduces no independent patch-operation version axis. An
explicit translation to another Semantic API compatibility contract forms a
new proposal and receives a new proposal identity, even when an adapter judges
the result equivalent.

The compatibility identifier and negotiation/encoding mechanism remain
Provisional under ADR-0020. Representation, transport, crate/package,
diagnostic-provider, and runtime revision versions remain separate axes.

### Semantic base and stale behavior

Every proposal binds one exact semantic base reference. The reference MUST be
sufficient under the owning context/runtime contract to distinguish the
semantic context and revision against which Propose was evaluated.

Base equality is a semantic optimistic-concurrency precondition. It means exact
semantic revision identity, not equality of semantic content. A proposal
matches only the same semantic revision occurrence against which it was formed.
Any intervening canonical semantic publication makes the proposal stale,
including an unrelated semantic change. Later canonical state that is
semantically equivalent to the original base does not restore the original
revision identity or make the old proposal current again.

Base equality is not defined by `.roproj` bytes, paths, timestamps, UI state,
provider metadata, or Git objects.

Before an existing proposal is re-evaluated, authorized, or executed against a
current semantic context, the trusted application/runtime boundary MUST compare
that context with the proposal base. A mismatch is `Stale` and MUST:

- fail before constructing or publishing a candidate against the changed base;
- publish no semantic state;
- perform no implicit rebase, merge, retarget, or best-effort replay; and
- leave the immutable proposal unchanged.

For approval-gated Execute, this ordering governs internal stale detection and
candidate construction, not permission to disclose the result. The
authorization layer MUST retain the base comparison internally while it
authenticates the caller and verifies the complete trusted proposal/Approval
binding. An unauthenticated, unbound, or mismatched caller receives only a
disclosure-safe authorization or binding denial. Only an authenticated bound
executor with a verified complete binding may receive `Stale`, and
current-revision or other semantic details still require sufficient Query
authority. Delaying disclosure in this way does not delay the ADR-0024
comparison or permit re-evaluation or candidate construction against a changed
base. Non-Approval paths remain subject to their applicable authentication and
disclosure policy without acquiring an Approval requirement from this rule.

Re-proposing against a newer base re-runs command construction/binding and
authoritative Propose evaluation and receives a new proposal identity.
Issue #29's provisional lifecycle now returns an internal stale outcome over a
host-supplied opaque revision reference. Exact revision-token types, equality
mechanics, session scope, persistence, concurrency algorithms, and public
stale-result DTOs remain #93 or later transport work.

### Preconditions

SemanticPatch defines no generic `preconditions[]` language.

The semantic base is the envelope-level concurrency precondition. Any
additional semantic precondition belongs to the typed Command whose meaning
requires it and is included in `ExactChangeBinding`.

Authorization, approval, expiry, replay/revocation policy, durable-write
availability, and external-effect permission remain separate enforcement
conditions under ADR-0026 and the relevant host authority. JSON Pointer
predicates, storage checks, UI-coordinate tests, provider claims, and arbitrary
scripts do not become semantic preconditions.

### Proposal evidence

Propose may return a candidate, semantic diff, validation report, gate outcome,
calculated impact, or other operation-specific review evidence. These are
derived observations over the bound base and exact typed change.

Derived evidence does not replace `ExactChangeBinding`, grant authorization,
or become a mutation program. Rendered diff prose is presentation. A semantic
diff explains base-to-candidate meaning. Validation success does not grant
permission. A malformed request may fail before a reviewable proposal exists;
an invalid or gate-rejected candidate publishes nothing.

The trusted semantic/application boundary derives ADR-0026
`AuthorizationFootprint` from typed operation meaning and relevant
base/candidate relationships. Its disclosure scope includes every subject
revealed by preview, diff, dependencies, impact, and diagnostics. Propose
authority does not grant arbitrary Query authority: evidence outside live Query
scope MUST be denied or safely reduced. The client cannot authoritatively
declare its own footprint. Mutation coverage retains each associated
operation-family/mutation-class/scope requirement; the requested action is
combined with every tuple at its authorization check. Independently unioning
operation-family, class, and scope sets cannot authorize crossed combinations
that no live Grant covers.

Once proposal identity is issued, validation, review, rejection, or stale
outcomes do not mutate the proposal record. A later execution must perform the
authoritative base, authorization, and gate checks required for the state it
actually acts on rather than trust stale client-rendered evidence.

### Preview

A Preview is a client/product projection of proposal facts for review. It is not
an independent canonical semantic state or mandatory protocol phase.

### Finalization and gates

Finalization means applying an operation-specific authoritative gate to the
candidate/purpose before publication. It does not require a long-lived public
prepare/finalize/commit state machine.

Execute MUST evaluate the authoritative preconditions/gates for the semantic
state it actually acts on. A client MUST NOT convert an earlier gate decision
into ambient authority for a later changed state.

ADR-0024 fixes proposal occurrence immutability, exact-change binding, Semantic
API contract binding, semantic-base pinning, and fail-closed stale meaning.
ADR-0026 fixes structural exact Approval, live authorization, and
consume-with-successful-publication laws without selecting a public token or
DTO. Issue #29 supplies the current provisional in-process Approval lifecycle
types. Proposal-ID/revision encoding, public Approval DTOs, and concrete
session/commit mechanics remain Provisional or Deferred to #93 and transport
work.

## Semantic atomicity

### Single command

A semantic command either publishes the complete authoritative semantic
transition or publishes no semantic transition.

### Atomic command batch

An Atomic Command Batch:

- is an ordered collection of semantic commands evaluated against one semantic
  base/context;
- forms one candidate semantic transition; and
- publishes all of that final transition or none of it.

No failed batch prefix becomes authoritative semantic state.

An implementation is not required to apply the final operation gate after each
internal command in the batch. Intermediate working candidates MAY contain
higher-level diagnosable invalidity when a later command in the same batch is
intended to repair it, provided:

- intrinsic admission/representability invariants remain satisfied; and
- the final candidate passes the authoritative operation gate required for
  publication.

This enables one explicit atomic semantic operation to remove/retarget inbound
references together with deletion as allowed by ADR-0015 without weakening the
final validation contract.

### Not implied by atomic batch

Atomic batch does not by itself define:

- nested transactions;
- `begin` / `commit` / `rollback` handles;
- database isolation levels;
- distributed transactions;
- filesystem durability/rollback;
- runtime concurrency or revision-conflict algorithms;
- event sourcing or operation logs;
- undo/redo history;
- durable proposal-store or approval lifecycle mechanics; or
- intra-batch temporary-object handle syntax.

ADR-0022 likewise does not turn semantic atomicity into a specific runtime
commit/swap/locking/cloning algorithm.

## Result contract

The Semantic API result must preserve operation-specific semantic meaning.
There is no requirement that all operations return one universal response bag.

A conforming client must be able to distinguish, where applicable:

1. completed semantic operation results;
2. failure before a new admissible semantic candidate exists;
3. unsupported Semantic API compatibility or proposal identity/content
   mismatch;
4. authorization, Approval, or host-effect denial without semantic
   publication;
5. stale semantic base;
6. semantic precondition/inapplicability failure;
7. rejection by the authoritative operation gate, including relevant validation
   and gate facts; and
8. typed operation-specific outcomes such as merge conflict/reconciliation
   results.

Exact public enum names, generic type constructors, tagged-union representation,
field spelling, and Rust error hierarchy are Provisional.

`WorkspaceError`, `CalculationError`, `EditPreview`, or another current internal
Rust type is not automatically a public Semantic API type.

## Failure family boundaries

### Admission / construction failure

A request can fail before a new structurally admissible semantic candidate
exists. Examples include newly authored formula source that fails Accepted
parse/bind/type construction or other Accepted intrinsic representability
barriers.

Exact API error encoding remains Provisional.

### Proposal contract failure

A durable/transported proposal whose Semantic API compatibility contract is not
supported fails before candidate construction. Reuse of one proposal identity
with different contents is rejected rather than treated as a replacement.

For Approval-gated Execute, version support may be detected internally before
authorization disclosure, but an unsupported-version result MUST NOT be exposed
until the complete trusted proposal/Approval binding is verified. A missing,
unrelated, mismatched, or unverifiable proposal receives the same
disclosure-safe binding denial regardless of internally detected version
support. Only after exact binding is proven may the unsupported-version outcome
be returned. This detect-versus-disclose ordering does not delay ADR-0024's
internal base comparison, re-evaluate a stale proposal, or permit candidate
construction against a changed base.

The #30 in-process adapter now supplies provisional stable code meanings for
its current failure families. Exact external code/DTO mapping, integrity
verification, digest, and transport behavior remain Provisional/Deferred under
ADR-0026 and future transport profiles.

### Authorization / Approval / host-effect denial

Authorization and Approval failures are distinct from proposal-contract,
stale-base, semantic-precondition, and gate failures. A conforming client can
preserve the machine meaning defined by
[`semantic-authorization.md`](semantic-authorization.md), including
principal/capability/scope denial, approval required or unusable, lost live
authority, and separately denied host effects, without disclosing semantic
content outside authorized Query scope. #30 implements the current in-process
safe code projection; public code catalogues and transport mappings remain
Provisional under later transport work.

### Stale base

An otherwise supported immutable proposal whose semantic base does not equal
the current context revision is stale. It fails before candidate construction
against the changed base and publishes nothing. Stale is distinct from semantic
command inapplicability and gate rejection.

### Semantic precondition failure

An otherwise well-formed semantic command can be inapplicable to the current
semantic state, for example because a stable target does not exist or the
operation's semantic precondition does not hold.

Exact taxonomy remains Provisional.

### Gate rejection

A structurally admissible candidate can be rejected for publication by an
authoritative operation gate. The result must preserve the relevant
`ValidationReport` and gate outcome meaning.

### Operation-specific domain outcome

Some operations have typed outcomes that should not be flattened into generic
diagnostics. A semantic merge conflict is an example.

### Representation and host failure

Storage/version/migration errors remain representation-local under ADR-0017.
Filesystem, browser-host, transport disconnect, IPC, authentication, and similar
host/transport failures remain outside semantic diagnostics unless a separate
Accepted contract says otherwise.

Adapters MAY combine these result families for a client transport, but they
must preserve which authority produced the failure.

## ValidationReport contract

When an operation performs authoritative semantic validation, the result uses
the diagnostic meaning Accepted by ADR-0019 and `diagnostics-contract.md`.

Stable semantic observations include, where applicable:

- published symbolic diagnostic code meaning;
- stable semantic subject identity;
- semantically relevant related subjects/facts;
- validator/provider provenance;
- a machine-readable classification concept; and
- formula facts already Accepted by ADR-0018.

The following are not stabilized by this specification:

- exact Rust `ValidationReport` layout/methods;
- exact severity enum;
- exact diagnostic code namespace/catalog spelling, except that published code
  meanings cannot be silently reused;
- exact primary/related/facts container;
- localized message/help text;
- human-key paths;
- source spans;
- selected cycle witnesses;
- exact ordering implementation; and
- external wire schema.

## Gate outcome contract

Diagnostic classification/severity and operation gating are separate concepts.

A client MUST use the authoritative operation gate outcome to decide whether the
requested semantic operation may publish. It MUST NOT derive semantic
allow/deny from:

- severity ordinal alone;
- localized message wording;
- presence/absence of any diagnostic whatsoever; or
- a client-maintained copy of validation rules.

Interactive editing, strict mutation, export, and CI/workflow policy may apply
different gates to the same underlying diagnostic meaning without changing the
diagnostic code identity.

## Formula outcome relationship

ADR-0018 remains formula authority.

New authoring input that fails parse/bind/type construction does not create a
new semantic candidate and is reported through the admission/command-failure
side of the Semantic API.

For an existing structurally admissible semantic candidate, formula static/
graph/evaluation failures participate in ADR-0019 Stage 4/5 validation using the
ADR-0018 stable semantic facts such as stable field subjects, SCC membership,
direct failed-dependency sets, and evaluation-failure meaning.

Successful calculated values are operation/query facts, not diagnostics.

## Capability-addressability

Every semantic operation or operation family MUST be independently addressable
for authorization/capability purposes.

The following minimum authority dimensions are distinct:

- Query by operation family and disclosure scope;
- Propose by operation family, mutation class, and write scope;
- Execute by operation family, mutation class, and write scope; and
- Approve by operation family, mutation class, and write scope.

Operation-family identity is an independent checked dimension. Granting one
family MUST NOT authorize another family merely because its action, mutation
class, and semantic scope are otherwise equal. Granting one action MUST NOT
imply another. Value, Formula, Structure, Schema, and Destructive mutation
authority likewise do not imply one another. Unknown or unclassified operation
families fail closed.

ADR-0026 and [`semantic-authorization.md`](semantic-authorization.md) define
the representation-neutral Principal, Grant, stable-ID semantic scope,
`AuthorizationFootprint`, exact Human Approval, expiry/replay/revocation,
minimum provenance, and external-effect separation laws that consume these
operations. Exact operation-family identifiers and catalogue are Provisional.
Exact capability strings, DTOs, storage, and result codes remain Provisional;
canonical bytes, digest/token profiles, and wire security mechanisms remain
Provisional/Deferred.

## Compatibility and versioning

Semantic API versioning is independent from:

- `.ro` / `.roproj` representation versions;
- Rust crate/package SemVer;
- diagnostic provider implementation versions;
- transport protocol versions; and
- runtime/session revisions.

### Stable semantic contract

Only an explicitly specified and stability-classified semantic law, operation,
capability, result fact, or code meaning is a public compatibility promise.

Rust source visibility and serde derivation do not confer this status.

### Breaking semantic change

A change is breaking when a conforming client relying on Stable semantic meaning
must change its semantic assumptions. Examples include:

- changing an existing Stable command's intent or semantic side effects;
- making a Stable Query publish semantic mutation;
- changing stable-ID targeting semantics;
- changing Accepted single/batch atomicity;
- silently reinterpreting a published stable diagnostic code;
- removing or incompatibly changing a Stable operation/capability;
- adding a mandatory input to a Stable operation without a compatible version
  path; or
- changing Accepted gate/formula/validation semantics without the corresponding
  authority/version transition.

Correcting implementation that violated an already Accepted contract is a
conformance fix rather than automatic stabilization of undocumented buggy
behavior.

### Additive evolution

Potentially additive changes include:

- a new opt-in query/command/capability;
- new optional semantic projections/facts that older clients may ignore;
- new presentation-only fields;
- new transport adapters; and
- new diagnostic codes following the published unknown-code rules.

Adding a new blocking semantic rule is not necessarily additive merely because
an encoded report only gained a new code. If the semantic gate contract changes,
that change follows the semantic decision/version process.

### Diagnostic unknown-code rule

A published diagnostic code meaning MUST NOT be silently reused for an unrelated
rule.

A conforming client MUST be able to preserve/represent an unknown diagnostic
code as an opaque machine finding according to the relevant transport mapping.
It MUST NOT require an exhaustive known-code switch to derive operation gate
policy.

## M04 semantic analysis conformance requirements

The first provider-neutral implementation of the Issue #33 contract must prove,
without promoting incidental Rust/CLI/wire shapes, at least:

1. bounded typed selection/filter over a schema/type domain with stable
   semantic targeting, including a candidate entity that omits an optional
   predicate field and therefore does not match that predicate, plus an
   explicit bounded EntityId set that is applied as the required narrowing
   intersection rather than ignored;
2. one formula-backed Number predicate that consumes the ADR-0018 calculated
   value and turns an authoritative calculation failure into structured analysis
   failure rather than non-match;
3. one grouped entity-count result with complete selected membership
   partitioned exactly once across groups;
4. grouped `Count`, Number `Min`, and Number `Max` evaluated per group, with no
   implicit simultaneous global reduction;
5. a selected entity missing the grouping field producing a structured
   missing-group-value failure rather than omission or a synthetic null group,
   and a Formula-valued grouping key producing an unsupported-grouping failure
   rather than grouping by formula structure or calculated Number;
6. exact ungrouped `Count`, Number `Min`, and Number `Max` over supported
   observations;
7. an empty selection returning `Count = 0`, a structured empty-aggregate
   outcome for requested ungrouped `Min`/`Max`, and no synthesized empty group;
8. one formula-backed numeric metric that demonstrably consumes ADR-0018
   calculation authority rather than a second evaluator;
9. repeated equal exact context(s) plus equal context-independent normalized
   definition and relevant deterministic configuration producing equal
   underlying results, including paired A/B evaluation that changes only the
   supplied execution contexts and does not renormalize the definition;
10. unresolved or wrong-typed metric, grouping, or predicate targets;
    unsupported metric/group/aggregate kinds; calculation failure; and selected-
    member metric incompleteness preserving structured failure meaning, including
    operation-wide metric failure with no successful group or `Count` payload;
11. a complete selected membership, grouped result, or per-member observation
    collection exceeding the finite result profile producing a structured
    result-too-large outcome with no truncation, sampling, partial-success claim,
    or implicit pagination;
12. authorization evidence proving that a predicate-bearing schema/domain query
    cannot use post-filter membership to bootstrap authority: the complete
    candidate domain and requested predicate/group/metric/calculation scopes are
    covered before predicate evaluation, while an explicit bounded EntityId
    narrowing set is applied as the candidate-domain intersection without
    granting authority, and unresolved or wrong-domain supplied IDs are not
    silently ignored after authorized classification;
13. a disclosure case where membership, group existence, count, aggregate,
    per-member observations, result-size classification, missing-group-value
    classification, or lineage would otherwise reveal unauthorized facts,
    proving complete-or-denied behavior rather than a visible-subset aggregate;
14. the same context-independent normalized analysis definition evaluated over
    two explicit exact semantic contexts, with no history lookup, definition
    renormalization, or parallel revision semantics, including independent
    complete authorization in A and B plus authorization of the combined paired
    projection, and one whole-operation disclosure-safe denial when any check
    fails; and
15. lineage sufficient for a consumer to explain and reproduce the deterministic
    result without an LLM reconstructing selection or aggregation semantics.

The implementation Issue may choose reversible finite request/result limits,
internal plan structures, output ordering, Rust/result types, and CLI spelling.
It may not silently add Sum/Mean, ranking, statistics, general boolean query
ASTs, joins, UDFs, persistence, pagination, or another analytics authority.

## M04 semantic analysis conformance evidence

Issue #150 exercises the requirements above through one provider-neutral
`workspace-engine` Query family and a structured CLI projection. Focused
conformance fixtures cover request-local bounded admission, stable-ID domain
narrowing, optional predicate absence, stored and ADR-0018 formula-backed
Number values, per-group and ungrouped reductions, empty aggregates, complete
metric failure, collection limits, repeated and paired exact contexts, and
structured lineage.

Authorization regressions provision the existing ADR-0026 lifecycle with an
independent Analysis Query operation family. They prove that complete candidate
membership, requested field facts, and static formula-dependency scopes are
covered before target classification, calculation, or predicate truth; that an
explicit narrowing set grants nothing; that empty-domain existence is not
revealed without scope; and that final projection or result-size facts are
complete-or-denied. The portable production-semantic corpus executes fixed
successful, structured-failure, authorization, and paired-context Analysis
records natively and under `wasm32-unknown-unknown`.

The current finite limits, equality/Number-ordering predicate catalogue,
stable-ID output order, Rust types, CLI grammar, and JSON projection are
Provisional implementation evidence. No Deferred analysis behavior is added.

## M04 formula/scenario conformance evidence

Issue #144 exercises the Accepted logical contract with provider-neutral
structured fixtures. At minimum, the current game-balance domain demonstrates:

1. inspection and authoritative evaluation of a DPS-style bound formula;
2. deterministic stable-ID direct-input and direct-dependent facts;
3. one Number override scenario that changes requested derived values while
   proving that canonical semantic revision/state identity is unchanged, that
   no publication or persistence event occurred, and that canonical
   representation remains byte-for-byte or structurally unchanged;
4. repeated equal source revision, effective deterministic validator
   configuration, normalized overrides, and requested targets producing equal
   underlying structured semantic outcomes before disclosure projection; equal
   Query disclosure authority producing equal exposed target outcomes; and a
   changed validator configuration being distinguished whenever it changes
   validation facts;
5. invalid override; unresolved and unsupported-kind requested targets;
   division/evaluation failure; validation failure; and source-cycle cases
   returning structured evidence with no publication, with each otherwise
   admitted requested target producing exactly one outcome and an unresolved
   target revealing no target-history claim;
6. a valid typed formula update becoming one ADR-0024 SemanticPatch whose exact
   binding contains the complete bound expression and references;
7. invalid, rebound/stale-target, and cycle-inducing formula updates failing
   through the existing admission, validation, or gate families while proving
   unchanged canonical semantic revision/state identity and no canonical
   semantic publication; an authoring or admission failure issuing no proposal
   occurrence and retaining semantic-base-dependent diagnostics internally
   unless Query-authorized for external projection; a later validation, gate, or
   stale rejection retaining immutable non-authoritative proposal and diagnostic
   evidence only when a reviewable proposal was validly issued; and each
   command's authoritative semantic transition publishing atomically in full or
   not at all;
8. Query disclosure limits applying independently to reasoning, scenarios,
   formula authoring/admission, and proposal preview evidence, including
   request-local syntax evidence revealing no semantic-base fact; trusted
   non-disclosing target/reference resolution, binding, and type-checking before
   Query-authorized authoring/admission projection or a disclosure-safe denial;
   before any scenario source/target resolution, finite-envelope rejection,
   duplicate override-target rejection, and duplicate requested-target
   normalization, all revealing request-local facts only; trusted non-disclosing
   resolution of actual document occurrence and `EntityField` membership before
   scenario authorization; caller-supplied membership granting nothing; an
   unprovable document occurrence failing closed; broader scope applying only
   within the same proven occurrence; insufficient source/override coverage
   denying before classification, exposure, or candidate derivation; an
   insufficiently covered requested target yielding only its disclosure-safe
   denial; and unauthorized dependency, impact, affected-subject, or diagnostic
   evidence being safely reduced without false completeness or causing a
   disclosure-safe target or scenario denial; and
9. Delegated formula Execute using ADR-0026's existing exact finite Human
   Approval and Formula-class footprint without new formula-specific evidence.

These are semantic observations, not a requirement for shared fixture bytes,
Rust types, command names, or transport encoding.

## SemanticPatch conformance scenarios

Future implementation and transport mappings MUST preserve these logical
fixtures without requiring common bytes or Rust types:

1. one stable-ID-targeted typed field update remains inert under Propose and
   changes identity if its target or value changes;
2. one ordered multi-entity AtomicBatch forms one candidate and publishes no
   prefix, while reordering creates a different proposal;
3. a formula update binds its complete typed bound AST and stable references,
   not only source spelling or rendered addresses;
4. a proposal created against revision `R` fails stale against any current base
   other than `R`, before candidate construction and without implicit rebase;
5. an invalid typed command produces no published semantic state; and
6. two formula updates that are individually valid against the base but create
   a cycle together fail the final batch gate with no partial publication.

Conformance also covers unsupported Semantic API compatibility, reuse of one
proposal identity with different content, generated-ID binding, and equivalent
Stable native/WASM outcomes where the same capability is exposed. Issue #29
implements current field-value/batch lifecycle fixtures. Issue #144 adds
formula reasoning, scenario, and formula-update conformance; complete catalogue,
runtime, and transport conformance remains #93 and later work.

## Stability classification

| Concept | State |
| --- | --- |
| Headless Semantic API as mandatory first-party semantic boundary | Accepted |
| `workspace-engine` as current Rust implementation/application authority | Accepted under ADR-0016/ADR-0020 |
| Resident shared Rust runtime as preferred interactive topology | Accepted under ADR-0022 |
| Frontend projection/cache/authoring state is non-authoritative | Accepted under ADR-0022 |
| Host persistence/capabilities remain outside workspace-engine | Accepted under ADR-0016/ADR-0022 |
| Native/WASM equivalent Stable semantic observations where capabilities overlap | Accepted under ADR-0022 |
| Current workspace-engine Rust surface as external API | Internal / Provisional |
| Query does not publish semantic state | Accepted |
| Command is typed semantic intent rather than representation CRUD | Accepted |
| Stable semantic-ID targeting authority | Accepted under ADR-0015 |
| Propose is non-publishing and shares command semantics/gates with Execute | Accepted |
| SemanticPatch as immutable envelope around `Propose(Command | AtomicBatch)` | Accepted under ADR-0024 |
| Opaque proposal occurrence identity and complete-record immutability | Accepted under ADR-0024 |
| Representation-neutral `ExactChangeBinding` law | Accepted under ADR-0024 |
| Semantic API compatibility binding with no independent patch-operation version | Accepted under ADR-0024 |
| Exact semantic-base pinning and fail-closed stale behavior | Accepted under ADR-0024 |
| Proposal-ID, revision-token, and transport encoding | Provisional / #93 |
| Hash/digest/signature/MAC/canonical proposal bytes | Deferred under ADR-0026 |
| Preview is proposal projection, not independent canonical state | Accepted |
| Finalization is operation-gate meaning, not mandatory stateful two-phase protocol | Accepted |
| Single-command semantic atomicity | Accepted |
| Ordered Atomic Command Batch all-or-nothing semantic publication | Accepted |
| Intrinsically admissible but higher-level-invalid intermediate batch working candidate | Accepted within ADR-0019 constraints |
| `ValidationReport` semantic observations as result meaning | Accepted |
| Exact Rust/wire `ValidationReport` shape | Provisional |
| Gate outcome distinct from diagnostic severity | Accepted |
| Formula Stage 4/5 facts remain ADR-0018/ADR-0019 authority | Accepted |
| Bounded formula-reasoning Query structured meaning and shared-engine requirement | Accepted under ADR-0020 / #32 |
| Exact-revision, Number-override scenario Query is transient and non-publishing | Accepted under ADR-0020 / #32 |
| Scenario provenance, baseline/outcome, affected-subject, validation, and dependency meaning | Accepted under ADR-0020 / #32 |
| Typed formula-update Command binds complete bound meaning before proposal identity | Accepted under ADR-0020 / #32 |
| Formula-update reuse of SemanticPatch and ADR-0026 authorization/Approval | Accepted under ADR-0020 / #32 |
| Bounded typed semantic Analysis Query family | Accepted under ADR-0020 / #33 |
| Analysis selected membership, Count, Number Min/Max, and bounded per-member Number observations | Accepted under ADR-0020 / #33 |
| Analysis exact-context reproducibility and structured lineage | Accepted under ADR-0020 / #33 |
| Analysis grouped/count/min/max complete-or-denied disclosure | Accepted under ADR-0020 / #33 and ADR-0026 |
| Analysis result persistence / `AnalysisId` / analytics datastore | Deferred |
| Sum/Mean, ranking/top-k, statistics, general predicate ASTs, joins, UDFs | Deferred |
| Exact operation names, family identifiers, request limits, predicate catalogue, normalization encoding, and result DTOs | Provisional |
| Production formula-reasoning/scenario/formula-update implementation | Provisional provider-neutral workspace/CLI slice implemented by #144; public wire/SDK remains undefined |
| Production semantic analysis implementation | Provisional provider-neutral workspace/CLI slice implemented by #150; public wire/SDK remains undefined |
| Capability-addressability of operation/family | Accepted principle |
| Capability/scope/Grant/Approval/provenance meaning | Accepted under ADR-0026 |
| Exact authorization identifiers/DTOs/storage/wire representation | Provisional / Deferred |
| Semantic API version independent from storage/crate/transport versions | Accepted |
| Published diagnostic code meaning not silently reusable | Accepted |
| Complete externally Stable operation catalogue | Provisional, promote operation-by-operation |
| Exact semantic result tagged union / field spelling | Provisional |
| Exact effect/diff projection shape | Provisional |
| Revision/concurrency/precondition token | #93 / Provisional |
| Intra-batch temporary-object handle syntax | Provisional |
| Public embedded Rust SDK / dedicated API crate | Deferred |
| Native/WASM/IPC/FFI/network serialization | ADR-0022-constrained future transport work / Deferred |

## Internal bypass policy

The following are implementation roles below or beside the Semantic API, not
alternate first-party client policy paths:

- `workspace-engine -> semantic-core/formula/diff/merge` under ADR-0016;
- storage codec/migration -> semantic model at the ADR-0017 representation
  boundary;
- host composition `load -> semantic operation -> save`;
- focused tests directly invoking an owner contract; and
- deterministic domain/extension validators through ADR-0019.

A first-party semantic client may not bypass the contract merely because it is
in the same process, language, or repository.

## ADR-0022 runtime/transport mapping rule

ADR-0022 fixes runtime ownership and host-separation laws: interactive
authoritative semantic state belongs to the shared Rust semantic/application
runtime; resident topology is preferred; frontends do not own a second semantic
authority; full snapshots are explicit boundaries; and native/WASM preserve
equivalent Stable semantic meaning where capabilities overlap.

Concrete resident session handles, revision/concurrency mechanics, Worker
lifecycle, projection delivery, IPC/FFI/network serialization/ABI, and
persistence/recovery remain Deferred to #93–#95 and future host/transport
implementation as applicable.

Every mapping MUST preserve the Semantic API Stable laws and outcomes. Runtime
or transport topology is not independent semantic authority. A mapping of
SemanticPatch also preserves ADR-0024 occurrence immutability, exact-change and
compatibility binding, base equality, and stale failure without making its
transport bytes the semantic proposal.

## #104 reference pressure

Project Memory may use this contract as a later reference/dogfood application.
It remains a domain model/research hypothesis and does not add `Decision`, `ADR`,
`GitHubIssue`, `Commit`, provenance workflow, or Project Memory-specific queries
to semantic core by virtue of using the API.

## Explicitly not defined here

- JSON/Protobuf/MessagePack or any other wire encoding;
- JSON-RPC, HTTP, IPC, FFI, or WASM ABI;
- exact resident session/handle representation;
- revision/concurrency/conflict protocol;
- exact runtime commit/swap/locking/cloning mechanism;
- Worker lifecycle/loading/startup/memory behavior;
- proposal-ID/revision-token field encoding;
- exact formula-reasoning, scenario, formula-update, and analysis operation
  identifiers, family IDs, finite request limits, predicate-operator catalogue,
  normalization representation, internal plan shapes, and result DTO fields;
- canonical proposal bytes, hash, digest, signature, or MAC;
- exact Approval/capability/Grant/provenance/expiry/replay/revocation DTO or
  wire format;
- projection patch/delivery/invalidation protocol;
- native/browser persistence/recovery implementation;
- plugin ABI/runtime/sandbox;
- `.roproj` physical layout;
- generic CRUD/JSON Patch;
- generic transaction scripting language;
- persisted scenarios, scenario mutation, parameter sweeps, optimization,
  randomness, statistics, SQL compatibility, a general relational/dataframe
  query language, general OR/NOT predicates, multi-key grouping, joins,
  subqueries, windows, arbitrary query expressions/UDFs, Sum/Mean, ranking,
  top-k, outliers, percentiles, partial aggregates, persisted analysis objects,
  or an analytics datastore;
- event sourcing / operation log / undo history;
- complete Stable operation catalogue;
- stable public Rust SDK; or
- Project Memory/provenance domain semantics.

## Related authority

- [ADR-0007](../decisions/ADR-0007-ai-semantic-interaction-model.md)
- [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md)
- [ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md)
- [ADR-0017](../decisions/ADR-0017-versioned-storage-and-canonical-representation.md)
- [ADR-0018](../decisions/ADR-0018-bound-formulas-and-deterministic-binary64.md)
- [ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md)
- [ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md)
- [ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md)
- [ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md)
- [ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md)
- [Semantic authorization](semantic-authorization.md)
- [Diagnostics contract](diagnostics-contract.md)
- [Validation engine](validation-engine.md)
- Issues #10, #17, #27, #28, #29, #32, #33, #93, #94, #95, #104
