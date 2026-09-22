# ADR-0039: Native trusted-host composition for delegated Human approval

## Status

Accepted

Decision issue: [#439](https://github.com/nurockplayer/tachiko-work/issues/439)

Delivery owner: [#361](https://github.com/nurockplayer/tachiko-work/issues/361)

Preserved implementation evidence:
[PR #413](https://github.com/nurockplayer/tachiko-work/pull/413)

Related authority:
[ADR-0007](ADR-0007-ai-semantic-interaction-model.md),
[ADR-0020](ADR-0020-first-class-headless-semantic-api.md),
[ADR-0022](ADR-0022-resident-semantic-runtime-and-host-boundary.md),
[ADR-0024](ADR-0024-revision-pinned-semantic-patch.md), and
[ADR-0026](ADR-0026-scoped-semantic-authorization-and-approval.md).

## Context

Issue #361 requires one bounded delegated proposal / Human approval / Execute
composition for a first-party desktop client. The accepted authorization model
requires Human approval to bind the exact delegated proposal, semantic base,
occurrence, authority, and effective policy. Renderer, provider, or delegated
client input cannot manufacture that Human authority.

PR #413 provided the decisive implementation evidence. In the current Designer
composition, the authoritative resident `DesignerRuntime`, semantic session, and
`PatchLifecycle` occurrence live inside browser Worker WASM. The existing Tauri
host owns local-document ingress but does not own that same semantic occurrence.
Moving only an approval method into native Rust would therefore place approval
and Execute against different runtime instances. Matching document bytes,
proposal identifiers, or snapshots would not prove proposal/base/occurrence
continuity.

The browser Worker composition remains valid for ordinary capabilities that do
not require this trusted Human approval path. The problem is narrower: a
delegated mutation that requires ADR-0026 Human approval needs one trusted host
that owns the same retained occurrence through proposal, review, approval, and
publication.

## Decision

### 1. The desktop delegated occurrence has one native owner

For the bounded desktop delegated-approval capability, the existing Tauri host
owns one native instance of the existing Rust Designer runtime for the entire
semantic occurrence.

That one native-owned instance retains the resident semantic session,
`PatchLifecycle`, proposal occurrence, authorization state, trusted clock, and
publication authority needed by the delegated flow. The implementation MUST
reuse the existing Rust semantic/runtime authority rather than create a second
semantic model or duplicate lifecycle implementation.

The same delegated occurrence MUST NOT also have an authoritative Worker-WASM
runtime. State synchronization, snapshot copying, matching identifiers, or
content equality between two runtime instances is not a substitute for one
continuous authoritative occurrence.

This decision selects ownership, not a public Rust ABI, stable crate facade, or
general desktop-host API.

### 2. Browser and provider inputs remain untrusted and narrowly typed

The renderer, browser Worker, AI provider, and other delegated inputs may only
use a bounded request surface for the delegated capability.

That surface may request the separately authorized operations needed for the
bounded flow, such as scoped Query, scalar Propose, disclosure-authorized
Preview, Execute, and observation. The trusted host assigns the delegated
principal/context and checks exact semantic revision preconditions against its
retained state.

Untrusted payloads MUST NOT select or supply:

- Human or delegated principal identity;
- Grant identity, capability scope, or authorization policy;
- trusted time;
- occurrence authority;
- Human approval state;
- publication privilege; or
- a bearer approval credential.

The desktop bridge MUST NOT expose arbitrary `DesignerRequest` forwarding.
Existing Human scalar-edit credentials, broad query paths, project-replacement
imports, or unrelated privileged operations must not become alternate routes
into the delegated occurrence merely because they already exist elsewhere in
Designer.

### 3. Human review and consent are trusted-host responsibilities

A trusted native Human-review path obtains review evidence from the same
native-owned runtime and retained proposal that would later be executed.

The Human must be shown the disclosure-authorized material decision evidence for
the retained proposal, including the exact target and typed change and its
association with the retained proposal/base/occurrence. The trusted decision
callback records consent against that retained context.

A renderer may request that review begin, but renderer/provider content MUST NOT
be authoritative review evidence, MUST NOT return an approval boolean that the
host treats as Human authority, and MUST NOT invoke the approval operation
through the ordinary untrusted semantic-request channel.

Merely implementing a callback in Rust does not establish Human consent. The
concrete desktop implementation must identify and test the trusted local Human
interaction boundary. This ADR does not mandate a particular operating-system
dialog toolkit or presentation technology.

### 4. Approval remains host-internal and exact

ADR-0026 approval binding remains unchanged. Approval for this profile remains
bound to the exact authorization domain, proposal occurrence, complete approved
change/base, originator, executor, associated write requirements, effective
policy, approver authority, finite validity, and consumption state required by
the accepted authorization contract.

No approval object, reusable token, hidden browser message, or bearer credential
needs to cross the untrusted boundary.

The implementation may keep host-private pending-review state, but that state is
not semantic authority and cannot weaken live lifecycle checks.

### 5. Review cannot freeze the runtime or weaken revalidation

The host MUST NOT hold an exclusive semantic-runtime borrow or lock while
waiting for Human review.

When the Human decision returns, the host revalidates the retained review
context before recording approval. Execute and semantic publication then pass
the existing lifecycle/publication checks again.

The flow fails closed when required continuity no longer holds, including base
advancement, revocation, approval expiry, policy change, close/reopen, lost
occurrence continuity, or other accepted lifecycle invalidation.

Approval cannot publish by itself. Successful Execute still requires the
existing atomic semantic publication boundary and consumes approval according to
ADR-0026.

### 6. The capability is desktop-only and operation-bounded

This decision authorizes only the architecture profile for a desktop-hosted
experimental delegated approval capability.

The first concrete implementation, if separately made Ready, is limited to the
already-approved directly stored scalar `SetFieldValue` / `Value` operation
profile with independently scoped disclosure and write/approval authority.

Semantic publication does not imply durable save, filesystem publication,
network effects, provider effects, or any other external effect.

The standalone browser kit remains unable to claim trusted Human approval under
this profile. Requiring trusted approval in a standalone browser deployment
would need a separately Accepted trust/composition decision.

### 7. Worker-only and split-owner repair paths are rejected

The following are not compliant realizations of this decision:

- keeping the Worker as occurrence owner while adding only a native approval
  helper;
- duplicating lifecycle/session state in native and Worker runtimes and
  synchronizing snapshots, bytes, or identifiers;
- hidden Worker messages, renderer approval flags, or magic approval tokens;
- treating transport secrecy as Human authority; or
- adding a local service, shared database, or new persistence mechanism solely
  to bridge the current split ownership when the existing native Rust runtime
  can own the occurrence directly.

A trusted native host that embeds the existing WASM runtime could preserve
single ownership in principle, but it adds an embedding mechanism without
current need because the existing Designer runtime is already reusable native
Rust. It is therefore not the selected bounded profile.

### 8. Production implementation remains separately owned

This ADR does not resume PR #413 and does not itself make #361 Ready.

PR #413 remains implementation evidence for the rejected split-owner
composition and must not be repaired or rebased into the new architecture lane.

After this ADR is merged, the Project Steward may separately authorize one
Guarded desktop tracer-bullet implementation from current `main` that proves:

1. one native-owned occurrence;
2. one bounded scalar proposal;
3. disclosure-authorized trusted Human review;
4. exact approval against the retained proposal/base/occurrence;
5. one Execute and authoritative semantic publication;
6. no approval replay;
7. fail-closed behavior after relevant review-time invalidation; and
8. no delegated authority leaking through the untrusted request surface.

The retained M2-01…M2-08 acceptance remains applicable and should execute
through a two-party harness sharing the same native-owned runtime instance.
Additional focused security tests should cover forged renderer approval,
privileged direct-route rejection, review/consent races, delayed callbacks after
close/reopen, disclosure on returned projections, and truthful observation of
attempted host effects.

## Consequences

### Positive

- Human approval and Execute operate against one continuous authoritative
  proposal/base/occurrence.
- The accepted Rust semantic and authorization implementations are reused
  rather than duplicated.
- Browser/provider clients remain untrusted semantic requesters rather than
  accidental authorization authorities.
- Desktop capability can advance without stabilizing a public protocol, token
  scheme, identity service, or persistence layer.
- Standalone browser behavior remains honest about its weaker trust boundary.

### Trade-offs

- The delegated approval capability is not deployment-symmetric: desktop can
  support it while the standalone browser kit cannot.
- Native ownership requires a bounded host bridge instead of keeping every
  interactive semantic runtime inside Worker WASM.
- The trusted Human review interaction becomes an explicit host responsibility
  that needs platform-specific implementation and security evidence.

## Relationship to prior decisions

- **ADR-0007** remains authoritative: model/provider output is untrusted evidence
  and never gains mutation authority by itself.
- **ADR-0020** remains authoritative for transport-neutral Semantic API meaning;
  this ADR does not stabilize a wire protocol or arbitrary native request API.
- **ADR-0022** already places authoritative interactive state in the shared Rust
  resident runtime and permits replaceable host placement. This ADR selects
  native placement only for the bounded trusted desktop approval profile.
- **ADR-0024** remains authoritative for immutable exact-base proposal binding.
- **ADR-0026** remains authoritative for Principal, Grant, Authorization,
  Approval, expiry, revocation, replay, provenance, and semantic/external-effect
  separation. This ADR fixes one compliant host composition for applying those
  laws; it does not weaken them.

No existing Accepted ADR is superseded.
