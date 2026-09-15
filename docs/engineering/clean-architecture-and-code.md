# Clean Architecture and Clean Code playbook

Decision issue: [#402](https://github.com/nurockplayer/tachiko-work/issues/402)

**Decision state: Adopted engineering guidance, effective when
[PR #403](https://github.com/nurockplayer/tachiko-work/pull/403) lands on `main`;
not Accepted product authority.**
The Steward adopted these engineering conventions in
[#402](https://github.com/nurockplayer/tachiko-work/issues/402#issuecomment-5668289450).
Existing Accepted rules remain binding through their original authorities. New
guards and source refactors require separate scoped readiness. Reading or
merging explanatory text must not silently stabilize a new product contract.

Baseline inspected: `fb9fe34a5777ff31b5f1951941914372ae6d55ea` on 2026-09-15 JST.
This is not a full source audit or a claim that all repository tests were run.
Re-read live authority and active ownership before applying the playbook.

## 1. Purpose and authority

The goal is a maintainable, testable semantic engine that releases useful
products, not a codebase that resembles a textbook diagram. Preserve user data,
one semantic authority, and practical spreadsheet interoperability while making
future changes easier to place, verify, and review.

The [Product Constitution](../vision/product-constitution.md), Accepted ADRs and
governance, and normative specifications outrank generic Clean Code advice.
Follow [Knowledge Authority](../governance/knowledge-authority.md) and the
[reconciliation register](../governance/canonical-reconciliation-register.md).
An older ADR's historical risk or migration paragraph is not proof of a current
missing implementation.

Use four clearly distinguished kinds of statements in this playbook:

- **Existing authority:** a restatement with a link to the governing decision.
- **Proposed engineering guidance:** a review convention, not a new data or API law.
- **Illustration:** a possible organization, not a mandatory directory migration.
- **Future work:** unimplemented checks or refactors needing their own Issue.

These labels describe this document; they do not replace repository decision
states, risk classes, handoff fields, or Ready semantics.

Clean Architecture contributes inward source dependencies and replaceable
mechanisms. Clean Code contributes understandable intent, cohesive behavior and
explicit failure handling. Neither requires inheritance, one interface per
function, an application server, a particular folder count, or microservices.

## 2. Architecture to preserve

### Existing authority: responsibility ownership

[ADR-0016](../decisions/ADR-0016-milestone-02-rust-crate-layering.md) owns the
current crate baseline and exact dependency DAG. The
[crate architecture](../architecture/rust-crate-architecture.md) explains its
implementation. This table maps those existing owners to Clean Architecture;
it does not establish a second normative dependency graph.

| Owner | Responsibility | Keep out |
| --- | --- | --- |
| `semantic-core` | Stable typed identities, document/schema/entity/value/reference meaning, bound formula representation, intrinsic invariants and diagnostics | Files, storage DTOs, UI, provider state, ID-generation mechanisms and other workspace crates |
| `formula-engine` | Bounded parsing and binding, dependency analysis, deterministic calculation | I/O, UI, persistence, application authorization and provider behavior |
| `diff-engine`, `merge-engine` | Focused pure semantic algorithms and their typed outcomes | Persistence, presentation and alternate application workflows |
| `workspace-engine` | Shared semantic use cases, authoritative operation gates, validation/calculation orchestration, patch lifecycle, trusted authorization and resident state publication | Filesystem/browser/network/Git access, storage codecs, UI or provider SDKs |
| `storage` | Versioned representation DTOs, codecs, migrations, canonical materialization and host persistence APIs | Alternate semantic calculation, mutation or authorization policy |
| `ai-api` and client/transport adapters | Admission and projection over the shared semantic boundary | A second implementation of semantic business rules |
| CLI and native/browser composition roots | Assemble trusted engine and host capabilities, decode/load, invoke semantic operations, encode/save, render results | Reimplementing those semantic operations |

The conceptual invocation path is client -> trusted application boundary ->
semantic algorithms/model. **This is runtime flow, not a new Cargo dependency
rule.** The source dependency rule remains ADR-0016. In particular, storage and
workspace-engine are siblings: the composition root calls both, and neither
acquires the other's responsibility. An adapter may call down to approved
semantic contracts for conversion; this is not permission to bypass application
policy for client operations.

Do not add `domain`, `application`, `common`, `types`, `ports`, `repository`,
`validation`, or `infrastructure` crates merely to match a template. New crates
or direct workspace edges need evidence and the normal ADR-0016 amendment
path. A boundary can be a module, visibility rule or explicit argument before
it needs to be a separately released package.

### Existing authority: one meaning, multiple execution targets

[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md),
and the [frontend/backend boundary](../architecture/frontend-backend-boundary.md)
keep semantic authority in the shared Rust application/runtime. A local engine
need not be a localhost HTTP server: in-process calls, WASM and IPC are physical
mechanisms, not alternate semantics.

`tachiko-sheet` is a separate client, not a dependency of the core. It may own
selection, viewport, accessibility, rendering, pending interactions and raw
editing buffers. It may not own a competing canonical Document or authoritative
formula, validation, mutation, diff or merge rules. Existing app-private
Designer runtime behavior is implementation evidence, not automatically a
stable cross-client API; relocating such behavior requires explicit ownership
and consumer analysis rather than copying it into another client.

Keep orchestration tooling, SCD scheduling and conductor concerns outside
semantic product authority. Reusing operational tooling must not make the
product engine dependent on the agent that developed it.

## 3. State and boundary models

### Existing authority: do not confuse these representations

| Representation | Owner and allowed use |
| --- | --- |
| Semantic Document, typed identities and bound references | Domain meaning owned by semantic-core, with one authoritative resident occurrence in the application runtime |
| Command, Query, candidate, proposal and operation result | Application behavior governed by the Semantic API and applicable lifecycle authority |
| Transport request/response | Adapter-owned admission and projection; internal Rust `pub` items or serde derives do not automatically define a public wire contract |
| Versioned storage DTO and canonical bytes/tree | Storage-owned representation under the applicable format/migration contracts |
| Derived indexes, calculation cache and query projection | Rebuildable runtime/client-derived state with appropriate occurrence/revision context, never a second truth |
| Raw editor draft and optimistic presentation | Client workflow state that does not claim semantic acceptance before the engine result |

See [ADR-0015](../decisions/ADR-0015-stable-semantic-identity.md),
[ADR-0019](../decisions/ADR-0019-staged-semantic-validation-and-diagnostics.md),
[ADR-0020](../decisions/ADR-0020-first-class-headless-semantic-api.md) and
[ADR-0022](../decisions/ADR-0022-resident-semantic-runtime-and-host-boundary.md).

A label, A1-style address, path or JSON pointer can be input to an explicit
resolver or format adapter; it must not silently replace stable semantic
identity. Raw incomplete input remains a draft until admitted under the actual
operation contract. A diagnostic is not by itself an authorization decision.

### Proposed engineering guidance: explicit conversions, no ritual copying

Decode and validate external representations at their owning boundary, then
pass typed values inward. Keep lossy conversions explicit and preserve or
report unsupported content according to the accepted interoperability scope.
Do not reuse a storage DTO as the engine's edit model just because its fields
look similar.

The goal is independent ownership, not mandatory copies of the same type at
every call. Within a trusted Rust boundary, borrowed domain values and explicit
internal results can be appropriate. Add a DTO mapping when it protects a real
representation, trust or compatibility boundary; do not serialize to JSON and
back merely to call another Rust module.

Do not ban serde categorically: the existing architecture permits internal
serde usage. Review which layer owns the schema and whether external formats
are leaking inward. Stable serialization is selected by format/transport
authority, not by the convenience of a derive macro.

## 4. Use cases and publication

### Existing authority: one semantic lifecycle

[ADR-0024](../decisions/ADR-0024-revision-pinned-semantic-patch.md),
[ADR-0026](../decisions/ADR-0026-scoped-semantic-authorization-and-approval.md),
[Semantic API](../specs/semantic-api.md), and
[semantic authorization](../specs/semantic-authorization.md) own the exact laws.
The following is a reasoning checklist, not a new public method sequence or
permission model.

For an existing supported edit, the adapter submits typed intent and trusted
context to the application boundary. The application establishes the relevant
occurrence/base, derives requirements from typed meaning, prepares a candidate,
and applies the operation's actual validation/calculation and authorization
gates. Delegated publication uses the exact proposal/Approval requirements;
this does not add AI-style approval to every ordinary human edit.

Before state publication, verify the applicable live base, Grant, Approval and
other conditions through the established trusted lifecycle. Publish semantic
state and consume any required Approval atomically. Do not add a client fast
path that mutates Document fields, claims its own footprint, silently rebases
a stale proposal, or converts a preview into permission.

Read authorization matters too. Queries, previews, diffs and diagnostics may
reveal data; they must pass the relevant trusted disclosure boundary. Internal
runtime primitives are not automatically safe public endpoints. The current
`resident_session.rs` explicitly documents this distinction.

Failure behavior must preserve the exact operation contract. A rejected or
stale operation cannot partially install a candidate or consume a required
Approval as though publication succeeded. Do not invent a universal rule that
all diagnostics reject every edit; staged validation and operation-specific
gates remain authoritative.

### Existing authority: semantic publication is not durable save

The [runtime/host boundary](../architecture/frontend-backend-boundary.md)
separates accepted in-memory changes from persistence and external effects.
A host explicitly takes an authorized snapshot/materialization input and
performs the permitted durable write. The [`.roproj` editable-directory
representation](../specs/storage-versioning-and-migration.md) remains
version-owned under its applicable format and migration authority; support and
durable behavior are version- and host-operation-specific. It is not a second
live interactive state owner.

### Existing authority: truthful semantic and external-effect outcomes

[ADR-0034](../decisions/ADR-0034-team-workspace-policy-and-recovery-boundary.md)
requires each coordinated effect domain to retain its actual outcome. A later
persistence, projection, delivery or reporting failure cannot rewrite a known
semantic-publication outcome.

### Proposed engineering guidance: make effect truth observable

Make it possible to distinguish an operation rejected before publication, a
semantic operation that succeeded, and a later projection or persistence
failure. These are conceptual distinctions, not a new mandatory receipt DTO.
A rendering failure must not be reported as though an accepted command never
happened. A completed in-memory edit must not be labelled durably saved before
the host's real success evidence.

Do not blindly retry a mutation after an ambiguous response. Idempotency,
retry, cancellation, autosave, durable recovery and Undo contracts are not
created by this playbook. Preserve each existing scoped behavior; new promises
need explicit decisions and failure tests. A proposed save optimization must
not silently weaken existing no-overwrite or publication guarantees.

## 5. Organize existing crates by responsibility

### Proposed engineering guidance

Group code by the reason it changes, not by vague technical categories such as
`helpers`, `managers` or `utils`. Keep a use case readable at its orchestration
level; its validation, scope checks, candidate construction and publication
steps may be named helpers without fragmenting every expression.

`lib.rs` should trend toward a clear entry surface and module composition when
real code changes justify it. It is not required to contain zero behavior or to
be rewritten solely to become short.

At the inspected baseline, workspace-engine already contains
`analysis_operations.rs`, `capability_discovery.rs`, `formula_operations.rs`,
`keyed_grouped_sum_operations.rs`, `patch_lifecycle.rs` and
`resident_session.rs`. Treat those existing responsibilities as starting points.
File size alone is not a defect, and total size includes documentation and may
include tests. This inspection did not measure per-function complexity or prove
that any particular extraction is necessary.

### Illustration, not a prescribed migration

If a scoped change demonstrates a cohesive extraction, possible module groups
inside the **existing** workspace-engine crate are:

```text
src/
  lib.rs                 # deliberate internal facade and composition
  operations/            # existing semantic use cases, grouped by behavior
  patch_lifecycle/       # proposal binding, authorization and execution helpers
  resident_session/      # state owner, projections and derived runtime state
```

Do not create these directories without a concrete benefit. Preserve existing
module/API consumers through a scoped migration; forwarding internal exports
may be a reversible technique, but are not a new permanent SDK promise.

For `patch_lifecycle`, separate readable responsibilities without creating a
second publication owner. Only the actual publication path may install the
candidate and atomically consume Approval. A cosmetic extraction that exposes
raw commit setters or mutable authorization state is a regression.

For `resident_session`, distinguish owned state, revision-pinned queries,
rebuildable derived state and guarded publication. Splitting files must not
create duplicate Documents, independently authoritative caches, or clocks
selected by untrusted callers.

For `storage`, keep codec/migration logic separable from native I/O inside the
existing crate where useful. This does not authorize a storage crate split or
an engine dependency on a generic persistence repository.

## 6. Practical Rust Clean Code conventions

The following are proposed review conventions. Existing correctness, security,
compatibility and portability authority remains binding independently.

### Name the intent and preserve useful types

Prefer names such as `validate_candidate`, `derive_write_requirements`,
`query_fields` or `encode_portable_package` over `process`, `handle_data` or
`do_work`. Names should identify responsibility rather than merely restate the
implementation. Examples here are descriptive, not required new APIs.

Reuse existing typed IDs and enums where they express the domain. Avoid new
public APIs made of unrelated strings or several positional booleans. An enum
can clarify genuinely exclusive states; do not impose an elaborate type-state
framework when a small validated structure is clearer. A Rust type name or
private constructor is not proof of authentication across an untrusted host
boundary.

### Keep one coherent abstraction level

A function should have one understandable purpose. A use case may legitimately
coordinate several steps, and a numeric algorithm may legitimately be long.
Extract when doing so separates a responsibility, makes invariants explicit or
reduces independent change pressure, not to meet an arbitrary line count.
Keep command/query distinction at the semantic boundary; internal cache
maintenance does not turn an observational query into a semantic mutation.

### Make failures explicit

Use typed domain/application errors where callers need semantic distinctions.
Keep format and I/O failures owned by storage/hosts; map errors outward without
flattening actionable categories into a success-shaped string. Add context
without leaking credentials or unauthorized data.

Do not turn a parse, permission, persistence or calculation failure into a
plausible default using `unwrap_or_default`, catch-all success, or an empty
result. A genuinely optional missing value or empty diagnostic list may have a
legitimate default. Review the meaning, not just the token spelling.

Likewise, do not mass-remove `expect`/`unwrap` from tests or proven internal
invariants. New untrusted-input and recoverable-I/O paths should return explicit
errors. An invariant panic needs a stated invariant and appropriate evidence;
replacing it with silent success is not safer.

### Expose the smallest useful interface

Prefer private items, then scoped visibility, then `pub(crate)`, and only the
needed `pub` surface. Do not export internal caches, raw setters, security state
or host capabilities merely to simplify a test. Preserve actual consumers
before narrowing existing visibility.

Document what an API promises, its relevant error cases and why an invariant
exists. Comments should explain rationale, authority and unusual trade-offs,
not narrate obvious syntax. Remove obsolete branches only with evidence that
their behavior is no longer required.

### Abstract at real variation points

Use an existing trait or introduce a small port when there is a real
replaceable capability or testing boundary. Existing host-supplied ID and
publication-time seams are examples. The consumer of an abstraction owns the
required behavior; the outer host supplies the mechanism.

Do not add `IService`-style wrappers to pure functions, generic CRUD repositories
around Document, service locators, global containers, an event bus, or a common
utility crate without a concrete need. A function and an explicit parameter
are often enough. Static or dynamic dispatch should follow actual ownership,
complexity and performance needs, not ideology.

### Remove duplicate policy, not every similar shape

Clients must not duplicate semantic formulas, validation, authorization or
mutation rules. A storage decoder and an input-admission validator can still
perform different valid checks at different boundaries. Do not merge them
merely because their loops look similar.

Prefer straightforward code to speculative generalization. Reuse an existing
abstraction when it has the same meaning, not merely the same fields.

### Keep effects, determinism and costs visible

Portable semantic/application code must not gain ambient clocks, randomness,
filesystem, network, environment or UI dependencies. Host-supplied values must
remain trusted where required. Preserve accepted numeric, ordering, identity
and serialization contracts; sorting every collection indiscriminately is not
a substitute for those contracts.

Borrow or move when clear. Clone when snapshot isolation or ownership warrants
it. Do not enforce zero-clone purity or introduce shared mutable indirection
solely to avoid a small copy. Profile important paths before complexity-driven
optimization. Preserve full-oracle equivalence when changing incremental state.

Bounded viewport projection reduces transfer/rendering work. It does not allow
skipping off-screen semantic dependencies needed for a correct visible result.
Do not make ordinary edits ship a complete Document across the client boundary
by default; explicit snapshot boundaries and internal Rust cloning remain valid.

## 7. Test architecture, not aesthetic preferences

### Existing inspected enforcement

[`workspace-dependency-check.mjs`](../../scripts/workspace-dependency-check.mjs)
reads locked Cargo metadata and compares workspace package names and declared
local dependency edges against ADR-0016, without filtering away development
kinds. Preserve that single ownership source when extending tooling.

That check alone does **not** establish absence of transitive host capabilities,
all feature/target behaviors, internal module leaks, duplicate semantic rules,
or correct authorization. Do not advertise a passing dependency check as a
complete architecture or security audit.

[CONTRIBUTING](../../CONTRIBUTING.md) prescribes formatting, Clippy, workspace
tests, the complete release-equivalent gate, native/WASM conformance and
applicable component checks. This playbook does not claim to have executed them
or change their applicability.

### Future work: focused executable fitness checks

Before adding a guard, name its actual invariant and blind spots. Prefer
existing scripts and toolchains; introducing a parser or new framework is an
explicit tooling decision. Cargo dependency metadata, compiler visibility and
actual behavioral contracts are generally stronger evidence than broad text
searches. A source scan may be useful as a bounded check, but must not claim to
prove every alias, macro, feature or runtime behavior.

Suggested acceptance families for later scoped work:

| Family | Required observable evidence |
| --- | --- |
| Dependency boundaries | Accepted graph passes; a forbidden direct/dev/build or applicable conditional edge in a controlled fixture is rejected |
| Guard failure behavior | Missing tool, non-zero execution, invalid metadata and unexpectedly empty input do not become a green result |
| Portable behavior | Same accepted semantic cases agree natively and under supported WASM/feature profiles; compilation alone is not semantic parity |
| Candidate atomicity | Applicable rejected/stale/batch-failure cases preserve semantic state and required Approval consumption semantics |
| Read/write authorization | The actual trusted public admission path cannot disclose or publish outside the accepted scope; adapter-only flags are insufficient |
| Representation boundary | Canonical codec/migration fixtures preserve the accepted version contract; malformed and unsupported inputs do not silently become valid data |
| Resident derived state | Incremental and fresh full-oracle outcomes agree after relevant edit/dependency changes and cache rebuilds |
| Effect reporting | Accepted semantic publication followed by applicable projection/save failure is not misreported as a rejected edit or durable success |
| Client parity | Supported equivalent CLI/client/AI cases route through shared meaning under equivalent context and authorization, not equal permissions for all principals |

These are test design families, **not delivered executable acceptance tests**.
Before production readiness, the Steward must link real test/fixture paths,
exact baseline/seed commits, authority-to-case mapping and actual results.
Existing tests may already cover a family; inspect and reuse them rather than
claiming the gap exists or inventing a redundant harness.

For a new checker, demonstrate that it detects a deliberately introduced
violation and remains green for a lawful control. Do not weaken the checker or
its acceptance because a feature/alias/target complicates implementation.
Narrow an overstated claim explicitly through Steward reconciliation instead.

## 8. SCD operating contract

The canonical [delivery workflow](../governance/project-governance.md),
[PR Decomposition Policy](../governance/pr-decomposition-policy.md), and
[Delivery Throughput Policy](../governance/delivery-throughput-policy.md) remain
controlling. This playbook adds no handoff field, scheduler, Ready shortcut or
merge gate of its own.

The current operational [profile #374](https://github.com/nurockplayer/tachiko-work/issues/374)
requires Terra to obtain an actual bounded Astra consultation before a new
discretionary choice not settled by live authority or still-applicable prior
consultation. This includes architecture, algorithm and repair directions, not
only unusually difficult work. Applying a settled instruction mechanically
does not require one consultation per line or commit.

For each bounded lane:

1. Re-read live main, authority, Issue/PR, active writer and canonical handoff.
   Identify the owning layer and distinguish accepted behavior from internal
   mechanics. Do not take over another lane because a refactor seems useful.
2. Confirm genuine Ready scope, acceptance/evidence and decomposition. New
   discretionary choices go to Astra under #374; durable product/architecture
   conflicts or material acceptance changes go through Steward authority.
3. Implement or delegate the settled direction with applicable unit tests and
   evidence. A worker returns new choices to Terra instead of bypassing
   consultation. Keep refactoring limited to the accepted task and maintain
   one publication owner.
4. Run applicable checks at the exact final material head. Account for every
   material acceptance amendment and unresolved substantive review finding.
5. Obtain a fresh independent final review at the required risk depth. In the
   current profile this is Sol; design/implementation participants cannot count
   as independent reviewers of their own solution.
6. Follow existing handoff, Steward-watch, merge and post-merge recalibration
   rules. This playbook does not grant merge authority or relax an Issue's
   explicit no-self-merge instruction.

Record architecture considerations in the existing handoff narrative, without
new machine fields: owning layer, relevant authority, changed boundaries, real
alternatives and consultation disposition, compatibility/effect risks, actual
checks, and next action. Record observable model/session attribution; a role
name is not evidence of a model call. Missing Astra response is not consent.

Treat real correctness, security, data-integrity, Accepted-authority and scope
failures according to existing blocking policy. A long function, naming
preference or optional extraction is not automatically P2. Pure-maintainability
suggestions must not create an endless review loop. Conversely, cosmetic
labels must not hide a real authorization or publication defect.

## 9. Incremental adoption, not a rewrite

### This Issue: documentation only

Review and explicitly adopt or revise the engineering guidance. Preserve
existing authority, add a short AGENTS entry point, and change no product code,
manifest, test assertion or workflow. Product executable acceptance and
implementation unit-test exceptions are separately recorded in #402 for this
documentation-only scope. Applicable document/repository checks and fresh
independent review remain required. No failing product seed is manufactured.

### Later: guard preflight and focused enforcement

Inventory existing tests and guards first. A tests-only preflight may establish
real baseline/negative-control evidence but does not itself authorize production
changes. A separately Ready child may add one bounded guard after the Steward
has supplied its executable acceptance and reviewed tooling implications.

### Later: evidence-backed local refactoring

Choose a seam only when a real change exposes mixed responsibility, duplicate
policy, repeated adjacent failures, ownership ambiguity or measured cost. Map
existing contract tests before behavior-preserving refactors; do not invent a
RED for unchanged behavior. Add characterization evidence where needed, but do
not mistake captured legacy output for authority when it contradicts the
contract.

Preserve relevant semantic results, canonical formats, public contracts,
error/authorization behavior, consumer builds and runtime costs. Refactor one
cohesive seam per reviewable PR. Do not combine a feature, a format change,
new dependency and a file-wide rewrite into one convenient cleanup.

Rollback is an ordinary non-force revert or bounded correction under repository
policy. Migration of durable user data is not part of a cosmetic refactor.
Internal mechanics are replaceable; durable user contracts require explicit
compatibility and migration treatment. Do not preserve an early implementation
mistake merely because it already exists.

No repository-wide cleanliness percentage or arbitrary line limit is a release
prerequisite. Prioritize changes that reduce concrete delivery/correctness risk
without delaying the promised Sheet user journey for cosmetic perfection.

## 10. Completion criteria for engineering work

A well-designed change can answer: which layer owns it, which authority defines
its behavior, what observable evidence proves it, what compatibility/effect
boundary it touches, and which exact reviewed head is eligible for delivery.

Do not claim complete correctness from code style, Rust, green tests, or AI
review alone. The intended outcome is smaller reasoning surfaces, explicit
failure boundaries and evidence that can expose mistakes, including mistakes
made by the specification author.

## References

Repository authority is linked at each relevant boundary above. Methodological
references are explanatory, not an additional product authority:

- Robert C. Martin, [The Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html): inward source dependencies and boundary independence.
- Rust API Guidelines, [Type safety](https://rust-lang.github.io/api-guidelines/type-safety.html): useful types and explicit distinctions.
- The Rust Programming Language, [Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html): recoverable errors and panic distinctions.
