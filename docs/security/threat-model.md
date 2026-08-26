# Threat Model

Decision state: Mixed. The semantic authorization threats and laws summarized
from ADR-0007 and ADR-0026 are Accepted. Supply-chain controls, exact trust
labels, instruction/data handling, bypass enforcement, diagnostics, and
host-effect enforcement remain Provisional, Deferred, or separately owned by
Issue #30 and their domain decisions.

## Security Philosophy

Tachiko Work handles documents, structured data, computation, plugins, and AI
operations.

Security must protect:

- user data;
- document integrity;
- execution safety;
- authorization and approval integrity; and
- supply chain trust.

## Untrusted Inputs

The following are untrusted data unless a narrower trusted boundary explicitly
proves otherwise:

- imported Office or other external files;
- document text, comments, labels, formulas, and metadata;
- external plugin and connector results;
- AI-generated operations, explanations, capability claims, approval claims,
  and validation claims;
- network collaboration events; and
- client-supplied proposal, approval, principal, grant, or provenance records.

Untrusted data may influence a typed proposal or review evidence. It does not
become platform instruction, principal identity, privilege, approval, or
semantic truth merely because a model emitted it in structured form.

## Security Boundaries

External input:

```text
Import / Plugin / Agent / Network
        |
        v
Admission + instruction/data boundary
        |
        v
Typed Semantic API request / immutable SemanticPatch
        |
        v
Deterministic validation and operation gate
        |
        v
Trusted authorization + exact Human approval boundary
        |
        v
Authorized semantic publication
```

Host effects remain beside, not inside, the semantic authorization domain:

```text
semantic publication
    != durable persistence
    != filesystem/network/process/Git/plugin/deployment authority
```

## Authorization and Approval Threats

The platform must prevent at least:

- a model, document, plugin, or request body selecting or upgrading its own
  PrincipalId;
- Query or Propose authority silently becoming Execute authority;
- a broad `editor` or `write:*` grant silently including Formula, Structure,
  Schema, Destructive, or external effects;
- an agent authoritatively declaring an incomplete disclosure/write footprint;
- Propose authority revealing preview/diff/diagnostic content outside Query
  scope;
- validation success being treated as authorization;
- authorization or approval overriding a failed semantic gate;
- approval of rendered prose or a diff being reused for a changed proposal;
- publication after the exact base changes, an authorizing Approve Grant is
  revoked, a required principal is disabled, or Approval expires, is revoked,
  or is consumed;
- replay or concurrent double use of one ApprovalId;
- provider/model identity becoming privilege;
- Delegated self-approval satisfying the Human approval requirement;
- raw semantic-core, storage, `.roproj`, filesystem, or host mutation bypassing
  the authorized Execute path; and
- ordinary semantic authority granting network, process, Git, plugin,
  deployment, credential, or persistence effects.

## Required Security Laws

The normative authorization contract is ADR-0026 and
[`semantic-authorization.md`](../specs/semantic-authorization.md). This
threat-oriented summary maps risks to that authority and does not create an
independent authorization contract.

- AI and automation have no intrinsic authority.
- Effective principal identity comes from a trusted host/session boundary.
- Authorization is default-deny and based on explicit immutable,
  non-reusable Grant occurrences with terminal trusted revocation state.
- Grant coverage combines the requested action with each associated
  mutation-class/scope requirement; independent unions cannot manufacture
  crossed authority.
- Query, Propose, Approve, and Execute are independent actions.
- Value, Formula, Structure, Schema, and Destructive mutation authority are
  independently grantable and additive.
- Grants use finite unions of stable-ID, document-local scope atoms; paths,
  JSON Pointers, UI/Git/storage coordinates, wildcards, and natural-language
  scope are not authority.
- The trusted application derives disclosure scope, canonical write scope, and
  mutation classes; clients do not authoritatively declare them.
- Propose does not grant arbitrary Query authority; out-of-scope review evidence
  is denied or safely reduced.
- A patch originated by a Delegated principal or executed using Delegated
  authority requires exact Approval from one authorized Human principal.
- Approval binds one proposal occurrence, complete ADR-0024
  `ExactChangeBinding`, originator, exact executor, complete associated
  mutation-class/scope write requirements, and authorization-policy version;
  the trusted record also identifies the Human approver and authorizing
  Approve Grants.
- Approval has finite lifetime, is revocable, and can authorize at most one
  successful semantic publication.
- Successful semantic publication consumes Approval atomically; failure before
  publication does not consume it.
- Stale base fails closed before candidate construction against the changed
  base and never performs implicit rebase or replay.
- Deterministic validation and operation gates remain authoritative.
- Security-relevant denials are machine-readable and distinguish authorization,
  approval, stale, semantic-gate, representation, and host-effect failures.
- Provenance records authorization domain, proposal/exact binding, originator,
  executor, approver, Grants, footprint, policy version, Approval, gate/report,
  and base/result revisions without becoming canonical semantic state or privilege.
- Unverifiable principal, Grant, Approval, time, lifecycle, or
  structural-binding state fails closed.

## Effect Separation

ADR-0007, ADR-0022, and ADR-0026 keep these authority domains separate:

- semantic Query/Propose/Approve/Execute;
- durable storage/materialization;
- filesystem and browser persistence;
- network access;
- process and shell execution;
- Git operations;
- plugin/connector execution;
- deployment/publication; and
- credentials/secrets access.

An authorization in one domain does not imply another. A host may materialize
or externally publish an already-authorized semantic result only under separate
host authority.

## Core Principles

- Never execute unvalidated transformations.
- Never treat semantic validity as authorization.
- Never treat model/provider claims as privilege or approval.
- Preserve user ownership and document integrity.
- Make security behavior observable and machine-readable.
- Prefer explicit, scoped, finite, revocable permissions.
- Fail closed on stale, modified, expired, revoked, consumed, or unverifiable
  approval.
- Keep semantic and host/external effects explicitly separated.
